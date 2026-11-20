#!/usr/bin/env bash
# Generate the system suite's typed Python client from the served OpenAPI document.
#
# The document is published only by a server built with the `api-docs` feature, so the client is generated from the
# image under test: `script/system.sh` builds that image with `api-docs test-seed`, calls this script before pytest and
# therefore never runs a case against a stale client. Nothing here is committed: `test/generated/` is ignored, and a
# handler change changes the client the next time it is generated.
#
# Modes:
#   generate   read `/doc/openapi.json` from a throwaway container of the image and generate the client (default)
#   clean      remove the generated descriptor and package
#
# Env toggles (no argument parsing):
#   IMAGE=...  image to read the document from (default: $MNEMORIUM_IMAGE, else mnemorium:latest)
#
# A generator warning means the document declares something the suite cannot express — a media type, a parameter, a
# body. Declare it in the handler's `#[utoipa::path(...)]`/`ToSchema` (docs/development/TechnicalDesign.md, "System
# test"); the only accepted alias is the dynamic content media type in `test/openapi_client.yaml`.
#
# The container runs unseeded, with its working directory on a tmpfs: the document describes the routes, so no fixture
# and no writable mount is involved. The tmpfs must be world-writable, because the image's server runs as the unprivileged
# `mnemorium` account and writes the instance state there.

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly REPO_ROOT
readonly GENERATED_DIR="${REPO_ROOT}/test/generated"
readonly DESCRIPTOR="${GENERATED_DIR}/openapi.json"
readonly PACKAGE="${GENERATED_DIR}/mnemorium_client"
readonly CONFIG="${REPO_ROOT}/test/openapi_client.yaml"
readonly IMAGE="${IMAGE:-${MNEMORIUM_IMAGE:-mnemorium:latest}}"
readonly CONTAINER_PORT=4080
readonly STARTUP_TIMEOUT_S=60

CONTAINER_NAME=""

log() {
	printf '==> %s\n' "$*" >&2
}

die() {
	printf 'error: %s\n' "$*" >&2
	exit 1
}

cleanup() {
	local status=$?
	if [[ -n ${CONTAINER_NAME} ]]; then
		docker rm -f "${CONTAINER_NAME}" >/dev/null 2>&1 || true
		CONTAINER_NAME=""
	fi
	exit "${status}"
}
trap cleanup EXIT

require_image() {
	docker image inspect "${IMAGE}" >/dev/null 2>&1 ||
		die "image ${IMAGE} not found; build it with --build-arg CARGO_FEATURES='api-docs test-seed'"
}

start_server() {
	CONTAINER_NAME="mnemorium-openapi-$$-${RANDOM}"
	log "reading the document from ${IMAGE} (${CONTAINER_NAME})"
	docker run --rm -d --name "${CONTAINER_NAME}" --tmpfs /app:rw,mode=1777 "${IMAGE}" >/dev/null
	local deadline=$((SECONDS + STARTUP_TIMEOUT_S))
	while ((SECONDS < deadline)); do
		if docker exec "${CONTAINER_NAME}" wget -qO- "http://127.0.0.1:${CONTAINER_PORT}/health" >/dev/null 2>&1; then
			return 0
		fi
		if [[ $(docker inspect -f '{{.State.Running}}' "${CONTAINER_NAME}" 2>/dev/null) != true ]]; then
			docker logs "${CONTAINER_NAME}" >&2 || true
			die "the server exited before it answered /health"
		fi
		sleep 0.2
	done
	die "the server answered no health check within ${STARTUP_TIMEOUT_S}s"
}

fetch_document() {
	mkdir -p "${GENERATED_DIR}"
	if ! docker exec "${CONTAINER_NAME}" \
		wget -qO- "http://127.0.0.1:${CONTAINER_PORT}/doc/openapi.json" >"${DESCRIPTOR}" 2>/dev/null; then
		die "the server served no /doc/openapi.json; its image must be built with the 'api-docs test-seed' features"
	fi
	[[ -s ${DESCRIPTOR} ]] || die "the server served an empty document"
	grep -q '"openapi"' "${DESCRIPTOR}" || die "${DESCRIPTOR} carries no openapi member"
}

generate_client() {
	command -v openapi-python-client >/dev/null 2>&1 ||
		die "openapi-python-client is not on PATH; enter the dev shell or pip install -r requirements.txt"
	rm -rf "${PACKAGE}"
	local log_file
	log_file="$(mktemp)"
	log "generating ${PACKAGE}"
	if ! openapi-python-client generate \
		--path "${DESCRIPTOR}" \
		--config "${CONFIG}" \
		--meta none \
		--output-path "${PACKAGE}" >"${log_file}" 2>&1; then
		cat "${log_file}" >&2
		rm -f "${log_file}"
		die "openapi-python-client failed; its log is above"
	fi
	if grep -q 'WARNING' "${log_file}"; then
		cat "${log_file}" >&2
		rm -f "${log_file}"
		die "the generator dropped part of the document; declare it in the handlers' utoipa macros"
	fi
	rm -f "${log_file}"
	log "the client is at ${PACKAGE}"
}

case "${1:-generate}" in
generate)
	require_image
	start_server
	fetch_document
	generate_client
	;;
clean)
	rm -rf "${GENERATED_DIR}"
	log "removed ${GENERATED_DIR}"
	;;
*)
	die "unknown mode '${1}'; use generate or clean"
	;;
esac
