# Tasks

## 1. Range semantics

- [x] 1.1 Parse and resolve one `Range: bytes=` value inline in the content use case: a closed, open-ended or suffix
      range resolves against the opened size, an unsatisfiable range is a distinct outcome, and a multi-range, unknown
      unit or malformed value means "serve the whole content"; verify a table-driven application test covers every case
      including a suffix longer than the content, a start past the end and empty content.
- [x] 1.2 Replace `FileStorage::read` with a bounded open that returns a reader for `[start, start + len)`; verify an
      adapter integration test reads an exact span, an open-ended span, a suffix span, a zero-length span and a span
      past EOF, and that a missing file is `None` and an unreadable file is an error.
- [x] 1.3 Replace the whole-vector response in `get_asset_content` with the ranged reader and the opened file's size,
      and keep `HEAD` on a headers-only path; verify application tests assert the requested span for an authorized file,
      no read for an unreadable asset, and no whole-file fallback when a range is supplied.

## 2. Content responses

- [x] 2.1 Emit `Accept-Ranges: bytes`, `Content-Range`, `Content-Length`, `Content-Type` and the conditional outcomes
      (`200`, `206`, `416`, `304`) from the shared content path; verify the application test classifies the four
      outcomes and the E2E task group exercises them over HTTP.
- [x] 2.2 Classify the three failures: `404` for an unknown or unreadable asset, `503` when the opened file is absent
      and the library root is absent, `500` when the file is absent and the root exists; verify application tests cover
      each classification and that an absent root performs no authorization read.
- [x] 2.3 Answer `ETag` from the stored digest when it exists, `Last-Modified` from `modified_at` always, and honour
      `If-None-Match` and `If-Range`; verify application tests cover a matching and a stale `If-None-Match`, a matching
      and a stale `If-Range`, and a file without a digest serving `Last-Modified` only.

## 3. Delegated authorization

- [x] 3.1 Resolve audio through its recording and the recording's album, resolve a video through its scope, and fall
      back to the movie or episode that names it, without adding a scope column; verify application tests cover a
      catalogue movie, a catalogue episode, a gallery video, an album track, an unattached audio and an unattached
      video.
- [x] 3.2 Make `access_file` consider the audio row so file content follows the same rule as the typed audio view;
      verify a test where a catalogue track's file is readable by a caller with `library.read` and concealed from one
      without it.
- [x] 3.3 Add the repository lookups the resolution needs (movie and episode by `video_id`, recording by `audio_id`);
      verify repository tests cover a hit, a miss and filter strictness (`TEST-005`).

## 4. Typed audio and video resources

- [x] 4.1 Add the audio and video read use cases with their responses (measurements, `file_id`, links, absent optional
      audio measurements); verify application tests cover a found row, an absent measurement and an unknown row.
- [x] 4.2 Add `GET /asset/audio/{audio_id}`, `GET /asset/video/{video_id}`, their `303` `/file` aliases and their
      `/content` routes, registered with the asset router; verify the routes appear in the served OpenAPI document and
      the E2E task group reads them.
- [x] 4.3 Emit `_links.content` and the typed media link on the file representation, and `_links.content` on the image,
      audio and video representations; verify the E2E task group follows each link.

## 5. Content link credential

- [x] 5.1 Add a port for minting and verifying a file-scoped, expiring playback credential, implemented with an HMAC
      over a domain-separated message using the instance signing secret; verify an adapter integration test covers a
      round trip, an expired credential, a credential for another file and a tampered value.
- [x] 5.2 Build the content credential into every representation that carries `_links.content` and answer those
      representations with `Cache-Control: no-store`; verify application tests assert the credential's file scope and
      expiry and that the link composes the existing content href.
- [x] 5.3 Split the content routes onto a credential extractor that accepts the bearer header or `exp`/`sig`, rejects
      both present with `400` and rejects an invalid or expired credential with `401`; verify middleware unit tests for
      header-only, credential-only, both, neither, expired and tampered, and a test proving the query string never
      reaches a log sink (`OBS-003`).

## 6. Reconciliation

