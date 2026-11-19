# Design

## Context

See `proposal.md` — Why. The constraints that shape the approach:

- Each setting already exists in `src/lib/domain/model/configuration.rs` with a default and a validation range; this
  change adds no key and changes no default.
- `openspec/specs/configuration/spec.md` owns configuration as a mechanism (layers, sparse file, refusal, reporting,
  immutability) and states in its Purpose that each setting's meaning belongs to the capability that consumes it.
- Four capabilities already specify the behaviour those settings steer, but describe them generically ("the configured
  maximum"). Five settings are already named by path, so the naming convention is established rather than invented.
- `REQ-UPLOAD-015` is the one requirement whose setting (`asset.upload.reap_batch_size`) has no stated effect at all.

## Goals / Non-Goals

**Goals:**

- Every in-scope setting is greppable from the durable spec that owns its behaviour.
- `asset.upload.reap_batch_size` gains a stated effect without changing what the reaper does.
- Every `REQ-*` and `SCN-*` identifier and every normative statement survives the archive unchanged in meaning.

**Non-Goals:**

- No new capability, requirement or scenario; no change to accepted values, defaults, validation or behaviour.
- No edit to `openspec/specs/configuration/spec.md`.
- No spec for `persistence.sqlite3.max_connections`.
- No code, test or `docs/development/` change, and no mechanical "key must appear in a spec" check.

## Decisions

### D1 — Name the path in the requirement, and in the scenario that parameterizes it

Each setting's full path goes into the description of the requirement that owns its behaviour, and into the `WHEN`
clause of a scenario only where that scenario varies the setting. Scenarios that merely observe an outcome keep their
existing wording.

Alternative: name the path only in scenarios (rejected — a reader arrives from the key, not the scenario, and several
requirements have no scenario that sets the value). Alternative: name it in every scenario of the requirement (rejected
— repetitive, and it implies the setting is a precondition of scenarios that do not depend on it).

### D2 — One delta per capability, never a central settings table

The naming lives in `upload`, `media-ingest`, `logging` and `authentication`, because those specs already own the
behaviour. The `configuration` spec stays the mechanism spec.

Alternative: add a "setting → owning requirement" registry to `configuration/spec.md`. Rejected — it would create a
second source of truth that can drift from the capability requirement, and it contradicts the spec's own Purpose. The
existing startup-only registry in `TechnicalDesign.md` § 10.1 is an operator-facing table, not a behaviour contract.

### D3 — `REQ-UPLOAD-015` gains the bounded-batch clause (option A)

The requirement now says the reaper runs every `asset.upload.reap_interval_seconds` and that each phase handles at most
`asset.upload.reap_batch_size` items.

Alternative: name only the interval and leave `reap_batch_size` unnamed (option B). Rejected — it would name a setting
with no stated effect, which reads as coverage while hiding the one place where implementation detail escapes the spec.

### D4 — No new scenario for the batch bound

The batch clause states what `ReapUploads` already does (`take(batch_size)` per phase). Adding a scenario would declare
fresh testable behaviour, which the proposal excludes and which would require a new system or application test. The
clause is therefore descriptive of existing behaviour, and the existing reaping scenarios continue to own the proof.

Alternative: add `SCN-UPLOAD-015-F` proving the bound. Rejected for this change; if the bound later needs proof, it is a
behaviour change of its own and should bring its test.

### D5 — Leave `persistence.sqlite3.max_connections` and the configuration spec alone

The pool size has no consuming capability: it is consumed by `init_db` in the composition root and already recorded as a
startup-only setting in `TechnicalDesign.md` § 10.1. The configuration spec's Purpose already delegates meaning, so the
user's decision is to add nothing there.

### D6 — Preserve the blocks whole

Each delta reproduces its requirement from `### Requirement:` through the last scenario, so archive replaces exactly the
block and nothing is lost. Header text, identifiers and scenario count are unchanged; only prose gains the paths.

## Risks / Trade-offs

- **Requirement length** — `REQ-AUTHENTICATION-008` grows from 491 to ~577 characters, past the schema's 500-character
  guidance for a main-spec description. → The change still validates with `--strict`; the warning appears only after
  archive, and the principled fix (splitting the requirement into one behaviour each) is a separate change, as the
  schema itself directs. `REQ-UPLOAD-015` is the next closest at 456, so no further clause should be added there without
  the same split.
- **Repetition** — a block can name one path three times (e.g. `logging.max_files` in `REQ-LOGGING-002`). → Accepted:
  greppability is the point, and each mention carries a different fact (period, bound, zero-means-unbounded).
- **Drift can recur** — nothing mechanically checks that every setting is named. → Out of scope by decision; the
  immediate risk is the 14 silent keys, and this change removes it. A rule or script can follow if the convention holds.
- **Scenario wording changes are not behaviour changes** — the scenarios keep their identifiers and outcomes, but a test
  labelled `SCN-AUTHENTICATION-008-C` now reads a key name. → No test edit is needed; the Allure labels are unchanged
  and the asserted outcomes are identical.
