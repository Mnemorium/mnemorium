# Tasks

## 1. Schema and datastore invariant

- [x] 1.1 Add a nullable `cover_file_id INTEGER` referencing `file (file_id)` to the in-place `create_movie`,
      `create_series`, `create_season`, `create_music_album` and `create_music_playlist` migrations, naming the foreign
      key `fk_<table>_cover_file` and declaring no `ON DELETE CASCADE` (`PERS-014`, `ARCH-004`). Verify by deleting
      `mnemorium.db` with its `-wal` and `-shm` sidecars and starting the server: `sqlx::migrate!` applies every
      migration without `VersionMismatch`.
- [x] 1.2 Add one trigger migration pair per table, named `<version>_require_image_cover_<table>`, each holding the
      `BEFORE INSERT` and `BEFORE UPDATE` triggers that `RAISE(ABORT)` unless the referenced file's `mime_type.type` is
      `IMAGE`. Verify with a SQLx integration test that a cover pointing at an audio or video file answers
      `RepositoryError::Conflict` (`PERS-017`) and stores nothing, while a cover pointing at an image file is stored.
- [x] 1.3 Update the entity-relationship diagram with `cover_file_id: INTEGER <<FK>>` on the five entities and their
      `file` relationships, keeping the FK marker on the child (`PERS-011`, `PERS-013`), and extend the
      partial-index/constraint prose with the image-only invariant. Verify the diagram's columns match `migrations/` and
      that the Markdown gates (`lint:md`, `fmt:md`) pass.

## 2. Domain models

- [x] 2.1 Give `Movie`, `Series` and `Season` an optional cover file identifier with a getter, threaded through
      `Movie::try_new`, `Series::try_new` and `Season::new`. Verify with unit tests that a resource built with no
      artwork reports `None` and one built with artwork reports the identifier.
- [x] 2.2 Give `MusicAlbum` and `MusicPlaylist` the same optional cover file identifier and getter. Verify with unit
      tests covering the `Some` and `None` cases on both constructors.

## 3. Repository read paths

- [x] 3.1 Project `cover_file_id` in the movie read and map it onto `Movie`. Verify with an adapter test that reads one
      film with artwork and one without, asserting `Some` and `None`.
- [x] 3.2 Project the show's cover and each season's cover in the series aggregate read, mapping the season value
      through its `season_id` row without exposing that identifier (`D8`). Verify with an adapter test where the show
      carries artwork, one season carries its own and another carries none, asserting all three independently.
- [x] 3.3 Project `cover_file_id` in the album read and map it onto `MusicAlbum`. Verify with an adapter test covering
      both a covered and an uncovered album.
- [x] 3.4 Project `cover_file_id` in the playlist read and map it onto `MusicPlaylist`. Verify with an adapter test
      covering both a covered and an uncovered playlist.

## 4. Application use cases

- [x] 4.1 Carry the cover through the movie list and get response types. Verify with application tests that the field
      reaches the response for a film with artwork and stays `None` otherwise.
- [x] 4.2 Carry the show's cover through the series list and get responses, and the season's cover through the season
      read and the season summaries embedded in the series read. Verify with application tests asserting the show and
      its seasons report their own values with no inheritance.
- [x] 4.3 Carry the cover through the album list and get response types. Verify with application tests covering both
      cases.
- [x] 4.4 Carry the cover through the playlist list and get response types, including the `GET .../track` alias read
      that returns the same representation. Verify with application tests covering both cases.

## 5. REST representations, the cover link and the policy doc

- [x] 5.1 Add a single `asset_image_href(file_id)` helper under `handler/library` carrying the TODO that the Asset image
      route is not served yet, and route every cover link through it (`D5`). Verify with a unit test on the helper's
      output and that no other module formats an `/api/v1/asset/image/` path.
- [x] 5.2 Add an optional `cover` link to `MovieLinks`, `SeriesLinks`, `SeriesSeasonLinks`, `AlbumLinks` and
      `PlaylistLinks`, skipping serialization when absent, and pass the cover into every constructor call site in both
      list and detail conversions. Verify with handler tests asserting `_links.cover` is present for a covered resource
      and absent for an uncovered one.
- [x] 5.3 Add `cover_file_id: Option<NumericID>` to `MovieResponse`, `SeriesResponse`, `SeasonResponse`, `AlbumResponse`
      and `PlaylistResponse`, serialized as `null` when unset. Verify by updating the handler tests' exact payload
      assertions for both the covered and uncovered case.
- [x] 5.4 Amend the Technical Design HAL section: drop the "once a file endpoint exists" qualifier on the file link and
      add the paragraph stating that a representation carries a referenced file's `file_id` and links it, published
      whether or not the route is served (`API-037`, `D10`). Verify the paragraph reads correctly against
      `API-034`/`API-037` and that the Markdown gates pass.

## 6. Integration verification

- [x] 6.1 Regenerate `docs/openapi.json` with `cargo run --bin openapi_gen` and confirm the diff only adds the five
      `cover_file_id` properties, the link fields and no path. Verify with `git diff docs/openapi.json`.
- [x] 6.2 Run the full gate: `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check`,
      `cargo test`, and the coverage gate
      `cargo llvm-cov --lib --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80`. Verify every
      command exits zero.

## Workflow follow-up

- Archive the change after the project's review requirements are satisfied.
- Verify the archived result with `openspec validate`.
