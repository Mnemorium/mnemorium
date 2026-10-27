#!/usr/bin/env bash
# Run the configuration test suite against the mnemorium image.
#
# Env toggles (no argument parsing):
#   NO_BUILD=1  reuse an existing image instead of building it
#   IMAGE=...   image tag (default: mnemorium:latest), exported as MNEMORIUM_IMAGE
#
# See docs/development/TechnicalDesign.md, "Testing" and "Configuration test", TEST-055..TEST-061.

set -euo pipefail

readonly IMAGE="${IMAGE:-mnemorium:latest}"
readonly CONTAINER_PREFIX="mnemorium-config-"
readonly SUITE="test/configuration"

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
		echo "image ${IMAGE} not found; build it with ./script/e2e.sh or omit NO_BUILD" >&2
		exit 1
	fi
else
	echo "==> Building image ${IMAGE}"
	docker build -t "${IMAGE}" .
fi

echo "==> Running configuration tests"
MNEMORIUM_IMAGE="${IMAGE}" pytest -p no:cacheprovider -ra --tb=short "${SUITE}"
