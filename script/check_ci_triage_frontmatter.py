#!/usr/bin/env python3
"""Assert the CI triage agent's frontmatter loads and its containment holds.

Parse `.opencode/agents/ci-triage.md` rather than grepping static text: a
malformed frontmatter would take the containment deny-list down with it. opencode
applies permissions last-match-wins, so the guard asserts each required deny by
action *and* resource, that no later allow for the same action neutralizes it,
and the exact `external_directory` rule sequence.

Lives in `script/` and is run by the CI `python` job. See
docs/development/TechnicalDesign.md § 5 (CI and prompt assets), `TEST-051`.
"""

import re
import sys
from pathlib import Path

import yaml

AGENT = Path(".opencode/agents/ci-triage.md")

# (action, resource) pairs that must be denied and not re-allowed later. The
# `read`/`grep` denies must follow their broad allows to win under last-match.
REQUIRED_DENIES = {
    ("read", "*.env*"),
    ("grep", "*.env*"),
    ("shell", "*"),
    ("webfetch", "*"),
    ("websearch", "*"),
    ("edit", "*"),
    ("subagent", "*"),
    ("question", "*"),
}

# The full external_directory rule sequence (last match wins). The agent reads no
# file outside the worktree, so no allow is expected.
EXTERNAL_DIRECTORY_RULES = [("*", "deny")]


def fail(message: str) -> None:
    sys.exit(f"ci-triage.md: {message}")


def parse_frontmatter(text: str) -> dict:
    parts = re.split(r"(?m)^---\s*$", text, maxsplit=2)
    if len(parts) < 3:
        fail("missing YAML frontmatter")
    return yaml.safe_load(parts[1]) or {}


def main() -> None:
    if not AGENT.is_file():
        fail("missing")
    frontmatter = parse_frontmatter(AGENT.read_text(encoding="utf-8"))
    if frontmatter.get("mode") != "primary":
        fail("mode is not primary")

    permissions = frontmatter.get("permissions", [])
    for action, resource in sorted(REQUIRED_DENIES):
        deny_index = next(
            (
                index
                for index, rule in enumerate(permissions)
                if (rule.get("action"), rule.get("resource")) == (action, resource) and rule.get("effect") == "deny"
            ),
            None,
        )
        if deny_index is None:
            fail(f"missing deny rule for {action} {resource}")
        later_allow = next(
            (
                rule
                for rule in permissions[deny_index + 1 :]
                if rule.get("action") == action and rule.get("effect") == "allow"
            ),
            None,
        )
        if later_allow is not None:
            fail(f"deny for {action} {resource} is neutralized by a later allow: {later_allow}")

    external = [
        (rule.get("resource"), rule.get("effect")) for rule in permissions if rule.get("action") == "external_directory"
    ]
    if external != EXTERNAL_DIRECTORY_RULES:
        fail(f"external_directory rules must be {EXTERNAL_DIRECTORY_RULES}, got {external}")

    print("ci-triage frontmatter OK")


if __name__ == "__main__":
    main()
