# Tasks

## 1. Branch

- [x] 1.1 Resolve the delivery metadata: Conventional-Commit type `feat`, the single `GOV-001` scope `api` (the change's
      dominant area — the new REST surface; substitute another closed-list scope if review prefers), slug
      `browse-music-albums`, and pull-request title `feat(api): browse music albums`. The branch is
      `feat(api)/browse-music-albums`. Verified by the type/scope/slug/title all being non-empty and the scope appearing
      in the `GOV-001` scope table.
- [x] 1.2 Sync the default branch: `git fetch origin`, confirm `git status --porcelain` is empty and the current branch
      is not ahead of its upstream, switch to `main`, then `git pull --ff-only origin main`. Verified by the working
      tree being on `main` and clean, with no divergence.
- [x] 1.3 Branch inside a free worktree: run `script/free_worktree.sh`; when it prints a path, create the branch there
      with `git -C <path> switch -c feat(api)/browse-music-albums origin/main`; when it prints nothing, create one with
      `git worktree add /tmp/opencode/mnemorium-browse-music-albums -b feat(api)/browse-music-albums origin/main`; then
      move the session into the worktree with the `opencode.session_move` tool. Verified by the worktree existing on
      `feat(api)/browse-music-albums` and the session's working directory being that worktree.

## 2. Domain model and port

- [x] 2.1 Add `ProductionType::from_wire` / `as_wire` to `src/lib/domain/model/music_album.rs`, mapping the database's
      uppercase tokens (`COMPILATION`, `DJMIX`, `DEMO`, `LIVE`, `MIXTAPE`, `REMIX`, `SOUNDTRACK`, `STUDIO`), with an
      unknown token returning `None`. Add no test module to the domain file. The mapping is verified where it is
      consumed instead (`TEST-019`): `from_wire` by the album-projection repository integration test (§ 3.1) and
      `as_wire` by the album response handler test (§ 5.2).
- [x] 2.2 Add `src/lib/domain/model/music_track.rs` with a `MusicTrack` entity (identifier, `track_index`, the recording
      reference, and the `MusicTrackPosition` naming the container it is positioned by — a medium of an album or a
      playlist — so it holds no container of its own) and a validating constructor; register the module in
      `src/lib/domain/model.rs`. Add no test module to the domain file (`TEST-019`); `cargo build` proves it compiles,
      and the entity is exercised by the application-layer use-case tests (§ 4.2–4.3) and the track-projection
      repository integration test (§ 3.2).
- [x] 2.3 Add `src/lib/domain/port/music_album_repository.rs` named after the aggregate: declare the `MusicAlbumFilter`
      and `MusicTrackFilter` filters first, then the `MusicAlbumRepository` trait (with
      `#[cfg_attr(test, mockall::automock)]`), then the `MusicAlbumSummary` and `MusicTrackDetail` read models — the §
      8.3 declaration order (`STY-RUST-051`, `STY-RUST-052`). The trait carries `search` (the album query) and
      `search_tracks`, mirroring `GalleryRepository::search` and `search_items` (§ 8.1). Verified by `cargo test --lib`
      compiling and generating `MockMusicAlbumRepository`.
- [x] 2.4 Extend `src/lib/domain/port/library_unit_of_work.rs` with a `music_albums()` accessor. Implement it for
      `TestUnitOfWork` in `src/lib/lib.rs`, add a `MockMusicAlbumRepository` field, and update `gallery_factory` /
      `gallery_unit_of_work` (and their call sites) for the extra field. Verified by `cargo test --lib` compiling and
      the existing gallery tests passing.

## 3. SQLx adapter

- [x] 3.1 Add the music-album row models under `src/lib/infrastructure/outbound/sqlx/model/` and implement
      `SqlxMusicAlbumRepository::search` (album fields, the album's medium identifier, aggregated genres from
      `music_album_genre`, track count, `name` filter, ordered by album identifier, mapping the stored `production_type`
      token through `ProductionType::from_wire`). Verified by in-memory migrated-database tests covering filter, stable
      order, genre aggregation, the production-type mapping (a known token and an unknown one) and the empty result.
- [x] 3.2 Implement `SqlxMusicAlbumRepository::search_tracks` (join `music_track` → `music_medium` for album scope, →
      `music_recording` → `audio` → `file` for the backing `file_id`; playlist-attached rows stay out), ordered by
      ascending medium index then track index with a `music_track_id` tiebreak, resolving the display name from the
      recording name with the audio name as fallback. The read model mirrors the gallery item's: it carries `file_id`
      and `name`, as `GalleryItemDetail` does. Verified by tests for album scoping, ordering across mediums, the detail
      fields, and a no-match album.
- [x] 3.3 Implement `music_albums()` on `SqlxUnitOfWork` in `src/lib/infrastructure/outbound/sqlx/unit_of_work.rs`,
      wiring `SqlxMusicAlbumRepository`. Verified by `cargo test --lib` passing the adapter and unit-of-work tests.

## 4. Application use cases

- [x] 4.1 Add `application/port/list_albums.rs` (command with caller id, `name`, `limit`, `offset`; response with items
      and total; error enum) and `application/use_case/list_albums.rs`, plus the `list_albums` accessor on
      `LibraryUseCaseFactory`. Authorization mirrors the rest of Library: the use case loads the caller through
      `library_access::load_caller` exactly as `ListGalleries` does, and carries no `Forbidden` variant because an album
      read is never denied. Verified by use-case tests for the happy path, filter/pagination forwarding, `NoSuchCaller`
      (rollback), repository failure and commit failure.
