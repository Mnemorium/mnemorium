# Tasks

> The Rust type change cascades: the crate compiles green at the end of group 2, and the REST layer compiles at the end
> of group 3. Each group still lands and verifies the tests and documentation its own work calls for.

## 1. Schema and ER diagram

- [x] 1.1 In `migrations/20260904012838_create_music_medium.up.sql`, replace `uq_music_medium_music_album_id` with
      `UNIQUE (music_album_id, medium_index)`, add `CHECK (medium_index BETWEEN 0 AND 255)`, and add the stored-type
      check for `'CD'`; verify `cargo test --lib infrastructure::outbound::sqlx` passes on a fresh in-memory database.
- [x] 1.2 In `migrations/20260904012849_create_music_track.up.sql`, add the exactly-one-container `CHECK`, the medium
      `track_index` `CHECK (music_medium_id IS NULL OR track_index BETWEEN 0 AND 255)`, and the two partial unique
      indexes on `(music_medium_id, track_index)` and `(music_playlist_id, track_index)`; verify the same adapter tests
      pass.
- [x] 1.3 Update the entity-relationship diagram in `docs/development/TechnicalDesign.md` (`PERS-011`) to show the new
      unique keys, checks, and indexes; verify each diagram annotation matches the migration files.

## 2. Domain model, port, adapter, and read path

- [x] 2.1 Define `TrackIndex` (`MediumIndex(u8)`, `PlaylistIndex(u32)`) in `src/lib/domain/model/music_recording.rs` and
      `MediumIndex` (`Cd(u8)`) with `from_wire`/`as_wire` in `src/lib/domain/model/music_medium.rs`; verify
      `cargo build` reports no error in these modules.
- [x] 2.2 Add `index: TrackIndex` to `MusicRecording` and change `try_new`; change `MusicMedium` to
      `index: MediumIndex, tracks: Vec<MusicRecording>`; change `MusicPlaylist.tracks` to `Vec<MusicRecording>`; delete
      `MusicTrack`, `MusicTrackPosition`, `MusicTrackError`, and `src/lib/domain/model/music_track.rs`, and remove its
      `model.rs` entry; verify `cargo build`.
- [x] 2.3 Remove `search_tracks` and `MusicTrackFilter` from `src/lib/domain/port/music_album_repository.rs`, including
      the `&mut T` impl and the port docs; verify `cargo build`.
- [x] 2.4 Add the medium type/index and track index to `MusicAlbumRow`, delete `MusicTrackRow` and
      `sqlx/model/music_track.rs` (plus its module entry), and rewrite `SqlxMusicAlbumRepository::search` to hydrate
      mediums and recording occurrences from the one join, mapping stored type/index and `u8` conversions and reporting
      malformed values as `RepositoryError::DataIntegrityViolation`; verify
      `cargo test --lib infrastructure::outbound::sqlx::music_album_repository`.
- [x] 2.5 Rewrite the adapter tests: cover one and several mediums, medium/track ordering, two tracks referencing one
      recording, a playlist-attached row excluded, a medium with no track, an unknown stored type, an out-of-range
      medium/track index, and a malformed recording; delete the `search_tracks` tests; verify the adapter test module
      passes.
- [x] 2.6 Update the application ports and use cases: `ListAlbumTracksCommand`/`Item`/`Response` (medium-scoped, `id` =
      track index, `recording_id`, `medium_index`), `GetAlbumTrackCommand`/`Response` (`medium_index`, `track_index`),
      `ListAlbumsItem` (mediums collection with index, type, track count), and a new `GetAlbumMedium`
      command/response/error/use-case; verify `cargo build`.
- [x] 2.7 Add `NoSuchMedium` and remove the in-memory `find` by `recording_id` and the `Unknown(anyhow!(...))` miss from
      `ListAlbumTracks` and `GetAlbumTrack`; `GetAlbumMedium` resolves the medium from the album read; verify
      `cargo test --lib application::use_case`.
- [x] 2.8 Rewrite the application tests and fixtures for the occurrence model (recordings carry their index, mediums
      carry `MediumIndex`), including unknown album/medium/track, repository failure, and commit/rollback; verify
      `cargo test --lib application::use_case` passes.
- [x] 2.9 Wire `GetAlbumMedium` through `LibraryUseCaseFactory` (trait, production factory, test mock/fixture) so the
      REST layer can resolve it; verify `cargo build`.

## 3. REST API

- [x] 3.1 Update `src/lib/infrastructure/inbound/rest/handler/library.rs`: add `MediumResponse`/`MediumLinks`, add
      `mediums` to the album response, change `TrackResponse` to `id` = track index plus `recording_id`/`medium_index`
      and the `album`/`medium`/`self` links; verify `cargo build`.
- [x] 3.2 Replace the album-track handlers and routes with `get_album_medium`, the medium-scoped `get_album_track_list`,
      and the medium-scoped `get_album_track`; add `u8` path parsers for `medium_index` and the track index with stable
      `400` messages, and remove the old routes; verify `cargo build`.
- [x] 3.3 Rewrite the handler tests for the new paths, parsers, JSON payloads, and HAL links, including `400`, `404`,
      and `401`; verify `cargo test --lib infrastructure::inbound::rest`.
- [x] 3.4 Regenerate and commit `docs/openapi.json` with `cargo run --bin openapi_gen`; verify the new paths, the `u8`
      medium/track parameters, the medium schema, and `recording_id` appear in the generated spec.

## 4. E2E and documentation

- [x] 4.1 Update `test/e2e/library/uc012_browse_music_albums.py` and the `test/e2e/conftest.py` fixtures for the
      medium-scoped paths, and add the medium, unknown-medium, and malformed-index cases; verify
      `pytest -p no:cacheprovider test/e2e/library/uc012_browse_music_albums.py` passes against the `mnemorium-e2e`
      server.
- [x] 4.2 Update the UC-012 entry in `docs/development/UseCases.md` for the new paths and representations; verify every
      trigger and happy-path step names the medium index and track index and no old path remains.
- [x] 4.3 Align the domain model diagram and glossary in `docs/development/Overview.md` with the model in `design.md`
      (`DOC-002`), removing the `MusicTrack` domain entity; verify the diagram lists no `MusicTrack`.

## 5. Integration verification

- [x] 5.1 Run `cargo test` and verify all unit and integration tests pass.
- [x] 5.2 Run `cargo clippy --all-targets --all-features -- -D warnings` and `cargo fmt --all -- --check`; verify both
      are clean.
- [x] 5.3 Run `cargo llvm-cov --lib --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80`; verify the
      gate passes.
- [x] 5.4 Run `devenv test` and `pytest -p no:cacheprovider test/e2e`; verify the full pre-commit gate and the E2E suite
      pass.

## Workflow follow-up

- Archive the change after the project's review requirements are satisfied.
- Verify the archived result.
