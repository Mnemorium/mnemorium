# Tasks

Each numbered section is one reviewable, separately committable chunk: it maps to one scope of `GOV-001` and can be
reviewed, reverted or run on its own. Sections are ordered so every one of them can be verified against what landed
before it.

Three rules govern every test this change adds:

- Write the failing test first, watch it fail for the expected reason, then implement until it passes; a bug fix starts
  with a test that reproduces the defect. Put each proof at the lowest level that can observe the behaviour, and never
  re-assert at a higher level what a lower one already owns.
- Mark every test the change touches with the three Allure labels: `epic` for the functional area, `feature` for the
  requirement id and `story` for the scenario id.
- Read a response body with `json.loads(response.content)` inline; do not call or add a shared `payload` helper in a
  reworked module, so each case shows the raw document it asserts (a generated model drops fields the document omits).

A declared status, media type, response header or `_links`/`_embedded` field belongs to Schemathesis
(`script/api_fuzz.sh`), not to a system case. A system case proves only the effect the fuzzer cannot observe.

A reworked case drives a declared operation through the generated client (`Sut.client`): the operation, its parameters
and its body come from the served document, so the case names neither a route nor a payload shape. A link the scenario
names is still read from the representation and followed by its `href` (`link_of`, `Sut.follow`), and `Sut.request` is
kept only for the requests the document cannot declare — `HEAD` on a content route, the operator-configured
client-address header, the `/health` probe. Anything else the client cannot express is a missing declaration: add it in
the handler's `#[utoipa::path(...)]`/`ToSchema`, never hand-roll the request.

Do not run `script/e2e.sh`, `script/configuration.sh` or `devenv test` in this change.

## 1. Seed mechanism — the profile as configuration

- [x] 1.1 In `src/lib/domain/model/configuration.rs`, behind `#[cfg(feature = "test-seed")]`, add
      `SeedProfile { Api, Perf, System }` with a case-insensitive `Deserialize` (a preprocessor, because
      `rename_all = "lowercase"` is case-sensitive) and `Seed { profile: Option<SeedProfile> }` with `profile()`; add
      `seed`/`seed()` to `Configuration`. Unit-test the parser at the domain level: each of the three values in lower,
      upper and mixed case, the absent value as `None`, and an unknown value as an error naming `seed.profile` and not
      the value.
- [x] 1.2 Add `PartialSeed { profile: Option<SeedProfile> }` and `PartialConfiguration.seed` to
      `src/lib/domain/model/partial_configuration.rs` under the same `cfg`.
- [x] 1.3 In `src/lib/application/use_case/load_configuration.rs`, resolve `seed.profile` in `merge` and record the
      setting's name in `applied` (never its value, `OBS-004`). Extend the existing `load_configuration` tests with the
      resolved value and the unknown-value refusal.
- [x] 1.4 In `src/lib/infrastructure/outbound/envy/configuration.rs`, add `EnvConfiguration.seed_profile` under the
      `cfg`, mapped into `PartialSeed`; confirm `MNEMORIUM_SEED_PROFILE` lands there by the adapter's existing
      prefix/path rule.
- [x] 1.5 Pass `configuration.seed().profile()` from `src/lib/app.rs` and delete the variable read and the "everything
      is `api`" fallback. `cargo build --bin server` and `cargo build --features test-seed --bin server` both pass.
- [x] 1.6 Update every caller that relied on the implicit `api` default so it names its profile: `script/api_fuzz.sh`,
      `script/zap_scan.sh`, `.github/workflows/typescript.yml`, `README.md`'s seeded run, the seeded-run row in
      `AGENTS.md`, and `Test.md`'s seed prose. `script/performance.sh` already names `perf`.
- [x] 1.7 Add `--build-arg CARGO_FEATURES=test-seed` to `script/system.sh`'s build and name the feature in its
      `NO_BUILD` error message.

## 2. Seed tree, profile dispatch and the system fixture

- [x] 2.1 Move `test/seed/api` to `test/api/seed` and `test/seed/perf` to `test/performance/seed`, media included
      (`test/api/seed/media` holds the API bytes, `test/performance/seed` holds SQL only because its bulk bases stay
      generated at runtime). Update `sqlx::migrate!`/`include_bytes!` in `src/lib/infrastructure/seed.rs`,
      `script/generate_perf_seed.sh`, `.ls-lint.yml`, `.github/workflows/sql.yml` and the `.sqlfluffignore` names.
- [x] 2.2 Rewrite `seed.rs` as the registry of design § 2:
      `apply(pool, pepper, library_root, profile: Option<SeedProfile>)` dispatching to `seed_api`, `seed_api` +
      `seed_perf`, or `seed_system`; keep one file, and give `seed_files` the profile's `is_public`/`PUBLIC` resource
      choice through a `SeedFile` flag.
- [x] 2.3 Add the `_sqlx_seeds_system` marker and the system fixture's identity step: `coverart`, credential and user id
      `10000`, `STANDARD`, a fixed `SYSTEM_SEED_PASSWORD` beside `SEED_PASSWORD`, hashed with the live pepper exactly as
      `seed_identities` hashes, then the rows `register_user` writes (`USER` resource under `INSTANCE 0` with the Admin
      owner grant). Print the password on the boot that creates it, mirroring `show_demo_accounts`.
- [x] 2.4 Add the system file writer: each `SeedFile` is written under `media/system/`, `origin = 'SCANNED'`,
      `integrity_hash = NULL`, size and `modified_at` read after the write so the startup scan leaves it untouched, and
      a `PUBLIC` `FILE` resource per row.
- [x] 2.5 Add `test/system/seed/sql/0001_cover_art.sql` … `0006_file.sql` and the committed media listed in the module
      sections below; document each script's identifier slots in its header.
- [x] 2.6 Remove the module's `#[cfg(test)] mod tests` and add `src/lib/infrastructure/seed.rs` to the coverage
      exclusion in `devenv.nix` (hook and HTML task), `.github/workflows/rust.yml` and the `AGENTS.md` coverage row;
      reword `.sqlfluffignore`'s rationale to cite the performance dataset gate (`script/performance.sh`).
- [x] 2.7 Run the moved fixtures against the existing suites: `MNEMORIUM_SEED_PROFILE=api` keeps the API suite green and
      `perf` keeps the performance dataset gate green.

## 3. Stored-file collection — the one product surface

- [x] 3.1 Application: add `ListFilesCommand`/`Response`/`Error`, the `ListFilesUseCase` port and `list_files`, paging,
      total, exact root-relative `path` filter, per-item `Discover` through `allows_many`, and the typed-row resolution
      `get_file` performs. Unit-test the path filter, the discovery drop, paging and total, and the absent-caller
      refusal.
