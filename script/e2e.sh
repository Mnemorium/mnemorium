#!/usr/bin/env bash
# Build the container image, start the E2E server, wait for health, run the
# pytest suite, and remove the container on exit.
#
# Env toggles (no argument parsing):
#   NO_BUILD=1  reuse an existing `mnemorium` image instead of building it
#   KEEP=1      leave the `mnemorium-e2e` container running for inspection
#
# See docs/development/TechnicalDesign.md, "Testing", TEST-039..TEST-045.

set -euo pipefail

readonly IMAGE="mnemorium"
readonly CONTAINER="mnemorium-e2e"
readonly PORT="4080"
readonly BASE_URL="http://127.0.0.1:${PORT}"
readonly HEALTH_TIMEOUT_S=60
readonly JUNIT_DIR=".artifacts/e2e"
readonly JUNIT_XML="${JUNIT_DIR}/junit.xml"

cleanup() {
	local status=$?
	if [[ -z ${KEEP:-} ]]; then
		docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true
	fi
	exit "${status}"
}
trap cleanup EXIT

if [[ -z ${NO_BUILD:-} ]]; then
	echo "==> Building image ${IMAGE}"
	docker build -t "${IMAGE}" .
fi

mkdir -p "${JUNIT_DIR}"

echo "==> Starting container ${CONTAINER}"
docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true
docker run -d --name "${CONTAINER}" -p "${PORT}:4080" \
	-e MNEMORIUM__SECURITY__RATE_LIMIT__BURST_SIZE=1000 \
	"${IMAGE}" >/dev/null

echo "==> Waiting for ${BASE_URL}/health"
healthy=0
for _ in $(seq 1 "${HEALTH_TIMEOUT_S}"); do
	if curl -fsS "${BASE_URL}/health" >/dev/null 2>&1; then
		healthy=1
		break
	fi
	sleep 1
done
if [[ ${healthy} -ne 1 ]]; then
	echo "server not healthy within ${HEALTH_TIMEOUT_S}s" >&2
	docker logs "${CONTAINER}" >&2 || true
	exit 1
fi

echo "==> Running E2E tests"
MNEMORIUM_E2E_BASE_URL="${BASE_URL}" PYTHONPATH="test/e2e" \
	pytest -p no:cacheprovider -ra --tb=short --junitxml="${JUNIT_XML}" test/e2e
