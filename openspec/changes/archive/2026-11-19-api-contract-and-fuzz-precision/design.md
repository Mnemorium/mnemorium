# Design

## Context

See `proposal.md` for motivation. The constraints that shape the approach:

- The served document is generated from the handler macros, so every contract fix lands in a `#[utoipa::path(...)]`
  declaration or a `ToSchema`/`IntoParams` derive, never in a hand-written document.
- Schemathesis 4.33.1 exposes `response.headers` as a plain `dict[str, list[str]]` with lowercase keys
  (`schemathesis.core.transport.Headers`), and validates response-header values with `validate_formats=True` always.
- The project's own validator refuses a scenario rename inside a `MODIFIED` requirement: a changed scenario header
  counts as a dropped scenario. The archive applies `RENAMED → REMOVED → MODIFIED → ADDED`.
- The application layer may not call a third-party crate directly except `anyhow`, `chrono` and `tracing`
  (`STY-RUST-075`), and the login event now carries no client value at all.

The plan's evidence was re-derived before drafting: the document has 24 `_date`/`_at` properties, 16 without a format;
four content endpoints, whose bodies are already binary; no existing document test; `test/api` is already in
`testpaths`; the `test:api` devenv task is already the fuzz run; the project venv has no Schemathesis.

## Goals / Non-Goals

**Goals:**

- Make the served document the complete statement of the contract each requirement names, so the fuzz suite exercises it
  and the system suite stops restating it.
- Keep the rule IDs append-only: `API-046`–`API-050` are added, nothing renumbered.
- Keep the contract test and the custom checks cheap enough to run in the existing jobs.

**Non-Goals:**

- No name policy for a season or an album-track recording; their current rules are left as they are.
- No pagination for the user collection; the collection-links check applies to paginated collections only.
- No enum for a response-only vocabulary the request never carries (`FileResponse.origin`, `DecisionResponse.reason`).
- No correlation handle for a rejected unknown identifier; no digest, no client value. The loss is a follow-up issue.
- No `map_case` hook; the login pin is the chosen trade-off.

## Decisions

### 1. The fuzz run and the system suite prove the contract

The served document is proved by `script/api_fuzz.sh`: Schemathesis validates every response against the document it
reads (`response_schema_conformance`, `response_headers_conformance`), and a black-box system case proves the effect the
document cannot express. There is no Rust document-test module: the developer removed the planned one because the
handler tests belong to the fuzz and system suites.

### 2. Date and timestamp formats, and the two renamed properties

Every response DTO gains `#[schema(format = "date")]` or `#[schema(format = "date-time")]`; the request DTOs already
declare `date` where they need it. Timestamps render RFC 3339 UTC through one shared constant; the database columns are
naive UTC, so appending `Z` states the truth without a timezone conversion.

The contract test derives the expected format from the property suffix (`_date` → `date`, `_at` → `date-time`). Two
properties are date-valued under an `_at` name: `ImageResponse.created_at` and `FileResponse.uploaded_at`. They are
renamed to `created_date` and `uploaded_date` so the naming rule is total; the columns and domain types keep their
names, so no migration is involved. The alternative — an explicit property→format table — was rejected because it stops
the naming convention from being a check.

### 3. Header schemas are declared inline, not by reference

`utoipa-gen` collects component schemas from response media types only; a named `ToSchema` type in a `headers(...)`
tuple yields a `$ref` that never reaches `components.schemas`. The response-header types are therefore declared with
`inline(Type)`, or, for the shared `401` challenge, by setting the schema on the `HeaderBuilder` in
`UnauthorizedChallengeAddon`.

`Content-Length` becomes an integer (`u64`), not a string. `Last-Modified` is an HTTP-date, so it carries a pattern
(`^[A-Z][a-z]{2}, [0-9]{2} [A-Z][a-z]{2} [0-9]{4} [0-9]{2}:[0-9]{2}:[0-9]{2} GMT$`), never `format: date-time` —
`jsonschema_rs` rejects the emitted value against `date-time`. `ETag`, `Content-Range`, `Allow`, `Accept-Ranges` and
`Cache-Control` likewise carry patterns; `Retry-After` stays the integer it already is. `Location` is a tiny
`UriReference` type with `format: uri-reference`.

### 4. `Link.href` is an RFC 6570 template, not a URI reference

A HAL `href` can be a plain relative reference or a template such as `/api/v1/library/movie{?name,limit,offset}`. The
schema is `min_length = 1`, `format = "uri-template"` and `pattern = "^/"`:

- `format: uri-reference` was rejected because `jsonschema_rs` rejects every templated link against it, and
  `response_schema_conformance` validates formats by default;
- `uri-template` accepts both forms and rejects a value with a space; the anchor pattern keeps the target relative.

`API-050` therefore states “a non-empty relative URI reference or an RFC 6570 template”, not only a URI reference.

### 5. Closed vocabularies use handler-local wire enums

