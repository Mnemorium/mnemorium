# Tasks

> The main `library/music-albums` spec is stale relative to the implemented medium-scoped album API. The unarchived
> `refactor-music-track-identity` change owns that sync; do not edit the album spec or album behavior here.

## 1. Schema and ER diagram

> Edit the two initial migrations in place. Do not add a new migration file and do not use `ALTER TABLE`. This relies on
> `PERS-014`: confirm the schema is still unreleased before editing, and recreate any local database that already
> applied the old definitions so the edited migrations run from scratch.

- [x] 1.1 Edit `migrations/20260904012848_create_music_playlist.up.sql` in place to add
      `last_modified_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP`, set `is_public BOOLEAN NOT NULL DEFAULT 0`, and add
      `CONSTRAINT uq_music_playlist_user_id_name UNIQUE (user_id, name)`; verify a fresh in-memory database applies the
      migration and the playlist create/list adapter tests pass.
- [x] 1.2 Edit `migrations/20260904012849_create_music_track.up.sql` in place to add `ON DELETE CASCADE` to
      `fk_music_track_playlist`, add the partial unique index `uq_music_track_playlist_recording` on
      `(music_playlist_id, music_recording_id)` where `music_playlist_id` is not null, and add
      `chk_music_track_playlist_track_index CHECK (music_playlist_id IS NULL OR track_index <= 4294967295)`; verify a
      fresh in-memory database applies it, the cascade test removes the playlist's placements while recordings survive,
      and the duplicate-recording test passes.
- [x] 1.3 Update the entity-relationship diagram and the partial-index paragraph in
      `docs/development/TechnicalDesign.md` (`PERS-011`) for the new column, unique constraint, cascade, and third
      index; verify every annotation matches the edited migrations.

## 2. Domain services and models

- [x] 2.1 Add `domain/service/name_policy.rs` with `MAX_NAME_LENGTH = 100`, `NamePolicyError::{Empty, TooLong}`, and
      `NamePolicy::validate` that trims, rejects empty/too-long values, and returns the normalized name; register the
      module in `domain/service.rs`; verify `cargo build`.
- [x] 2.2 Refactor `Gallery::try_new` and `Gallery::set_name` to use `NamePolicy`, map its errors onto `GalleryError`,
      and update `MAX_GALLERY_NAME_LENGTH` references; verify the gallery create/update application tests cover
      trimming, empty, over-long, and duplicate names and pass.
- [x] 2.3 Extend `MusicPlaylist` with `last_modified_at`, `set_name`, `set_is_public`, `is_owned_by`, and `touch`, and
      route its name validation through `NamePolicy`; verify `cargo build` and the application tests that construct
      playlists compile.
- [x] 2.4 Add `MusicPlaylist::ensure_recording_absent` and `MusicPlaylist::validate_order`, including the
      `RecordingAlreadyPresent` and `InvalidOrder` errors; verify the aggregate rules are exercised by the update and
      reorder application tests added in group 4.

## 3. Repository port, adapter, and unit of work

- [x] 3.1 Add `MusicPlaylistFilter` (`id`, `name`, `user_id`, `is_public`) and the `MusicPlaylistRepository` port with
      `search`, `create`, `save`, `delete`, `add_track`, `remove_track`, and `replace_tracks`, plus the `&mut T` test
      impl; verify `cargo build`.
- [x] 3.2 Add `LibraryUnitOfWork::music_playlists` and wire it through `SqlxUnitOfWork`, the test unit of work, and the
      existing test harness; verify `cargo build` and the existing library tests still pass.
- [x] 3.3 Add the SQLx row model and `SqlxMusicPlaylistRepository::search` with the single aggregate join, ordering by
      playlist identifier and track index, `u32` index conversion, and `stored_data_invalid` classification; verify the
      new adapter test module passes.
- [x] 3.4 Implement `create`, `save`, `delete`, `add_track`, `remove_track`, and `replace_tracks`; make `add_track`
      validate and append atomically and return the hydrated occurrence, and make `replace_tracks` delete and reinsert
      in one transaction; verify the adapter tests pass.
- [x] 3.5 Add adapter tests for an empty playlist, one and several tracks, ordering, every filter, a medium-attached row
      excluded, `u32` overflow, malformed recording data, unique name, duplicate recording, cascade delete,
      add/remove/replace, and `last_modified_at` persistence; verify
      `cargo test --lib infrastructure::outbound::sqlx::music_playlist_repository`.

## 4. Application ports and use cases

- [x] 4.1 Add `ListPlaylists` (port, use case, factory accessor) with the visibility retain and in-memory pagination;
      verify the use-case tests cover public/owned/root-admin visibility, filters, pagination, total, unknown caller,
      repository failure, and commit/rollback.
