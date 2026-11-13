#!/usr/bin/env bash
# Run the system test suite against the mnemorium image.
#
# Env toggles (no argument parsing):
#   NO_BUILD=1  reuse an existing image instead of building it
#   IMAGE=...   image tag (default: mnemorium:latest), exported as MNEMORIUM_IMAGE
#   ALLURE_RESULTS_DIR=DIR  also write Allure results to DIR
#
# See docs/development/TechnicalDesign.md, "Testing" and "System test".

set -euo pipefail

readonly IMAGE="${IMAGE:-mnemorium:latest}"
readonly CONTAINER_PREFIX="mnemorium-system-"
readonly SUITE="test/system"

cleanup() {
	local status=$?
	local name
	# Sweep containers a killed run left behind; a running test removes its own.
	while IFS= read -r name; do
		if [[ -n $name ]]; then
			docker rm -f "$name" >/dev/null 2>&1 || true
		fi
	done < <(docker ps -aq --filter "name=${CONTAINER_PREFIX}" || true)
	exit "${status}"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

if [[ -n ${NO_BUILD:-} ]]; then
	if ! docker image inspect "${IMAGE}" >/dev/null 2>&1; then
		echo "image ${IMAGE} not found; build it with docker build -t mnemorium:latest . or omit NO_BUILD" >&2
		exit 1
	fi
else
	echo "==> Building image ${IMAGE}"
	docker build -t "${IMAGE}" .
fi

PYTEST_ARGS=(-p no:cacheprovider -ra --tb=short)
if [[ -n ${ALLURE_RESULTS_DIR:-} ]]; then
	PYTEST_ARGS+=(--alluredir="${ALLURE_RESULTS_DIR}")
fi

echo "==> Running system tests"
MNEMORIUM_IMAGE="${IMAGE}" MNEMORIUM_CONTAINER_PREFIX="${CONTAINER_PREFIX}" pytest "${PYTEST_ARGS[@]}" "${SUITE}"
