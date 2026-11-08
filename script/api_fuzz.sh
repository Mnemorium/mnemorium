#!/usr/bin/env bash
# Launch a disposable, seeded Mnemorium server, capture the one-time Root Admin
# password printed on first start, and run Schemathesis against it.
#
# The server runs in a temporary working directory, so it owns a fresh
# mnemorium.db, state.yaml and config.yaml: every run initializes the Root
# Admin and the caller's development datastore is never touched.
#
# The binary is built with the `test-seed` feature, so the run also seeds the
# demo accounts (alice and bobby), the media bytes under `media/seed` and the
# demo music, video and gallery rows. Schemathesis therefore fuzzes a populated
# library rather than an empty one.
#
# The fixture identifiers live in the repository `schemathesis.toml`, so a read
# addresses an existing row instead of 404ing on a generated one. The
# provider-backed `apply` operations are deselected there: they need a live
# TMDB or MusicBrainz response, and this server configures no metadata provider.
#
# One upload is created, chunked and completed before the run, and its
# identifier is injected into the generated config, so the completion endpoint
# addresses a finished session rather than an unknown one.
#
# Schemathesis authenticates with the captured credentials through its
# dynamic-token auth config: it calls POST /api/v1/identity/login and reuses
# the returned access token for the `bearer_auth` scheme.
#
# Env toggles (no argument parsing):
#   NO_BUILD=1       reuse the existing target/debug/server binary; it must have
#                    been built with `--features "api-docs test-seed"`
#   KEEP=1           keep the temporary server directory (contains the password)
#   SHOW_PASSWORD=1  print the captured password instead of only using it
#   REPORT=1         write a self-contained HTML report under .artifacts/
#   REPORT_DIR=DIR   directory for that report (default .artifacts/schemathesis)
#
# Schemathesis resolves each operation against the `servers` entries in the
# served document, so `/health` stays at the service root while the contexts
# sit under `/api/v1`. Do not pass `-u`/`--base-url`: an explicit base URL
# overrides every operation-level server and sends `/health` to
# `/api/v1/health`, where nothing is routed.
#
# Extra arguments are passed to `schemathesis run`, for example:
#   script/api_fuzz.sh --max-examples 10 --phases fuzzing
#
# The hard-coded port is 4080, so stop any other server first.

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly REPO_ROOT

readonly BASE_URL="http://127.0.0.1:4080"
readonly SCHEMA_URL="${BASE_URL}/doc/openapi.json"
readonly ROOT_ADMIN_USERNAME="root"
readonly STARTUP_TIMEOUT_S=60
readonly HEALTH_TIMEOUT_S=60

DATA_DIR="$(mktemp -d)"
readonly DATA_DIR
readonly SERVER_LOG="${DATA_DIR}/server.log"
readonly SCHEMATHESIS_CONFIG="${DATA_DIR}/schemathesis.toml"

SERVER_PID=""

cleanup() {
	local status=$?
	if [[ -n ${SERVER_PID} ]] && kill -0 "${SERVER_PID}" 2>/dev/null; then
		kill -TERM "${SERVER_PID}" 2>/dev/null || true
		wait "${SERVER_PID}" 2>/dev/null || true
	fi
	if [[ -z ${KEEP:-} ]]; then
		rm -rf "${DATA_DIR}"
	else
		echo "kept the disposable server directory at ${DATA_DIR}" >&2
		echo "warning: ${SERVER_LOG} contains the one-time Root Admin password" >&2
	fi
	exit "${status}"
}
trap cleanup EXIT

# Build the login payload with Python so the generated password never needs
# shell or JSON escaping.
login_payload() {
	python3 -c 'import json, sys; print(json.dumps({"username": sys.argv[1], "password": sys.argv[2]}))' "$1" "$2"
}

# Authenticate as the Root Admin and print the bearer token.
login() {
	curl -fsS -X POST "${BASE_URL}/api/v1/identity/login" \
		-H 'Content-Type: application/json' \
		--data "$(login_payload "${ROOT_ADMIN_USERNAME}" "${PASSWORD}")" |
		python3 -c 'import json, sys; print(json.load(sys.stdin)["access_token"])'
}

