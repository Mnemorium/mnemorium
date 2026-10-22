# Design

## Context

See `proposal.md` - Why for the motivation. The design is shaped by these facts in the current tree:

- `RegisterUser` already authorizes the caller, checks username and email uniqueness, applies the password policy,
  hashes the credential and creates both rows in one unit of work.
- `User::try_new` and `User::set_username` validate the username inline against `MIN_USERNAME_LENGTH = 4`; the API
  publishes `max_length = 100` without enforcing it, and neither the model nor the API rejects uppercase letters or
  surrounding whitespace. `User::validate_email` uses the parser's default, which accepts a display-name mailbox.
- `LoginUser` and `ListUsers` resolve a username with an exact match. Once only valid lowercase usernames can be stored,
  those exact lookups are already correct and need no change.
- `PATCH /api/v1/user/{id}` can rename a user but performs no uniqueness pre-check, so a duplicate reaches the unique
  index and surfaces as an internal error.
- The architecture change (`ARCH-005`, the `Domain Service` Glossary term, and the `NamePolicy` extraction from
  `Gallery` and `MusicPlaylist`) is a prerequisite. The current working tree does not contain it, so it must merge
  before this change is applied.

## Goals / Non-Goals

**Goals:**

- Give the capability one written acceptance criterion, owned by `user/account-creation`.
- One username rule — 4..=100 characters, no uppercase letter, no leading or trailing whitespace — applied by creation
  and rename.
- No repair: an invalid username is rejected with `422`, and a valid one is stored exactly as submitted.
- A duplicate username or email answers `409` on both paths, including a lost race.
- Keep the wire representation, the role matrix, the login lookup and the password policy unchanged.

**Non-Goals:**

- Trimming, lowercasing or otherwise normalizing input. The system validates; it does not make an invalid value right.
- Case-insensitive email or username comparison. The stored values are canonical because invalid ones are refused.
- Accepting an uppercase username by folding it to lowercase.
- Self-service rename: `PATCH /user/{id}` stays Admin-only.
- Clearing an email through PATCH: the current `Option<String>` cannot distinguish an absent field from an explicit
  null.
- UC-005 deletion, and the `DELETE` path it needs.

## Decisions

### 1. `UsernamePolicy` is a domain service invoked by the use cases

New `domain/service/username_policy.rs`:

```rust
pub const MIN_USERNAME_LENGTH: usize = 4;
pub const MAX_USERNAME_LENGTH: usize = 100;

pub enum UsernamePolicyError {
    TooShort,
    TooLong,
    ContainsUppercase,
    SurroundingWhitespace,
}

pub struct UsernamePolicy;

impl UsernamePolicy {
    pub fn validate(username: &str) -> Result<(), UsernamePolicyError>;
}
```

`RegisterUser` and `UpdateUser` call `validate`, map every variant to their existing `InvalidUsername` error (`422`),
and store the submitted value unchanged. This follows `ARCH-005`: the use case sees the typed outcome and owns the `422`
mapping (`API-045`). Two callers meet the Glossary's "two or more use cases" threshold.

