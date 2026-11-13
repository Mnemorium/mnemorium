# Tasks

Schemathesis already proves a requirement's declarative contract against the served document: every declared status,
media type, response header, envelope shape and `_links`/`_embedded` field belongs to the API fuzz test
(`script/api_fuzz.sh`). Do not add a system case that restates it — a system case proves only the effect the fuzzer
cannot observe, and the task names the fuzz operation that covers the rest.

Do not run the E2E suite (`script/e2e.sh` or `pytest test/e2e`), the configuration suite (`script/configuration.sh`) or
`devenv test` in this change, with no exceptions.

## 1. Validate the specification deltas

- [x] 1.1 Run `openspec validate specify-typed-assets --strict` and verify it passes; fix any description-length or
      scenario-format finding in the delta files without changing the stated behavior.
- [x] 1.2 Verify `specs/image/spec.md`, `specs/audio/spec.md` and `specs/video/spec.md` each carry a `## Purpose`, a
      contiguous `REQ-{IMAGE,AUDIO,VIDEO}-*` sequence, at least one `SCN-*` scenario per requirement and four hashtags
      on every scenario heading; record the requirement and scenario totals in the change. Recorded: `image` 4
      requirements (`REQ-IMAGE-001`–`004`) and 10 scenarios; `audio` 4 (`REQ-AUDIO-001`–`004`) and 9 scenarios; `video`
      5 (`REQ-VIDEO-001`–`005`) and 10 scenarios — 13 requirements and 29 scenarios in total, no requirement without a
      scenario and no scenario heading with other than four hashes.
- [x] 1.3 Verify `specs/library/assets/spec.md` removes exactly five requirements, each with a **Reason** and
      **Migration**, and every removed header matches the main spec's header text exactly; `openspec validate` reports
      no unmatched requirement. Recorded: five removals — `A typed image is readable`,
      `A typed image addresses its file`, `A typed audio is     readable`, `A typed video is readable`,
      `An upload names its media row after the uploaded file` — each matching a header in
      `openspec/specs/library/assets/spec.md`, each carrying **Reason** and **Migration**, and
      `openspec validate specify-typed-assets --strict` reporting no unmatched requirement.
- [x] 1.4 Verify the migration mapping is complete: every removed typed read and alias names its
      `REQ-{IMAGE,AUDIO,VIDEO}-*` successor, and the removed naming requirement names `REQ-IMAGE-002` and
      `REQ-AUDIO-002` plus the intake rule that chooses the name; no scenario is lost without a named owner. Recorded:
      every removed typed read and alias names its successor; the naming requirement names `REQ-IMAGE-002`,
      `REQ-AUDIO-002` and both intake rules. One gap: neither `upload`'s delta nor `library/media-ingest` states the
      rule that chooses the name, so the two removed naming scenarios map to a rule that does not exist yet. Recorded as
      a risk in `design.md` instead of a delta change, because stating it here would collide with `specify-upload`'s
      delta on the same capability.

## 2. System suite: image

- [x] 2.1 Add `test/system/image.py` with `@allure.epic("Image")` on every case and the `feature`/`story` labels for the
      requirement and scenario each proves; verify `pytest --collect-only -q test/system/image.py` lists it without an
      entry in `pytest.ini`. Recorded: `pytest --collect-only -q test/system` collects `image.py`'s 10 cases once each
      through the `test/system/conftest.py` hook, and `pytest.ini` is unchanged.
- [x] 2.2 Write the image cases first and run them against the current image: upload a landscape, portrait and square
      PNG through the conftest `upload()` helper and assert the reported orientation, name and creation date; request
      the typed route by following the file's `_links.image`; assert the representation fields and `_links.self`,
      `_links.file` and `_links.content`; assert an unknown image answers `404` and the `/file` alias answers `303` with
      the file route. Verify the module passes and record any divergence before changing a test. Recorded: no divergence
      — all 10 cases pass against `mnemorium:latest` on the first run, so no assertion was weakened and no production
      code changed.
