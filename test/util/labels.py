"""Scenario labels for the system suite.

A case declares what it proves with one decorator per scenario:

    @scenario("SCN-USER-004-A", TestType.POSITIVE)
    @scenario("SCN-USER-004-B", TestType.NEGATIVE)
    def test_the_email_rule(...) -> None: ...

The identifier carries the capability and the requirement, so neither is repeated at the call site and neither can
disagree with the specification. A type is optional: an untested-but-written case may cite its scenario before the
proof's kind is decided, and `docs/development/artifacts/SystemTestCoverage.md` counts that scenario as unclassified
until it is.

The type is also a pytest marker, registered in `pyproject.toml`, so `pytest -m security` selects the security proofs
without a run-time filter. `--strict-markers` turns an unregistered type into a collection error rather than a silent
no-op.
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
    rather than the investigation generator.
    """
    if not _IDENTIFIER.fullmatch(identifier):
        raise ValueError(f"{identifier!r} is not a scenario identifier of the form SCN-<capability>-<number>-<letter>")

    def decorate(case: _Case) -> _Case:
        declared: list[Scenario] = getattr(case, "__scenarios__", [])
        declared.insert(0, (identifier, kind))
        case.__scenarios__ = declared
        if kind is not None:
            return getattr(pytest.mark, kind.value)(case)
        return case

    return decorate
