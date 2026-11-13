#!/usr/bin/env bash
# Run the Rust test suite and the system test suite with Allure result
# collection, then generate the HTML report from both.
#
# Env toggles (no argument parsing):
#   NO_BUILD=1  reuse an existing mnemorium image instead of building it
#   OPEN=1      serve the generated report in the browser after generation
#
# See docs/development/TechnicalDesign.md, "Testing".

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly REPO_ROOT

readonly RESULTS_ROOT="${REPO_ROOT}/.artifacts/allure/results"
readonly RUST_RESULTS="${RESULTS_ROOT}/rust"
readonly SYSTEM_RESULTS="${RESULTS_ROOT}/system"
readonly REPORT_DIR="${REPO_ROOT}/.artifacts/allure/report"

status=0

if ! command -v allure >/dev/null 2>&1; then
	echo "error: allure is not on PATH; run 'devenv tasks run setup:allure'" >&2
	exit 1
fi

rm -rf "${RUST_RESULTS}" "${SYSTEM_RESULTS}" "${REPORT_DIR}"
mkdir -p "${RUST_RESULTS}" "${SYSTEM_RESULTS}"

{
	printf 'project=mnemorium\n'
	printf 'branch=%s\n' "$(git -C "${REPO_ROOT}" rev-parse --abbrev-ref HEAD)"
	printf 'commit=%s\n' "$(git -C "${REPO_ROOT}" rev-parse --short HEAD)"
	printf 'rust=%s\n' "$(rustc --version)"
	printf 'python=%s\n' "$(python3 --version)"
} >"${SYSTEM_RESULTS}/environment.properties"

echo "==> Running Rust tests"
if ! (
	cd "${REPO_ROOT}"
	ALLURE_RESULTS_DIR="${RUST_RESULTS}" cargo test --no-fail-fast
); then
	echo "error: the Rust tests failed" >&2
	status=1
fi

echo "==> Running system tests"
if ! (
	cd "${REPO_ROOT}"
	ALLURE_RESULTS_DIR="${SYSTEM_RESULTS}" "${SCRIPT_DIR}/system.sh"
); then
	echo "error: the system tests failed" >&2
	status=1
fi

echo "==> Generating the Allure report"
allure generate "${RUST_RESULTS}" "${SYSTEM_RESULTS}" --output "${REPORT_DIR}" --clean
echo "Report: ${REPORT_DIR}/index.html"

if [[ -n ${OPEN:-} ]]; then
	allure open "${REPORT_DIR}"
fi

exit "${status}"
