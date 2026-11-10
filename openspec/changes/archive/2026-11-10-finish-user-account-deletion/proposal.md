# Proposal

## Why

UC-005 "Delete User Account" is catalogued in `docs/development/UseCases.md` but has never been built: there is no
`DELETE /api/v1/user/{id}` route, no application use case, no capability spec, and no E2E file. `rule_enforcer.yaml`
carries an explicit `TODO` that removes the `TEST-039` exemption once that E2E file lands, and `UserRepository::delete`
is declared `dead_code` because no caller exercises it.

The capability cannot be completed without deciding two things the code does not currently answer: who may delete whom,
and what happens to a user's media that another user's gallery or the shared catalogue still references.

## What Changes

- Add `user.delete` to the permission catalog: `NONE` for Standard, `ALL` for Admin, Root Admin short-circuit;
  configurable like the other `user.*` actions.
- Extend the relation guard: a caller who is not the Root Admin may delete only a non-Admin account. The Root Admin may
  delete an Admin, and its own account is never deletable.
- Add `DELETE /api/v1/user/{user_id}` answering `204`, with `401`, `403` and `404` for the documented refusals.
- Orchestrate the deletion in one unit of work: permission overrides, galleries and their items, playlists and their
  tracks, upload sessions and their chunks, the user's files and private media rows, then the user and its credential.
- Private media (no referrer outside the user's own resources): delete the media row, the file row, and the bytes after
  the commit.
- Shared media (referenced by another user's gallery or playlist, or by the catalogue): transfer the file row to the
  Root Admin (`user_id = 0`) and keep the media row and its bytes.
- On transfer, when the Root Admin already owns a file with the same integrity hash, clear the digest (`NULL`) on the
  transferred row, because `uq_file_user_id_integrity_hash` ignores `NULL`.
- Revise the UC-005 entry and `docs/Permissions.md` to match, remove the `TEST-039` exemption, and add the E2E file.

**BREAKING**: none. The change adds an endpoint and a permission action; no existing behavior changes.

## Capabilities

### New Capabilities

- `user/account-deletion`: deleting a user account — authorization and target rules, atomic removal of owned data,
  disposition of shared media, and rejection of a deleted user's tokens.

### Modified Capabilities

- `identity/authorization`: the permission catalog gains `user.delete`, and the Admin preset and relation guard describe
  deletion of non-Admin accounts.

## Impact

- `src/lib/domain/model/permission.rs` (new action), `src/lib/domain/service/authorization.rs` (relation guard).
- New `application/port/delete_user.rs` and `application/use_case/delete_user.rs`, plus factory wiring.
- New `infrastructure/inbound/rest/handler/user/delete_user.rs`, route registration and OpenAPI declaration.
- `domain/port/file_repository.rs` and the SQLx adapter: reassign ownership and clear the digest on transfer.
- `docs/development/UseCases.md` (UC-005), `docs/Permissions.md` (action matrix and guards).
- `rule_enforcer.yaml` (`TEST-039` exemption) and `test/e2e/user/uc005_delete_a_user_account.py`.
- No schema change and no migration.
