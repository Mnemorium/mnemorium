# Proposal

## Why

The user account is implemented end to end — creation, read, list, self, update and deletion — but its contract is split
across two capability specs that cover only creation and deletion, and predate the repository's `REQ-*`/`SCN-*`
requirement convention. `GET /user`, `GET /user/{id}`, `GET /user/me` and `PATCH /user/{id}` have no spec at all, and
the two existing specs mix the account contract with authorization rules that belong to the authorization capability. A
reader cannot tell what an account is, what an update guarantees, or which requirements describe the account and which
describe who may touch it.

## What Changes

- Add a `user` capability at `openspec/specs/user/spec.md` that owns the account contract: the account model
  (identifier, username, email, role), its profile, and the create, read, list, self, update and delete operations.
- Retire `user/account-creation` and `user/account-deletion`. Their account semantics move into the new capability;
  their authorization requirements are discarded here and recorded in `design.md` as a handoff to the authorization
  workstream, which is being specified separately.
- State the boundary once, in the new capability's Purpose: authorization and authentication are specified by their own
  capabilities. The new spec neither restates nor references their requirements.
- Make email uniqueness case-insensitive: the submitted value is stored unchanged, but `Alice@example.com` and
  `alice@example.com` name one address, and the `email` filter follows. **BREAKING** for a client that relied on two
  case-variant addresses coexisting.
- Reconcile `chk_user_email` with the address parser: reject a value with surrounding whitespace or an empty side of the
  `@`, and stop requiring a dot in the domain, which currently rejects `admin@mailserver1` even though the parser
  accepts it.
- Prove the new capability's scenarios black box in `test/system/user.py`; the creation and deletion cases that already
  prove account outcomes in `test/e2e/user/` move there and are deleted once replaced.
- No other API, code or configuration behavior changes. The endpoints already exist; this change records their contract,
  corrects email uniqueness and the datastore check, and proves the result.
- Record the remaining contract gaps the spec must not silently bless — the unpaginated collection, the absence of a way
  to clear an email, and the absence of self-service profile update — as decisions in `design.md`.

## Capabilities

### New Capabilities

- `user`: the user account — its attributes and profile, and the operations that create, read, list, update and delete
  it, plus the caller's own-account read. Authorization and authentication are out of scope.

### Modified Capabilities

- `user/account-creation`: retired. Its account requirements move to `user`; its authorization requirements are handed
  to the authorization workstream.
- `user/account-deletion`: retired. Its account requirements move to `user`; its authorization requirements are handed
  to the authorization workstream.

## Impact

- **Specs**: a new `user` capability is created; `user/account-creation` and `user/account-deletion` lose every
  requirement and their files are deleted on archive; `identity/authorization` and `authentication` are untouched.
- **Tests**: a new `test/system/user.py` proves the capability's scenarios, including the case-insensitive email rules
  at creation, update and filter. The cases it replaces in `test/e2e/user/uc001_create_a_user_account.py` and
  `test/e2e/user/uc005_delete_a_user_account.py` are deleted with it, so no scenario is proved twice; the files keep
  only the journey cases the spec does not own. Adapter tests prove the datastore email check and the column collation,
  which the container cannot observe; no application test asserts the uniqueness rule.
- **Persistence**: `migrations/20260904012829_create_user.up.sql` gives `email` a `NOCASE` collation and reconciles
  `chk_user_email`. The pair is edited in place while the schema is unreleased (`PERS-014`), so a local database must be
  recreated.
- **API**: a case-variant email now answers `409 Conflict` instead of creating a second account, and `GET /user?email=…`
  matches case-insensitively. **BREAKING** for a client that relied on two case-variant addresses. No route, request or
  response schema changes.
- **Code**: no Rust behavior changes beyond the email lookup inheriting the column collation and the adapter tests.
- **Docs**: none required. `docs/development/UseCases.md` and `docs/Permissions.md` stay as they are.
- **Risk**: the discarded authorization requirements have no owner until the authorization spec lands, and the
  case-insensitive rule breaks a client that relied on case-distinct addresses. `design.md` records both.
