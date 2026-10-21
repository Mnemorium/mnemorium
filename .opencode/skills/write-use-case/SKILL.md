---
name: write-use-case
description: Authors one new business use case in docs/development/UseCases.md. Interviews the
  caller for the actor, goal, single triggering event and data, verifies the capability against the
  code, then writes the entry under the bounded-context section that owns it, adds its node to that
  section's diagram, and records the E2E obligation. Use when the user wants to add a business use
  case, catalog a capability, or asks how a use case is written in this project.
---

# Write a Use Case

Add one business use case to `docs/development/UseCases.md` without inventing format. The contract is
[TechnicalDesign.md](../../../docs/development/TechnicalDesign.md) § 5 (`DOC-005`, `DOC-007`, `DOC-008`); read it
first and follow it rather than this page when the two disagree.

## When to use

The caller wants to catalogue a business capability: a new one, or one that the catalog is missing. Not for adding
an endpoint, not for renaming an application use case, and not for recording a validation rule — rules belong to the
capability's specification (§ 5.6).

## Process

1. **Interview the caller.** Ask, and do not draft until each answer is concrete:
   - Who initiates it? One of the Glossary actors — `Standard User`, `Admin`, `Root Admin` — never “the system”.
   - What single event starts it? An external, discrete, state-changing event, not a goal, not a state, not the flow.
   - What data does that event carry? Business names and optionality, never field names.
   - What must already be true? That is `Pre condition(s)`, not the trigger.
   - What does the business guarantee afterwards? That is `Post condition(s)`.
   - What can go wrong on the way? Only flows worth a test.
2. **Verify against the code.** Locate the capability and its outcome in `src/` (the REST route, the application use
   case, the domain rule). Confirm the bounded context that owns it. If nothing implements it, say so and continue —
   the catalog is capability-oriented; file the gap as an issue instead of dropping the entry.
3. **Place it.** Append the entry to the section of the package that owns it — a bounded context (`## Identity`,
   `## User`, `## Asset`, `## Configuration`) or one of its sub-packages (`### Gallery`, `### Music` under
   `## Library`). Take the **next free** `UC-NNN` identifier; never renumber an existing entry, and never reuse a
   retired one.
4. **Draft the entry** in the § 5 order: `Description`, `Primary actor`, `Parameters`, `Pre condition(s)`, `Trigger`,
   `Happy path`, `Alternative flow` (only when needed), `Post condition(s)`. The entry heading is `###` in a context
   section and `####` in a sub-package; its fields sit exactly one level below it. Write business prose only, in every
   field: the entry names no route, no method and no payload field. Which entry point carries the event is the
   implementation's concern, because one use case may be served by several.
5. **Add it to the diagram.** In the same section, add the node inside the package —
   `usecase "UC-NNN - <business name>" as UCNNN` — and an association from every primary actor. `DOC-008` and
   `check_doc_008` fail until the node is there. List `Root Admin` only when the use case is theirs alone; otherwise
   `Admin` covers it.
6. **Record the E2E obligation.** The matching test is `test/e2e/<context>/uc<NNN>_<name>.py` (`TEST-041`). When the
   test is not part of the change, add the identifier to `exempt_use_cases` in `rule_enforcer.yaml` with a `TODO`
   comment naming the missing test — the precedent is UC-005.
7. **Verify.** Run `script/rule_enforcer.sh check` and the Markdown gates
   (`markdownlint-cli2 '**/*.md'`, `prettier --check '**/*.md'`). Report what ran.

## Interview anti-patterns

| The caller says | Why it fails | Ask instead |
| --- | --- | --- |
| “The endpoint `POST /x` creates a thing.” | a route is not an event, a route is not a goal, and a route never appears in an entry | “Who submits what, and what event does the business observe?” |
| “The user wants a playlist.” | a goal, not an event | “What does the user do that starts it?” |
| “The user is authenticated.” | a precondition | “Then what happens?” — and keep the first answer in `Pre condition(s)`. |
| “It validates the name, saves it, then returns it.” | the flow, folded into the trigger | “What single action starts the use case?” |
| “Add an Image or Video to a Gallery.” | two capabilities in one name | “Add an Image…” and “Add a Video…”, or “Add a Gallery Item” for the shared capability. |
| “It must be ≤ 100 characters.” | a validation rule | “What can the caller observe when it fails?” — the threshold goes to the specification. |

## Definition of done

- The entry sits under the owning package's section and carries every required field in order, one heading level below
  the entry.
- The name states one capability and joins no alternatives with “and” or “or”.
- `Trigger` is one external, atomic event; no field of the entry names its entry point.
- `Parameters` lists the event's data in business terms, or says the event carries none.
- No `#### Business rules`, `#### Bounded context(s)` or `#### Secondary actor` heading exists.
- The identifier is the next free one and appears once.
- The use case has its node in the section's diagram, with associations from its primary actors.
- The E2E file exists, or the exemption is recorded with a TODO.
- `script/rule_enforcer.sh check` passes.

## References

- [TechnicalDesign.md](../../../docs/development/TechnicalDesign.md) § 5 — the entry contract, trigger doctrine,
  parameters, vocabulary excluded from the narrative (`DOC-005`, `DOC-007`).
- [TechnicalDesign.md](../../../docs/development/TechnicalDesign.md) § 6 — the diagram conventions (`DOC-008`).
- [UseCases.md](../../../docs/development/UseCases.md) — the catalog; UC-013 is the worked example.
- [Glossary.md](../../../docs/development/Glossary.md) — the actors and the business terms a narrative field may use
  (`Caller`, `Owner`, `Visibility`, `Position`, `Last-modified time`, `Parameters`, `Triggering event`, `System`).
- [Overview.md](../../../docs/development/Overview.md) — the bounded contexts the sections are named after.