- [x] 2.3 Prove `SCN-IMAGE-001-A`–`C`: after an image upload exactly one image backs the file, the same bytes registered
      again create no second image, and an audio or video upload creates no image; verify through the file
      representation's `_links.image` and the typed route. Recorded: the three cases pass; "exactly one" is proven by
      reading the typed row and by the `404` the next identifier answers after a first and after a duplicate
      registration.
- [x] 2.4 Update `docs/development/TechnicalDesign.md` § 5's system-suite narrative to name `image.py` beside the
      existing feature modules, and verify the Markdown gate passes with `markdownlint-cli2 '**/*.md'`. Recorded: the
      module list now reads `api.py`, `authentication.py`, `authorization.py`, `configuration.py`, `image.py`,
      `logging_events.py`, `state.py` and `user.py`; `markdownlint-cli2 '**/*.md'` reports 0 issues.
- [x] 2.5 Add the glossary entries **Typed asset**, **Image**, **Asset scope**, **Orientation** to
      `docs/development/Glossary.md` in the media-area order, reusing the Overview model definitions where they already
      exist; verify `prettier --check docs/development/Glossary.md` passes. Recorded: **Image** builds on the Overview
      model entry of the same name; **Orientation** is the derived classification;
      `prettier --check docs/development/Glossary.md` and `markdownlint-cli2` both pass.

## 3. System suite: audio and video

- [x] 3.1 Add `test/system/audio.py` with `@allure.epic("Audio")`, the `feature`/`story` labels and cases for
      `REQ-AUDIO-001`–`004`; upload an audio fixture and assert the name, codec and every reported measurement, then a
      fixture that reports no bit depth, block size, duration or sample rate and assert those fields are absent rather
      than fabricated; assert the links, the unknown `404` and the `303` alias. Verify the module passes. Recorded: all
      9 cases pass against `mnemorium:latest`. Fixtures are hand-built PCM WAVs — 16-bit reports bit depth, duration and
      sample rate; 64-bit reports no bit depth and no block size; a zero-sample WAV reports no duration. Divergences:
      the image under test upper-cases the probed codec name (`PCM_S16LE`) while the source passes ffprobe's lowercase
      name through, so a codec is compared case-insensitively; no fixture makes the probe report a block size or omit a
      sample rate, so those two clauses stay unproven black box and rest on the application tests in 4.1.
- [x] 3.2 Add `test/system/video.py` with `@allure.epic("Video")`, the `feature`/`story` labels and cases for
      `REQ-VIDEO-001`–`004`; upload a video fixture and assert codec, colour identifier, duration, frame count,
      dimensions and scan type; assert the typed representation carries no name; assert the links, the unknown `404` and
      the `303` alias. Verify the module passes. Recorded: all 10 cases pass. The fixture streams `PROGRESSIVE`; the
      discrete scan-type set is declared by the served document, which Schemathesis validates, and the probe emits only
      `PROGRESSIVE` or `INTERLACED`, so `INTERLACED`, `MBAFF` and `PAFF` have no black-box fixture — see the note
      recorded under 4.3.
- [x] 3.3 Prove `REQ-VIDEO-005` from an existing film and episode read: assert the embedded summary carries the video
      identifier, backing file, codec, duration, width, height and scan type, and carries no colour identifier, frame
      count or links; verify against `GET /api/v1/library/movie` and the episode read, and leave the catalogue fields to
      `movies` and `series`. Recorded: one case creates a film on one video and a show, season and episode on another,
      reads both back and asserts the summary's exact key set, so an added colour identifier, frame count or link fails
      it.
- [x] 3.4 Add the glossary entries **Audio**, **Video**, **Scan type** to `docs/development/Glossary.md`; verify
      `prettier --check docs/development/Glossary.md` passes. Recorded: the entries sit beside **Image**, **Asset
      scope** and **Orientation** in media-area order; prettier and markdownlint both pass.

