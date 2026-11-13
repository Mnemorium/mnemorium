# Proposal

## Why

The end-to-end suite has been replaced scenario by scenario by the system suite, which starts a fresh container per case
and proves the same outcomes without cross-test state. What is left in `test/e2e/` is 46 tests: 26 duplicate journeys, 7
cases written against the retired scope-based permission model, and 13 cases that are the only proof of a handful of
exception paths. Keeping the directory costs a second harness, a second image lifecycle and a permanent drift risk,
while six requirements or exception paths would lose their only proof if it were deleted as-is.

## What Changes

- Retire `test/e2e/` (27 files, 46 tests, its fixture and its scope-model fixtures) and `script/e2e.sh`.
- Add black-box system cases for every requirement the E2E suite alone proves:
  - `SCN-API-015-C` — an unauthenticated caller following an index link to a protected entry receives `401`.
  - `REQ-AUTHORIZATION-019` `-A`…`-E` — account administration: an **Admin** creates a **Standard User**, cannot create
    an Admin, cannot delete an Admin peer or itself, the root manages Admins, and the root account is untouchable.
  - `REQ-AUTHORIZATION-020` `-A` — an **Admin** replaces a **Standard User**'s grants. (`-B` and `-C` already have Rust
    proofs.)
  - `SCN-AUTHORIZATION-017-A` — the **Root Admin** reads, contributes to and deletes another caller's private gallery.
  - `SCN-GALLERY-006-C` — a readable **Gallery** the caller may not delete answers `403`.
  - `SCN-AUTHORIZATION-038-A` — adding to or removing from a Gallery the caller cannot read answers `404`.
- Delete the references to the retired suite: `pytest.ini`, the CI `system` job, `.dockerignore`, the build hint in
  `script/configuration.sh` and `script/system.sh`, `AGENTS.md`, `README.md`, the implementation and review agent
  prompts, and the testing section of `TechnicalDesign.md`.
- Correct the documentation that still describes the retired designation: the UC-024 entry in `UseCases.md` (per-user
  action scopes) and the UC-009/UC-020 alternative flow 4a ("an Admin … uses every file").
- Do **not** migrate the stale cases: `uc024_manage_user_permissions` (the per-user override route no longer exists) and
  `uc009`'s "an Admin can add another caller's media" (a private typed asset is owner-only under the resource-and-grant
  model).
- Leave two residuals explicitly out of scope: no shared-instance soak coverage, and the `test-e2e` commit-scope id.

## Capabilities

### New Capabilities

None. This change removes a test suite and adds proofs for behavior that already exists; it introduces no capability.

### Modified Capabilities

None. Every requirement the new system cases carry is already published — `api`, `authorization`, `gallery` and `user` —
and no requirement text changes. The change adds the missing black-box proof and retires documentation drift.

## Impact

- **Tests**: `test/e2e/**` deleted; cases added in `test/system/api.py`, `test/system/authorization.py` and
  `test/system/gallery.py`.
- **Tooling/CI**: `script/e2e.sh` deleted; `pytest.ini`, `.github/workflows/ci.yml`, `.dockerignore` and the two suite
  scripts updated.
- **Documentation**: `docs/development/TechnicalDesign.md`, `docs/development/UseCases.md`, `AGENTS.md`, `README.md`,
  and the implementation/review agent prompts.
- **No** runtime, API, schema, migration or dependency change.
