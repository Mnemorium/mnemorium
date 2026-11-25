"""Scenario labels for the system suite.

A case declares what it proves with one decorator per scenario:

    @scenario("SCN-USER-004-A", TestType.POSITIVE)
    @scenario("SCN-USER-004-B", TestType.NEGATIVE)
    def test_the_email_rule(...) -> None: ...

The identifier carries the capability and the requirement, so neither is repeated at the call site and neither can
disagree with the specification. A type is optional: an untested-but-written case may cite its scenario before the
proof's kind is decided, and `docs/development/artifacts/RequirementsTraceabilityCoverage.md` counts that scenario as
unspecified until it is.

A case whose scenarios all prove one kind may declare it once for the whole case, and every scenario inherits it:

    @positive
    @scenario("SCN-USER-001-A")
    @scenario("SCN-USER-005-A")
    def test_a_created_account_reads_back_its_profile(...) -> None: ...

A scenario that names its own type keeps it, so a case that mixes kinds writes the type per scenario and carries no
case-level decorator. The two forms may sit in either order: the case-level decorator fills what a scenario leaves open.

Every type is also a pytest marker, registered in `pyproject.toml`, so `pytest -m security` selects the security proofs
without a run-time filter. A case-level type marks the case, so a case that declares two kinds is selected by each of
them. `--strict-markers` turns an unregistered type into a collection error rather than a silent no-op.
"""

import re
from collections.abc import Callable
from enum import StrEnum
from typing import Any, TypeVar

import pytest

_IDENTIFIER = re.compile(r"SCN-[A-Z0-9-]+-\d{3}-[A-Z]")

_Case = TypeVar("_Case", bound=Callable[..., Any])


class TestType(StrEnum):
    """The kind of proof a scenario carries; a closed set."""

    # Pytest treats any module-level class named `Test*` as a test class; this
    # one is a label vocabulary a case imports, never a case.
    __test__ = False

    POSITIVE = "positive"
    NEGATIVE = "negative"
    BOUNDARY = "boundary"
    SECURITY = "security"
    ERROR_HANDLING = "error_handling"
    RECOVERY = "recovery"
    ALTERNATE_FLOW = "alternate_flow"
    BUSINESS_RULE = "business_rule"


# One declared scenario: its identifier and the kind of proof it carries, when one is known.
Scenario = tuple[str, TestType | None]


def scenario(identifier: str, kind: TestType | None = None) -> Callable[[_Case], _Case]:
    """Declare that the decorated case proves one scenario, optionally of one type.

    Decorators stack; the declarations keep source order. The identifier is validated here so a typo fails collection
    rather than the investigation generator. A scenario that names no type inherits the case's, whichever decorator
    ran first.
    """
    if not _IDENTIFIER.fullmatch(identifier):
        raise ValueError(f"{identifier!r} is not a scenario identifier of the form SCN-<capability>-<number>-<letter>")

    def decorate(case: _Case) -> _Case:
        resolved = kind or getattr(case, "__test_type__", None)
        declared: list[Scenario] = getattr(case, "__scenarios__", [])
        declared.insert(0, (identifier, resolved))
        case.__scenarios__ = declared
        if resolved is not None:
            return getattr(pytest.mark, resolved.value)(case)
        return case

    return decorate


def _typed(kind: TestType) -> Callable[[_Case], _Case]:
    """Declare the kind every scenario of the decorated case inherits, unless a scenario names its own."""

    def decorate(case: _Case) -> _Case:
        case.__test_type__ = kind
        declared: list[Scenario] = getattr(case, "__scenarios__", [])
        case.__scenarios__ = [(identifier, own or kind) for identifier, own in declared]
        return getattr(pytest.mark, kind.value)(case)

    return decorate


# The case-level form: one decorator names the kind its scenarios share.
positive = _typed(TestType.POSITIVE)
negative = _typed(TestType.NEGATIVE)
boundary = _typed(TestType.BOUNDARY)
security = _typed(TestType.SECURITY)
error_handling = _typed(TestType.ERROR_HANDLING)
recovery = _typed(TestType.RECOVERY)
alternate_flow = _typed(TestType.ALTERNATE_FLOW)
business_rule = _typed(TestType.BUSINESS_RULE)
