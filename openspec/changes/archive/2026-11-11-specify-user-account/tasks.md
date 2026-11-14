# Tasks

## 1. Validate the specification delta

- [x] 1.1 Run `openspec validate specify-user-account --strict` and verify it passes; fix any requirement-description
      length or scenario-format finding in the delta files without changing the stated behaviour.
- [x] 1.2 Read the three delta files and verify the retirement is complete: `specs/user/account-creation/spec.md`
      removes all 7 requirements and `specs/user/account-deletion/spec.md` removes all 8, each with a **Reason** and
      **Migration**, and no requirement name differs from the main spec's header.
- [x] 1.3 Verify the new capability's identifiers: `specs/user/spec.md` carries 19 `REQ-USER-` requirements and 62
      `SCN-USER-` scenarios, every requirement has at least one scenario, and no identifier repeats.

## 2. System suite scaffold and case-insensitive email uniqueness

- [x] 2.1 Add `test/system/user.py` with the shared harness setup — start one instance per case, log in as the Root
      Admin through `Sut.root_password()` and `Sut.login` — and verify `pytest --collect-only -q test/system/user.py`
      lists it without adding an entry to `pytest.ini` (`test/system/conftest.py` collects the unprefixed module).
- [x] 2.2 Add the failing system cases for `SCN-USER-004-G` (creation), `SCN-USER-010-E` (filter) and `SCN-USER-013-D`
      (update), each labelled with its requirement and scenario; run them against the current image and verify they fail
      for the expected reason — a case variant creates a second account, the filter misses it and the update succeeds.
- [x] 2.3 Add the failing SQLx adapter case in `src/lib/infrastructure/outbound/sqlx/user_repository.rs`: `save` returns
      `RepositoryError::AlreadyExist` for an email differing from a stored address only by case, `search` with a
      case-variant email returns the stored row, and the stored value keeps its submitted case; run it and verify it
      fails against the current collation.
- [x] 2.4 Give `email` a `NOCASE` collation in `migrations/20260904012829_create_user.up.sql` in place (`PERS-014`) so
      `uq_user_email` and the repository's `email = ?` filter compare case-insensitively; rebuild the image, run the
      adapter case and the three system cases, and verify all pass and the stored value keeps the submitted case.
- [x] 2.5 Recreate the local database (`mnemorium.db` and its `-wal`/`-shm` sidecars), start the server, and verify the
      edited migration applies and `GET /user?email=Alice@example.com` returns the account stored as
      `alice@example.com`.

## 3. Datastore email check

- [x] 3.1 Add the failing adapter cases with a raw `INSERT INTO user`: values the check must reject (empty, surrounding
      whitespace, a missing side of the `@`) and parser-accepted edge cases it must keep accepting (`admin@mailserver1`,
      `" "@example.org`, `"john..doe"@example.org`, `user%example.com@example.org`); run them and verify the rejected
      cases pass the current check while the edge cases already fail it.
- [x] 3.2 Reconcile `chk_user_email` in the same migration: `NULL`, or a trimmed, non-empty value with at least one
      character before and after an `@`, and no domain-dot requirement; run `cargo test user_repository` and verify
      every case passes.
- [x] 3.3 Verify the parser stays authoritative through the API: create accounts whose email is one of the
      parser-accepted edge cases and verify `201 Created`, and submit a value the parser rejects and verify `422` with
      no constraint failure; run this in the system module rather than at the application layer.

## 4. System suite: account model and read surface

- [x] 4.1 Prove the account model, role values and profile (`REQ-USER-001`, `REQ-USER-002`, `REQ-USER-005`) with one
      `@allure.story` per scenario and `@allure.epic("User")`; run the module and verify every case passes.
- [x] 4.2 Prove read by identifier, the filtered collection and the caller's own account (`REQ-USER-009`,
      `REQ-USER-010`, `REQ-USER-011`), including the empty collection and the three malformed filters; run the module
      and verify every case passes.

## 5. System suite: creation

- [x] 5.1 Prove the username and email rules on creation (`REQ-USER-003`, `REQ-USER-004`), including both length
      boundaries and the display-name rejection; run the module and verify every case passes.
- [x] 5.2 Prove atomic creation (`REQ-USER-007`), including one credential and no account or credential after a
      rejection; the request-shape rejections of `REQ-USER-006` are declared status and schema conformance, proved by
      the API fuzz suite; run the module and verify every case passes.
- [x] 5.3 Prove the created profile and `Location` (`REQ-USER-008`); run the module and verify the `Location` header
      addresses the created account.

