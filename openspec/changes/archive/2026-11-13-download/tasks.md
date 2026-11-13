# Tasks

Schemathesis already proves a requirement's declarative contract against the served document: every declared status,
media type, response header, envelope shape and `_links`/`_embedded` field belongs to the API fuzz test
(`script/api_fuzz.sh`). Do not add a system case that restates it — a system case proves only the effect the fuzzer
cannot observe, and the task names the fuzz operation that covers the rest.

Do not run the E2E suite (`script/e2e.sh` or `pytest test/e2e`), the configuration suite (`script/configuration.sh`) or
`devenv test` in this change, with no exceptions.

## 1. Move the context

- [x] 1.1 Keep the four content handlers and the helpers only they use (`content_command`, `asset_content_response`,
      `content_disposition`) in `src/lib/infrastructure/inbound/rest/handler/asset/`, register their routes from the
      asset context module, and keep the four content paths under `/api/v1/asset/…`. Download stays inside the Asset
      bounded context. Verify `cargo build --bin server` succeeds and `cargo test` passes unchanged.
- [x] 1.2 Verify `docs/development/Overview.md` names no separate `handler/download` folder: the content handlers stay
      under `handler/asset`.

## 2. Authenticate with the session token only

- [x] 2.1 Add the `REQ-DOWNLOAD-001` cases to `test/system/download.py` with their `epic`/`feature`/`story` labels: a
      bearer request reads content, a headerless request answers `401`, and `exp`/`sig` query values answer `401`.
      Verify the query-value case fails because the minted credential is still accepted.
- [x] 2.2 Collapse `ContentAccess` to the bearer path: delete the `?exp&sig` parsing and the ambiguous-credential `400`,
      take `AuthenticatedUser` in the four handlers, and drop the `400` declaration from their `#[utoipa::path]`
      responses. Add a `rest.rs` OpenAPI test asserting the content operations declare no `400`. Verify the
      `REQ-DOWNLOAD-001` cases and the new document test pass.
- [x] 2.3 Delete the credential itself: `ContentLinkProvider`, `JwtContentLinkProvider`, `CONTENT_LINK_TTL_SECONDS`, the
      `AppState` `FromRef` impl, the composition-root wiring and the four `mint` calls; reduce `content_href` to a plain
      `Link`. Verify `cargo test` passes and `cargo clippy --all-targets --all-features -- -D warnings` reports no dead
      code.
- [x] 2.4 Replace the credential paragraph in `docs/development/TechnicalDesign.md` and the playback-link sentence in
      UC-031 of `docs/development/UseCases.md` with header-only authentication. Verify neither document describes a
      minted credential, an `exp`/`sig` value or a query string carrying a token.

## 3. Decide one action per route

- [x] 3.1 Add the `REQ-DOWNLOAD-002` cases to `test/system/download.py`: image content needs `View`, audio and video
      content need `Stream`, file content needs `Download`, and a grant removed between two requests refuses the next
      one. Verify all four fail against the current decision, which is `Download` for every kind.
- [x] 3.2 Decide the action per addressed target in `get_asset_content` and verify the `REQ-DOWNLOAD-002` cases pass.
- [x] 3.3 Add the `REQ-DOWNLOAD-008-A/B` cases (undiscoverable asset answers `404`, discoverable asset whose action
      refuses answers `403`) and a Rust test proving a refusal opens no bytes (`REQ-DOWNLOAD-009`); verify the two
      system cases fail against the current all-`404` mapping.
- [x] 3.4 Map a refusal to `403` or `404` from the decision the use case already takes and verify the cases and the Rust
      test pass.
- [x] 3.5 Update the action paragraph of `docs/development/TechnicalDesign.md` to name `View`, `Stream` and `Download`
      per route with the `403`/`404` split. Verify the paragraph names all three actions and no longer says every
      content route decides `Download`.

## 4. Carry over the delivery contract

- [x] 4.1 Add the `REQ-DOWNLOAD-003`, `REQ-DOWNLOAD-004`, `REQ-DOWNLOAD-005`, `REQ-DOWNLOAD-006`, `REQ-DOWNLOAD-008-C`
      and `REQ-DOWNLOAD-008-D` cases to `test/system/download.py`: stored media type per kind, `HEAD` without a body,
      the five range forms, the four validator conditions, an absent library root (`503`) and absent bytes (`500`).
      Verify every case passes against the moved context — the behavior is carried over, so a failure means the move
      changed it.

## 5. Save surface

- [x] 5.1 Add the `REQ-DOWNLOAD-007-A/B/C` cases to `test/system/download.py` (a typed file names its original media-row
      name, a bare file names its stored path's final segment, display routes carry no disposition), and a Rust test for
      the encoding of a quote, a separator, a non-ASCII name and a CR/LF attempt (`REQ-DOWNLOAD-007-D`). Verify the Rust
      test fails before the encoder exists.
- [x] 5.2 Emit `Content-Disposition: attachment` on the file content route with RFC 6266 encoding and sanitized
      fallback, resolve the name from the typed media row with the stored-path fallback, and declare the header in that
      operation's `#[utoipa::path]` plus a `rest.rs` assertion. Verify the cases, the Rust test and the document test
      pass.

## 6. Cacheable representations

- [x] 6.1 Add a case asserting a file representation answers `Cache-Control: private, max-age=60` and no `no-store`.
      Verify it fails against the current header.
- [x] 6.2 Replace `no-store` with `private, max-age=60` in the four asset representations and their `#[utoipa::path]`
      headers; verify the case passes and `docs/development/TechnicalDesign.md` records the new value beside the
      representation description.

## 7. Integration and retirement

- [x] 7.1 Retire `test/e2e/asset/uc031_read_asset_content.py`. Verify `test/system/download.py` carries a case for every
      `SCN-DOWNLOAD-*` scenario in the delta spec and no file named `uc031_read_asset_content.py` remains under
      `test/e2e/asset/`.
- [x] 7.2 Run the full gate — `script/rule_enforcer.sh check`,
      `cargo clippy --all-targets --all-features -- -D     warnings`, `cargo clippy --all-targets -- -D warnings`,
      `cargo fmt --all -- --check` and the coverage command from `AGENTS.md` — and verify every gate passes.
- [x] 7.3 Run `openspec validate download --strict` and verify it reports the change valid.

## Workflow follow-up

- Archive the change after review: the archive creates `openspec/specs/download/spec.md` and applies the five removals
  to `openspec/specs/library/assets/spec.md`.
- After archive, verify `library/assets` holds six requirements and `download` holds its nine.
