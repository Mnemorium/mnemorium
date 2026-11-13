# Design

## Context

See `proposal.md` — Why.

The logging contract currently has three homes: `docs/development/TechnicalDesign.md` § 9 holds eight rules (`OBS-001` …
`OBS-008`) and the 32-row catalog its `check_obs_006` parses; `src/lib/infrastructure/logging.rs` holds the two sinks
and the non-suppressible floor; and six capability specs state individual events as prose. 197 emission sites already
carry `target: "security"`, so the behaviour exists — only its owner does not.

Three constraints shape the approach:

- **The schema forbids some edits.** `openspec validate --strict` rejects a `##` section outside a requirement (archive
  would drop it) and rejects a `MODIFIED` block that omits a scenario the current spec still has. Both were verified
  against this change.
- **The suite already observes logs black box.** `test/configuration/harness.py` exposes `Sut.logs()` (container
  stdout), `Sut.log_text()` (the concatenated log directory), `Sut.listing()` and `Sut.read()`, and
  `test/system/authentication.py` already matches `event="…"` on those lines. The system suite can therefore own the
  scenarios rather than delegating to Rust.
- **Rotation is observed without a clock.** The existing `test/configuration/logging_settings.py` proves rotation by the
  file name and pruning by a pre-seeded `logs/` directory, so no case waits for a period to elapse. The same technique
  carries over.

## Goals / Non-Goals

**Goals:**

- One normative home for logging: sinks, verbosity, layer, once-only, content, redaction, severity, the closed catalog,
  and every event's trigger, severity and fields.
- A catalog the build can still verify mechanically — parsed from the specification instead of the rulebook.
- Scenarios the system suite can prove attached to their requirement, with Rust keeping only what the container cannot
  provoke.
- No behaviour change: every requirement describes what the server already does.

**Non-Goals:**

- `OBS-008` (external parsers on untrusted input) stays a rulebook obligation with its own check.
- Log shipping, retention and collection outside this server.
- A default, range or enum spelling for any `logging` setting.
- New events, new severities or new sinks.

## Decisions

### 1. The boundary is the sentence's subject

A clause moves to `logging` when its subject is a log sink or a catalogued event; it stays when its subject is the
capability's own state. `authentication` keeps "the stored credential holds only a hash"; `logging` takes "a login
success records `authn_succeeded`". `media-ingest` keeps what a `removed` count means; `logging` takes the `ingest`
event's seven fields and its counts-only rule.

_Alternatives considered:_ keep events with their emitting capability and let `logging` own the mechanism only (rejected
— the in-flight `upload` capability names no event, so `upload` and `upload_reap` would stay unspecified); move whole
requirements into `logging` (rejected — it would duplicate "what a removed count means" and dissolve the capability
boundaries the rest of the spec tree relies on).

### 2. The catalog lives inside `REQ-LOGGING-010`

A table after the requirement's scenarios, introduced by the literal line `The security-event catalog:`, with the
columns event, severity, fields and recorded when. `REQ-LOGGING-010` states the properties (closed set, reserved target,
declared severity, exactly the declared fields, completeness); the table declares the content, including each event's
trigger.

_Alternatives considered:_ a `## Security-event catalog` section (rejected — `openspec validate` reports that archive
does not carry it into the main spec); one requirement per event, 32 of them (rejected — each description would be a
name and a field list, and the per-area requirements already give the scenarios their anchor); a table in `design.md`
(rejected — design is not normative).

Placing the rows after the scenarios keeps `REQ-LOGGING-010`'s description inside the 500-character budget.

### 3. Eighteen requirements: eleven mechanism, seven by emitting area

`001`–`011` state sinks, rotation, verbosity, non-suppression, layers, once-only, content, redaction, severity, the
closed catalog and the claimed-identity exception. `012`–`018` group the catalog by the area that triggers it
(authentication, authorization and business logic, faults, lifecycle and administration, library and metadata, upload,
personal-data access) and carry the interesting scenarios: one event per outcome, uniform rejection reason, counts only,
no content.

_Alternatives considered:_ one requirement per event (rejected above); one catalog requirement with every observation as
a scenario (rejected — it hides which capability triggers what, and 30 scenarios under one identifier make Allure
traceability useless).

### 4. `check_obs_006` parses the specification

`TechnicalDesign.md` § 9.6 becomes a citation of `REQ-LOGGING-010`; `check_obs_006` reads the catalog from
`openspec/specs/logging/spec.md`, anchored on `The security-event catalog:` and stopping at the next `### Requirement:`,
keeping its existing column indices and its existing "parsed no event" and completeness failures.