- [x] 6.1 Implement the ordered removal in one unit of work: clear cover references, delete gallery items, remove the
      movie/episode and its streams, remove the audio's recording and its placements, then the media row and the file
      row; verify application tests assert the order, a retained record when a step fails, and the aggregate counts.
- [x] 6.2 Add the repository delete operations the cascade needs; verify repository tests against `:memory:` assert a
      nulled cover with the owner retained, a removed movie, episode, stream and gallery item, and a removed recording
      with its track placements, and that a dependent left behind makes the unit of work fail (`TEST-029`–`TEST-031`).
- [x] 6.3 Replace the scan's `report_absent` with the removal path and report `removed` separately from `missing`;
      verify application tests cover a removal counted as removed, a failed removal counted as missing and the scan
      continuing, and a renamed file not counted as removed.

## 7. Startup policy

- [x] 7.1 Add `library.startup.media_root_wait_seconds` (default 30) and `library.startup.fail_without_media_root`
      (default false) to the configuration value objects with validation (a zero wait is valid, a malformed value fails
      startup); verify `load_configuration` tests cover the defaults, an override, a zero wait and a malformed value
      (`STY-RUST-078`), and the settings are marked startup-only (`STY-RUST-076`).
- [x] 7.2 Replace the presence check with a wait-and-report use case that polls up to the window, and make the
      composition root refuse startup when the root is required and absent; verify a paused-clock application test
      covers present, appears-within-window, absent-and-required and absent-and-not-required.
- [x] 7.3 Run the scan even when the root is absent and reconcile the library as empty in the not-required case; verify
      an application test starts with an absent root and asserts every registered file is removed.

## 8. HTTP contract coverage

- [x] 8.1 Add UC-031 (read an asset's content) to `docs/development/UseCases.md` with its diagram node; verify the
      Markdown checks pass.
- [x] 8.2 Export the host media directory from `script/e2e.sh` to pytest and add its fixture; verify the E2E file can
      locate a registered file on the host.
- [x] 8.3 Write `test/e2e/asset/uc031_read_asset_content.py` covering whole content, closed/open-ended/suffix ranges,
      `416`, `HEAD`, validators, `304`, `If-Range`, the typed audio and video views and their aliases, every
      `_links.content`, the delegated reads, the credential-only read and the reconcile-to-`404` flow; verify
      `script/e2e.sh` passes.
- [x] 8.4 Add the audio and video operations and the `Range`/`If-Range`/`If-None-Match` parameters to
      `schemathesis.toml`, with fixture identifiers for the seeded media and content statuses that admit `206`, `304`
      and `416`; verify `script/api_fuzz.sh` passes.
- [x] 8.5 Declare the header parameters and the `206`/`416`/`304` responses in the handlers (`API-009`, `API-027`) and
      regenerate `docs/openapi.json`; verify the document validates and the discussed headers appear.

## 9. Configuration suite and documentation

- [x] 9.1 Let the configuration harness start without creating `media/` and add `library_settings.py` cases: required
      and absent refuses startup and writes nothing, not required and absent starts and reconciles the rows away, and a
      present root does not wait; verify the configuration suite passes without sleeping for the window (`TEST-056`).
- [x] 9.2 Add the two settings to `config.yaml` and to the configuration table in `TechnicalDesign.md` § 10 with their
      startup-only effect; verify the Markdown checks pass.
- [x] 9.3 Record the retrieval contract (ranges, statuses, credential) in `TechnicalDesign.md` § 3 and the removal
      cascade in § 4; verify the documented commands and examples match the implementation.

## 10. Full gate

- [x] 10.1 Run `cargo test`, the `cargo llvm-cov --lib` 80% gate, both clippy runs, `cargo fmt --all -- --check` and
      `script/rule_enforcer.sh check`; verify all pass.
- [x] 10.2 Run `script/e2e.sh`, `script/api_fuzz.sh` and the configuration suite against the built image; verify all
      pass.

## Workflow follow-up

- Announce the removal behaviour in the release notes before upgrading, since a missing file's records are now deleted
  rather than retained.
- Archive the change after the project's review requirements are satisfied.
