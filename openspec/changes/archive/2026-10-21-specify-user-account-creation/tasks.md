# Tasks

## 1. Delivery and prerequisite

- [x] 1.1 Confirm the architecture prerequisite has landed: `ARCH-005` is in `docs/development/TechnicalDesign.md`, the
      `Domain Service` Glossary term reads "two or more use cases", `rule/arch-005.yml` exists, and
      `grep -rn 'domain::service' src/lib/domain/model/` returns nothing. Verify with `script/rule_enforcer.sh check`.
      If it has not landed, stop and merge it first; this change's `UsernamePolicy` call sites depend on it.
- [x] 1.2 Resolve the delivery metadata: Conventional-Commit type `feat`, the `GOV-001` scope `api`, slug
      `specify-user-account-creation`, and pull-request title `feat(api): specify user account creation`. The user
      waived the new worktree and clean-tree requirement: work continues on the current branch
      `chore(development)/use-cases`, which carries the architecture prerequisite as uncommitted edits.

## 2. Domain: username policy and model

- [x] 2.1 Add `src/lib/domain/service/username_policy.rs` with `MIN_USERNAME_LENGTH = 4`, `MAX_USERNAME_LENGTH = 100`,
      `UsernamePolicyError::{TooShort, TooLong, ContainsUppercase, SurroundingWhitespace}`, and
      `UsernamePolicy::validate(username: &str) -> Result<(), UsernamePolicyError>`, which rejects a character count
      outside 4..=100, any `char::is_uppercase`, and any value where `trim()` differs from the input. Register the
      module in `domain/service.rs`. Add the `TEST-024` unit module covering exactly 4, exactly 100, 3, 101, an ASCII
      uppercase, a Unicode uppercase, a leading space, a trailing tab or newline, an internal space that remains legal,
      and a valid lowercase value. Verify with `cargo test --lib domain::service::username_policy`.
- [x] 2.2 Change `User::try_new` and `User::set_username` to accept an already-validated username and assign it without
      validating it; remove `UserError::UsernameTooShort`; keep `InvalidEmail` and the `Unknown` catch-all; update the
      constructor and setter documentation. Verify `cargo build`.
- [x] 2.3 Make `User::validate_email` parse with `email_address::Options::default().without_display_text()` so a mailbox
      such as `Alice <alice@example.com>` is rejected; keep the raw accepted value unchanged. Verify with an application
      test for the display-name form and the existing valid and invalid format cases.
- [x] 2.4 Update every remaining `UserError::UsernameTooShort` match arm and test fixture, including `RegisterUser`,
      `UpdateUser`, and `InitializeRootAdmin`; the root bootstrap keeps constructing the constant `ROOT_ADMIN_USERNAME`
      without a policy call. Verify `cargo test --lib application::use_case` compiles and the root-admin tests pass.

## 3. Application: create path

- [x] 3.1 In `RegisterUser`, call `UsernamePolicy::validate(command.username())` before constructing the user and map
      every `UsernamePolicyError` variant to `RegisterUserError::InvalidUsername`; pass `command.username().to_owned()`
      unchanged to both `User::try_new` calls. Verify the application tests cover a valid lowercase username, an ASCII
      and a Unicode uppercase letter, a leading and a trailing whitespace character, an internal space, the 4/100
      boundaries, a blank username, and a 101-character username.
- [x] 3.2 Change `RegisterUserError::InvalidUsername`'s message to state the rule (4..=100 characters, no uppercase
      letter, no leading or trailing whitespace) and update the handler tests that assert the message. Verify
      `cargo test --lib infrastructure::inbound::rest::handler::identity::post_register`, and confirm a padded email is
      rejected with `422` by the existing parser.