The domain keeps `as_wire`/`from_wire` as the authority. A handler-local enum deriving
`Serialize`/`Deserialize`/`ToSchema` (the `GalleryItemType`/`MusicKindRequest` pattern) is bound to the `String` field
with `#[schema(value_type = ...)]` or `#[param(value_type = ...)]`. The handler keeps parsing the raw value and answers
`422` through `ApiError::reject_enum`, so the declaration never bypasses `enum_validation_failed`.

The published set is the **accepted** set, not the domain's whole vocabulary:

| Property                                 | Published values                                                                           |
| ---------------------------------------- | ------------------------------------------------------------------------------------------ |
| `PutPermissionRequest.visibility`        | `PRIVATE`, `RESTRICTED`, `PUBLIC`                                                          |
| `ShareRequest.role`                      | `VIEWER`, `DOWNLOADER`                                                                     |
| `GrantPayload.role`                      | `VIEWER`, `DOWNLOADER`, `CONTRIBUTOR`, `EDITOR`, `MANAGER` (`OWNER` is implicit, rejected) |
| `GrantPayload.effect`                    | `ALLOW`, `DENY`                                                                            |
| `GrantPayload.child_kind`                | `ALL` and every resource kind except `ADMIN` (root-only, rejected)                         |
| `PrincipalPayload.kind`                  | `USER`, `ROLE`                                                                             |
| `PrincipalPayload.role`                  | `ADMIN`, `STANDARD`                                                                        |
| `GET /identity/permission.action`        | every `Action`                                                                             |
| `GET /identity/permission.resource_kind` | every `ResourceKind`                                                                       |
| permission routes' `kind`                | every `ResourceKind`                                                                       |
| video identify `provider`                | `tmdb`, `omdb`                                                                             |
| music identify `provider`                | `musicbrainz`                                                                              |
| music identify `kind`                    | `album`, `release`, `recording`                                                            |

The response permission bodies reuse the same enums where they mirror a request value (`Visibility`, `ResourceKind`,
`ResourceRole`, `GrantEffect`, `ChildKind`, `Principal.kind`, `PermissionSummaryResponse.role`). `origin` and the
decision `reason` stay strings.

### 6. Name policy rejects, never repairs

`NamePolicy` gains `SurroundingWhitespace` and stops trimming; it returns the submitted value. Its seven callers
(gallery, film, album, group, playlist, show, episode) already pass the submitted value. Each request `name` declares
`min_length = 1`, `max_length = 100` and `pattern = "^\S(?:[\s\S]*\S)?$"`: the pattern is lookahead-free so the
generator can draw from it, and it allows interior whitespace (including a newline) exactly as the policy does. No data
migration: stored names are already trimmed.

Season and album-track names stay out. `PutSeriesSeasonRequest`/`PatchSeriesSeasonRequest` have no length or whitespace
rule, and `MusicRecording::validate_name` rejects only an empty value. Publishing a pattern without enforcement would
break `API-045` and `negative_data_rejection`, so the spec clause names the seven resources. Extending the policy to
seasons and recordings is a separate behaviour change if it is ever wanted.

### 7. Username policy allows uppercase and forbids whitespace anywhere

`UsernamePolicyError::ContainsUppercase` is removed; `SurroundingWhitespace` becomes `ContainsWhitespace` and rejects
any whitespace, and `ContainsNul` rejects a NUL character the datastore cannot measure. The schema declares
`pattern = "^[^\s\x00]+$"` beside the bounds. A migration edit is required: `chk_user_username` currently asserts
`username = lower(username)`; it keeps the length check and the exact uniqueness (usernames are case-sensitive and
compared exactly) and drops the lowercase assertion. The whitespace assertion in SQL is weaker than the Unicode rule the
domain applies, so the domain remains the authority and the check is defence-in-depth, not a second rule.

### 7a. An undeclared query parameter is rejected

Every query DTO denies unknown fields, so a parameter the operation does not declare answers `400` (`REQ-API-022`). The
alternative — relying on `serde_urlencoded`'s default of ignoring an unknown key — leaves a negative fuzz case accepted
and a client's typo silently ineffective.

### 8. Login takes an identifier and resolves username first

`LoginRequest.username` becomes `identifier`; `LoginUserCommand` and `LoginAttemptKey::Username` follow. The use case
looks the value up as a username first and, only if that finds nothing, as an email address. `UserFilter` already
supports both; email uniqueness is `COLLATE nocase`, so an email is matched case-insensitively while a username stays
exact. The ambiguity rule (a value that is one account's username and another's email resolves as the username) is in
the spec and the code comment.

The absent-account throttle key keeps its existing shape: `LoginAttemptKey::Identifier`, digested by the infrastructure
limiter exactly as `AccountKey::UsernameDigest` digests a username today. The existing-user key stays the resolved user
id. Response uniformity is unchanged.

### 9. `authn_failed` carries only its reason

