#!/usr/bin/env bash
# Regenerate the requirement and scenario matrix at docs/development/artifacts/SystemTestCoverage.md.
#
# The generator reads the specification tree and the suites' own annotations, never pytest and never a container:
# `test/util/investigation.py` parses `openspec/specs/**/spec.md`, the `@scenario` decorators of `test/system/*.py` and
# the `// proves:` comments of `src/**/*.rs`. Authored `Cause` and `N/A` cells are preserved, so a regeneration with
# nothing changed produces no diff.
#
# Prettier formats the file when it is on PATH (it always is in the dev shell); the generator itself is standard
# library only and emits Prettier's table padding.
#
# See docs/development/Test.md, "System test coverage".

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly REPO_ROOT
readonly ARTEFACT="${REPO_ROOT}/docs/development/artifacts/SystemTestCoverage.md"

PYTHONPATH="${REPO_ROOT}/test/util" python3 -m investigation "${REPO_ROOT}"

if command -v prettier >/dev/null 2>&1; then
	prettier --write "${ARTEFACT}"
fi