# Create and complete one upload from a seeded image, and print its identifier.
#
# Completion is idempotent, so the fuzzer may address the finished session
# repeatedly: it returns the caller's file for the same digest every time.
seed_upload() {
	local token="$1"
	local fixture="${REPO_ROOT}/test/seed/media/cover.png"
	local digest hex response size upload_id

	size="$(stat -c %s "${fixture}")"
	hex="$(sha256sum "${fixture}" | cut -d' ' -f1)"
	digest="$(python3 -c 'import base64, hashlib, sys; print(base64.b64encode(hashlib.sha256(open(sys.argv[1], "rb").read()).digest()).decode())' "${fixture}")"
	response="$(curl -fsS -X POST "${BASE_URL}/api/v1/asset/upload" \
		-H "Authorization: Bearer ${token}" \
		-H 'Content-Type: application/json' \
		--data "$(python3 -c 'import json, sys; print(json.dumps({"content_type": "image/png", "file_name": "fixture.png", "file_size": int(sys.argv[1]), "integrity_hash": sys.argv[2]}))' "${size}" "${hex}")")"
	upload_id="$(printf '%s' "${response}" | python3 -c 'import json, sys; print(json.load(sys.stdin)["upload_id"])')"
	curl -fsS -o /dev/null -X PUT "${BASE_URL}/api/v1/asset/upload/${upload_id}/chunk/0" \
		-H "Authorization: Bearer ${token}" \
		-H 'Content-Type: application/octet-stream' \
		-H "Content-Range: bytes 0-$((size - 1))/${size}" \
		-H "Content-Digest: sha-256=:${digest}:" \
		--data-binary "@${fixture}"
	curl -fsS -o /dev/null -X POST "${BASE_URL}/api/v1/asset/upload/${upload_id}/complete" \
		-H "Authorization: Bearer ${token}"
	printf '%s' "${upload_id}"
}

if ! command -v schemathesis >/dev/null 2>&1; then
	echo "error: schemathesis is not on PATH; run 'devenv tasks run setup:schemathesis'" >&2
	exit 1
fi

# The server always binds 0.0.0.0:4080, so a running instance would make the
# disposable one fail to bind and never print its password.
if curl -fsS --max-time 1 "${BASE_URL}/health" >/dev/null 2>&1; then
	echo "error: something already answers on ${BASE_URL}/health; stop it first" >&2
	exit 1
fi

