#!/usr/bin/env bash
# Run the system test suite against the mnemorium image.
#
# The image is built with the `test-seed` feature, so the suite boots a server that can seed the profile it names; a
# boot that names no profile applies no fixture at all. It also carries `api-docs`, because the suite drives the typed
# client generated from the image's own `/doc/openapi.json`: `script/openapi_client.sh` regenerates it before pytest,
# so a case can never run against a client the handlers have outgrown. The client is never committed.
#
# Two container kinds: each pytest worker boots one instance that every case shares, and a
# case marked `own_instance` starts its own container. Both keep the `mnemorium-system-`
# prefix, so the sweep below removes what a killed run left behind.
#
# Env toggles (no argument parsing):
#   NO_BUILD=1  reuse an existing image instead of building it
#   IMAGE=...   image tag (default: mnemorium:latest), exported as MNEMORIUM_IMAGE
#   PYTEST_WORKERS=...  run that many pytest-xdist workers in parallel (default: serial)
#   JUNIT_XML=FILE  also write the JUnit report to FILE
#   BUILDX_CACHE_DIR=DIR  build with a local buildx cache at DIR (default: docker build)
#
# See docs/development/TechnicalDesign.md, "Testing" and "System test".

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
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
		echo "image ${IMAGE} not found; build it with CARGO_FEATURES='api-docs test-seed' docker build -t mnemorium:latest . or omit NO_BUILD" >&2
		exit 1
	fi
else
	echo "==> Building image ${IMAGE} with the test-seed and api-docs features"
	if [[ -n ${BUILDX_CACHE_DIR:-} ]]; then
		docker buildx build --load -t "${IMAGE}" \
			--build-arg CARGO_FEATURES="api-docs test-seed" \
			--cache-from "type=local,src=${BUILDX_CACHE_DIR}" \
			--cache-to "type=local,dest=${BUILDX_CACHE_DIR}-new,mode=max" .
	else
		docker build --build-arg CARGO_FEATURES="api-docs test-seed" -t "${IMAGE}" .
	fi
fi

echo "==> Generating the typed client from ${IMAGE}"
MNEMORIUM_IMAGE="${IMAGE}" "${SCRIPT_DIR}/openapi_client.sh" generate

PYTEST_ARGS=(-p no:cacheprovider -ra --tb=short)
if [[ -n ${PYTEST_WORKERS:-} ]]; then
	PYTEST_ARGS+=(-n "${PYTEST_WORKERS}" --dist load)
fi
if [[ -n ${JUNIT_XML:-} ]]; then
	PYTEST_ARGS+=("--junitxml=${JUNIT_XML}")
fi

echo "==> Running system tests"
MNEMORIUM_IMAGE="${IMAGE}" MNEMORIUM_CONTAINER_PREFIX="${CONTAINER_PREFIX}" pytest "${PYTEST_ARGS[@]}" "${SUITE}"
