#!/usr/bin/env bash
# Launch a disposable, seeded Mnemorium server, capture the one-time Root Admin
# password printed on first start, and run the OWASP ZAP active scan plan in
# `security/zap.yaml` against the OpenAPI document the server serves.
#
# The server runs in a temporary working directory, so it owns a fresh
# mnemorium.db, state.yaml and config.yaml: every run initializes the Root
# Admin and the caller's development datastore is never touched.
#
# The binary is built with the `api-docs` and `test-seed` features: the scan
# imports the document served at /doc/openapi.json, and the seed data gives the
# scanner populated rows instead of an empty library.
#
# The active scanner is destructive, so this script only ever scans the
# throwaway instance it starts: it refuses to run while anything else answers
# on the fixed port, and it removes the instance afterwards.
#
# Env toggles (no argument parsing):
#   NO_BUILD=1       reuse the existing target/debug/server binary; it must
#                    have been built with `--features "api-docs test-seed"`
#   KEEP=1           keep the temporary working directory (contains the password)
#   REPORT_DIR=DIR   directory for the HTML and JSON reports
#                    (default .artifacts/zap)
#   ZAP_IMAGE=...    OWASP ZAP image
#                    (default zaproxy/zap-stable:latest)
#   QUICK=1          run ZAP's shipped "API-Minimal" policy (23 active rules)
#                    instead of the default policy
#   ZAP_POLICY=...   named ZAP scan policy to run, for example "API-Minimal"
#
# The hard-coded port is 4080, so stop any other server first.

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly REPO_ROOT

readonly BASE_URL="http://127.0.0.1:4080"
readonly ZAP_TARGET_URL="http://host.docker.internal:4080"
readonly ZAP_CONTAINER="mnemorium-zap"
readonly ZAP_IMAGE="${ZAP_IMAGE:-zaproxy/zap-stable:latest}"
readonly REPORT_DIR="${REPORT_DIR:-${REPO_ROOT}/.artifacts/zap}"
readonly STARTUP_TIMEOUT_S=60
readonly HEALTH_TIMEOUT_S=60

WORK_DIR="$(mktemp -d)"
readonly WORK_DIR
readonly SERVER_DIR="${WORK_DIR}/server"
readonly SERVER_LOG="${SERVER_DIR}/server.log"
readonly ZAP_WORK_DIR="${WORK_DIR}/zap"

SERVER_PID=""

