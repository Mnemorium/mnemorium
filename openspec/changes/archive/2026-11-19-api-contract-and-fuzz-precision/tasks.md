# Tasks

Each numbered section is one reviewable, separately committable chunk. Sections are ordered so every one can be verified
against what landed before it.

Three rules govern every test this change touches:

- Write the failing test first, watch it fail for the expected reason, then implement until it passes; a bug fix starts
  with a test that reproduces the defect. Put each proof at the lowest level that can observe the behaviour, and never
  re-assert at a higher level what a lower one already owns.
- Mark every test the change touches with the three Allure labels: `epic` for the functional area, `feature` for the
  requirement id and `story` for the scenario id. The replaced requirements use their new ids (`REQ-USER-020`,
  `REQ-MUSIC-030`).
- A declared status, media type, response header or `_links`/`_embedded` field belongs to Schemathesis, not to a system
  case. A system case proves only the effect neither can observe.

Do not run `script/e2e.sh` or `script/configuration.sh`; they are unaffected. Do not run `devenv test` until the
verification section.

## 1. Rulebook and the document test

- [x] 1.1 Author `API-046`–`API-050` in `docs/development/TechnicalDesign.md` § 3, appended after `API-045`: wire
      formats, closed vocabularies, response headers, stored bytes and link targets. Extend the constraint-attributes
      paragraph with the enum/`422`/`enum_validation_failed` promise and the asset-content-retrieval detail with the
      published headers and statuses.
- [x] 1.2 Update § 5 (API fuzz test): state the carve-out rule with its two categories (a concrete server behaviour the
      document cannot express, or a tool limitation), record `valid_hal_links` (`REQ-API-003`/`004`/`008`) and
      `valid_collection_links` (`REQ-API-007`), the `test:api-cases` task, and the corrected seed-data paragraph.
      Correct the system-suite sentence that still places `hal_test.py` under `test/system/`.
- [x] 1.3 ~~In `src/lib/infrastructure/inbound/rest.rs`, add a `#[cfg(all(test, feature = "api-docs"))]` test module~~ —
      removed at the developer's instruction: no Rust test module in `rest.rs`, the fuzz run and the system suite prove
      the document.

## 2. Wire formats

- [x] 2.1 Failing document assertion: every `_date` property declares `format: date`, every `_at` property
      `format: date-time`, and no such property lacks a format (`API-046`). The assertion was the removed Rust document
      test; the fuzz run now enforces the formats.
- [x] 2.2 Failing system assertions: a created resource's date is `YYYY-MM-DD`; a timestamp is RFC 3339 UTC ending in
      `Z`; a malformed date in a create or patch body answers `422`. Update `test/system/file.py:83` and
      `test/system/media_ingest.py:427` from the `%Y-%m-%dT%H:%M:%S` equality to the `Z` form, and add the round-trip
      assertions the owning modules lack.
- [x] 2.3 Add `#[schema(format = ...)]` to every response DTO, rename `ImageResponse.created_at` to `created_date` and
      `FileResponse.uploaded_at` to `uploaded_date`, and update `test/system/image.py:162` and `test/system/file.py`.
- [x] 2.4 Render timestamps as RFC 3339 UTC through one shared constant: `library.rs`'s `format_timestamp`, `asset.rs`'s
      two sites. Keep the `Last-Modified` HTTP-date formatter separate.
- [x] 2.5 Regenerate the system client and confirm the date/timestamp model fields become `datetime.date`/
      `datetime.datetime` without a new runtime dependency; adjust any case that reads those fields from a model.
- [x] 2.6 Delete the `_valid_date` filter from `test/api/hooks.py`; a fuzz run shows no positive case carries an invalid
      date.

## 3. Username

- [x] 3.1 Rewrite the failing system cases in `test/system/user.py`: `Alice` and `aliceSmith` are created (`201`) and
      read back; `" Alice"`, `"alice "` and `"al ice"` answer `422` and store nothing; 4 and 100 characters are
      accepted, 3 and 101 rejected. Relabel the rejection case to the new requirement (`REQ-USER-020`, `SCN-USER-020-A`,
      `SCN-USER-020-B`).
