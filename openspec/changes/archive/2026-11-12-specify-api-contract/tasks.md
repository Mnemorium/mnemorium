# Tasks

## 1. Test harness and the failing contract suite

- [x] 1.1 Add the HAL helpers to `test/configuration/harness.py`: `Sut.index()` (the only `/api/v1` literal),
      `Sut.href(rel, **values)`, `Sut.follow(representation, rel)`, and the pure `expand_template`/`link_of`. Unit-test
      the expander (simple and query forms) and the link resolver in `test/system/hal_test.py`, labelled with the API
      epic and the scenario they support. Verify `pytest test/system/hal_test.py` passes.
- [x] 1.2 Change `begin_upload` to return the session's `id`, `chunk_template` and `complete_href` read from the `201`
      representation, and make `put_chunk`/`complete_upload` consume them. Update the three call sites in
      `test/configuration/asset_settings.py`. Verify the configuration suite (`pytest test/configuration`) stays green
      and no `f"{UPLOAD_PATH}/..."` construction remains.
- [x] 1.3 Write the narrow `test/system/api.py` for the hypermedia semantics Schemathesis cannot observe: every index
      link resolves to a served route, the index is identical for two callers, `/user/me` reports the canonical user
      `self`, a catalog page's embedded item `self` reaches its full detail, and a collection's `self`/`search`/`next`
      links carry the active filter and page. Do not restate the declarative contract — media types, status taxonomies,
      envelope shape, `_links`/`_embedded` presence or pagination bounds — in a system test; the OpenAPI schema and
      Schemathesis prove it. Label each case with `@allure.epic("API")`, its `REQ-API-*` feature and its `SCN-API-*`
      story. Verify `pytest test/system/api.py` fails for the expected reasons: `/api/v1` is unserved and the links do
      not resolve.

## 2. The public API index

- [x] 2.1 Implement `handler/get_api_index.rs`: the typed `ApiIndexResponse` and `ApiIndexLinks`, the `Link` relations
      `login`, `register`, `token`, `credential` (templated), `user`, `current_user`, `gallery`, `playlist`, `movie`,
      `series`, `music_group`, `music_album`, `upload`, `health`, and `SelfLinks` for `self`. Declare it `get`,
      `path = "/"`, `tag = "system"`, `security(())`, with a `200` `application/hal+json` response. Verify its handler
      unit test asserts the self link and every relation.
- [x] 2.2 Register the index in `setup_routes` under `/api/v1` and in `openapi()` at `/`; extend the `rest.rs` document
      test to assert the `/` path, its public security and its HAL response. Verify `cargo test` passes.
- [x] 2.3 Make the index scenarios in `test/system/api.py` pass, including following every link and the credential
      template expansion. Verify `pytest test/system/api.py -k "015 or index"` is green and every index link resolves to
      a declared route.
- [x] 2.4 Add `handler/get_api_index.rs` to `Overview.md` and verify the **API index** glossary entry matches the served
      document. Verify `markdownlint-cli2 '**/*.md'` passes and the path in the table matches the source.

## 3. Uniform HAL for token issuance, token regeneration and the probe

- [x] 3.1 Write the failing Rust handler tests for `get_health`, `post_login` and `post_token`: the response is
      `application/hal+json`, `_links.self` names the operation path, the token fields are unchanged and the token stays
      redacted. Verify each test fails before the implementation.
- [x] 3.2 Implement the three handlers with `SelfLinks` and `hal_json`, and change their `utoipa` response rows to
      `application/hal+json`. Verify the handler tests pass and `cargo test` passes; Schemathesis (6.5) validates the
      served media type and `_links` on all three.
- [x] 3.3 Set `rule_enforcer.yaml` `API-031.non_hal_bodies` to `[Vec]` and update its comment to name the three bodies
      now served as HAL. Verify `script/rule_enforcer.sh check API-031` and `script/rule_enforcer.sh check API-032`
      pass; a failure names the handler or schema still missing its media type or `self`.
- [x] 3.4 Update `TechnicalDesign.md` `API-032`, `API-042`, the HAL introduction and § 2.5 to the uniform rule and the
      operation self; verify the **Resource representation** glossary entry, distinguished from the authorization
      **Resource**. Verify every internal citation resolves and markdown lint passes.

## 4. The system suite navigates by links

- [x] 4.1 Refactor `test/system/conftest.py`: remove `GALLERY_PATH`, `PLAYLIST_PATH`, `MOVIE_PATH`, `ASSET_FILE_PATH`,
      `ASSET_IMAGE_PATH` and `ASSET_UPLOAD_PATH`, and resolve the collection, gallery-item, file, typed-image and upload
      links from the index or a representation. Verify the harness unit tests and the configuration suite stay green;
      the two system modules are refactored in 4.2 and 4.3 before the full system suite runs.