BINARY="${REPO_ROOT}/target/debug/server"
if [[ -n ${CARGO_TARGET_DIR:-} ]]; then
	if [[ ${CARGO_TARGET_DIR} == /* ]]; then
		BINARY="${CARGO_TARGET_DIR}/debug/server"
	else
		BINARY="${REPO_ROOT}/${CARGO_TARGET_DIR}/debug/server"
	fi
fi
readonly BINARY

if [[ -z ${NO_BUILD:-} ]]; then
	echo "==> Building the server with the api-docs and test-seed features"
	(
		cd "${REPO_ROOT}"
		cargo build --features "api-docs test-seed" --bin server
	)
elif [[ ! -x ${BINARY} ]]; then
	echo "error: ${BINARY} is missing; run again without NO_BUILD=1" >&2
	exit 1
fi

echo "==> Starting a disposable server in ${DATA_DIR}"
mkdir -p "${DATA_DIR}/media"
(
	cd "${DATA_DIR}"
	exec env \
		MNEMORIUM_SECURITY_LOG_ROOT_ADMIN_PASSWORD=true \
		MNEMORIUM_SECURITY_RATE_LIMIT_BURST_SIZE=10000 \
		"${BINARY}"
) >"${SERVER_LOG}" 2>&1 &
SERVER_PID=$!

echo "==> Waiting for the Root Admin default password"
PASSWORD=""
for _ in $(seq 1 $((STARTUP_TIMEOUT_S * 5))); do
	PASSWORD="$(sed -n "s/.*Root admin initialized; use the default password to authenticate and change it: '\([^']*\)'.*/\1/p" "${SERVER_LOG}" | head -n 1)"
	if [[ -n ${PASSWORD} ]]; then
		break
	fi
	if ! kill -0 "${SERVER_PID}" 2>/dev/null; then
		echo "error: the server exited before printing the default password" >&2
		sed '/Root admin initialized; use the default password/d' "${SERVER_LOG}" | tail -n 40 >&2
		exit 1
	fi
	sleep 0.2
done
if [[ -z ${PASSWORD} ]]; then
	echo "error: the Root Admin password was not printed within ${STARTUP_TIMEOUT_S}s" >&2
	echo "hint: set KEEP=1 to inspect ${SERVER_LOG}" >&2
	exit 1
fi

echo "==> Waiting for ${BASE_URL}/health"
healthy=0
for _ in $(seq 1 $((HEALTH_TIMEOUT_S * 2))); do
	if curl -fsS --max-time 1 "${BASE_URL}/health" >/dev/null 2>&1; then
		healthy=1
		break
	fi
	if ! kill -0 "${SERVER_PID}" 2>/dev/null; then
		echo "error: the server exited before becoming healthy" >&2
		sed '/Root admin initialized; use the default password/d' "${SERVER_LOG}" | tail -n 40 >&2
		exit 1
	fi
	sleep 0.5
done
if [[ ${healthy} -ne 1 ]]; then
	echo "error: the server was not healthy within ${HEALTH_TIMEOUT_S}s" >&2
	exit 1
fi

echo "==> Preparing the fixture upload"
TOKEN="$(login)"
UPLOAD_ID="$(seed_upload "${TOKEN}")"
echo "==> Fixture upload ${UPLOAD_ID} is complete"

# The repository config carries the static fixture catalogue. The upload
# identifier is run-specific and the auth table carries the captured password,
# so both are appended here. The placeholders stay literal: Schemathesis
# resolves `${NAME}` from the environment, so the password never needs TOML
# escaping.
{
	cat "${REPO_ROOT}/schemathesis.toml"
	cat <<TOML

[[operations]]
include-path = "/asset/upload/{upload_id}/complete"
parameters = { upload_id = ${UPLOAD_ID} }

[[operations]]
include-operation-id = "get_upload"
parameters = { upload_id = ${UPLOAD_ID} }
TOML
	cat <<'TOML'

[auth.dynamic.openapi.bearer_auth]
path = "/identity/login"
payload = { username = "${LOGIN_USERNAME}", password = "${LOGIN_PASSWORD}" }
extract_selector = "/access_token"
TOML
} >"${SCHEMATHESIS_CONFIG}"

export LOGIN_USERNAME="${ROOT_ADMIN_USERNAME}"
export LOGIN_PASSWORD="${PASSWORD}"

if [[ -n ${SHOW_PASSWORD:-} ]]; then
	echo "Root Admin username: ${ROOT_ADMIN_USERNAME}"
	echo "Root Admin password: ${PASSWORD}"
	SEED_PASSWORD="$(sed -n "s/.*the demo accounts .* share the password '\([^']*\)'.*/\1/p" "${SERVER_LOG}" | head -n 1)"
	if [[ -n ${SEED_PASSWORD} ]]; then
		echo "Seeded demo password (alice and bobby): ${SEED_PASSWORD}"
	fi
fi

SCHEMATHESIS_ARGS=("$@")
if [[ -n ${REPORT:-} ]]; then
	has_report=""
	for arg in "$@"; do
		case ${arg} in
		--report | --report=* | --report-dir | --report-dir=*) has_report=1 ;;
		esac
	done
	if [[ -n ${has_report} ]]; then
		echo "note: REPORT=1 is ignored because --report or --report-dir is already passed" >&2
	else
		REPORT_DIR="${REPORT_DIR:-${REPO_ROOT}/.artifacts/schemathesis}"
		mkdir -p "${REPORT_DIR}"
		SCHEMATHESIS_ARGS+=(--report html --report-dir "${REPORT_DIR}")
		echo "==> Writing the HTML report under ${REPORT_DIR}"
	fi
fi

echo "==> Running Schemathesis against ${SCHEMA_URL}"
# Run from the repository root so the relative `hooks` path in the config
# resolves; the schema URL and the generated config are absolute.
(
	cd "${REPO_ROOT}"
	schemathesis --config-file "${SCHEMATHESIS_CONFIG}" run "${SCHEMA_URL}" "${SCHEMATHESIS_ARGS[@]}"
)
