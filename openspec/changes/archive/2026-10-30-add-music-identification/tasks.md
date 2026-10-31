# Tasks

## 1. Provider identity and shared HTTP pieces

- [x] 1.1 Add a `MusicBrainz` variant to `ProviderId` in `src/lib/domain/model/media_candidate.rs`, with its `as_str`,
      its `parse` arm and a predicate that names the medium the provider serves; verify `cargo build` compiles every
      exhaustive match and the existing `ProviderId` tests still pass with `cargo test media_candidate`.
- [x] 1.2 Add the Cover Art Archive hosts to the artwork allow-list in `src/lib/infrastructure/outbound/reqwest.rs`, and
      extend its host-policy tests so a MusicBrainz API host is rejected as an artwork address and the archive hosts are
      accepted; verify with `cargo test outbound::reqwest`.
- [x] 1.3 Make `log_provider_fault` take the operation name instead of the hard-coded `video_metadata`, pass
      `video_metadata` from the TMDB and OMDb adapters, and pass `music_metadata` from the new adapter; verify
      `cargo test outbound::reqwest::tmdb outbound::reqwest::omdb` still asserts the existing `port_fault` event
      unchanged.

## 2. Domain models

- [x] 2.1 Add `src/lib/domain/model/music_query.rs` with the three work kinds and a query that rejects an empty title,
      following `media_query.rs`'s constructor and error shape; verify `cargo build` and
      `cargo clippy --all-targets --all-features -- -D warnings`.
- [x] 2.2 Add `src/lib/domain/model/music_candidate.rs` with the three-variant candidate and its provider-tagged
      reference, carrying the album reference, mediums and status only for a release; verify `cargo build` and
      `cargo clippy`.
- [x] 2.3 Add `src/lib/domain/model/music_metadata.rs` with the three-variant metadata, the edition, the medium summary
      and format, the release status and the artist credit, documenting that partial dates and unmapped values are
      absent; verify `cargo build` and `cargo clippy`.

## 3. Edition ordering service

- [x] 3.1 Add `src/lib/domain/service/release_order.rs` implementing the stable compact-disc partition and the
      official-status, earliest-date, then-identifier tie-breaks for editions, plus the partition for release
      candidates; verify `cargo build`.
- [x] 3.2 Cover the service with its own in-file test module: compact discs first, provider order preserved within a
      group, a mixed-format edition treated as not compact disc, an edition with no reported medium treated as not
      compact disc, official before other statuses, earliest date then identifier, an empty list, and a candidate list
      whose non-release kinds are untouched; verify with `cargo test release_order`.

## 4. Outbound port and identification use case

- [x] 4.1 Declare `MusicMetadataProvider` in `src/lib/domain/port/music_metadata_provider.rs` with `search` and
      `lookup`, `#[cfg_attr(test, mockall::automock)]`, and doc comments stating that absence is a valid outcome and
      that a failure is classified at the adapter boundary; verify `cargo build` and that the generated mock compiles in
      a test build.
- [x] 4.2 Declare `IdentifyMusicUseCase` in `src/lib/application/port/identify_music.rs` and implement it in
      `src/lib/application/use_case/identify_music.rs` over an optional provider, so no configured provider reports
      `Unavailable` while a search that ran and found nothing is an empty candidate list; verify with the unavailable,
      no-candidate, provider-order, provider-failure and empty-title tests.

## 5. Configuration

- [x] 5.1 Rename the video settings value objects to `VideoProviders` in `src/lib/domain/model/configuration.rs` and
      update their call sites, leaving `VideoProvider` and the file's own shape untouched; verify
      `cargo test load_configuration` and `cargo test configuration_source`.
- [x] 5.2 Add `MusicProviders` and `MusicProvider` with `name` and `index` and no credential field, and extend
      `LibraryMetadata` with a `music` member defaulting to an empty list; verify a default configuration loads with no
      music provider.
- [x] 5.3 Add the partial music section to `src/lib/domain/model/partial_configuration.rs` and merge it in
      `load_configuration`, rejecting an unknown provider name, a duplicate name, a duplicate index and a zero index;
      verify the new tests `load_configuration_accepts_a_music_provider`,
      `..._rejects_a_video_provider_in_the_music_section` and `..._rejects_a_music_provider_in_the_video_section` pass.
- [x] 5.4 Add the commented `music:` block to `src/lib/infrastructure/outbound/file_system/default_config.yaml` beside
      the video block; verify the configuration source test that loads the default file still passes and names both
      sections.