- [x] 3.2 REST: add `get_file_list.rs` (`operation_id = "list_files"`, tag `asset`, path `/file`) beside
      `get_file::get_file`, lift `FileResponse` to `pub(crate)`, reuse the gallery listing's `input_validation_failed`
      limit check, and register the route from the asset context module.
- [x] 3.3 Index: add the plain `file` relation at `/api/v1/asset/file` (`REQ-API-015`), and publish the filters only
      through a templated `_links.search` (`REQ-API-006`, `REQ-API-007`).
- [x] 3.4 Ingest: make `register` write the `PRIVATE`, Root-Admin-owned `FILE` resource that `complete_upload` writes,
      tolerating `AlreadyExist`; unit-test that a rescan and a restart stay idempotent and that a found file resolves by
      its identifier.
- [x] 3.5 Regenerate the served specification and run Schemathesis over `list_files`; confirm `test/system/api.py`'s
      index cases need no edit because the relation is protected.

## 4. Suite plumbing and the generated client

- [x] 4.1 `test/util/harness.py`: add `LOG_FILE`, `Sut.events(name)`, `Sut.written_events(name)`,
      `Sut.wait_for_events(name)`, `Sut.wait_until(condition, description)`, `MSG_MEDIA_ROOT_ABSENT` (from media
      ingest's `ROOT_ABSENT_REFUSAL`), `STAGING_DIRECTORY` and `UploadSession.from_body(body)`.
- [x] 4.2 `test/system/conftest.py`: add the shared environment (with `MNEMORIUM_SEED_PROFILE=system` for the worker's
      instance), `SEED_MEDIA_DIRECTORY = HERE / "seed" / "media"`, repoint `FIXTURE_DIRECTORY` to `test/api/seed/media`,
      and add the request helpers `file_at`, `ingest_report`, `wait_for_ingest`, `begin`, `session_state`,
      `staged_path`, `store_chunk` and `completed_file`. A helper addresses a route through the generated client, or
      through a representation's link when that link is what the case proves.
- [x] 4.3 Move `test/system/hal_test.py` to `test/api/hal_test.py`; set `testpaths = ["test/system", "test/api"]` in
      `pyproject.toml` and say in its comment that a bare `pytest` runs both; keep `script/system.sh` on
      `SUITE="test/system"`.
- [x] 4.4 `.github/workflows/api.yml`: install `requirements.txt` (with the pip cache keyed on it) and run
      `pytest test/api -p no:cacheprovider` before the fuzz; add `test/util/**`, `pyproject.toml` and `requirements.txt`
      to its path filters; keep `system.yml` triggering on `test/util/**`.
- [x] 4.5 Update the seed paths in `.sqlfluffignore`, `.ls-lint.yml`, `.github/workflows/sql.yml`, the `devenv` coverage
      exclusion and the `pyproject.toml` comment that names the suite roots.
- [x] 4.6 Run the system suite unmodified against the `api` profile first, then switch the shared fixture to `system`
      and confirm the modules that predate this change are unaffected by the seeded rows.
- [x] 4.7 Add `openapi-python-client==0.30.0`, `attrs==26.1.0` and `httpx2==2.16.2` to `requirements.txt`, and
      `test/openapi_client.yaml` with the one allowed alias: the dynamic content media type (`*/*` →
      `application/octet-stream`). Every other generator option would hide a missing declaration.
- [x] 4.8 Add `script/openapi_client.sh` (`generate`/`clean`): a throwaway, unseeded container of the image under test,
      its `/doc/openapi.json` written to `test/generated/`, and `openapi-python-client generate --meta none` into
      `test/generated/mnemorium_client`. The script fails on any generator warning, so a dropped operation or response
      cannot reach the suite.
- [x] 4.9 Wire the generator: `devenv.nix`'s `system:client`/`system:clean` tasks, `script/system.sh`'s
      `api-docs test-seed` build and pre-pytest generation, `.gitignore`, `.ls-lint.yml`, `pyproject.toml`'s
      `pythonpath`/`extraPaths`/ruff exclusion, and `system.yml`'s path filters.
- [x] 4.10 Audit the served document against what the suite drives: every path, query and header parameter, request
      body, response body and asserted status is declared, and anything missing is added in the handler's utoipa macros
      rather than worked around in a case. Acceptance: no generator warning, one generated function per declared
      operation (89 today), and the four `*/*` content responses keep the one alias.
- [x] 4.11 `test/util/harness.py`: add the cached `Sut.client(token)` and `payload(response)`, and state the three-tier
      request rule (generated client, published href, `Sut.request`) in the module docstring; `Sut.request` keeps
      `requests` for the escape hatch.
- [x] 4.12 `test/system/conftest.py`: stop the run with the generation hint when `mnemorium_client` is missing, and move
      the shared request helpers (`file_at`, `upload`, `create_gallery`, `add_to_gallery`, `begin`, `session_state`,
      `store_chunk`, `completed_file`) onto the generated client, keeping each signature. A link a case proves is still
      read with `link_of`; the index stays readable through `Sut.index` where a case proves a relation.
- [x] 4.13 Declare the four content responses as binary in their handlers (a `value_type = String, format = Binary`
      schema) so the generated body type is `bytes` rather than an integer list; rerun Schemathesis over the four
      content operations, and read `Response.content` in a case either way.

## 5. Cover art

- [x] 5.1 Seed `0001_cover_art.sql` with the rows this section's cases need — a covered group, a bare group, a covered
      album, a bare album, an album track case, a playlist-track case, an episode case, the playlist
      create/replace/clear rows, a `LIBRARY` image with no referrer, and a `USER`-scoped gallery item owned by `10000` —
      with the `resource` rows the create use cases write. Library resources are `PUBLIC`; the three playlist rows are
      `PRIVATE`; genres link to `UNKNOWN`; no image is attached twice.
- [x] 5.2 Commit ten distinct 64×64 PNGs, two copies of `track.flac`, one `episode.mp4` and the gallery image under
      `test/system/seed/media/`, and register each with the system file writer.
- [x] 5.3 Rework `test/system/cover_art.py`: one docstring line, imports `allure` and `harness` only, no helper, no
      fixture, no container; every case logs in with `server.login(ACCOUNT, PASSWORD)` and follows links. Rebuild the
      thirteen cases: `test_a_resource_with_its_own_artwork_reports_it` (001-A, 004-A),
      `test_a_resource_without_artwork_reports_null` (001-B, 004-B), `test_a_covered_album_links_its_cover` (004-A),
      `test_a_bare_album_links_no_cover` (004-B), `test_an_album_track_reports_no_artwork` (006-A, 006-C),
      `test_a_playlist_track_reports_no_artwork` (006-D), `test_an_episode_reports_no_artwork` (006-B, 006-C),
      `test_a_new_playlist_stores_its_cover` (003-B, 005-A, 007-A), `test_a_playlist_replaces_its_cover` (005-B, 007-B),
      `test_a_playlist_clears_its_cover_with_null` (005-C, 007-B), `test_a_playlist_refuses_library_artwork` (003-C,
      007-D), `test_a_playlist_refuses_an_unknown_image` (002-B, 007-D) and `test_gallery_media_is_refused_as_artwork`
      (003-D). A cover link is proved by following it; `cover_href` and its literal disappear.
- [x] 5.4 Fix the two body defects while rewriting: the episode route's last segment is the index, not the identifier,
      and the album-track route is `/music/album/{id}/medium/1/track/0`.

## 6. Music

- [x] 6.1 Seed `0002_music.sql`: per case slot a group, an album with or without a medium, a medium at index 1,
      recordings with their audio, and `LIBRARY` cover images, with the `resource` rows the use cases write. Commit six
      distinct 64×64 PNGs and the recording audio under `test/system/seed/media/`.
- [x] 6.2 Rework `test/system/music.py` to the shared instance with no helper: imports `allure`, `pytest`, `harness` and
      `conftest.provision` only; catalogue preconditions from the seed; playlists and users created per case; collection
      reads filtered on a value the case owns (an album name shared by its two albums, a group name, the provisioned
      owner's id); no upload and no `sqlite3`.
- [x] 6.3 Rebuild the album cases (15): `test_the_album_collection_carries_partial_albums` (001-A), `…_filters_on_name`
      (001-B), `test_an_album_detail_read_carries_its_metadata` (002-A), `…_omits_unreported_values` (002-B),
      `test_an_unknown_or_malformed_album_is_refused` (002-C, 002-D), `test_a_new_album_is_created` (008-A),
      `test_an_unnamed_genre_is_stored_as_unknown` (008-C), `test_an_existing_album_is_returned_unchanged` (008-B), and
      the three `008-D` refusals (`test_an_album_with_an_unusable_name_is_refused`,
      `…_with_an_unknown_group_is_refused`, `…_with_an_empty_genre_is_refused`), each asserting creation and update
      refuse and store nothing, plus `test_an_albums_release_date_and_genres_are_updated` (029-A),
      `…_optional_value_is_cleared` (029-B), `…_group_change_that_collides_is_refused` (029-C),
      `test_an_absent_album_field_keeps_its_value` (029-D).
- [x] 6.4 Rebuild the medium cases (6): `test_a_medium_is_created` (003-B, 009-A), `test_a_medium_is_retyped` (009-B),
      `test_a_medium_write_is_idempotent` (009-C), `test_a_medium_read_carries_its_type_and_index` (004-A),
      `test_an_unknown_or_malformed_medium_is_refused` (004-B, 004-C — add the missing malformed-index assertion) and
      `test_an_invalid_medium_type_is_refused` (009-D).
- [x] 6.5 Rebuild the recording and track cases (14): `test_a_recording_is_placed` (010-A), `test_a_recording_is_moved`
      (010-B), `test_a_placement_of_an_unknown_recording_is_refused` (010-C),
      `test_a_placement_into_an_unknown_destination_is_refused` (010-D — add the missing assertion),
      `test_a_placement_into_an_occupied_destination_is_refused` (010-E), `test_a_recording_rename_reaches_the_playlist`
      (011-A), `test_a_recording_field_is_cleared` (011-B), `test_a_duplicate_isrc_is_refused` (011-C),
      `test_a_malformed_isrc_is_refused` (011-C), `test_an_unknown_track_is_refused` (011-D),
      `test_editing_a_default_album_track_is_refused` (011-E), `test_a_track_read_carries_its_recording` (007-A, 006-A),
      `test_a_mediums_tracks_are_listed` (005-A) and `test_an_unknown_or_unsequenced_track_position_is_refused` (005-B,
      006-B, 006-C).
- [x] 6.6 Rebuild the group cases (18): `test_a_group_is_created` (014-A), `test_a_group_stores_its_library_artwork`
      (014-B), `test_a_duplicate_group_name_is_refused` (014-C), `test_a_group_with_an_invalid_name_is_refused` (014-D),
      `test_a_group_with_unusable_artwork_is_refused` (014-D), `test_the_group_collection_lists_and_filters` (012-A,
      012-B), `test_a_group_detail_read_carries_its_representation` (013-A), `test_an_unknown_group_is_refused` (013-B),
      `test_a_group_is_renamed_keeping_its_artwork` (015-A), `test_group_artwork_is_replaced` (015-B),
      `test_group_artwork_is_cleared` (015-C), `test_a_group_rename_that_collides_is_refused` (015-D),
      `test_a_group_rename_with_an_invalid_name_is_refused` (015-E), `test_the_album_representation_names_its_group`
      (017-A), `test_an_album_moves_group` (017-B), `test_the_default_album_and_group_are_readable` (016-A),
      `test_a_default_group_write_is_refused` (016-B) and `test_a_default_album_write_is_refused` (016-C).
- [x] 6.7 Rebuild the playlist cases (27): `test_a_private_playlist_is_created` (019-A),
      `test_a_public_playlist_is_created` (019-B), `test_a_playlist_with_a_malformed_name_is_refused` (019-C),
      `test_a_playlist_name_is_trimmed` (020-A), `test_a_duplicate_playlist_name_is_refused` (020-B),
      `test_two_owners_may_share_a_playlist_name` (020-C), `test_the_playlist_collection_lists_owned_and_public`
      (018-A), `test_another_owners_private_playlist_is_filtered_out` (018-C), `test_the_playlist_collection_filters`
      (018-B), `test_a_playlist_detail_read_carries_its_tracks` (021-A, 021-B, 023-A),
      `test_an_unknown_or_malformed_playlist_is_refused` (021-C), `test_playlist_metadata_is_replaced` (024-A, with the
      last-modified advance), `test_an_empty_playlist_update_changes_nothing` (024-B),
      `test_a_playlist_rename_that_collides_is_refused` (024-C),
      `test_a_playlist_rename_with_an_invalid_name_is_refused` (024-C), `test_a_recording_is_appended` (025-A),
      `test_an_unusable_target_recording_is_refused` (025-C), `test_a_playlist_track_read_carries_its_recording` (022-A,
      023-B), `test_an_unsequenced_playlist_position_is_refused` (022-B), `test_a_track_is_removed` (026-A),
      `test_a_removed_tracks_recording_survives` (026-B), `test_removing_an_unsequenced_position_is_refused` (026-C),
      `test_the_tracks_of_a_playlist_are_reordered` (027-A), `test_an_incomplete_permutation_is_refused` (027-B),
      `test_a_playlist_is_deleted` (028-A), `test_a_deleted_playlists_recordings_survive` (028-B) and
      `test_deleting_an_unknown_playlist_is_refused` (028-C). Deleting a playlist also removes its `MUSIC_PLAYLIST`
      resource in the same unit of work, so a later creation cannot reuse the freed identifier and collide with a stale
      resource; `delete_playlist`'s unit test covers it (`fix(library)`).
- [x] 6.8 Leave the three cases section 18 now owns: the catalogue curation split (labels 022-A, 022-B, `023-B` for the
      film and `023-G` for the group and album creations), the playlist-creation denial (023-F) and the unreadable
      recording (040-A).

## 7. Movies

- [x] 7.1 Seed `0003_movies.sql`: per case slot a film with full metadata and a cover (003-A), a film with no date or
      country (003-B), two films for the ordered listing (001-A/001-D/002-A), three sharing one name for the filtered
      page (001-B), a film with its video for the repeat (004-B), a film to patch (005-A/005-C) and one to clear
      (005-B), a video attached to a seeded show's episode (004-C), and one free video per creation case that submits a
      usable video (004-A, 004-D and the refusals of a name, genre, date or country) — with `MOVIE`, `SERIES` and `FILE`
      resources. Commit 17 videos, four 64×64 PNGs and their files.
- [x] 7.2 Rework `test/system/movies.py` to the shared instance with no helper: imports `allure` and `harness` only,
      writes its summary assertion inline against a seeded video, and uses only `root_token`.
- [x] 7.3 Rebuild the 15 cases: `test_the_collection_lists_partial_films_in_identifier_order` (001-A, 001-D, 002-A),
      `test_the_name_filter_and_page_are_applied` (001-B), `test_the_detail_read_carries_the_film_metadata` (003-A),
      `test_a_film_without_a_date_or_country_reports_null` (003-B), `test_a_new_film_is_created_from_a_free_video`
      (004-A), `test_a_repeated_creation_returns_the_existing_film` (004-B),
      `test_a_video_attached_to_an_episode_is_refused` (004-C), `test_an_unknown_genre_is_stored_and_linked` (004-D),
      `test_a_film_with_an_unknown_video_is_refused` (004-E), `test_a_film_with_an_empty_name_is_refused` (004-E),
      `test_a_film_with_an_empty_genre_is_refused` (004-E), `test_a_film_with_a_malformed_date_is_refused` (004-E),
      `test_a_film_with_a_malformed_country_is_refused` (004-E), `test_the_metadata_is_updated_partially` (005-A, 005-C)
      and `test_null_clears_an_optional_value` (005-B). Each `004-E` case proves nothing was stored by filtering on its
      own name.
- [x] 7.4 Confirm the cases that leave are owned elsewhere: `001-C` → section 18 (`039-A`), the cover refusal → section
      5 (`003-D`), the standard-user creation refusal → section 18 (`023-B`), and the declared rejections and
      concealment duplicates → the API fuzz suite and section 18's `001-C` concealment proof.

## 8. Audio

- [x] 8.1 Commit the six WAVs under `test/system/seed/media/` — one second 16-bit, short 16-bit for creation, dedupe and
      the read target, short 64-bit, and zero-sample — and seed `0004_audio.sql` with one `audio` row each, mirroring
      what the probe would report: bit depth on the 16-bit rows, none on the 64-bit row, no duration on the empty row,
      `block_size` NULL everywhere.
- [x] 8.2 Register the six files with the system file writer so the scan leaves them untouched, and note that audio is
      authorised through its file and is not a resource of its own.
- [x] 8.3 Rebuild `test/system/audio.py` to six shared cases, no helper, imports `allure`, `harness` and
      `conftest.upload`: `test_an_audio_file_creates_one_audio` (001-A), `test_a_file_backs_one_audio` (001-B),
      `test_every_reported_measurement_is_reported` (002-A), `test_a_missing_bit_depth_and_block_size_stay_absent`
      (002-B), `test_a_missing_duration_stays_absent` (002-C) and `test_an_audio_carries_its_representation` (003-A).
      Inline `typed_audio` as `server.follow(file, "audio", token=…)` and delete the three `process_state` markers.

## 9. Download

- [x] 9.1 Seed `0005_download.sql`: read-only typed image, audio and video rows, a typed image named `holiday.png`, a
      typed image named `odd "name".png`, and a bare `download-bare.png` whose bytes are not a decodable PNG; one file
      per mutating case (an image, an audio, a video and the merged refusal's image). All are `PUBLIC` library files.
- [x] 9.2 Rework `test/system/download.py` to 24 cases with no helper, imports `allure`, `harness` and
      `conftest.provision`; inline `typed`, `content` and `filed_content`, and turn `share_viewer` into the permission
      `PUT`. Keep `008-C` (`restart`) and `008-D` (`global_filesystem`) own; every other case is shared, and the four
      mutating cases use a seeded file and a per-case user.
- [x] 9.3 Rebuild the 24 cases: `test_a_bearer_request_reads_content` (001-A, 003-A); `test_image_content_decides_view`
      (002-A), `test_audio_content_decides_stream` (002-B), `test_video_content_decides_stream` (002-B);
      `test_audio_content_carries_the_stored_type` (003-B), `test_video_content_carries_the_stored_type` (003-B),
      `test_file_content_carries_the_stored_type` (003-C); `test_a_head_request_answers_without_content` (004-B);
      `test_a_closed_range` (005-A), `test_an_open_ended_range` (005-B), `test_a_suffix_range` (005-C),
      `test_an_unsatisfiable_range` (005-D), `test_more_than_one_range` (005-E);
      `test_a_content_response_carries_its_validators` (006-A), `test_unchanged_content_answers_not_modified` (006-B),
      `test_a_range_is_conditional_on_the_content` (006-C), `test_a_stale_validator_answers_the_whole_content` (006-D);
      `test_a_typed_file_keeps_its_original_name` (007-A), `test_a_bare_file_keeps_its_stored_name` (007-B),
      `test_a_display_route_is_not_an_attachment` (007-C), `test_an_awkward_file_name_is_encoded` (007-D);
      `test_a_visible_asset_whose_action_refuses_answers_forbidden` (002-C, 008-B, 009-A),
      `test_an_absent_library_root_answers_unavailable` (008-C, own `restart`) and
      `test_a_registered_file_without_bytes_answers_server_error` (008-D, own `global_filesystem`). The merges are
      `001-A`+`003-A` and `002-C`+`008-B`+`009-A`; the splits are `002-B` and `003-B` into their audio and video halves.
- [x] 9.4 Relocate the refused-decision property: `test_an_owner_replaces_the_set` in section 18 gains
      `SCN-DOWNLOAD-002-D`'s "the next request uses it" assertion.

## 10. File

- [x] 10.1 Seed `0006_file.sql` with these four rows — `file-linked.png` (image `15201`, 64×64 `SQUARE`),
      `file-audio.flac` (audio `15202`), `file-video.mp4` (video `15203`) and `file-bare.png` (no typed row) — owned by
      the Root Admin, `PUBLIC`, with their bytes committed and their typed rows mirroring the probe's output.
- [x] 10.2 Rework `test/system/file.py` to nine cases with no helper, imports `allure`, `pytest`, `harness` and
      `conftest`'s `upload`, `start_instance` and `SEED_MEDIA_DIRECTORY`; read every record through the stored-file
      collection at its exact path, and never through SQLite.
- [x] 10.3 Keep `test_a_found_file_records_its_content` (001-B/001-C) as the module's one own case
      (`global_filesystem`): copy `file-linked.png` under the root as `found.png` at a chosen instant, boot, poll the
      collection by path inline, and assert `SCANNED`, the root-relative path, the media type, the size, the chosen
      instant as `YYYY-MM-DDTHH:MM:SS` and today's registration date.
- [x] 10.4 Rebuild the remaining cases: `001-A`, `002-A`, `003-A`/`004-A`/`004-B` (starting from the collection), the
      split `003-B`/`003-C` (image, audio, video), `003-D` on the seeded bare file, and `004-C` with `file-linked` as a
      value that only starts like the stored path.

## 11. Image

- [x] 11.1 Rework `test/system/image.py` to seven cases on the shared instance: no request helper, `upload_image`,
      `typed_image` and `unknown_image` inlined through `server.follow(file, "image", token=root_token)`, and
      `_png_chunk`/`png` kept as the payload builder. Imports become `allure`, `struct`, `zlib`, `datetime`,
      `itertools`, `conftest`'s `SEED_MEDIA_DIRECTORY` and `upload`, and `harness`'s `Sut`.
- [x] 11.2 Rebuild the seven cases: `test_an_image_file_creates_one_image` (001-A), `test_a_file_backs_one_image`
      (001-B), `test_audio_and_video_content_create_no_image` (001-C, uploading the seed's `track.flac` and `clip.mp4`),
      `test_a_wider_image_is_landscape` (002-A), `test_a_taller_image_is_portrait` (002-B),
      `test_a_square_image_is_square` (002-C) and `test_the_name_and_creation_date_are_reported` (002-D). `001-A` keeps
      `image["file_id"] == file["id"]` and the file's `image` link; every case drops the trailing "the next identifier
      is a `404`" assertion, and the three `process_state` markers go.
- [x] 11.3 Confirm the module seeds nothing and claims no identifier block; the representation, unknown-image and alias
      cases are owned by the API suite and section 18 (a note in the module docstring records it).

## 12. User

- [x] 12.1 Rework `test/system/user.py` to 31 cases, one own (`010-A`, `exact_collection` on an unseeded container) and
      the rest shared; name every account from a module-level counter, and read each back through that name; media bytes
      come from `SEED_MEDIA_DIRECTORY`.
- [x] 12.2 Inline `file_profile` as its file read with its `200` assertion; read the owner from the Root Admin's
      `_links.self` in `current_user`; follow the file's typed link in `018-B`/`018-C` instead of `linked_id`.
- [x] 12.3 Fix `015-A`: read the permission summary through the index's `permission` relation with `?user_id=`, the
      route `get_permission` builds; drop `019-A`'s digest read and leave its clearing to the `delete_user` unit tests
      (`delete_user_clears_a_colliding_digest_on_transfer` beside the repository's
      `clear_integrity_hash_removes_the_digest`). Deleting an account also removes its `USER` resource in the same unit
      of work, so a later account cannot reuse the freed identifier and collide with a stale resource; the unit test is
      `delete_user_removes_its_resource` (`fix(identity)`).
- [x] 12.4 Rebuild the 31 cases by group: **profile (2)** `test_a_created_account_reads_back_its_profile` (001-A, 005-A,
      009-A) and `test_the_caller_reads_its_own_account` (011-A); **role (2)** `test_a_declared_role_is_stored` (002-A)
      and `test_an_undeclared_role_is_rejected` (002-B); **username (3)**
      `test_a_username_with_whitespace_or_uppercase_is_rejected` (003-A, 003-B),
      `test_username_boundary_lengths_are_accepted` (003-C) and `test_a_taken_username_or_email_is_refused` (003-F,
      004-F); **email (3)** `test_an_omitted_or_null_email_is_stored_as_none` (004-A, 004-B),
      `test_an_invalid_mailbox_is_rejected` (004-C, 004-D, 004-E) and `test_an_email_case_variant_is_a_conflict`
      (004-G); **creation (3)** `test_a_created_account_has_one_credential` (007-A),
      `test_a_rejected_creation_leaves_nothing` (007-B) and `test_a_creation_returns_the_profile_and_location` (008-A,
      008-B); **collection (4)** `test_no_filter_returns_every_account` (010-A, own), `test_filters_narrow_the_result`
      (010-B), `test_an_empty_result_carries_no_items` (010-C) and `test_the_email_filter_ignores_case` (010-E);
      **update (6)** `test_an_update_changes_each_submitted_field` (012-A, 012-B, 012-C),
      `test_an_update_leaves_omitted_fields_and_an_empty_body_unchanged` (012-D, 012-E),
      `test_an_update_onto_a_taken_username_or_email_conflicts` (013-A, 013-B),
      `test_an_update_keeping_the_own_value_is_not_a_collision` (013-C), `test_a_case_only_email_change_conflicts`
      (013-D) and `test_an_update_with_an_invalid_value_is_rejected` (014-A, 014-B and the untested 014-C, added);
      **deletion (8)** `test_a_deletion_removes_the_identity` (015-A), `test_a_deletion_removes_owned_data` (016-A,
      016-B, 016-C), `test_private_gallery_media_is_removed_with_its_bytes` (017-A),
      `test_the_accounts_own_public_gallery_is_not_a_shared_reference` (017-C),
      `test_an_image_in_another_gallery_survives_under_account_zero` (018-A), `test_a_video_backing_a_film_survives`
      (018-B), `test_catalogue_artwork_survives` (018-C) and
      `test_a_transferred_file_with_a_colliding_digest_stays_readable` (019-A). A refusal proves it stored nothing by
      filtering on the refused value and on the value it would normalize to.

## 13. Video

- [x] 13.1 Rework `test/system/video.py` to six cases on the shared instance with no helper: `upload_video`,
      `typed_video`, `unknown_video`, `create_film`, `create_episode` and `assert_summary` disappear, reads inline as
      `server.follow(file, "video", token=root_token)`, and `video_bytes` stays as the payload builder. `001-C` uploads
      `library-cover.png` and `track.flac` from `SEED_MEDIA_DIRECTORY`.
- [x] 13.2 Rebuild the six cases: `test_a_video_file_creates_one_video` (001-A), `test_a_file_backs_one_video` (001-B),
      `test_image_and_audio_content_create_no_video` (001-C), `test_the_measurements_are_reported` (002-A),
      `test_a_progressive_stream_reports_a_progressive_scan` (002-B) and `test_a_video_carries_no_name` (002-C).
      `001-A`/`001-B` drop the `id + 1` assertion, `002-B` keeps only the `PROGRESSIVE` equality, and the three
      `process_state` markers go.
- [x] 13.3 Record in the module docstring that the representation, unknown-video and alias proofs are the API suite's,
      and that the embedded-summary proof (`REQ-VIDEO-005`) is the movies module's and `series.py`'s.

## 14. Logging

- [x] 14.1 Rewrite `test/system/logging_events.py` to 35 cases, thirteen shared and 22 own; no helper, no
      `_control_server`, no module-level `pytestmark`; each own case states one reason (`log_sink`, `restart`,
      `global_filesystem`, `first_boot`). Use `Sut.events`/`written_events`/`wait_for_events` and an owned value
      (`actor`, `claimed_identity`, `upload`, a query sentinel) in every shared assertion.
- [x] 14.2 Rebuild the thirteen shared cases, each filtering on the value it owns:
      `test_the_standard_output_sink_holds_the_event` (001-B, 012-A), `test_standard_output_is_uncoloured_by_default`
      (001-D), `test_a_rejected_login_is_recorded_as_a_warning` (006-C, 009-A, 012-B),
      `test_a_control_character_is_escaped` (011-A), `test_the_identity_is_bounded` (011-B),
      `test_a_denial_is_recorded_once` (006-A, 013-A), `test_filtering_a_listing_records_no_denial` (013-B),
      `test_an_out_of_order_action_is_suspicious` (013-C), `test_a_query_string_is_withheld` (007-A),
      `test_a_personal_data_read_is_recorded` (008-D, 018-A, 018-B), `test_a_token_regeneration_names_the_actor`
      (012-E), `test_user_administration_names_actor_and_target` (015-D) and `test_an_upload_lifecycle_is_recorded`
      (017-A).
- [x] 14.3 Rebuild the 22 own cases with their one reason: `test_the_file_sink_holds_the_event` (001-A),
      `test_the_file_sink_is_never_coloured` (001-C), the three ladder cases (003-D),
      `test_a_fault_only_verbosity_cannot_silence_a_success` (004-A), `test_the_lowest_declared_severity_survives`
      (004-B), `test_rotation_replaces_the_active_file` (002-A, `restart`), `test_pruning_bounds_the_retained_files`
      (002-B), `test_a_zero_maximum_keeps_every_file` (002-C), `test_startup_records_a_lifecycle_event` (015-A),
      `test_a_file_name_is_withheld_from_a_storage_fault` (007-B, 009-B), `test_login_secrets_stay_out_of_every_sink`
      (008-A), `test_the_configured_instance_secrets_stay_out_of_every_sink` (008-B),
      `test_the_generated_instance_secrets_stay_out_of_every_sink` (008-B, `first_boot`),
      `test_token_rejections_name_their_reason` (012-C), `test_a_throttled_request_names_its_route` (012-D),
      `test_a_credential_rotation_names_the_actor` (012-E), `test_identity_creation_is_reported` (015-B, 015-C,
      `first_boot`), `test_an_ingest_reports_counts_only` (016-A), `test_a_degraded_observer_is_reported` (016-B) and
      `test_a_skipped_provider_reports_its_classification` (016-C, 016-D).
- [x] 14.4 Build the severity ladder: `003-D` becomes the three `error`/`warn`/`info` cases with one environment
      override and one authenticated request each, and `007-A` is the `debug` rung, comparing the traced path with
      `urlsplit(server.href("gallery")).path`.
- [x] 14.5 Split `012-E`: rotation stays own (only the root rotates on a fresh instance), regeneration runs shared
      against a provisioned user; keep `slow` only on `002-A`, `016-A` and `016-B`.
- [x] 14.6 Delete `test_a_reaping_run_reports_counts_only` and carry its labels (`REQ-LOGGING-017`, `SCN-LOGGING-017-B`)
      onto section 17's reaper case.

## 15. Authentication

- [x] 15.1 Rework `test/system/authentication.py` to 23 cases: fourteen own (`restart`, `log_sink`, `first_boot`,
      `process_state`) and nine shared; drop the module-level marker and name each own case's reason.
- [x] 15.2 Rebuild the cases by group. **Bootstrap (3)** `test_first_start_creates_the_root_admin` (001-A, shared),
      `test_restart_does_not_recreate_or_reveal_the_root_admin` (001-B, own `restart`),
      `test_generated_password_follows_the_declared_shape` (002-A, shared); **reveal (3)**
      `test_the_password_is_printed_once_and_never_logged` (003-A, own `log_sink`),
      `test_a_first_start_without_the_reveal_is_refused` (003-B, own `first_boot`),
      `test_disabling_the_reveal_after_initialization_still_starts` (003-C, own `restart`); **login (2)**
      `test_a_login_issues_a_usable_bearer_token` (004-A, 004-B, 004-C, merged) and
      `test_bad_credentials_are_rejected_uniformly` (005-A); **limiter (5)** `test_a_login_burst_is_throttled` (008-A),
      `test_the_login_quota_refills` (008-B), `test_a_client_address_header_cannot_mint_a_new_quota` (008-C),
      `test_one_account_is_throttled_across_addresses` (008-D) and `test_an_unknown_username_is_throttled_identically`
      (008-E); **token (3)** `test_invalid_tokens_are_rejected` (010-B, 010-C, 010-D),
      `test_rotation_and_regeneration_leave_issued_tokens_valid` (012-A, 013-C, own `process_state`) and
      `test_regenerating_a_token_keeps_the_caller` (013-A, 013-B, shared); **rotation (5)**
      `test_the_root_admin_rotates_its_own_credential` (014-A, own),
      `test_the_root_admin_rotates_another_users_credential` (014-B, own),
      `test_a_non_root_caller_cannot_rotate_a_credential` (014-D, own),
      `test_a_rotation_that_fails_the_policy_changes_nothing` (015-A, shared) and
      `test_a_compliant_password_is_accepted_at_rotation` (018-A, own); **creation policy (2)**
      `test_a_compliant_password_is_accepted_at_creation` (018-A, shared) and
      `test_a_short_or_symbol_free_password_is_rejected_at_creation` (018-B, 018-C, shared).
- [x] 15.3 Merge `004-A/B/C`'s reads into the login case, split `018` by entry point, delete the unlabelled login smoke
      and the `_links.self` literal comparison, and keep the shared limiter raised so the read cases do not throttle.
- [x] 15.4 Move `007-A` to `test/system/configuration.py` beside the TTL probes, with `base64`, `json` and `time` and
      the module's `sut.start`; label it `REQ-CONFIGURATION-001`/`SCN-CONFIGURATION-001-E` beside
      `SCN-AUTHENTICATION-007-A`.
- [x] 15.5 Keep the credential-identifier inference documented in the module: `1` for the root, `user_id + 1` on a fresh
      instance, which is why the rotation cases keep their containers.

## 16. Media ingest

- [x] 16.1 Rework `test/system/media_ingest.py` to 29 cases, seven shared and 22 own (`global_filesystem`), with no
      helper and no datastore read: records through `file_at`, typed rows through the item's own link, the owner through
      `link_of(item, "owner")` compared with `current_user`'s `self`, and the report through `Sut.events("ingest")`
      parsed by `ingest_report` and polled with `wait_for_ingest`.
- [x] 16.2 Inline `place`/`preplace` as `sut.write` plus `os.utime`, keep `mp3_frame_bytes`, and take the payload bytes
      from `SEED_MEDIA_DIRECTORY`.
- [x] 16.3 Rebuild the seven shared cases, each filtering on the path it owns:
      `test_a_promoted_upload_is_addressed_under_the_root` (001-A), `test_removing_a_gallery_leaves_the_content_alone`
      (012-A, 012-B), `test_a_file_added_while_the_server_runs_is_registered` (013-A),
      `test_a_file_still_being_written_is_registered_once_complete` (013-B), `test_a_renamed_file_keeps_its_record`
      (014-A), `test_a_file_removed_while_the_server_runs_is_reconciled` (015-A) and
      `test_a_reappearing_file_is_a_new_record` (015-C). Drop `012-A`'s whole-directory listing, prove `014-A`'s "no
      second record" by the surviving identifier and its media row by the `image` link, and poll `file_at` instead of
      the report in the polled cases.
- [x] 16.4 Rebuild the 22 own cases, all `global_filesystem`: `test_a_found_file_is_addressed_under_the_same_root`
      (001-B), `test_a_present_root_is_examined_at_startup` (002-A),
      `test_a_root_that_appears_within_the_window_is_examined` (002-B), `test_an_absent_required_root_refuses_startup`
      (002-C, gaining a third boot that restores the root unchanged and observes `untouched == 1`, `registered == 0`
      over HTTP), `test_an_absent_optional_root_starts_and_reconciles_empty` (002-D),
      `test_a_request_during_the_scan_is_answered` (003-A), `test_an_absent_optional_root_does_not_delay_readiness`
      (003-B), `test_the_content_decides_the_media_type` (004-A), `test_content_without_a_media_type_is_skipped`
      (004-B), `test_a_file_that_is_not_media_is_skipped` (005-A), `test_an_unsupported_media_type_is_skipped` (005-B),
      `test_unsuitable_files_do_not_abort_the_scan` (005-C), `test_an_image_is_registered_with_its_media_row` (006-A),
      `test_a_video_is_registered_with_its_media_row` (006-B),
      `test_an_audio_file_is_registered_with_absent_measurements` (006-C),
      `test_a_registered_file_carries_its_size_and_instant` (007-A), `test_a_found_file_belongs_to_the_root_admin`
      (008-A), `test_an_unchanged_file_is_reported_untouched` (009-A), `test_a_changed_file_is_reported_and_left_alone`
      (009-B), `test_a_vanished_file_is_reconciled_as_removed` (009-C),
      `test_the_ingest_leaves_the_content_byte_identical` (011-A) and
      `test_an_unreadable_file_does_not_damage_the_ingest` (011-B). Replace every `"/api/v1"` literal with `API_PREFIX`,
      assert `"block_size" not in audio` in `006-C`, compare the whole item before and after in `009-A`/`009-B`, and
      assert the instant as `YYYY-MM-DDTHH:MM:SS` in `007-A`.
- [x] 16.5 Add `010-A` to the docstring's "cannot observe" list with its use-case proof
      (`scan_completes_a_registered_audio_file_without_its_row`), and note `015-A`'s cascade as the same unobservable
      behaviour the `016` entry describes.

## 17. Upload

- [x] 17.1 Rewrite `test/system/upload.py` to 36 cases, 25 shared and eleven own, with no helper: the five shared
      helpers come from `conftest`, `STAGING_DIRECTORY` and the session constructor from `harness`, and the remaining
      helpers inline. Fixtures come from `SEED_MEDIA_DIRECTORY`; `WIDE_WAV` and the `audio` import go.
- [x] 17.2 Rebuild the invalid-declaration case as one own `global_filesystem` case looping the five declarations on one
      `max_file_size_bytes: 2048` container, asserting that nothing was staged and nothing registered, with the
      `Location` assertion dropped.
- [x] 17.3 Rebuild the 25 shared cases: `test_a_valid_request_creates_a_session` (001-A),
      `test_a_valid_chunk_is_stored_at_its_offset` (004-A), `test_a_resent_chunk_changes_nothing` (004-B),
      `test_a_digest_mismatch_stores_no_bytes` (004-C), `test_a_range_mismatch_stores_no_bytes` (004-D),
      `test_a_malformed_chunk_header_stores_no_bytes` (004-E), `test_a_missing_header_stores_no_bytes` (004-F),
      `test_a_chunk_for_a_finished_session_is_refused` (005-B), `test_a_known_session_is_read` (006-A),
      `test_a_finished_session_reports_a_complete_bitmap` (006-C), `test_no_collection_of_sessions_exists` (006-D),
      `test_a_matching_hash_completes` (007-A), `test_an_image_is_completed` (009-A), `test_a_video_is_completed`
      (009-B), `test_an_audio_file_is_completed` (009-C), `test_unreported_audio_values_stay_absent` (009-D),
      `test_a_declared_audio_type_over_a_video_track_is_stored_as_declared` (010-A),
      `test_a_declared_image_type_is_stored_unchanged` (010-C), `test_a_caller_reuses_its_own_content` (011-A),
      `test_another_caller_receives_its_own_copy` (011-B), `test_a_pending_session_is_cancelled` (013-A),
      `test_a_finished_session_cannot_be_cancelled` (013-B), `test_completing_a_finished_session_returns_the_same_file`
      (014-A), `test_a_finished_identifier_is_not_reused` (014-B) and
      `test_a_completed_file_is_attributed_to_its_uploader` (017-A, 017-B, 017-C). `006-A` keeps the bitmap after one
      chunk, the null `file_id` until completion, the expiry instant and the three links; `011-A`/`011-B` drop their
      directory listing and count in favour of identifier equality and the two distinct paths; `007-B` drops its
      `_links` assertion in favour of `file_id is None`.
- [x] 17.4 Keep the eleven own cases: `test_an_invalid_declaration_creates_nothing` (001-B, 001-C, 001-D, 001-E, 001-F),
      `test_a_failed_begin_leaves_nothing` (002-A), `test_a_leftover_staging_file_is_reclaimed` (002-B),
      `test_a_new_chunk_size_applies_to_sessions_created_after_a_restart` (003-B), `test_an_expired_session_is_gone`
      (005-A, 012-A, 012-B, 012-C, 012-D), `test_a_hash_mismatch_registers_nothing` (007-B),
      `test_an_incomplete_session_registers_nothing` (007-C),
      `test_content_without_the_declared_family_registers_nothing` (009-E, 010-B),
      `test_unprobeable_content_registers_nothing` (009-F), `test_the_reaper_removes_expired_and_orphaned_content`
      (015-A, 015-C, 015-D, 015-E, plus `SCN-LOGGING-017-B`) and `test_an_absent_library_root_refuses_uploads` (016-A,
      016-B, 016-C).
- [x] 17.5 Add `SCN-UPLOAD-005-C` and `008-A/B/C` to the docstring's "cannot observe" list with their three use-case
      proofs, and state that `003-A` stays in the spec unproved, because the configuration is startup-only and `003-B`
      carries the restart.

## 18. Authorization

- [x] 18.1 Rework `test/system/authorization.py` to 39 cases, all shared and none with a container; usernames from a
      module-level counter, gallery names literal because each owner is fresh, imports `allure`, `itertools.count`,
      `conftest`'s `SEED_MEDIA_DIRECTORY`, `add_to_gallery`, `create_gallery`, `linked_id`, `provision` and `upload`,
      and `harness`'s `ROOT_ADMIN_USERNAME`, `SECRET_PASSWORD`, `Sut`, `begin_upload`, `link_of` and `register`.
- [x] 18.2 Correct the four labels that name a neighbouring scenario: `016-A`→`016-C`, `018-A`→`018-B`, `025-A`→`025-B`;
      relabel `022-B`'s admin creation to `023-D` and merge it into `019-B`; merge `034-A` into `032-D`.
- [x] 18.3 Extend `test_an_owner_replaces_the_set` (`032-B`) with `SCN-DOWNLOAD-002-D`: bob reads the gallery after the
      replacement and is concealed once the set drops him.
- [x] 18.4 Add the nine moved/new cases: the catalogue read (`022-A`) and rename refusal (`022-B`), the group and album
      creations (`023-G`), the film creation refusal (`023-B`), the playlist denial (`023-F`), the unreadable recording
      (`040-A`), the filtered film listing (`039-A`), and the concealment proof (`001-C`) that a `DENY` on a seeded film
      answers `404` for `GET` and `PATCH` and that another user's private file answers `404`. The concealment proof
      exposed `delete_gallery` and `delete_gallery_item` keeping their `GALLERY` and `GALLERY_ITEM` resources, so a
      deleted gallery's freed identifier answered `500` on the next creation; both use cases remove their resource now,
      with unit tests.
- [x] 18.5 Express each deny as a replacement of the resource's user grant set at its current visibility, and confirm
      the seeded role grants stay untouched; a later case may drop its own deny once its assertions have run.

## 19. Modules that only lose imports

- [x] 19.1 `test/system/series.py`: inline its own copies of `assert_summary`, `typed_video`, `upload_video` and their
      constants, because section 13 removes them from `video.py`; run the module unchanged in behaviour.
- [x] 19.2 Confirm no module under `test/system/` imports `sqlite3` any more, and that `mnemorium.db` is only ever a
      container artifact.

## 20. Documentation, glossary and agent prompts

- [x] 20.1 `docs/development/Glossary.md`: add **Default album** and **Default group**, and add **Seed profile** beside
      the test vocabulary.
- [x] 20.2 `docs/development/TechnicalDesign.md`: § 5's seed mechanism, the shared-instance model, the sink-as-a-
      collection rule and "no system case reads the datastore"; § 10 and the seed-data prose; move the HAL-helper
      sentence from the system-suite paragraph to the API-contract prose.
- [x] 20.3 `docs/development/Test.md`: the profiles, the seed trees, the shared instance and the own-container reasons.
- [x] 20.4 `docs/development/UseCases.md`: the browse-stored-files use case for the new collection.
- [x] 20.5 `README.md` and `AGENTS.md`: the seeded run names `api`, the system row names `test-seed`, and the coverage
      row names the seed file.
- [x] 20.6 `docs/Permissions.md`: no rule changes; confirm the file collection's per-item `Discover` is already
      described and add nothing.
- [x] 20.7 `docs/development/TechnicalDesign.md`: the generated-client rule in § System test and, in § OpenAPI
      generation, that the declaration is the only place a request's shape is written; `README.md` and `AGENTS.md` gain
      the `script/openapi_client.sh` command and the system rows name the generated client.

## 21. Verification

- [x] 21.1 `cargo clippy --all-targets --all-features -- -D warnings`, the same without features, and
      `cargo fmt --all -- --check`.
- [x] 21.2
      `cargo llvm-cov --lib --features test-seed --ignore-filename-regex 'src/lib/infrastructure/inbound/rest/handler|src/lib/infrastructure/seed.rs'`,
      with the three 80 % gates; `list_files` and the ingest's resource write are covered by unit tests.
- [x] 21.3 `script/rule_enforcer.sh check`, `script/check_scopes.sh`, ruff, sqlfluff over
      `migrations test/api/seed/sql test/performance/seed/sql test/system/seed/sql`, ls-lint, yamllint, shellcheck,
      shfmt, nixfmt, markdownlint and prettier.
- [x] 21.4 `openspec validate system-seed-and-scenario-split --strict`.
- [x] 21.5 `script/system.sh` serial and with `PYTEST_WORKERS=4`; then each reworked module alone against the seeded
      shared instance. The other shared cases must still pass — that is the empirical check that the seeded rows are
      inert.
- [x] 21.6 `pytest test/api` (the moved HAL cases) and `script/api_fuzz.sh`, including a Schemathesis run over the new
      `list_files` operation; `script/performance.sh`; and `NO_BUILD=1` against a pre-change image to confirm its
      failure message names `test-seed api-docs`.
- [x] 21.7 `script/openapi_client.sh generate` twice in a row produces an identical `test/generated/` tree and prints no
      generator warning; `git status --short` stays clean afterwards, `import mnemorium_client` succeeds, a missing
      client stops `pytest test/system --collect-only` with the generation hint, and `pytest test/api` runs without the
      client.
- [x] 21.8 Remove section 4's transitional entries from `[tool.vulture]`'s `ignore_names` (`LOG_FILE`,
      `MSG_MEDIA_ROOT_ABSENT`, `written_events`, `wait_for_events`, `SEED_MEDIA_DIRECTORY`, `file_at`,
      `wait_for_ingest`, `session_state`, `staged_path`, `store_chunk`, `completed_file`) once the module sections call
      them, and rerun `vulture`: the gate must stay green with no exemption for a helper the reworks kept.
