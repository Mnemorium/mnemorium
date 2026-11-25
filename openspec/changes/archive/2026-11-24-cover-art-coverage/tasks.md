# Tasks

## 1. Specification deltas

- [x] 1.1 Validate the change (`openspec validate cover-art-coverage --strict`) and confirm the `cover-art` delta
      removes `SCN-COVER-ART-004-C` and narrows the one-referrer sentence to `USER` images, and the `authorization`
      delta adds the cover pair and `SCN-AUTHORIZATION-026-D`. Verify by reading the delta against
      `openspec/specs/cover-art/spec.md` and `openspec/specs/authorization/spec.md`.

## 2. System proofs (written first, seen to fail for the expected reason)

- [x] 2.1 Add `test_an_update_that_omits_the_cover_keeps_it` to `test/system/cover_art.py` (`SCN-COVER-ART-005-D`):
      create a playlist and an album with a cover, patch only the name, and assert the cover and its link survive on the
      response and on a fresh read.
- [x] 2.2 Add `test_a_file_identifier_is_not_a_cover` (`SCN-COVER-ART-002-C`): read seeded file `11101`, offer it as a
      playlist cover, assert `409` with the error envelope and no stored playlist, then patch a covered playlist with it
      and assert the stored cover is unchanged.
- [x] 2.3 Add `test_a_new_image_is_accepted_and_marked_user` (`SCN-COVER-ART-002-A`, `SCN-COVER-ART-003-B`): upload a
      PNG, create a playlist with it, assert the reported identifier and a readable link, then assert a catalogue
      creation naming it answers `409` and stores nothing.
- [x] 2.4 Add `test_a_user_image_backs_one_cover` (`SCN-COVER-ART-003-E`): upload one image, let one playlist take it,
      and assert a second playlist naming it answers `409` and does not exist.
- [x] 2.5 Add `test_a_catalogue_cover_marks_the_image_library` (`SCN-COVER-ART-003-A`, own instance): for a group,
      album, film, show and season, upload a fresh image, create the row with it, assert the cover is reported and
      readable, then assert a playlist naming it answers `409` and stores nothing.
- [x] 2.6 Add `test_a_playlist_without_a_cover_reports_none` (`SCN-COVER-ART-007-C`): create a playlist with no cover,
      assert `cover_image_id` is `null`, the `_links.cover` key is absent, and a fresh read agrees.
- [x] 2.7 Add `test_a_cover_needs_a_viewable_image` to `test/system/cover_art.py` (`SCN-AUTHORIZATION-026-D`): upload an
      image as one Standard user, have a second Standard user create a playlist naming it, and assert `409` with the
      envelope and no stored playlist; assert the owner's own creation succeeds.
- [x] 2.8 Tighten the existing cases: replace `body["_links"].get("cover") is None` with an absence assertion at every
      site, and pin the unknown-image refusal to `409`.

## 3. Application-layer cover check

- [x] 3.1 Add the shared cover-readability service under `src/lib/application` (image by identifier, its backing file,
      `View` on the file) with unit tests for the absent, readable, unknown and unreadable cases.
- [x] 3.2 Call it from every cover write path: `create_music_group`, `update_music_group`, `create_music_album`,
      `update_music_album`, `create_movie`, `update_movie`, `create_series`, `update_series`, `put_series_season`,
      `update_series_season`, `create_playlist` and `update_playlist`; return the use case's existing conflict variant
      on refusal. Verify with the affected unit modules, adding repository expectations only to cases that name a cover.
- [x] 3.3 Map a playlist cover already backed by another playlist to `InvalidCover` in `create_playlist` and
      `update_playlist` by searching `cover_image_id` before writing, keeping the unique index as the race guard. Verify
      with unit tests for both use cases.
- [x] 3.4 Update the twelve handlers' conflict descriptions and the application ports' doc comments to state that the
      cover must be a viewable image.

## 4. Documentation

- [x] 4.1 Update the `Cover art` definition in `docs/development/Glossary.md`: user scope has one referrer, library
      scope may be shared, and the scope is fixed by the first attachment.
- [x] 4.2 Extend the `docs/Permissions.md` cover sentence with the target-side rule: a cover write needs `View` on the
      image.

## 5. Verification

- [x] 5.1 `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`,
      `cargo clippy --all-targets -- -D warnings` and `cargo fmt --all -- --check` exit zero.
- [x] 5.2 `script/system.sh` passes, and `NO_BUILD=1 script/system.sh` still passes on the same image.
- [x] 5.3 `script/investigation.sh` regenerates the coverage matrix with `cover-art` at 24 of 24 scenarios and
      `SCN-AUTHORIZATION-026-D` proven; `script/rule_enforcer.sh check` passes.
- [x] 5.4 Archive the change (`openspec archive cover-art-coverage`) and confirm `openspec validate --specs` passes.