## 6. System suite: update

- [x] 6.1 Prove partial update behaviour (`REQ-USER-012`): one field per case, an omitted field unchanged, an empty body
      changing nothing, and an unknown identifier answering `404`; run the module and verify every case passes.
- [x] 6.2 Prove rename and email uniqueness on update (`REQ-USER-013`), including keeping the account's own current
      value; run the module and verify every case passes.
- [x] 6.3 Prove update validation (`REQ-USER-014`), including an invalid username and an invalid email, each leaving the
      stored values unchanged; the undeclared-role and malformed-body rejections are schema conformance, proved by the
      API fuzz suite; run the module and verify every case passes.

## 7. System suite: deletion

- [x] 7.1 Prove identity removal (`REQ-USER-015`): `204`, the account no longer readable, its credential no longer
      authenticating, its permission overrides gone, and an unknown identifier answering `404`; run the module and
      verify every case passes.
- [x] 7.2 Prove that deletion removes the account's owned data (`REQ-USER-016`) by deleting an account that owns a
      gallery with an item, a playlist and an unfinished upload session, then verifying each resource answers `404`; no
      recording is creatable through the API, so the playlist carries no track and the placement cascade stays with the
      application tests; run the module and verify every case passes.
- [x] 7.3 Prove private-media disposition (`REQ-USER-017`): an image referenced only by the account's own gallery and an
      image referenced only by the account's own public gallery are gone with their bytes; an uploaded file with no
      media row is not reachable through the API and stays with the application tests (`PrivateMedia::Orphan`); run the
      module and verify every case passes.
- [x] 7.4 Prove shared-media survival (`REQ-USER-018`): an image referenced by another account's gallery, a video
      backing a film and a catalogue cover remain readable, and the account with identifier `0` owns the transferred
      file; run the module and verify every case passes.
- [x] 7.5 Prove the digest collision rule (`REQ-USER-019`) with two accounts that uploaded byte-identical content; run
      the module and verify both files are readable and the second transferred row carries no digest.

## 8. HAL boundary audit

- [x] 8.1 Inspect `src/lib/infrastructure/inbound/rest/handler/user/*.rs` for HAL construction (`_links`, `SelfLinks`,
      `user_self_href`, `hal_json`). If the handler builds the self link itself, extract that construction into
      `src/lib/infrastructure/inbound/rest/hal.rs` (or a HAL-owned helper) so the user handler returns the profile and
      its URI only; add no spec requirement. Verify `cargo test`, the system user module, and the served OpenAPI still
      declare `application/hal+json` with `_links.self`; if the construction is already centralized, close the task with
      the evidence and no code change.

## 9. Replace the E2E cases the system suite owns

- [x] 9.1 Delete the account-outcome cases that `test/system/user.py` now proves from
      `test/e2e/user/uc001_create_a_user_account.py`, keeping the authorization cases (unauthenticated access, the
      forbidden Admin creations); verify the remaining file passes with
      `pytest test/e2e/user/uc001_create_a_user_account.py`.
- [x] 9.2 Delete the account-outcome cases that `test/system/user.py` now proves from
      `test/e2e/user/uc005_delete_a_user_account.py`, keeping the authorization case and the post-deletion `401` and
      login assertions — split them into their own case if the account-outcome case is removed whole; verify the
      remaining file passes with `pytest test/e2e/user/uc005_delete_a_user_account.py`.
- [x] 9.3 Verify no scenario is proved twice: every scenario the module proves appears as a `story` label exactly once;
      the twelve it does not prove are either API-schema conformance (nine, proved by the API fuzz suite) or
      unobservable failure/orphan paths (three, proved by the application tests); the deleted E2E cases no longer exist.

## 10. Integration and handoff

- [x] 10.1 Run `script/system.sh` (or `NO_BUILD=1 script/system.sh` against an existing image) and verify the user
      module passes with `authentication`.
- [ ] 10.2 Run `script/rule_enforcer.sh check`, `cargo clippy --all-targets --all-features -- -D warnings`, and the
      Markdown gates (`lint:md`, `fmt:md`); verify they pass with the migration, the new module and the HAL extraction.
- [x] 10.3 Verify the authorization handoff is explicit: `design.md` names every discarded rule, and the list is linked
      from the authorization workstream's change or issue. Completion is the linked artifact plus a match between the
      table and the removed requirement text.

## Workflow follow-up

- Archive the change after review; verify `openspec/specs/user/spec.md` exists, the two child specs are gone, and
  `openspec list --specs` reports `user` once.