_Alternatives considered:_ leave the table in § 9.6 and cite it from the spec (rejected — the spec could not then
declare `upload` or `upload_reap`, and the normative document would be the rulebook, not the capability); generate one
from the other (rejected — a generated table needs a build step for a document no consumer reads at runtime).

The cost is that the rulebook reads `openspec/`. It is contained to one function and one anchor, and the parse fails
loudly rather than silently emitting nothing.

### 5. `logging` owns the meaning of its four settings; `configuration` owns resolution

`REQ-LOGGING-001`–`003` state what the sinks do, what rotation and pruning mean, and how a verbosity value is refused at
startup. `configuration` keeps how any value is resolved from the defaults, the file and the environment, and names no
setting.

_Alternatives considered:_ a per-setting requirement in `configuration` (rejected — that capability deliberately states
no setting, and these four values are meaningless without the sinks they configure).

### 6. `authorization` retires `027` and `028` and re-adds them as `038` and `039`

The schema refuses a `MODIFIED` block that drops a scenario, and both requirements carried an event scenario that must
leave. Retiring and re-adding keeps each remaining scenario and each non-event clause intact under a fresh identifier;
nothing in the repository references `027` or `028`.

_Alternatives considered:_ `MODIFIED` keeping a scenario that cites the logging event (rejected — it restates the
trigger the change is moving); leaving `authorization` untouched (rejected — two homes for one trigger).

The re-added requirements follow the existing numbering, so the identifiers are `REQ-AUTHORIZATION-038` (denials are
uniform and concealed) and `REQ-AUTHORIZATION-039` (a filtered listing is not a denial).

### 7. Witness per scenario, system first

| Scenario family                            | Witness                                                                                     |
| ------------------------------------------ | ------------------------------------------------------------------------------------------- |
| Sinks, colour, rotation, pruning           | **system**, through `Sut.logs`, `listing` and `read`; file content asserted after `stop()`  |
| Verbosity refused, reserved target refused | **system**, through the harness's start-expecting-failure path                              |
| Non-suppression                            | **system**: quiet verbosity, then a login                                                   |
| Layers                                     | **emission-site scan** by `grep`, because no black-box run can make the domain emit         |
| Once-only                                  | **system** for request paths; Rust `#[traced_test]` for faults the container cannot provoke |
| Content, redaction, escaping               | **system**                                                                                  |
| Catalog shape and completeness             | **system** assertions on emitted lines, plus the CI rule check                              |
| Area triggers                              | **system**, from the seeded instance                                                        |

_Alternatives considered:_ keep the cases in `test/configuration/logging_settings.py` (rejected — they prove logging
behaviour, not configuration resolution, and the change's scope is the system suite); assert everything in Rust
(rejected — log lines are observable black box, so the lower-cost black-box witness wins).

## Risks / Trade-offs

- **The rulebook starts reading `openspec/`.** → One anchored parser block, an explicit failure when the anchor is
  missing, and a task that verifies the anchor and the table's 32 rows directly, so CI's `OBS-006` run has a target.
- **Two normative tables during the transition.** → One task shrinks § 9.6 to a citation and re-points the check; the
  tasks order that after the spec lands and forbid leaving both.
- **Pruning and rotation are time-consuming to prove.** → Observe the rotated file name and seed `logs/` for pruning, as
  the existing cases do; no case sleeps for a period.
- **The non-blocking file writer buffers.** → Any file-content assertion stops the server first, which drops the
  `WorkerGuard` and flushes; the system harness already exposes `stop()`.
- **Renumbering `REQ-AUTHORIZATION-027`/`028`.** → Nothing references them today; the removal reason and migration name
  the replacements, so a later reader can follow the move.
- **A moved scenario may lose its Rust witness.** → The tasks pair each move with the label change and delete the old
  assertion, so no outcome is proven twice or not at all.

## Migration Plan

1. The specification lands with the catalog inside `REQ-LOGGING-010`.
2. `TechnicalDesign.md` § 9.6 cites the capability and `check_obs_006` reads the spec; CI's full rule check confirms the
   parse.
3. The system module and the moved cases are written test-first against the new identifiers; the configuration cases
   that only proved logging behaviour are deleted as they move.
4. Glossary, `Overview.md` and `docs/Permissions.md` point at the capability for the vocabulary they cite.

No data migration, no API change, no rollback beyond reverting the change: no runtime behaviour is touched.

## Open Questions

- Whether `OBS-002` (once-only) and `OBS-005` (severity) gain their own mechanical checks, as `OBS-006` has. Their
  requirements are stated and proven by scenarios either way; adding a check later changes no spec and no task here.
