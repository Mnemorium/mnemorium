"""The `TEST-056` gate: the instance-state module and the spec stay in step.

Three directions are checked. Every scenario of `openspec/specs/instance/state/spec.md` is mapped in `STATE_SCENARIOS`;
every `@pytest.mark.state_scenario` names a scenario the spec contains; and every test in `instance_state.py` carries a
marker. A decode-level scenario is mapped to the Rust store test that owns it as `"rust:<fn>"`, and that function must
exist in `src/`.
"""

import re
from pathlib import Path

import pytest
from instance_state import STATE_SCENARIOS

ROOT = Path(__file__).resolve().parents[2]
SPEC = ROOT / "openspec/specs/instance/state/spec.md"
STATE_PREFIX = "test/configuration/instance_state.py"


def scenarios() -> set[str]:
    """Every `<Requirement> / <Scenario>` pair the spec declares."""
    names: list[str] = []
    requirement = None
    for line in SPEC.read_text(encoding="utf-8").splitlines():
        if match := re.match(r"^### Requirement: (.+)$", line):
            requirement = match.group(1).strip()
        elif (match := re.match(r"^#### Scenario: (.+)$", line)) and requirement:
            names.append(f"{requirement} / {match.group(1).strip()}")
    return set(names)


def state_tests(request: pytest.FixtureRequest) -> dict[str, pytest.Item]:
    """The state module's tests, keyed by function name without its parameters."""
    return {item.originalname: item for item in request.session.items if item.nodeid.startswith(STATE_PREFIX)}


def marked_scenarios(request: pytest.FixtureRequest) -> dict[str, pytest.Item]:
    """The collected item each `state_scenario` marker names."""
    marked = {}
    for item in request.session.items:
        for mark in item.iter_markers("state_scenario"):
            marked[mark.args[0]] = item
    return marked


def test_every_scenario_is_mapped(request):
    """Each spec scenario is mapped, and each mapping resolves to a test or a Rust test."""
    missing = scenarios() - set(STATE_SCENARIOS)
    assert not missing, (
        "state scenarios without a test:\n  "
        + "\n  ".join(sorted(missing))
        + '\nAdd a test with @pytest.mark.state_scenario("<Requirement> / <Scenario>")'
        + " and a STATE_SCENARIOS entry in instance_state.py."
    )

    tests = state_tests(request)
    assert tests, f"no tests collected from {STATE_PREFIX}; check pytest's rootdir"

    rust_sources = "\n".join(path.read_text(encoding="utf-8") for path in (ROOT / "src").rglob("*.rs"))
    for scenario, target in STATE_SCENARIOS.items():
        if target.startswith("rust:"):
            function = target.removeprefix("rust:")
            assert f"fn {function}(" in rust_sources, f"{scenario!r} cites a missing Rust test: {target}"
        else:
            assert target in tests, f"{scenario!r} cites a missing test: {target}"


def test_every_marker_names_a_scenario(request):
    """No marker names a scenario the spec does not declare."""
    unknown = set(marked_scenarios(request)) - scenarios()
    assert not unknown, "markers name scenarios the spec does not contain: " + ", ".join(sorted(unknown))


def test_every_state_test_is_mapped(request):
    """No test of the state module is left without a marker."""
    tests = state_tests(request)
    assert tests, f"no tests collected from {STATE_PREFIX}; check pytest's rootdir"

    unmarked = [name for name, item in tests.items() if not list(item.iter_markers("state_scenario"))]
    assert not unmarked, "state tests without a state_scenario marker: " + ", ".join(sorted(unmarked))
