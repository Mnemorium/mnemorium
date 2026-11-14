# Proposal

## Why

`instance/state` is the last lifecycle capability whose specification predates the repository's `REQ-*`/`SCN-*`
convention, and it has drifted in both directions. It names configuration ordering and logging **security events** that
belong to other capabilities, while the state behaviour the server actually enforces — a malformed seed, an undecidable
datastore presence, an oversized document, an unsupported format version, the fixed `state.yaml` path, and a restored
state file readable by another account — is either absent or only implicit. Its proof sits in the configuration suite,
even though the state is a startup lifecycle, not a setting: the suite cannot label a scenario and its gate keys on
prose headings.

The in-flight `specify-logging` change already removes the creation-event and **redaction** requirement from
`instance/state`; `specify-configuration` leaves the state's secrets outside configuration. This change makes the state
capability own only the state, retires the old path, and moves the proof to the system suite that proves capability
scenarios black box.

## What Changes

- Add a top-level `state` capability at `openspec/specs/state/spec.md` with eight requirements: the site pepper, the
  token signing secret, the creation instant, creation and seeds, the state-before-datastore rule, the missing-state
  rule, persistence, and privacy.
- Retire `instance/state`. Its state requirements move to `state`; its logging requirement is already owned by the
  in-flight `logging` capability and is not restated here.
- Remove configuration ordering from the state spec. The state owns "before the datastore"; the ordering against
  configuration stays a startup statement in `TechnicalDesign.md` § 10.2. No `configuration` or `configuration/loading`
  delta is created.
- Add the behaviour the code already enforces but the spec omits: a malformed seed fails startup; an undecidable
  datastore presence fails closed; an oversized document is refused; any unsupported format version is refused; the
  fixed `state.yaml` path is part of the contract; the seed names are fixed.
- Add one behaviour the code does not enforce yet: a restored state file with a wider mode is tightened to `0600` with
  `std::fs::set_permissions` before it is read. **BREAKING** only for a restore the server cannot make private — a file
  owned by another account, or a file whose mode cannot be changed — which fails startup.
- Move the proof from `test/configuration/instance_state.py` and `test/configuration/test_scenarios.py` to
  `test/system/state.py`, with Allure labels; the system suite keeps no separate scenario map or meta-test.
- Update `TEST-053`, `TEST-054`, `TEST-055`, the configuration-test and system-test narratives, `pytest.ini`, § 10.2 and
  the glossary. No HTTP, OpenAPI or migration change is planned.

## Capabilities

### New Capabilities

- `state`: the instance's non-derivable state — the site pepper, the token signing secret and the creation instant; how
  it is seeded and created; how it couples to the datastore; and how it is persisted, versioned and kept private. It
  does not own configuration or logging.

### Modified Capabilities

- `instance/state`: retired. Its five state requirements are restated in `state`; its logging requirement is owned by
  the in-flight `logging` capability. The old spec is deleted on archive.

## Impact

- **Specs**: `state` is created; `instance/state` is retired through a `REMOVED` delta. No configuration or logging
  capability is modified.
- **Code**: `FileInstanceStateStore::load` gains a private-file check (regular file, owned by the server account) and
  repairs a wider mode with `std::fs::set_permissions` before reading; a new store error and `LoadStateError` mapping
  cover a foreign owner or a failed repair. The datastore-presence probe gets a seam so the undecidable case is
  testable. The `state.yaml` format is unchanged.
- **Tests**: a new `test/system/state.py`; `test/configuration/instance_state.py` and
  `test/configuration/test_scenarios.py` are deleted; the state cases carry `epic("State")`, their `REQ-STATE-*` and
  `SCN-STATE-*` labels.
- **Docs**: `TechnicalDesign.md` § 5 (`TEST-053`–`TEST-055`, configuration-test and system-test narratives) and § 10.2;
  `Overview.md` § Startup; `Glossary.md` (`Instance state`, `Site pepper`, `Token signing secret`, `Instance root`);
  `README.md` backup mode and ownership; `pytest.ini`.
- **Dependencies**: none. The owner check compares the state file's owner with the working directory's owner, so no
  process-UID crate is needed.
- **Integration**: `specify-logging` currently carries an `instance/state` delta and `specify-configuration` records the
  state gate and the configuration ordering as owned by `instance/state`. This change retires that path and removes the
  gate; the two in-flight changes must land first or be rebased in the same integration window.
- **API and persistence**: no route, representation, status, schema or migration change.
- **Risk**: a restore with a wider mode is repaired on start; a restore owned by another account refuses startup, and
  the remedy is `chown` to the account that owns the working directory.
