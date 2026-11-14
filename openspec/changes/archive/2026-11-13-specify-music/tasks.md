# Tasks

Schemathesis already proves a requirement's declarative contract against the served document: every declared status,
media type, response header, envelope shape and `_links`/`_embedded` field belongs to the API fuzz test
(`script/api_fuzz.sh`). Do not add a system case that restates it — a system case proves only the effect the fuzzer
cannot observe, and the task names the fuzz operation that covers the rest.

Do not run the E2E suite (`script/e2e.sh` or `pytest test/e2e`), the configuration suite (`script/configuration.sh`) or
`devenv test` in this change, with no exceptions.

## 1. State the stored cover and remove the resolved one

- [x] 1.1 Rewrite `test/system/cover_art.py` under the `Cover art` epic with one case per scenario of
      `REQ-COVER-ART-001` to `REQ-COVER-ART-005`, `REQ-COVER-ART-006` (an episode and a track carry no artwork) and
      `REQ-COVER-ART-007`, each carrying `epic`/`feature`/`story` labels. Verify every case passes against the rebuilt
      image.
- [x] 1.2 Delete `effective_cover_image_id` from every representation, its application response and its constructor.
      Keep `cover_image_id` as the sole cover field, make `_links.cover` follow it, and remove the album and show
      lookups that resolved a child's cover from the season, episode, album-track and playlist-track reads.
- [x] 1.3 Correct the application test in `get_series.rs` that asserted a season reports its show's cover, and delete
      the test helper that existed only for the album cover lookup. Verify `cargo test` is green.
- [x] 1.4 Add `Cover art` to `docs/development/Glossary.md` and remove the resolved-cover module from
      `docs/development/Overview.md`. Verify the Markdown gates (`markdownlint-cli2 '**/*.md'` and
      `prettier --check '**/*.md'`) pass on both files.

## 2. Store the missing cover writes

- [x] 2.1 Add a failing case for `SCN-MUSIC-015-C` (a group's cover cleared) to `test/system/music.py` and failing cases
      for `SCN-COVER-ART-007` (a playlist's cover set on creation, replaced, cleared and refused when unusable) and
      `SCN-COVER-ART-003-C` (a `LIBRARY` image refused for a playlist) to `test/system/cover_art.py`, each with the
      three Allure labels. Verify each fails for the expected reason.
- [x] 2.2 Change the group update command and `PatchMusicGroupRequest` to the tri-state field, and add `cover_image_id`
      to playlist creation and metadata update. Verify task 2.1 passes and an absent field still keeps the stored cover.
- [x] 2.3 Extend the repository integration tests to the playlist-cover trigger pair: a `LIBRARY` image is refused and
      an image already referenced elsewhere is refused, each mapped to the status `REQ-COVER-ART-007-D` declares. Verify
      `cargo test` covers both refusals.

## 3. Add the missing authorization checks

- [x] 3.1 Add failing cases to `test/system/music.py` for `SCN-MUSIC-019` created by a caller whose `CreateChild` on the
      playlist collection is denied, and `SCN-MUSIC-025-B` appending a recording whose metadata the caller may not read,
      with the three Allure labels. Verify both answer `403 Forbidden`.
- [x] 3.2 Decide `CreateChild` for a music playlist against the playlist collection in `create_playlist`, and
      `ViewMetadata` on the recording in `update_playlist` beside the existing `CreateChild`. Verify task 3.1 passes and
      a **Standard User** still creates and appends.

## 4. Repoint the served music documentation

- [x] 4.1 Rewrite the doc comments of the nine music endpoint handlers — album create, update, medium, track place,
      track edit, group create, group update, playlist create, playlist update — to name the action the route decides
      instead of `library.manage` or `playlist.moderate`. Verify the served OpenAPI description names the action and
      that no music handler under `handler/library/` still contains `library.read`, `library.manage`, `playlist.read` or
      `playlist.moderate`.
- [ ] 4.2 Verify `script/rule_enforcer.sh check` passes with the rewritten comments.

## 5. Retire the replaced proof

- [x] 5.1 Reduce the E2E modules UC-012 to UC-019 to their business flow and delete every case the two system modules
      now prove. Verify by reading the reduced modules that no assertion duplicates a system case, `pytest test/system`
      passes, and no scenario is proven twice.
- [x] 5.2 Name `music.py` and `cover_art.py` in `docs/development/TechnicalDesign.md` § 5's system-module list. Verify
      `markdownlint-cli2` passes on the file.

## 6. Integration

- [x] 6.1 Verify `openspec validate specify-music --type change --strict` passes.
- [x] 6.2 Verify `cargo test`, both clippy commands and `cargo fmt --all -- --check` pass.
- [ ] 6.3 Verify `script/rule_enforcer.sh check` passes with the reduced E2E modules still mapped.

## Workflow follow-up

- Archive the change once the project's review requirements are satisfied.
- Verify the archived result.
