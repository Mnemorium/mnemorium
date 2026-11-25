#!/usr/bin/env bash
# Regenerate the requirement and scenario report at docs/development/artifacts/RequirementsTraceabilityCoverage.html.
#
# The generator reads the specification tree and the suites' own annotations, never pytest and never a container:
# `test/util/investigation.py` parses `openspec/specs/**/spec.md`, the `@scenario` decorators of `test/system/*.py`, the
# scenario manifest `test/api/scenarios.toml` and the `// proves:` comments of `src/**/*.rs`. Authored annotations live
# in `RequirementsTraceabilityCoverage.json` beside the report and are never written by the generator.
#
# With `--check` the report is rendered in memory and compared with the committed file; nothing is written, and a stale
# report fails. The pre-commit hook and the investigation workflow use that mode.
#
# See docs/development/Test.md, "Requirements traceability coverage".

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly REPO_ROOT

PYTHONPATH="${REPO_ROOT}/test/util" python3 -m investigation "${REPO_ROOT}" "$@"