- [x] 3.2 Update `src/lib/domain/service/username_policy.rs` unit tests first: uppercase accepted, interior whitespace
      rejected, boundaries unchanged. Remove `ContainsUppercase`, replace `SurroundingWhitespace` with
      `ContainsWhitespace`, and update the two match arms in `register_user.rs` and `update_user.rs`.
- [x] 3.3 Declare `pattern = "^\S+$"` and the corrected description on `RegisterRequest.username` and
      `PatchUserRequest.username`, beside the existing bounds. Leave the `list_users` filter at its declared bounds; it
      is a lookup, not a write.
- [x] 3.4 Edit `migrations/20260904012829_create_user.up.sql`: `chk_user_username` keeps the length check and the exact
      uniqueness, and drops `username = lower(username)`. Confirm a boot with a lowercase row accepts a new uppercase
      row and still refuses an exact duplicate.
- [x] 3.5 Delete the `_valid_username` filter.

## 4. Names

- [x] 4.1 Failing system assertions per capability: a create or patch with a name that begins or ends with whitespace
      answers `422` and stores nothing — gallery, film, album, music group, playlist, show and episode. Rename
      `test/system/music.py::test_a_playlist_name_is_trimmed` and relabel it to `REQ-MUSIC-030`/`SCN-MUSIC-030-A`; add
      the surrounding-whitespace case to each existing refusal case.
- [x] 4.2 Update `src/lib/domain/service/name_policy.rs` unit tests first: `" x "` is `Err(SurroundingWhitespace)`,
      `"Late  night"` is accepted unchanged, the boundaries keep their behaviour. Add
      `NamePolicyError::SurroundingWhitespace` and stop trimming.
- [x] 4.3 Update the thirteen use-case error mappings (`create_*`, `update_*`, `put_series_episode`) to treat the new
      variant as the same invalid-name refusal; no call-site signature changes.
- [x] 4.4 Declare the shared `pattern = "^\S(?:[\s\S]*\S)?$"` and corrected description on every request `name` of the
      seven resources, beside `min_length`/`max_length`. Do not add it to a season or an album-track name.
- [x] 4.5 Delete the `_valid_name` filter and its dead `stage_name` branch. No data migration: stored names are already
      trimmed.

## 5. Identify position

- [x] 5.1 Add a failing system assertion that a partial `season`/`episode` pair answers `422` for
      `/library/series/identify/{provider}/{reference}`. The fuzz coverage cannot produce it while the parameters are
      pinned.
- [x] 5.2 Add `422` to the operation's `positive_data_acceptance.expected-statuses` in `schemathesis.toml` and delete
      the `_valid_position` filter.

## 6. Vocabularies, headers, stored bodies and link targets

- [x] 6.1 Failing document assertions: each listed property and parameter resolves to an enum schema; every content
      response declares its emitted headers and `Content-Length` is an integer; `Location` is a URI reference,
      `WWW-Authenticate` a bearer challenge, `ETag`/`Content-Range`/`Allow` carry their patterns; `Link.href` has a
      non-empty relative target; no content body is an integer array (`API-047`–`API-050`).
- [x] 6.2 Add the handler-local wire enums and bind them with `#[schema(value_type = ...)]`/`#[param(value_type = ...)]`
      for the table in `design.md`, including the permission and share bodies. Keep the raw-value parsing.
- [x] 6.3 Split the grant parsers from the domain vocabulary: a grant role accepts only the five grantable roles, a
      share role only `VIEWER`/`DOWNLOADER`, a child kind excludes `ADMIN`, and `PrincipalPayload.role` is
      `ADMIN`/`STANDARD`. A rejected value goes through `ApiError::reject_enum`, not a domain `Invalid`.
- [x] 6.4 Add the typed header schemas with `inline(Type)` and the `HeaderBuilder` schema on the shared `401` addon;
      complete the header lists on all four content handlers; make `Content-Length` an integer and `Last-Modified` an
      HTTP-date pattern.
