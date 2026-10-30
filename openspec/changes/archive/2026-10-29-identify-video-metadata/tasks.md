# Tasks

## 1. Domain vocabulary

- [x] 1.1 Add `MediaQuery`, `VideoKind` and `EpisodePosition` in `src/lib/domain/model/media_query.rs`; verify
      `cargo build --lib` passes.
- [x] 1.2 Add `ProviderId`, `ExternalRef` and `Candidate` in `src/lib/domain/model/media_candidate.rs`, with
      `ProviderId` parsing for the lowercase config names; verify `cargo build --lib` passes.
- [x] 1.3 Add `MediaMetadata` (`Movie`, `Series`, `Episode`) in `src/lib/domain/model/media_metadata.rs` and `Artwork`
      in its own `src/lib/domain/model/artwork.rs`; verify `cargo build --lib` passes.
- [x] 1.4 Add the `Genre` enum, structural normalization and alias table in `src/lib/domain/model/genre.rs`; verify
      `cargo build --lib` passes. Normalization behavior is asserted by the adapter tests (5.5 and 6.4), not by a model
      test (`TEST-019`).
- [x] 1.5 Declare the new modules and update the domain rows of `docs/development/Overview.md`; verify
      `markdownlint-cli2` and `prettier` pass on the edited file.

## 2. Path interpreter

- [x] 2.1 Implement `MediaNameInterpreter::interpret` in `src/lib/domain/service/media_name_interpreter.rs`: segment
      split, extension strip, token classification, year/episode/season/release-tag rules, ordered hypotheses and
      duplicate removal; verify `cargo build --lib` passes.
- [x] 2.2 Add the `rstest` corpus in the same file covering folder year, file year, numeric title, acronym, `SxxExx`,
      `1x02`, season folder, absolute numbering, multi-episode, brackets, non-ASCII, samples and extension-less names;
      verify every case passes with `cargo test`.
- [x] 2.3 Assert the corpus invariants: every title is non-empty, an episode implies a season, no release token survives
      in a title, and the order is deterministic; verify the invariant tests pass.
- [x] 2.4 Register the service in `src/lib/domain/service.rs` and the Overview module map; verify build and
      documentation lint pass.

## 3. Port and use case

- [x] 3.1 Add `VideoMetadataProvider` in `src/lib/domain/port/video_metadata_provider.rs` with `search`, `lookup`,
      `automock` and the documented absence/error semantics; verify `cargo build --lib` passes and
      `MockVideoMetadataProvider` is generated.
- [x] 3.2 Add the `IdentifyVideo` application port (command, response and error) in
      `src/lib/application/port/identify_video.rs`; verify `cargo build --lib` passes.
- [x] 3.3 Implement `IdentifyVideo<P: VideoMetadataProvider>` in `src/lib/application/use_case/identify_video.rs` with
      `Option<Arc<P>>`, ordered query iteration, first-hit return, empty advance and error stop; verify the tests of 3.4
      pass.
- [x] 3.4 Add the use-case tests with the mock port and the real parser: first hit, empty-then-hit, error stop, unusable
      name, no provider, `lookup` never called, and `#[traced_test]` proving the name is absent from logs; verify
      `cargo test` passes.
- [x] 3.5 Register the new application port and use case and update the Overview module map; verify build and
      documentation lint pass.

## 4. Configuration

- [x] 4.1 Add `Library`, `LibraryMetadata`, `VideoMetadata`, `VideoProvider` and the redacting `ApiKey` to
      `src/lib/domain/model/configuration.rs`, with defaults, validation and index-ordered access; verify unit tests for
      defaults, a valid two-entry list, unknown provider, duplicate provider, duplicate/zero index, missing key, index
      order, redacted `Debug` and skipped serialization.
- [x] 4.2 Add the partial model in `src/lib/domain/model/partial_configuration.rs` and merge the file layer in
      `src/lib/infrastructure/outbound/file_system/configuration_source.rs`; verify the file-source tests for parsing,
      rejection, template equality and partial-model equality.
- [x] 4.3 Update `src/lib/infrastructure/outbound/file_system/default_config.yaml` with the empty provider list, the
      commented example and the reworded secret note; verify the template/default and partial-model tests pass.
- [x] 4.4 Add the environment-ignored-variable test in `src/lib/infrastructure/outbound/envy/configuration.rs` proving
      no provider setting is environment-overridable; verify the test passes.
- [x] 4.5 Add the configuration section module under `test/configuration/` and register it with the suite; verify
      `script/configuration.sh` passes.

## 5. TMDB adapter

- [x] 5.1 Add `src/lib/infrastructure/outbound/reqwest.rs` and `src/lib/infrastructure/outbound/reqwest/tmdb.rs` with
      `search`, `lookup`, base-URL and timeout constructor inputs, and bearer/v3 authentication; verify
      `cargo build --lib` passes.