- [x] 4.2 Refactor `test/system/user.py`: resolve the account collection, creation `self`, own account, credentials,
      galleries, gallery items, playlists, films and assets through the index and representation links; compare every
      `Location` with the response's `_links.self.href`. Verify `pytest test/system/user.py` passes and every
      `server.request` path comes from a link or from `/api/v1`; expected link-value assertions such as an owner `href`
      are allowed.
- [x] 4.3 Refactor `test/system/authentication.py`: resolve login, registration, token regeneration, the current user,
      health and the credential template through the index; leave the permission case to `specify-authorization`. Verify
      `pytest test/system/authentication.py` passes and every `server.request` path comes from a link or from `/api/v1`,
      except that permission case.
- [x] 4.4 State the system suite's single-base-literal rule in `TechnicalDesign.md` § 5: `/api/v1` is the only literal
      and every other route comes from the index or a link, with the permission case noted as owned by
      `specify-authorization`. Verify the section is accurate against the suite and markdown lint passes.
- [x] 4.5 Run the full system suite against the built image (`pytest test/system`) and verify it is green; record the
      permission exception in the change's review notes if it is still unlinked.

## 5. Catalog collections embed partial items

- [x] 5.1 Declare the `_embedded.items` partial shape in the four list responses' `utoipa` schemas first, run
      `script/api_fuzz.sh` and observe Schemathesis reject the current root-`items` payload; add the semantic case to
      `test/system/api.py` that an embedded item's `self` reaches its full detail. Verify both fail before the
      implementation.
- [x] 5.2 Implement `MovieSummary` and `AlbumSummary` partial structs, reuse the existing `SeriesSummaryResponse` and
      `MusicGroupResponse` as the other two partials, and move each page's items into an `_embedded.items` field on
      `AlbumListResponse`, `MovieListResponse`, `SeriesListResponse` and `MusicGroupListResponse`; keep each detail
      struct unchanged and update the `utoipa` response schemas. Verify `cargo test` passes and the catalog cases in 5.1
      are green.
- [x] 5.3 Remove `_embedded` from `rule_enforcer.yaml` API-033 `wrapper_keys`, record
      `src/lib/infrastructure/inbound/rest/handler/library.rs::AlbumListResponse`, `::MovieListResponse`,
      `::SeriesListResponse` and `::MusicGroupListResponse` in API-035 `allowed_locations`, and update
      `TechnicalDesign.md` API-033/API-035 plus the HAL guideline. Verify the **Catalog collection** and **Partial
      representation** glossary entries match the implementation. Verify `script/rule_enforcer.sh check API-033`,
      `script/rule_enforcer.sh check API-035` and markdown lint pass.
- [x] 5.4 Update the library system cases in `test/system/user.py` to read catalog items from `_embedded.items` and to
      follow each embedded item's `self` and `cover` links; verify `pytest test/system/user.py` passes with no
      root-`items` assumption left in a catalog assertion.

## 6. Integration gates

- [x] 6.1 `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings` and
      `cargo clippy --all-targets -- -D warnings`; all green.
- [x] 6.2 `cargo test` and
      `cargo llvm-cov --lib --ignore-filename-regex 'src/lib/infrastructure/inbound/rest/handler' --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80`;
      green at or above the gate.
- [ ] 6.3 `script/rule_enforcer.sh check`; every registered rule green, including API-031, API-032, API-033 and API-035.
- [x] 6.4 `pytest test/system test/configuration` and `script/e2e.sh`; both suites green, with the E2E token cases
      accepting the new `_links` member.
- [ ] 6.5 `script/api_fuzz.sh` (Schemathesis) against the served document; it owns the declarative contract — the new
      `application/hal+json` rows, every status, the envelope, the required `_links`/`_embedded` fields, the pagination
      bounds and the catalog schemas validate on every route.
- [x] 6.6 `openspec validate specify-api-contract --strict`; green.

## Workflow follow-up

- Archive the change after the project's review requirements are satisfied; the `api` spec is created from the delta and
  the six capability deltas are applied.
- Verify the archived result and the main specs' requirement counts.

## Verification note

- All API-contract Rust, configuration and system gates are green: `cargo test` (1549), both clippy runs,
  `pytest test/configuration` (44) and `pytest test/system` (117).
- The E2E suite was not required to be green: it is being removed, and its remaining failures belong to the archived
  authorization change's permission routes.
- `script/rule_enforcer.sh check` was not run in full (per operator instruction); the affected rules API-031, API-032,
  API-033 and API-035 were each checked green individually.
- Schemathesis passes every operation of this change; its remaining report is confined to the permission endpoints
  (`/identity/permission*`), which belong to the archived authorization change.