- [x] 3.3 Replace `RegisterUserError::UserAlreadyExists` with `UsernameAlreadyExists` ("username is already taken") and
      `EmailAlreadyExists` ("email is already taken"), returning the one the pre-check found; keep `UserAlreadyExists`
      for the lost race, where the repository's `AlreadyExist` does not name the constraint. Map
      `RepositoryError::AlreadyExist` from `users().create` to that generic variant and leave the credential insert's
      `AlreadyExist` as `Unknown`, since a hash collision is not a business duplicate; map all three variants to
      `ApiError::Conflict` in `handler/identity/post_register.rs`. Verify with application tests for a duplicate
      username, a duplicate email, and a repository `AlreadyExist` race, asserting the variant and the rollback flag,
      plus handler tests for each `409` message.

## 4. Application: rename path

- [x] 4.1 In `UpdateUser`, call `UsernamePolicy::validate(name)` in the username branch and assign the submitted value
      through the now-infallible `set_username`; map every `UsernamePolicyError` variant to
      `UpdateUserError::InvalidUsername` and update its message to state the rule. Verify the application tests cover a
      valid value, uppercase, leading and trailing whitespace, the boundaries, and both length rejections.
- [x] 4.2 Add `UpdateUserError::{UsernameAlreadyExists, EmailAlreadyExists, UserAlreadyExists}` with the same messages
      as registration, and perform a uniqueness pre-check for the new username and the new email, excluding the target
      user, before saving. Verify with application tests for a duplicate username, a duplicate email, a rename that
      keeps the user's own username, and two email addresses that differ only by case remaining distinct.
- [x] 4.3 Map `RepositoryError::AlreadyExist` from `users().save` to `UpdateUserError::UserAlreadyExists` so a lost race
      answers `409`. Verify with an application test whose save returns `AlreadyExist`.
- [x] 4.4 In `handler/user/patch_user.rs`, map the three duplicate variants to `ApiError::Conflict`, declare the `409`
      response in `#[utoipa::path(...)]`, and publish `max_length = 100` on the username schema. Verify with handler
      tests asserting `409 Conflict` and each field-specific message.

## 5. Persistence and API

- [x] 5.1 Edit `migrations/20260904012829_create_user.up.sql` in place to extend `chk_user_username` with
      `length(username) between 4 and 100`, `username = lower(username)` and `username = trim(username)` (`PERS-014`),
      and record the local-database reset in the change notes. Verify a fresh in-memory database applies the migration
      and `cargo test --lib infrastructure::outbound::sqlx::user_repository` passes.
- [x] 5.2 Regenerate `docs/openapi.json` with `cargo run --bin openapi_gen` and verify the diff contains only the PATCH
      username `max_length`, the PATCH `409` response, and no unrelated schema drift (`DOC-004`).

## 6. E2E and coverage

- [x] 6.1 Extend `test/e2e/user/uc001_create_a_user_account.py` with: a `401` for no token, a `409` naming the taken
      username for a duplicate username, a `409` naming the taken email for a duplicate email, a `422` for an uppercase
      username, a `422` for a username with leading or trailing whitespace, a `422` for a display-name email, a
      successful creation with a null email, and the 4/100 length boundaries plus a 101-character rejection. Keep
      password-behaviour cases on their own explicit passwords (`TEST-044`). Verify with
      `pytest -p no:cacheprovider test/e2e/user/uc001_create_a_user_account.py`.
- [x] 6.2 Add application or handler coverage for every Spec scenario that is not reachable from the E2E file,
      especially the request-shape cases (`400` malformed JSON, missing field, wrong type, unlisted role) and the
      rejection-leaves-nothing checks. Verify with `cargo test`.
- [x] 6.3 Run the coverage gate:
      `cargo llvm-cov --lib --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80`.

## 7. Integration verification

- [x] 7.1 Run `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`,
      and `script/rule_enforcer.sh check`; all pass with no warnings.
- [x] 7.2 Run the container E2E suite with `pytest -p no:cacheprovider test/e2e` against the `mnemorium-e2e` container
      and confirm the whole suite passes.

## Workflow follow-up

- Archive the change after the project's review requirements are satisfied.
- Verify the archived capability exists at `openspec/specs/user/account-creation/spec.md` with the Purpose preserved.