- [x] 5.2 Map dates, country, genres and artwork, classify every `reqwest` failure into `ExternalServiceError`, and
      sanitize URLs and keys; verify the mapping tests pass.
- [x] 5.3 Add golden payloads under `src/lib/infrastructure/outbound/reqwest/fixtures/tmdb/` and a 1×1 image under
      `reqwest/fixtures/artwork/`; verify the wiremock tests cover a hit, an empty result, not found, 401, 403, 429,
      5xx, malformed JSON, timeout, connection refused, artwork success/failure/oversize/redirect.
- [x] 5.4 Add the sentinel-key test: wiremock receives the key while the port error's `Display`/`Debug` and our logs do
      not; verify the test passes.
- [x] 5.5 Assert the canonical film expectation (kind, year, country, genre set) shared with OMDb against the TMDB
      fixture; verify the test passes.
- [x] 5.6 Add the artwork-failure and unmatched-genre events to `src/lib/application/security_event.rs` and the
      `TechnicalDesign.md` § 9 catalog; verify the log tests and documentation lint pass.
- [x] 5.7 Update the Overview module map for the `reqwest` module; verify documentation lint passes.

## 6. OMDb adapter

- [x] 6.1 Add `src/lib/infrastructure/outbound/reqwest/omdb.rs` with `search`, `lookup`, the `Response:False` and 401
      handling, and the base-URL and timeout seams; verify `cargo build --lib` passes.
- [x] 6.2 Map `N/A` fields, country names to alpha-2 codes, genres and poster bytes, classify failures and sanitize the
      query key; verify the mapping tests pass.
- [x] 6.3 Add golden payloads under `src/lib/infrastructure/outbound/reqwest/fixtures/omdb/` and the wiremock grid,
      including `Response:False` with HTTP 200 and `Invalid API key!` with 401; verify the tests pass.
- [x] 6.4 Add the sentinel-key test and the same canonical film expectation as 5.5; verify both tests pass.
- [x] 6.5 Verify the artwork client rejects a cross-host redirect; verify the test passes.

## 7. Failover chain

- [x] 7.1 Add `src/lib/infrastructure/outbound/metadata.rs` and `src/lib/infrastructure/outbound/metadata/failover.rs`
      with the generic `FailoverVideoMetadataProvider<L>` and the production `ConfiguredProvider` enum; verify
      `cargo build --lib` passes.
- [x] 7.2 Implement ordered search failover, the classification table, the first-error rule and routed lookup; verify
      the mock matrix for a hit, an empty result, all empty, empty plus error, first error, all errors, invalid-request
      no-fallback, short-circuit and no retry.
- [x] 7.3 Add the lookup routing tests (routed provider, unconfigured reference, routed failure) and the fallback event
      assertion; verify the tests pass.
- [x] 7.4 Add the fallback event row to `TechnicalDesign.md` § 9 and the Overview module map; verify documentation lint
      passes.

## 8. Composition

- [x] 8.1 Extend `RuntimeLibraryUseCaseFactory` with the optional configured chain and the `identify_video()` accessor;
      verify `cargo build --lib` passes.
- [x] 8.2 Build the configured links in `src/bin/server.rs` from `library.metadata.video.providers` in index order and
      pass the chain, or `None`, to the factory; verify `cargo build --bin server` passes.
- [x] 8.3 Startup smoke: an empty provider list starts with identification unavailable; a configured provider with a
      sentinel key starts without a network call; an invalid configuration fails naming the setting and without the key;
      verify by running the binary with each configuration.
- [x] 8.4 Update the startup and composition text in `docs/development/Overview.md`; verify documentation lint passes.

## 9. Integration checks

- [x] 9.1 Run `cargo fmt --all -- --check` and `cargo clippy --all-targets --all-features -- -D warnings`; both are
      clean.
- [x] 9.2 Run `cargo test` and
      `cargo llvm-cov --lib --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80`; all pass.
- [x] 9.3 Run `cargo run --bin openapi_gen` and confirm `docs/openapi.json` is unchanged; verify with
      `git diff --exit-code docs/openapi.json`.
- [x] 9.4 Run `devenv test`, including markdownlint and prettier; all pass.
- [x] 9.5 Run `openspec validate identify-video-metadata`; the change is valid.
- [x] 9.6 Build the container image and confirm the `reqwest` TLS backend compiles for the Alpine/musl runtime; verify
      the image builds and the health check passes.

No E2E file is added: this change introduces no catalogued business use case and no route (`TEST-041`).

## Workflow follow-up

- Archive the change after the project's review requirements are satisfied.
