# Proposal

## Why

The instance state's "Concurrent first starts create at most one identity" requirement promises behaviour the product
does not need to promise. Two servers pointed at one working directory is an operator error, not a supported topology,
and the requirement's only test is a two-container race that can fail for harness reasons without saying why. The store
already refuses to overwrite a persisted identity, so the safety property survives without the promise.

## What Changes

- Remove the `instance/state` requirement **Concurrent first starts create at most one identity** and its **Concurrent
  creation** scenario.
- Delete the configuration test that asserts it (`test/configuration/instance_state.py`) and its mapping in
  `STATE_SCENARIOS`.
- Delete the adapter test that races four writers on one store path
  (`file_instance_state_store_concurrent_creation_persists_once`).
- Keep the production mechanism: the exclusive lock and create-new write still make a second creator fail with
  `Conflict`, which the surviving **No partially written identity** requirement and its **Identity persistence is
  atomic, versioned and private** requirement still need.
- Fix the configuration harness's startup poll so a container observed as `created` is polled instead of reported as a
  startup failure, and so a startup that never publishes a port explains itself.
- Out of scope: any change to how the identity is loaded, created, persisted or logged.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `instance/state`: removes the requirement **Concurrent first starts create at most one identity**; no other
  requirement changes.

## Impact

- **Spec**: `openspec/specs/instance/state/spec.md` drops one requirement (7 → 6).
- **Tests**: `test/configuration/instance_state.py` loses one test and its scenario mapping;
  `src/lib/infrastructure/outbound/file_system/instance_state_store.rs` loses one unit test and three imports. No
  production code changes.
- **Test harness**: `test/configuration/harness.py` treats only `exited`, `dead` and `removing` as terminal, and
  `StartupFailed` names why a startup failed.
- **API**: no HTTP endpoint or payload changes, so `docs/openapi.json` is unchanged.
- **Documentation**: no rule or design-document edit — § 10.2's rationale for the lock belongs to the surviving
  atomicity requirement, and every `TEST-*` statement about the configuration suite stays true.
