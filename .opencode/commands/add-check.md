---
description: Plan the mechanical check that enforces one rulebook rule — dispatch the rule-guard subagent with the rule ID and present its plan; apply nothing
---

Target: $ARGUMENTS

## Inputs

- **Target** — the command argument (`$ARGUMENTS`): one rule ID from
  `docs/development/TechnicalDesign.md`, for example `STY-RUST-010`.

## Posture

- Read-only. This command plans a check and applies nothing.
- One question at most: when the target is absent or names no rulebook rule.

## Steps

1. Confirm `$ARGUMENTS` names a rule with a row in the rulebook. When it is
   empty, run `devenv shell -- script/rule_enforcer.sh uncovered` and ask which
   rule to target. When it names no row, stop and say so.
2. Dispatch the `rule-guard` subagent with the rule ID and the instruction to
   plan the check and return it, applying nothing. Wait for its plan.
3. Print the plan unchanged.

## Report

```markdown
# Add check — <RULE-ID>

<the rule-guard plan, verbatim>

## Next

- Apply: <the files the plan names, or "none">
- Verify: <the plan's verification command>
```
