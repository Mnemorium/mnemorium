# Proposal

## Why

UC-001 "Create a User Account" is implemented as `POST /api/v1/identity/register`, but no capability spec owns it.
`TechnicalDesign.md` § 5.6 places a capability's validation criteria in the OpenSpec requirement that owns it, so the
username, email, password, role-grant and uniqueness rules currently live only in code and in
`test/e2e/user/uc001_create_a_user_account.py`.

The gap is already visible in behaviour: the endpoint publishes `max_length = 100` for the username but nothing enforces
it, padded and capitalized usernames are accepted, and a rename to a taken username answers `500` because
`PATCH /api/v1/user/{id}` never checks uniqueness before saving. Writing the spec first makes those the acceptance
criteria instead of accidents.

## What Changes

- Add the `user/account-creation` capability: an authenticated Admin creates a user account whose username, optional
  email, password and role are validated before anything is written.
- Add `UsernamePolicy` under `domain/service` and invoke it from `RegisterUser` and `UpdateUser`, following the
  application-layer invocation rule (`ARCH-005`). `User::try_new` and `User::set_username` stop validating the username;
  email validation stays in the model.
- **BREAKING** (small): a username must already be 4..=100 characters, contain no uppercase letter, and have no leading
  or trailing whitespace; an invalid username is rejected with `422`. The system stores the submitted value unchanged
  and never trims or lowercases it. All of these were previously accepted as given.
- Reject an email that carries a display name, such as `Alice <alice@example.com>`, with `422`; the parser's default
  currently accepts the mailbox and stores the raw string.
- `PATCH /api/v1/user/{id}` gains a uniqueness pre-check so a duplicate rename answers `409 Conflict`; the pre-check
  names the field that is taken, while create and rename also translate the repository's unique-constraint outcome to a
  generic `409`, so a race cannot surface as `500`.
- Add the username length, no-uppercase and no-surrounding-whitespace checks to `chk_user_username`, publish
  `max_length = 100` on the PATCH request schema, and regenerate `docs/openapi.json`.
- Extend the UC-001 E2E file with unauthenticated access, both duplicate outcomes, a null email, the uppercase and
  surrounding-whitespace rejections, and the 4/100 length boundaries.
- Unchanged: the role-grant matrix, the password policy, the response representation, and the `201`/`Location` contract
  already behave as the spec will state.

## Capabilities

### New Capabilities

- `user/account-creation`: an authenticated Admin creates a user account with a valid lowercase username, an optional
  validated email, a password that satisfies the password policy, and a role the caller may grant, while username and
  email remain unique across users.

### Modified Capabilities

- None. No existing capability's requirements change; the two Library capabilities are untouched.

## Impact

- **Domain:** new `domain/service/username_policy.rs` with its own unit tests (`TEST-024`); `User`, `UserError`, the
  `MIN_USERNAME_LENGTH` constant, and the `User::validate_email` parser options; `InitializeRootAdmin` follows the
  constructor change without gaining a policy call.
- **Application:** `RegisterUser` and `UpdateUser` call `UsernamePolicy`, map its errors to `InvalidUsername`, and map a
  repository `AlreadyExist` to `UserAlreadyExists` / `Conflict`. Login and the list filter keep their exact lookups: the
  system stores only usernames that already satisfy the rule, so there is nothing to normalize.
- **Persistence:** `migrations/20260904012829_create_user.up.sql` gains the length, ASCII-lowercase and ASCII-unpadded
  checks in `chk_user_username`, edited in place while the schema is unreleased (`PERS-014`); a local database already
  holding the old definition must be recreated.
- **API:** `patch_user` publishes the username ceiling; `post_register` and `patch_user` gain no route change;
  `docs/openapi.json` is regenerated (`DOC-004`).
- **Tests:** domain-service unit tests, application tests for the two write paths, handler tests for the `409` mapping,
  and the extended UC-001 E2E file; the 80% coverage gate still applies.
- **Docs:** the `UseCases.md` entry for UC-001 is already correct and stays unchanged; the capability spec is the new
  document.
- **Coordination:** this change assumes the architecture change has landed — `ARCH-005` in `TechnicalDesign.md`, the
  `Domain Service` Glossary term, and the `NamePolicy` calls moved out of `Gallery` and `MusicPlaylist`. The current
  working tree does not contain it; it must merge first, or the two `UsernamePolicy` call sites would violate the rule
  this change follows.