- [x] 4.2 Add `list_album_tracks` (port, use case, factory accessor) returning an album's tracks in order and
      `NoSuchAlbum` when the album does not exist; load the caller through `library_access::load_caller`, mirroring
      `GetGallery`. Verified by use-case tests for the ordered happy path, unknown album, `NoSuchCaller` and repository
      failure.
- [x] 4.3 Add `get_album_track` (port, use case, factory accessor) returning one track scoped to its album, with
      `NoSuchAlbum` and `NoSuchTrack` when either is absent or mismatched; load the caller through
      `library_access::load_caller`, mirroring `GetGallery`. Verified by use-case tests for the happy path, unknown
      album, a track outside the album, `NoSuchCaller` and repository failure.
- [x] 4.4 Wire the three new accessors in `RuntimeLibraryUseCaseFactory`
      (`src/lib/infrastructure/use_case_factory/library.rs`). Verified by
      `cargo test --lib infrastructure::use_case_factory` and the factory's unit tests.

## 5. REST handlers, routes and OpenAPI

- [x] 5.1 Add the shared representations and helpers to `src/lib/infrastructure/inbound/rest/handler/library.rs`
      (`AlbumLinks`, `AlbumResponse` with `genres` and `track_count`, `TrackLinks`, `TrackResponse` carrying `file_id`
      and `name` like `GalleryItemResponse`, `AlbumListLinks`, `AlbumListResponse`, `TrackListResponse`,
      `parse_album_id`, and the `Error → ApiError` mappings), and register the `/music/album`,
      `/music/album/{album_id}/track` and `/music/album/{album_id}/track/{track_id}` routes in `library_routes`. The
      album's shared helper stays single-use-safe: `parse_track_id` is not extracted, because a single-use extraction is
      rejected by `STY-RUST-001`, so `get_album_track` parses its own track identifier. Verified by
      `cargo test --lib ...handler::library` compiling and the existing route tests passing.
- [x] 5.2 Add `handler/library/get_album_list.rs` with its `utoipa::path` declaration and tests: `200` HAL collection
      shape and links (including the `production_type` wire value produced through `as_wire`), filter/limit/offset
      forwarding, out-of-range `limit` → `422`, malformed query → `400`, error mapping to `401`/`500`. Verified by the
      handler's tests.
- [x] 5.3 Add `handler/library/get_album_track_list.rs` with its `utoipa::path` declaration and tests: `200` collection
      shape, malformed `album_id` → `400`, unknown album → `404`, missing credentials → `401`. Verified by the handler's
      tests.
- [x] 5.4 Add `handler/library/get_album_track.rs` with its `utoipa::path` declaration and tests: `200` track shape
      (including `file_id`, mirroring a gallery item), malformed `album_id`/`track_id` → `400`, unknown album or track
      outside the album → `404`, missing credentials → `401`. Verified by the handler's tests.
- [x] 5.5 Register the three paths and their schemas in `src/lib/infrastructure/inbound/rest.rs`, then regenerate
      `docs/openapi.json` with `cargo run --bin openapi_gen` and commit it. Verified by the new paths and schemas
      appearing in `docs/openapi.json`.

## 6. Documentation and end-to-end coverage

- [x] 6.1 Add business use case `UC-012 - Browse Music Albums` to `docs/development/UseCases.md` following the `DOC-005`
      entry format (description, actor, preconditions, triggers — the three endpoints —, bounded context, business
      rules, happy path, alternative flows, post conditions). Verified by `devenv shell -- markdownlint-cli2` passing on
      the file.
- [x] 6.2 Add `test/e2e/library/uc012_browse_music_albums.py` and the album endpoints to `test/e2e/conftest.py`,
      covering the behaviours observable without seeding endpoints: missing credentials → `401`, the empty album
      collection shape, malformed identifier → `400`, and unknown album → `404`. Document in the module docstring that
      the populated listing/track paths are covered by the integration tests, because no endpoint creates music yet.
      Verified by `pytest -p no:cacheprovider test/e2e` against the `mnemorium-e2e` container.

## 7. Verification

- [x] 7.1 Run `cargo fmt --all -- --check` and `cargo clippy --all-targets --all-features -- -D warnings`; fix any
      finding.
- [x] 7.2 Run `cargo test` and confirm the full unit and integration suite is green.
- [x] 7.3 Run `cargo llvm-cov --lib --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80` and confirm
      the coverage gate holds.
- [x] 7.4 Run `devenv test` and confirm every pre-commit hook passes, including the Markdown gates.

## 8. Pull request

- [ ] 8.1 Stage and commit the work once with a Conventional Commit whose type is `feat` and whose scope is the delivery
      scope from § 1.1, running git through the environment (`devenv shell -- git commit`) so the repository's commit
      hook has devenv tooling (`AGENTS.md`), then push with `git push -u origin feat(api)/browse-music-albums`. Verified
      by the branch existing on the remote with the commit.
- [ ] 8.2 Open a pull request from `feat(api)/browse-music-albums` into `main`, titled `feat(api): browse music albums`
      — the `{type}({scope}): {description}` convention carrying exactly one `GOV-001` scope. Verified by the semantic
      pull-request gate in `.github/workflows/ci.yml` accepting the title.
- [ ] 8.3 Fill the pull-request body from `.github/pull_request_template.md`: what changed, the out-of-scope items
      listed in the proposal (playlist endpoints, album detail, writes), and the `browse-music-albums` change reference.
      Open it ready for review when § 7 is green, otherwise as a draft. Verified by the opened pull request's
      description and status.

## Workflow follow-up

- The change is read-only (see design — Migration Plan): no datastore migration and no data backfill, so archiving needs
  no migration step.
- Archive the change once the pull request is merged and the project's review requirements are satisfied.