- [x] 4.2 Add `CreatePlaylist` (port, use case, factory accessor) with `NamePolicy`, per-owner uniqueness, the private
      default, and `AlreadyExist` mapping; verify the tests cover success, default private, duplicate name, empty/
      over-long name, unknown caller, and commit/rollback.
- [x] 4.3 Add `GetPlaylist` and `GetPlaylistTrack` (ports, use cases, factory accessors); reject an invisible/unknown
      playlist as `NoSuchPlaylist`, resolve one track by index, and map a missing track to `NoSuchTrack`; verify the
      tests cover readable/invisible/unknown playlists, the track index, and repository failure.
- [x] 4.4 Add `UpdatePlaylist` (port, use case, factory accessor) accepting `name`, `is_public`, and `add_track`;
      enforce per-field authorization, name uniqueness excluding the target, `MusicPlaylist::ensure_recording_absent`,
      atomic apply, and one `touch`; verify the tests cover rename, visibility, empty body, duplicate name, invalid
      name, unknown recording, duplicate recording, mixed owner-only + add rejection, non-owner metadata rejection, and
      rollback.
- [x] 4.5 Add `DeletePlaylist` (port, use case, factory accessor) with read visibility and owner-only deletion; verify
      the tests cover owner delete, visible non-owner `Forbidden`, invisible `NoSuchPlaylist`, and rollback.
- [x] 4.6 Add `RemovePlaylistTrack` (port, use case, factory accessor) with read visibility, a missing index mapped to
      `NoSuchTrack`, gap preservation, and `touch`; verify the tests cover success, missing track, invisible playlist,
      and repository failure.
- [x] 4.7 Add `ReorderPlaylistTracks` (port, use case, factory accessor) with `MusicPlaylist::validate_order`,
      `replace_tracks`, contiguous `0..n-1` indices, and `touch`; verify the tests cover a valid permutation,
      incomplete/ duplicate/unknown orders, the empty playlist, invisible playlist, and rollback.
- [x] 4.8 Wire every new use case through `LibraryUseCaseFactory`, the production factory, and the test mock; verify
      `cargo build` and the factory wiring tests.

## 5. REST API

- [x] 5.1 Add the playlist and track representations, links, request payloads, path parsers, and error mappings to
      `handler/library.rs`, including the `tracks` alias link and the `application/hal+json` content type; verify
      `cargo build`.
- [x] 5.2 Add the endpoint files and routes for list, create, get, alias, get one track, patch, delete, remove track,
      and reorder track, with stable `400` parsers for `playlist_id` and `track_index`; verify `cargo build`.
- [x] 5.3 Add handler tests for every path and status (`200`, `201` + `Location`, `204`, `400`, `401`, `403`, `404`,
      `409`, `422`), payload shapes, HAL links, canonical alias `self`, and the zero-based track index; verify
      `cargo test --lib infrastructure::inbound::rest`.
- [x] 5.4 Regenerate and commit `docs/openapi.json` with `cargo run --bin openapi_gen`; verify every playlist path,
      request schema, response schema, and the `u32` track index appear in the generated spec.

## 6. E2E and documentation

- [x] 6.1 Add one E2E file per use case under `test/e2e/library/` (`uc013_create_a_music_playlist.py` through
      `uc019_delete_a_music_playlist.py`) covering unauthenticated access, the empty collection, create/list/get/update/
      delete of a private playlist, duplicate names, visibility, malformed identifiers, and the empty track
      alias/collection; verify `pytest -p no:cacheprovider test/e2e/library` passes against `mnemorium-e2e`.
- [x] 6.2 Add UC-013 through UC-019 to `docs/development/UseCases.md` with every trigger, business rule, happy-path
      step, and alternative flow from the spec; verify every endpoint, the 0-based index, the alias, and the
      duplicate-recording rule are named.
- [x] 6.3 Align the `MusicPlaylist` model and glossary in `docs/development/Overview.md` with `last_modified_at`, the
      playlist fields, and the zero-based index; verify the diagram and glossary no longer contradict the schema.
- [x] 6.4 Verify the gallery documentation and OpenAPI still describe trimmed names after the `NamePolicy` adoption;
      update the create-gallery wording only if it no longer matches the behavior.

## 7. Integration verification

- [x] 7.1 Run `cargo test` and verify all unit and integration tests pass.
- [x] 7.2 Run `cargo clippy --all-targets --all-features -- -D warnings` and `cargo fmt --all -- --check`; verify both
      are clean.
- [x] 7.3 Run `cargo llvm-cov --lib --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80`; verify the
      coverage gate passes.
- [x] 7.4 Run `devenv test` and `pytest -p no:cacheprovider test/e2e`; verify the full pre-commit gate and the E2E suite
      pass.

## Workflow follow-up

- Archive the change after the project's review requirements are satisfied.
- Verify the archived result.