## 4. Rust tests and the E2E boundary

- [x] 4.1 Label the existing application tests in `use_case/get_image.rs`, `get_audio.rs` and `get_video.rs` with the
      `allure.epic`/`feature`/`story` of the requirement each uniquely proves (projection and repository-failure cases);
      delete or reduce any case that only re-proves a system scenario, per `TEST-003`; verify `cargo test get_image`,
      `cargo test get_audio` and `cargo test get_video` pass. Recorded: 10 cases labelled — the projection case of each
      module with `SCN-*-003-A`, and the absent-row, unknown-caller and repository-failure cases with `SCN-*-003-B`. No
      case was deleted or reduced: each asserts an artifact the container cannot observe (the error variant, the commit
      flag, the minted content link, a file owned by a catalogue resource), which is what `TEST-003` keeps the
      application layer for. All three `cargo test` filters pass.
- [x] 4.2 Reduce `test/e2e/asset/uc031_read_asset_content.py`: delete the typed-view and alias cases the three system
      modules now prove, and keep the whole/ranged/conditional/`HEAD`/credential/reconciliation cases that belong to the
      serving capability; verify by reading the file that the kept cases name the content coverage they still prove.
      Recorded: `test_typed_view_links_and_aliases` (three parametrised cases) is gone with its path constants and the
      `audio`/`video` fixture imports; `test_ranges_validators_head_and_credential` proves whole, ranged, conditional,
      `HEAD` and credential-only reads and says so, and `test_a_removed_file_is_reconciled_to_not_found` proves the
      reconciliation, so the module now covers the serving flow only.
- [x] 4.3 Verify no scenario is proven twice: list every `SCN-IMAGE-*`, `SCN-AUDIO-*` and `SCN-VIDEO-*` scenario and
      name the one test that proves it, in the system module or the Rust module; record the map in the change. Recorded:
      the map below, 29 scenarios across the three modules, each with one proving test. The three Rust cases that carry
      `SCN-*-003-A`/`-B` restate a story at the application layer — the precedent `SCN-AUTHENTICATION-001-D` already
      sets across `initialize_root_admin.rs` and `unit_of_work.rs` — and the map names the black-box test as the
      scenario's proof.

## 5. Integration gates

- [x] 5.1 Run `cargo test` and verify the unit and integration suites pass. Recorded: 1561 passed, 0 failed.
- [x] 5.2 Run `cargo clippy --all-targets --all-features -- -D warnings`, `cargo clippy --all-targets -- -D warnings`
      and `cargo fmt --all -- --check`; verify all three are clean. Recorded: both clippy runs finish clean and
      `cargo fmt --all -- --check` reports no diff.
- [x] 5.3 Run
      `cargo llvm-cov --lib --ignore-filename-regex 'src/lib/infrastructure/inbound/rest/handler' --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80`
      and verify the coverage gate holds. Recorded: functions 90.25%, regions 93.93%, lines 88.88% — every threshold
      met.
- [ ] 5.4 Run `script/rule_enforcer.sh check` and verify every registered rule passes, including the `TEST-039` E2E
      mapping after the `uc031` reduction. Skipped at the operator's direction. The gate is therefore unverified by this
      change; `UC-031` still maps to `test/e2e/asset/uc031_read_asset_content.py`, whose kept cases now cover only the
      serving flow.

## Scenario proof map

No scenario is proven twice. Each of the 29 scenarios owns one proving test; the three Rust cases that carry
`SCN-*-003-A` or `SCN-*-003-B` assert an application-layer artifact the container cannot observe, at the same
requirement and story the system module reports.