cleanup() {
	local status=$?
	if [[ -n ${SERVER_PID} ]] && kill -0 "${SERVER_PID}" 2>/dev/null; then
		kill -TERM "${SERVER_PID}" 2>/dev/null || true
		wait "${SERVER_PID}" 2>/dev/null || true
	fi
	docker rm -f "${ZAP_CONTAINER}" >/dev/null 2>&1 || true
	if [[ -z ${KEEP:-} ]]; then
		rm -rf "${WORK_DIR}"
	else
		echo "kept the disposable working directory at ${WORK_DIR}" >&2
		echo "warning: ${SERVER_LOG} contains the one-time Root Admin password" >&2
	fi
	exit "${status}"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# Copy the reports ZAP wrote into the mounted working directory, whether the
# scan completed or failed, and list the files the caller can open.
collect_reports() {
	if ! compgen -G "${ZAP_WORK_DIR}/reports/*" >/dev/null; then
		echo "warning: ZAP produced no report" >&2
		return 0
	fi
	mkdir -p "${REPORT_DIR}"
	cp -R "${ZAP_WORK_DIR}/reports/." "${REPORT_DIR}/"
	echo "==> Reports written under ${REPORT_DIR}"
	local report
	for report in "${REPORT_DIR}/zap-report.html" "${REPORT_DIR}/zap-report.json"; do
		if [[ -f ${report} ]]; then
			echo "    ${report}"
		fi
	done
}

# Print alert instances per risk from the machine-readable report. The summary
# is a convenience: an unreadable report never fails the run.
summarize_alerts() {
	local report="${ZAP_WORK_DIR}/reports/zap-report.json"
	if [[ ! -s ${report} ]]; then
		return 0
	fi
	python3 - "${report}" <<'PY'
import json
import sys

path = sys.argv[1]
try:
    with open(path, encoding="utf-8") as handle:
        document = json.load(handle)
except (OSError, json.JSONDecodeError) as error:
    print(f"warning: cannot read the ZAP JSON report: {error}", file=sys.stderr)
    raise SystemExit(0) from error

labels = {"3": "High", "2": "Medium", "1": "Low", "0": "Informational"}
types = dict.fromkeys(labels.values(), 0)
instances = dict.fromkeys(labels.values(), 0)
sites = document.get("site") or []
if isinstance(sites, dict):
    sites = [sites]
for site in sites:
    for alert in site.get("alerts") or []:
        label = labels.get(str(alert.get("riskcode", "")))
        if label is None:
            continue
        types[label] += 1
        instances[label] += len(alert.get("instances") or []) or 1

print("==> Alert types:     " + "  ".join(f"{name}: {count}" for name, count in types.items()))
print("==> Alert instances: " + "  ".join(f"{name}: {count}" for name, count in instances.items()))
PY
}

# Set the active scan policy in the run's copy of the plan. ZAP's activeScan job
# does not expand variables in `policy`, so the runner renders the line itself.
# The checked-in plan keeps an empty value and this fails loudly if that line
# moves or is duplicated.
select_policy() {
	local policy="$1"
	local plan="$2"
	if [[ -z ${policy} ]]; then
		return 0
	fi
	python3 - "${plan}" "${policy}" <<'PY'
import sys

path, policy = sys.argv[1], sys.argv[2]
marker = '      policy: ""'
with open(path, encoding="utf-8") as handle:
    text = handle.read()
if text.count(marker) != 1:
    print('error: the ZAP plan has no single `policy: ""` line to set', file=sys.stderr)
    raise SystemExit(1)
with open(path, "w", encoding="utf-8") as handle:
    handle.write(text.replace(marker, f'      policy: "{policy}"'))
PY
}

if ! command -v docker >/dev/null 2>&1; then
	echo "error: docker is not on PATH" >&2
	exit 1
fi
if ! command -v python3 >/dev/null 2>&1; then
	echo "error: python3 is not on PATH" >&2
	exit 1
fi

# The server always binds 0.0.0.0:4080, so a running instance would make the
# disposable one fail to bind and never print its password. It would also be
# actively scanned, so refuse before doing anything.
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

mkdir -p "${SERVER_DIR}/media" "${ZAP_WORK_DIR}/reports"
cp "${REPO_ROOT}/security/zap.yaml" "${ZAP_WORK_DIR}/zap.yaml"
# The ZAP image runs as an unprivileged user whose uid need not match the
# caller's, so the mounted plan and report directory must be world-writable.
chmod -R 0777 "${ZAP_WORK_DIR}"

# `QUICK=1` swaps the default policy for ZAP's shipped quick set; ZAP_POLICY
# names any policy the image carries (for example "API-Minimal").
ZAP_POLICY="${ZAP_POLICY:-}"
if [[ -n ${QUICK:-} ]]; then
	ZAP_POLICY="${ZAP_POLICY:-API-Minimal}"
fi
select_policy "${ZAP_POLICY}" "${ZAP_WORK_DIR}/zap.yaml"

echo "==> Starting a disposable server in ${SERVER_DIR}"
(
	cd "${SERVER_DIR}"
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

echo "==> Running OWASP ZAP against ${ZAP_TARGET_URL}"
# `-e NAME` forwards the caller's environment without putting the password in
# the container command line.
export TARGET_URL="${ZAP_TARGET_URL}"
export ROOT_ADMIN_PASSWORD="${PASSWORD}"
echo "    scan policy: ${ZAP_POLICY:-default}"

# ZAP exits 2 when the plan ran but reported warnings, which is still a
# completed scan; only any other non-zero status is a failure.
scan_status=0
docker run --rm --name "${ZAP_CONTAINER}" \
	--add-host "host.docker.internal:host-gateway" \
	-e TARGET_URL -e ROOT_ADMIN_PASSWORD \
	-v "${ZAP_WORK_DIR}:/zap/wrk" \
	"${ZAP_IMAGE}" \
	zap.sh -cmd -autorun /zap/wrk/zap.yaml || scan_status=$?

collect_reports
summarize_alerts || echo "warning: could not summarize the ZAP report" >&2

if [[ ${scan_status} -ne 0 && ${scan_status} -ne 2 ]]; then
	echo "error: ZAP exited with status ${scan_status}" >&2
	exit 1
fi
