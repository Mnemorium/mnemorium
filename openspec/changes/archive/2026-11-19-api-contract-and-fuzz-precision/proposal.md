# Proposal

## Why

The served OpenAPI document is the contract Schemathesis and every client rely on, but it leaves formats, closed
vocabularies, response headers and stored-content bodies under-specified. `schemathesis.toml` and `test/api/hooks.py`
therefore carry exceptions that compensate for documentation gaps instead of server behaviour, and a fuzz run proves
less than it appears to. The same document publishes a username rule and a name rule the server no longer follows, and
the login operation still tells the log which identity was claimed. With the seed and scenario split landed, the fixture
is stable enough to fix the contract at the source and delete the compensations.

## What Changes

Contract precision:

- Every date property, request and response, declares `format: date` in `YYYY-MM-DD`; every timestamp declares
  `format: date-time` in RFC 3339 UTC. Timestamps render with `Z`, and a malformed request value answers `422`.
- **BREAKING** two date-valued properties named `_at` are renamed: `ImageResponse.created_at` becomes `created_date` and
  `FileResponse.uploaded_at` becomes `uploaded_date`, so `_at` always means a timestamp.
- Usernames are 4–100 characters, allow uppercase, contain no whitespace anywhere or NUL, are stored unchanged and are
  unique compared exactly. The user migration's lowercase check is relaxed with them.
- A named resource rejects a name that begins or ends with whitespace with `422` and stores nothing; it is no longer
  trimmed. Interior whitespace stays allowed and the submitted value is stored unchanged. The resources are gallery,
  film, album, music group, playlist, show and episode; a season name and an album-track name keep their current rule.
- The query string is a closed set: a parameter an operation does not declare answers `400` and applies nothing.
- Every closed vocabulary the server enforces is published as an enum schema; every response header is published with
  the schema its value obeys (an integer `Content-Length`, a URI-reference `Location`, a bearer `WWW-Authenticate`, an
  HTTP-date `Last-Modified`, and the `ETag`, `Content-Range`, `Allow`, `Accept-Ranges` and `Cache-Control` shapes).
  Stored content is published as a binary string, and a HAL link target is a non-empty relative URI reference or an RFC
  6570 template.

Authentication:

- **BREAKING** `POST /identity/login` takes an `identifier` — a username or an email address — instead of `username`. A
  value that matches a username resolves as that username; otherwise it is looked up as an email address. The uniform
  rejection and the rate-limiter keys follow the identifier.
- The `authn_failed` event drops `claimed_identity`, so no username, email or digest of one reaches a sink;
  `REQ-LOGGING-011` is removed with the field. A follow-up issue records the per-account correlation this gives up.

Fuzz configuration:

- Delete the `_valid_date`, `_valid_username`, `_valid_name` and `_valid_position` filters, and remove the config
  entries the contract makes obsolete. Only the invalid-header-name filter stays, with its tool-limitation reason.
- Pin `body.identifier` and `body.password` for `post_login` and drop `warnings = false`.
- Route the four vocabulary parsers through `ApiError::reject_enum`, so the `enum_validation_failed` event the config
  already names is the one emitted.
- Remove the no-op `examples` and `coverage` phase disables and the expected statuses that merely repeat a default;
  re-enable `stateful` for `delete_user`, and audit `ensure_resource_availability` and the `missing_test_data` displays
  by running the suite.
- Add two custom checks, `valid_hal_links` and `valid_collection_links`, with container-free unit cases, and a
  `test:api-cases` task that runs them.

Rulebook and documentation:

- Add `API-046`–`API-050` to `TechnicalDesign.md` § 3, with the constraint, asset-content and fuzz carve-out detail
  updates in § 3 and § 5.
- Record the exemption rule for a fuzz carve-out (a concrete server behaviour the document cannot express, or a tool
  limitation, named in the comment), the two custom checks, and the corrected seed-data and HAL-test sentences.

## Capabilities

### New Capabilities

None. No capability is introduced: every change revises an existing capability.

### Modified Capabilities

- `api`: adds the wire-format, closed-vocabulary, response-header, stored-content and link-target requirements.
- `user`: `REQ-USER-003` is replaced by `REQ-USER-020`, because its rule flips from lowercase-only to uppercase-allowed
  and whitespace-free, and its scenario titles cannot change in place.
- `gallery`: `REQ-GALLERY-003` rejects surrounding whitespace instead of trimming it.
- `music`: `REQ-MUSIC-008`, `014`, `015`, `019`, `024` and `029` reject surrounding whitespace; `REQ-MUSIC-020` is
  replaced by `REQ-MUSIC-030`, which stores the submitted playlist name unchanged and unique per owner.
- `movies`: `REQ-MOVIES-004` and `REQ-MOVIES-005` reject a film name with surrounding whitespace.
- `series`: `REQ-SERIES-008`, `009`, `012` and `013` reject a show or episode name with surrounding whitespace; the
  season requirements are unchanged.
- `authentication`: `REQ-AUTHENTICATION-004`, `005` and `008` accept a username or an email address as the identifier,
  keep the rejection uniform and key the limiter on that identifier.
- `logging`: `REQ-LOGGING-010` and `REQ-LOGGING-012` drop `claimed_identity`; `REQ-LOGGING-011` is removed.

## Impact

- **Code** — domain `UsernamePolicy` and `NamePolicy`; the `user` migration check; application `login_user` (identifier
  lookup and the event) and the seven name use cases; infrastructure `rest` (document declarations, header schema types,
  content body, RFC 3339 rendering, the two response property renames), `handler/identity` (request shape, vocabulary
  parsers, grant and principal payloads) and the SQLx user lookup; the composition root wherever a changed constructor
  is wired.
- **API** — the login field and the two response property names are **BREAKING**; response timestamps gain `Z`; new enum
  schemas and typed headers; content bodies stay wildcard-typed but become binary strings; `Link.href` declares a
  non-empty target.
- **Tests** — the served document is proved by the system suite and Schemathesis rather than by a Rust test module;
  updated domain unit tests; new system steps in `test/system` for the login, username, name, date and logging
  scenarios; new container-free cases in `test/api`.
- **Tooling and docs** — `schemathesis.toml`, `test/api/hooks.py`, a new `test:api-cases` devenv task, the API workflow,
  `TechnicalDesign.md` § 3/§ 5, `Glossary.md`, and the capability specs above.