- [x] 6.5 Declare `Link.href` with `min_length = 1`, `format = "uri-template"` and the relative anchor; give
      `FileResponse.mime_type` the same pattern `PostUploadRequest.content_type` declares.
- [x] 6.6 Regenerate the client and run the fuzz suite: the permission, share and grant routes reach the authorization
      engine, and `response_schema_conformance`/`response_headers_conformance` pass. Remove the config entries this
      makes obsolete (identify provider pins, `422` allowances whose only source was an undeclared vocabulary), keeping
      the genuine domain rules.

## 7. Custom checks

- [x] 7.1 Add `test/api/checks.py` with the two pure check bodies. The `test/api/hal_checks_test.py` failing cases were
      removed at the developer's instruction, so the checks are proved by the fuzz run alone. HAL: missing `_links`,
      missing `self`, empty `href`, `_links` as an array, a link that is not an object, an error envelope whose `self`
      names another path; passing: a success with `self` and a templated `search`, a `4xx` whose `self` is the request
      path with the query ignored, a non-HAL type, a HAL type with parameters. Collection: `self` drops a filter; `next`
      drops a filter or fails to advance by `limit`; `search` is not templated or names another collection; `prev`
      appears at offset 0; passing: a filtered page with defaults added, a page with `next` and `prev`, an empty page, a
      non-HAL response and a single-resource HAL response.
- [x] 7.2 Implement the checks against Schemathesis 4.33.1's lowercase, list-valued `response.headers`; read the content
      type from `response.headers.get("content-type")`. Apply the collection query-preservation rule only when `self`
      carries `limit`/`offset`.
- [x] 7.3 Register thin wrappers in `test/api/hooks.py`, each docstring citing its requirement. Keep the pure bodies in
      `checks.py` so `pytest test/api` runs without Schemathesis installed.
- [x] 7.4 Add the `test:api-cases` devenv task (`pytest test/api`) and confirm the API job's existing `pytest test/api`
      step needs no change. `test:api` stays the fuzz run.
- [x] 7.5 Run `script/api_fuzz.sh`: both checks are silent on a green run, and a deliberate local mutation of `_links`
      and of `next.offset` produces a named failure for each. Do not commit the mutation.

## 8. Login identifier and the failed-login log

- [x] 8.1 Failing system and unit tests: a login by email succeeds exactly as by username; a value matching a username
      resolves as the username even when another account's email matches; an unknown identifier and a wrong password are
      indistinguishable; an unknown identifier is throttled identically; an `authn_failed` event carries no username,
      email or derived value.
- [x] 8.2 Rename `LoginRequest.username` to `identifier`, `LoginUserCommand` and `LoginAttemptKey::Username` with it.
      Look the value up as a username first and as an email only when that finds nothing; keep the absent-account
      derived limiter key. Update `test/util/harness.py`, `script/api_fuzz.sh` and the generated client.
- [x] 8.3 Drop `claimed_identity` from the `authn_failed` event in `login_user.rs`; delete the derivation and the three
      unit tests, replacing them with a proof the submitted identifier never reaches a sink. Update
      `test/system/logging_events.py`: keep `test_a_rejected_login_is_recorded_as_a_warning` as the two-events,
      one-reason proof; delete `test_a_control_character_is_escaped` and `test_the_identity_is_bounded` with
      `REQ-LOGGING-011`.