| Scenario        | Proving test                                                                 |
| --------------- | ---------------------------------------------------------------------------- |
| SCN-IMAGE-001-A | `test/system/image.py::test_an_image_file_creates_one_image`                 |
| SCN-IMAGE-001-B | `test/system/image.py::test_a_file_backs_one_image`                          |
| SCN-IMAGE-001-C | `test/system/image.py::test_audio_and_video_content_create_no_image`         |
| SCN-IMAGE-002-A | `test/system/image.py::test_a_wider_image_is_landscape`                      |
| SCN-IMAGE-002-B | `test/system/image.py::test_a_taller_image_is_portrait`                      |
| SCN-IMAGE-002-C | `test/system/image.py::test_a_square_image_is_square`                        |
| SCN-IMAGE-002-D | `test/system/image.py::test_the_name_and_creation_date_are_reported`         |
| SCN-IMAGE-003-A | `test/system/image.py::test_an_image_carries_its_representation`             |
| SCN-IMAGE-003-B | `test/system/image.py::test_an_unknown_image_is_not_found`                   |
| SCN-IMAGE-004-A | `test/system/image.py::test_the_file_alias_redirects_to_the_file`            |
| SCN-AUDIO-001-A | `test/system/audio.py::test_an_audio_file_creates_one_audio`                 |
| SCN-AUDIO-001-B | `test/system/audio.py::test_a_file_backs_one_audio`                          |
| SCN-AUDIO-001-C | `test/system/audio.py::test_image_and_video_content_create_no_audio`         |
| SCN-AUDIO-002-A | `test/system/audio.py::test_every_reported_measurement_is_reported`          |
| SCN-AUDIO-002-B | `test/system/audio.py::test_a_missing_bit_depth_and_block_size_stay_absent`  |
| SCN-AUDIO-002-C | `test/system/audio.py::test_a_missing_duration_stays_absent`                 |
| SCN-AUDIO-003-A | `test/system/audio.py::test_an_audio_carries_its_representation`             |
| SCN-AUDIO-003-B | `test/system/audio.py::test_an_unknown_audio_is_not_found`                   |
| SCN-AUDIO-004-A | `test/system/audio.py::test_the_file_alias_redirects_to_the_file`            |
| SCN-VIDEO-001-A | `test/system/video.py::test_a_video_file_creates_one_video`                  |
| SCN-VIDEO-001-B | `test/system/video.py::test_a_file_backs_one_video`                          |
| SCN-VIDEO-001-C | `test/system/video.py::test_image_and_audio_content_create_no_video`         |
| SCN-VIDEO-002-A | `test/system/video.py::test_the_measurements_are_reported`                   |
| SCN-VIDEO-002-B | `test/system/video.py::test_a_progressive_stream_reports_a_progressive_scan` |
| SCN-VIDEO-002-C | `test/system/video.py::test_a_video_carries_no_name`                         |
| SCN-VIDEO-003-A | `test/system/video.py::test_a_video_carries_its_representation`              |
| SCN-VIDEO-003-B | `test/system/video.py::test_an_unknown_video_is_not_found`                   |
| SCN-VIDEO-004-A | `test/system/video.py::test_the_file_alias_redirects_to_the_file`            |
| SCN-VIDEO-005-A | `test/system/video.py::test_a_film_and_an_episode_embed_the_summary`         |

Three scenarios carry a gap the black-box suite cannot close, recorded here so the review can decide on an artifact
update rather than read them as proven:

- `SCN-AUDIO-002-A` — no content makes the probe report a block size, so only the reported measurements are asserted.
- `SCN-AUDIO-002-C` — no identifiable audio stream omits a sample rate, so only the missing duration is asserted.
- `SCN-VIDEO-002-B` — the probe maps every interlaced field order to `INTERLACED`, so `MBAFF` and `PAFF` (and an
  interlaced fixture) are unreachable black box; the discrete set itself is declared by the served document.

## Workflow follow-up

- Archive the change after review, then verify the five requirements left `library/assets` and the three new main specs
  carry their `## Purpose`.