_Alternatives considered._ Keeping the call inside `User` (today's pattern) would leave the invocation rule violated.
Taking a `&User` parameter is impossible on the creation path, where the model does not exist until the username has
been validated. Letting the policy return a repaired value would contradict the product rule that the system does not
normalize.

### 2. The model stops validating the username

`User::try_new` and `User::set_username` assign the value they receive; `UserError::UsernameTooShort` is removed, while
`InvalidEmail` and the `Unknown` catch-all stay. `InitializeRootAdmin` keeps constructing the constant
`ROOT_ADMIN_USERNAME` without a policy call; the constant already satisfies the rule.

Because `SqlxUserRepository::domain_user` maps every row back through `User::try_new`, reads no longer reject an
out-of-range username; a stored row stays readable. The `chk_user_username` check is the write-side backstop.

_Alternatives considered._ Re-validating in the model as a second line of defence would give the rule two sources of
truth and make every new construction path silently stricter than the use case.

### 3. Validation, not normalization

The policy checks three things on the submitted value:

- `username.chars().count()` is between 4 and 100 inclusive;
- no character satisfies `char::is_uppercase`;
- `username.trim() == username`, so no leading or trailing whitespace (spaces, tabs, newlines, any Unicode whitespace).

It returns `Ok(())`; the use case stores the submitted value exactly. Because only lowercase, unpadded values can be
stored, `Alice` never reaches the table: the request is a `422`, not a `409` against `alice`. The `UNIQUE (username)`
constraint and every exact `username = ?` lookup stay as they are, and login needs no folding.

Internal characters are unconstrained beyond the length rule, so an internal space is allowed. That is deliberate: the
rule is about the identifier's boundaries and case, not about policing the middle.

_Alternatives considered._ Trimming and lowercasing (the previous draft) repairs input the product wants rejected, and
it makes the stored value differ from what the client sent. Rejecting uppercase but trimming would be inconsistent.
`COLLATE NOCASE` is unnecessary once only lowercase values are storable.

### 4. Email is parsed as a bare address

`User::validate_email` parses with `Options::default().without_display_text()`, so a mailbox such as
`Alice <alice@example.com>` is rejected instead of being accepted and stored raw. The field holds an address, not a
mailbox, and the spec's "syntactically valid email address" already excludes the display form.

_Alternatives considered._ Leaving the parser default keeps a pre-existing hole: the raw mailbox string would pass the
column's `LIKE` check and be returned as an email address.

### 5. Duplicates answer `409` with a field-specific message

`RegisterUser` already pre-checks. `UpdateUser` gains the same check, excluding the target user, before saving. Each
pre-check reports which field collided: a taken username answers `409` with "username is already taken", a taken email
answers `409` with "email is already taken". The endpoint is Admin-only, so naming the field exposes nothing the caller
cannot already list.

Both use cases also map `RepositoryError::AlreadyExist` to a generic duplicate error, so a concurrent insert that wins
the race answers `409` rather than `500`; the repository error does not say which constraint lost, so that message names
neither field.

_Alternatives considered._ One merged message is less useful to the administrator fixing the request, and the Admin-only
caller has no reason to withhold the field. Relying on the database alone is enough for creation, but the rename path
currently turns the constraint into `Unknown`; mapping it would fix the status while still logging a constraint
rejection for a normal business outcome.

### 6. The username shape is a datastore check, not just a column width

`migrations/20260904012829_create_user.up.sql` extends `chk_user_username` with `length(username) between 4 and 100`,
`username = lower(username)` and `username = trim(username)`. SQLite does not enforce `VARCHAR(100)`, and the checks are
the backstop for any writer that bypasses the use case. `lower()` and `trim()` fold ASCII only, so the application
remains authoritative for Unicode uppercase and non-space whitespace. The schema is still unreleased, so the initial
migration is edited in place under `PERS-014`; a local database that already applied the old definition must be
recreated.

_Alternatives considered._ Adding a new migration would contradict `PERS-014` while the schema is unreleased and would
leave the initial schema inconsistent with the constraints it should have carried.

### 7. The wire contract gains only the published ceiling

`patch_user` declares `max_length = 100` on the username, matching what `post_register` already publishes. No route,
status, representation or link changes. `docs/openapi.json` is regenerated from source.

### 8. Test surface follows the layers

`UsernamePolicy` gets its own unit module (`TEST-024`): the 4 and 100 boundaries, 3 and 101 characters, an ASCII and a
Unicode uppercase letter, a leading and a trailing whitespace character, an internal space, and a valid lowercase value.
Application tests cover the two call sites, the duplicate pre-check, the race mapping, the display-name email rejection
and the existing role matrix. Handler tests cover the `400`/`422`/`409` mapping. The E2E file for UC-001 is extended,
not duplicated (`TEST-041`); the rename path has no catalogued use case, so its `409` is covered at the application and
handler layers.

## Risks / Trade-offs

- **Clients must send canonical values** → The `422` is the contract; the spec lists each rejected shape, so a client
  can validate before sending.
- **Reads stop rejecting out-of-range usernames** → The write-side check is the backstop, and existing rows stay
  readable instead of turning into `500`s.
- **Editing the initial migration breaks checksums** → Recreate the local database; the schema is unreleased and has no
  production data to migrate.
- **The datastore checks fold ASCII only** → The application performs the Unicode checks; the database checks are a
  backstop for writers that bypass the use case.
- **Internal spaces remain legal** → Deliberate; the length rule still bounds the value.
- **The rename fix is outside the capability's spec** → It is a consistency correction forced by the shared rule, not a
  new capability; the design and tasks record it so it is not mistaken for scope creep.

## Migration Plan

1. Land the architecture change first: `ARCH-005`, the Glossary term, the `NamePolicy` extraction, and `check_arch_005`.
2. Apply this change.
3. Recreate the local SQLite database (delete `mnemorium.db` with its `-wal` and `-shm` sidecars) so the edited initial
   migration runs from scratch.
4. Roll back by reverting the code and the migration, then recreating the database again.

## Open Questions

None.