The event drops `claimed_identity`; `REQ-LOGGING-011` is removed with it, and `SCN-LOGGING-012-B` no longer names the
identity. `login_user` logs `reason = invalid_credentials` only. A user id or a digest was considered and rejected: the
first does not exist for an unknown identifier and would reveal existence to a log reader; the second cannot tell an
operator what was typed, so its only use is grouping repeated attempts against one nonexistent name.

The three unit tests that assert the raw value become a redaction proof (the submitted identifier never appears), and
the two system cases whose subject is the identity are deleted with the rule. A task files an issue recording the
per-account correlation the event gives up and the `user:<id>` alternative, so the decision is tracked rather than
silently lost.

### 10. The custom checks are pure functions with thin registrations

`test/api/checks.py` holds the two check bodies as ordinary functions taking the primitives they need; `hooks.py`
imports them and registers thin `@schemathesis.check` wrappers. The pure module boundary keeps `pytest test/api`
runnable in the project venv, which has no Schemathesis; the fuzz run proves the bodies, as the developer removed the
unit-test module they were first proved with. The checks read `response.headers.get("content-type")` as a **list**,
lowercased; the plan's `response.headers.get("Content-Type", "")` would return `""` and silently skip every response.
They use `response.json()`, `case.operation.path` and `response.request.url`, all present in 4.33.1.

`valid_collection_links` classifies a collection by a templated `search` link, not by a field name, and applies the
query-preservation rule only to a collection whose `self` carries `limit`/`offset`. That matches `REQ-API-007` and
leaves `list_users.self`, which is deliberately unfiltered, outside the check.

A new `test:api-cases` devenv task runs `pytest test/api`; the API job already runs that command before the fuzz, so the
job needs no new step, only the task for local use.

### 11. The fuzz carve-out rule has two sanctioned categories

A `schemathesis.toml` or `hooks.py` exception is allowed when it names either a concrete server behaviour the document
cannot express, or a tool limitation (a generator or transport defect). The second category is required by existing
exceptions the plan wants to keep: `auth.auto-signup = false` (a detector crash) and the invalid-header-name filter (a
`requests` limitation). The comment names the category and the behaviour.

### 12. Two requirements are replaced, not modified

The project's validator rejects a `MODIFIED` block that omits a scenario the main spec still has, so a scenario title
cannot change in place. `SCN-USER-003-B — Uppercase letters are rejected` and `SCN-MUSIC-020-A — Names are trimmed` both
have titles that become false. `REQ-USER-003` is replaced by `REQ-USER-020` and `REQ-MUSIC-020` by `REQ-MUSIC-030`, each
as `REMOVED` + `ADDED` with the next free identifier. Allure labels and any rule citation update with them.

## Risks / Trade-offs

- **`Last-Modified` as `date-time`** → it fails the header validator. Mitigation: an HTTP-date pattern, measured against
  the emitted value.
- **`uri-reference` on `Link.href`** → every templated link fails. Mitigation: `uri-template` plus a relative anchor.
- **An enum broader than the accepted set** → valid-looking positive cases answer `422` and fail
  `positive_data_acceptance`. Mitigation: the subset table above, and a fuzz run per vocabulary.
- **A pattern the handler does not enforce** → `API-045` and `negative_data_rejection` fail. Mitigation: only the seven
  policy-governed resources receive a pattern.
- **The login field rename is breaking** → clients and the harness must send `identifier`. Mitigation: it is pre-1.0,
  the generated client regenerates, and the harness/script are updated in the same change.
- **The two response property renames are breaking** → a client reading `created_at`/`uploaded_at` must switch.
  Mitigation: same release note as the login change; no data migration.
- **The raw identifier no longer reaches the log** → per-account failure correlation is lost. Mitigation: the follow-up
  issue; the limiter still protects the account.
- **`pytest test/api` has no Schemathesis in the venv** → an importing unit test breaks collection. Mitigation: the pure
  module boundary above.
- **`test:api` is already the fuzz task** → a name collision. Mitigation: `test:api-cases`.
- **The generated client’s field types change with `format: date`/`date-time`** → parsed model fields become
  `datetime.date`/`datetime.datetime`. Mitigation: the system suite reads the affected values from raw JSON; no new
  dependency is needed (`openapi-python-client 0.30` uses `datetime.fromisoformat`).
- **The fuzz carve-out audit is run-driven** → the final config cannot be predicted from the source. Mitigation: B5/B6
  are explicit “run, classify, fix or carve out” tasks, not a blind removal.

## Migration Plan

1. Land the specs and the rulebook rules; the contract test and the domain unit tests fail first.
2. Correct the code and the document; regenerate the system client; run the fuzz suite.
3. Delete the hooks and config entries the contract makes obsolete; keep only the tool-limitation filter.
4. The login rename, the two response renames and the `Z` on timestamps are **BREAKING**; there is no stored-data
   migration. Rollback is the reverse commit sequence; a client falls back by sending `username` and reading the old
   property names only if the server rolls back too.

## Open Questions

None that change the specs or the task breakdown. The follow-up issue for the lost per-account correlation is a tracked
task, not an open design question.