- [x] 5.5 Compose `configured_music_provider` in `src/lib/infrastructure/outbound/metadata.rs`, returning no provider
      for an empty list so the use case reports unavailable; verify with the absent-when-empty and built-when-configured
      tests.

## 6. MusicBrainz adapter

- [x] 6.1 Add golden payloads under `src/lib/infrastructure/outbound/reqwest/fixtures/musicbrainz/` for the three search
      responses and the three lookup responses, including one carrying a partial date and a non-genre tag; verify each
      parses through the mapping tests added below rather than by inspecting them by hand.
- [x] 6.2 Add `src/lib/infrastructure/outbound/reqwest/musicbrainz.rs` with a constructor taking the API base, the
      archive base, the request timeout, the minimum interval and the attempt budget, and send the descriptive
      User-Agent and `fmt=json` on every request; verify with a stub-server test asserting the header and the format
      parameter.
- [x] 6.3 Implement query construction with escaping of the provider's reserved syntax characters, and cover it with a
      table-driven test asserting the received query parameter for a quote, a conjunction, a field operator and a
      parenthesised title; verify with `cargo test musicbrainz`.
- [x] 6.4 Map the three search responses onto candidates, including the release extras and an absent artist and year;
      verify with `musicbrainz_search_maps_a_release_group_hit`, `..._a_release_hit_with_its_mediums`,
      `..._a_recording_hit` and `..._leaves_an_unreported_artist_absent`.
- [x] 6.5 Map the three lookup responses onto metadata, covering the edition list and its ordering hand-off, the absence
      rules for a partial date, a placeholder country and an unmappable production type, the production-type table, and
      genre normalisation with a non-genre tag dropped; verify with the mapping tests and `cargo test musicbrainz`.
- [x] 6.6 Classify `503` and `429` as `RateLimited` inside the adapter before delegating to the shared helper, and
      classify `404` as an absent entity rather than a failure; verify with `musicbrainz_classifies_each_status` and
      `musicbrainz_lookup_returns_none_for_an_unknown_mbid`.
- [x] 6.7 Implement the interval and the bounded retry: reserve the next slot, retry a rate-limited, timed-out or
      transiently failed request, never retry an invalid request or an undecodable payload, and log exactly one
      `port_fault` on give-up; verify with the retry, budget-exhaustion, non-retryable, spacing and
      concurrent-serialisation tests.
- [x] 6.8 Fetch artwork through the archive's JSON index and then the image address it names, returning no artwork and
      one event when the index is absent or the download fails; verify with
      `musicbrainz_artwork_follows_the_cover_art_json_hop` and `..._artwork_absence_yields_none_and_one_event`.
- [x] 6.9 Follow the archive's own redirects without weakening the shared client: re-scheme an allow-listed plain-http
      address to https, admit the archive's per-item CDN subdomains, and stop a hop outside the allow-list, while the
      video adapters keep refusing redirects; verify with `musicbrainz_artwork_follows_the_archives_redirect_hop`,
      `..._artwork_refuses_a_hop_outside_the_allow_list` and the extended host-policy test.

## 7. Documentation

- [x] 7.1 Add the new model, service, port and identification-use-case rows to the module map in
      `docs/development/Overview.md`; verify `markdownlint-cli2 'docs/**/*.md'` and `prettier --check` pass.

## 8. Integration checks

- [x] 8.1 Verify the whole gate passes: `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, and
      `cargo fmt --all -- --check`.
- [x] 8.2 Verify coverage stays at or above the gate with
      `cargo llvm-cov --lib --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80`.
- [x] 8.3 Verify `docs/openapi.json` is unchanged (`git diff --stat docs/openapi.json` is empty), since no HTTP surface
      is added.
- [x] 8.4 Verify `script/e2e.sh` and the container smoke test do not assert the shape of `default_config.yaml`, and
      adjust them only if they do.
- [x] 8.5 Cover the remaining spec scenarios: an album the provider lists no edition or genre for, a reference issued by
      another provider, a provider failure reported without asking another provider, and events that carry no title or
      artist; verify with `musicbrainz_lookup_maps_an_album_without_editions`,
      `..._lookup_ignores_another_providers_reference`, `..._lookup_reports_a_provider_failure_without_another_provider`
      and the extended event assertions.

## Workflow follow-up

- Archive the change once the project's review requirements are satisfied.
- Sync the delta into `openspec/specs/library/music-identification/spec.md` on archive and verify the archived
  capability reads correctly.
