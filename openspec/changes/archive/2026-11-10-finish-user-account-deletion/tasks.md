# Tasks

## 1. Authorization action and target guard

- [x] 1.1 Add `Action::UserDelete` (wire name `user.delete`, target `User`, Standard `NONE`, Admin `ALL`, Root Admin
      `All`, configurable) to `src/lib/domain/model/permission.rs`, grow `ACTIONS` to 20 and update every exhaustive
      match; verify `cargo test --lib permission` passes and a unit test round-trips `Action::from_wire("user.delete")`.
- [x] 1.2 Extend the relation guard in `src/lib/domain/service/authorization.rs` so `UserDelete` refuses a privileged
      target for every non-root caller, including the caller itself; verify new authorization unit tests cover
      Admin-to-Admin, Admin-to-self, Root-to-Admin and Root-to-root.
- [x] 1.3 Add the `user.delete` row to the action matrix in `docs/Permissions.md` and extend its relation-guard and Root
      Admin paragraphs; verify `markdownlint-cli2 'docs/**/*.md'` passes and the documented matrix matches the enum.

## 2. File ownership transfer

- [x] 2.1 Add `reassign_owner` and `clear_integrity_hash` to `src/lib/domain/port/file_repository.rs` with `Ok(false)`
      for a missing row (`STY-RUST-046`); verify `cargo check` passes.
- [x] 2.2 Implement both methods in `src/lib/infrastructure/outbound/sqlx/file_repository.rs` and add adapter tests for
      an ownership change, a cleared digest and a missing identifier; verify `cargo test --lib file_repository` passes.

## 3. Delete-user use case

- [x] 3.1 Add `src/lib/application/port/delete_user.rs` (command, error enum, use-case trait) and the `delete_user()`
      accessor on `UserUseCaseFactory`; verify `cargo check` and the factory mock compile.
- [x] 3.2 Implement authorization and the ordered identity/library/upload removal in
      `src/lib/application/use_case/delete_user.rs`, emitting the `user_admin` event with action `delete` after the
      commit; verify unit tests cover the Standard and Admin targets, the four refusals, the unknown target, the atomic
      rollback and the gallery/playlist/upload removals.
- [x] 3.3 Implement media classification, private removal, shared transfer, digest collision handling and post-commit
      byte cleanup in the same use case; verify unit tests cover a private image, video and audio, an orphan file, an
      image in another user's gallery, a video backing a film or episode, an audio backing an album, the digest
      collision, and a cleanup failure that does not fail the request.
- [x] 3.4 Wire `RuntimeUserUseCaseFactory` to build `DeleteUser` with `FileSystemStorage`, following the
      `delete_gallery` accessor pattern; verify the factory unit tests pass and `cargo check` succeeds.
- [x] 3.5 Update `docs/development/UseCases.md` UC-005 (post-condition, happy-path step and alternative flows) and the
      `user_admin` row in `docs/development/TechnicalDesign.md` § 9 to include deletion; verify
      `script/rule_enforcer.sh check DOC-005 DOC-008` and `markdownlint-cli2 'docs/**/*.md'` pass.

## 4. REST endpoint and API contract

- [x] 4.1 Add `src/lib/infrastructure/inbound/rest/handler/user/delete_user.rs` with the
      `#[utoipa::path(delete, path = "/user/{user_id}", ...)]` declaration, the `204` response and the
      `401`/`403`/`404`/`500` error mapping; verify the OpenAPI unit test in `rest.rs` asserts the collected `delete`
      operation.
- [x] 4.2 Register the handler next to `get_user` and `patch_user` in `handler/user.rs` and reuse `parse_user_id`;
      verify the route is collected by the same OpenAPI test and `cargo check` succeeds.

## 5. End-to-end coverage and rule exemption

- [x] 5.1 Add `test/e2e/user/uc005_delete_a_user_account.py` covering: an Admin deletes a Standard user; the deleted
      user's token and login answer `401`; the user's private gallery, its items and their asset answer `404`; a shared
      image in another user's gallery survives and the Root Admin still reads its file; deleting the Root Admin and an
      Admin peer answers `403`; verify `script/e2e.sh` passes the new file.
- [x] 5.2 Remove `UC-005` and its `TODO` comment from `TEST-039`'s `exempt_use_cases` in `rule_enforcer.yaml`; verify
      `script/rule_enforcer.sh check TEST-039` passes.

## 6. Integration gates

- [x] 6.1 Run `cargo fmt --all -- --check`, both clippy commands from `AGENTS.md`, `cargo test` and the coverage gate;
      verify each passes.
- [x] 6.2 Run `script/rule_enforcer.sh check` and the Markdown gates (`markdownlint-cli2 '**/*.md'`,
      `prettier --check '**/*.md'`); verify each passes.
- [x] 6.3 Run the full `script/e2e.sh` suite; verify UC-005 and the existing files all pass.

## Workflow follow-up

- Archive the change after review and after the implementation is merged.
- Verify the archived `user/account-deletion` capability and the updated `identity/authorization` requirements.