- [x] 8.4 File an issue (the `create-issue` skill) recording the per-account failure correlation the event gives up, the
      `user:<id>` alternative and why it was declined. Link it from the task list; it is a follow-up, not a blocker.
      Filed as [#419](https://github.com/Mnemorium/mnemorium/issues/419).

## 9. Fuzz configuration

- [x] 9.1 Make the config comments true: route `parse_action`, `parse_resource_kind`, `parse_resource_role` and
      `parse_visibility` through `ApiError::reject_enum`; add a unit test on the emitted event. Correct the
      `get_resource_permission` comment to the path kind it actually takes.
- [x] 9.2 Remove the `examples` and `coverage` phase disables from the five destructive operations; re-enable `stateful`
      for `delete_user` (the pin is applied in `before_call`, after link inference). Keep the `put_upload_chunk`
      stateful disable and the four provider `apply` disables.
- [x] 9.3 Pin `body.identifier` and `body.password` for `post_login` and drop `warnings = false`; accept that the
      operation's negative body surface moves to the system suite.
- [x] 9.4 Verify the `delete_user` fixture: the API profile's disposable account owns no gallery, playlist, upload or
      file. Update `test/api/seed` and its documentation only if the fixture moved; the seed change owns the rows.
- [x] 9.5 Enable `ensure_resource_availability` globally, run the suite, and add a per-operation disable only where the
      run flags a genuine server behaviour (a POST-created container is empty by design), each with a comment naming it.
      A failure naming a missing seeded row is a fixture fix instead.
- [x] 9.6 Audit the six `missing_test_data` displays: pin `parameters` to live rows where possible and remove the
      display where the source is already a `422`/`409`. Keep it only where the flow genuinely cannot reach a row.
- [x] 9.7 Shrink every expected-status list to the entries that are not the default and encode a documented rule (`422`,
      `400`, `413`); drop the no-op `"5xx"`. Keep the content endpoints' real statuses as they are.
- [x] 9.8 Delete the remaining hooks with `_valid_date`, `_valid_username`, `_valid_name` and `_valid_position`;
      `hooks.py` keeps only the invalid-header-name filter, its comment naming the `requests` transport limitation.

## 10. Documentation and glossary

- [x] 10.1 Add the new terms to `docs/development/Glossary.md`: Login identifier, Closed vocabulary, Fuzz carve-out and
      Custom check, each with a one-sentence definition that cites no rule id. The developer removed all but Login
      identifier from the glossary.
- [x] 10.2 Update `AGENTS.md`'s test row and add `test:api-cases` beside the fuzz command. The served-document test
      claim was removed with the test module itself.
- [x] 10.3 Confirm `docs/development/Overview.md`'s schema diagrams still describe the database, not the wire, so the
      two response property renames need no edit there. Correct any sentence that names the old wire properties.
- [x] 10.4 Run the Markdown gates (`markdownlint-cli2`, `prettier`) over the edited documents.

## 11. Verification

- [x] 11.1 `cargo test --all-features`, `cargo test`, both clippy invocations and `cargo fmt --all -- --check`.
- [x] 11.2 The coverage gate, with the new domain branches covered.
- [x] 11.3 `openspec validate api-contract-and-fuzz-precision --strict`.
- [x] 11.4 `script/api_fuzz.sh` (build with `api-docs` + `test-seed`) and `script/api_fuzz.sh --start` plus the
      Schemathesis action; no warning except a deliberate display; both custom checks run and are silent.
- [x] 11.5 `pytest test/api` through `test:api-cases` and the API job.
- [x] 11.6 `script/system.sh` serial and with `PYTEST_WORKERS=4`; then each reworked module alone against the seeded
      shared instance.
- [x] 11.7 Read every remaining line of `schemathesis.toml` and `hooks.py` and answer: which concrete server behaviour
      or tool limitation makes this unavoidable? Delete or fix anything that has no answer.

## Added scope (approved during implementation)

Two findings from the verification run were fixed with the developer's approval, beyond the original task list:

- A username containing a NUL (`U+0000`) is rejected: the policy gains `ContainsNul`, the schema publishes
  `^[^\s\x00]+$`, and `REQ-USER-020` gains `SCN-USER-020-G`. SQLite measures a text value up to its first NUL, so the
  domain and the datastore check could not agree on the length and registration answered `500`.
- The query string is closed (`REQ-API-022`): every query DTO denies unknown fields, so an undeclared query parameter
  answers `400` instead of being ignored. `test/system/gallery.py` proves the refusal beside the malformed-value case.
- A wire date must be the exact padded `YYYY-MM-DD` form: `chrono` accepts a one-digit month or day, so the shared
  `parse_wire_date` checks the shape before parsing and the declared ten-character bound is enforced.
- The name and username policies treat `U+FEFF` as whitespace and the published patterns also exclude `U+0085`, so the
  pattern and the policy agree on the Unicode whitespace set instead of disagreeing on two code points.
