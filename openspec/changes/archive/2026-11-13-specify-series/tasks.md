# Tasks

Schemathesis already proves a requirement's declarative contract against the served document: every declared status,
media type, response header, envelope shape and `_links`/`_embedded` field belongs to the API fuzz test
(`script/api_fuzz.sh`). Do not add a system case that restates it — a system case proves only the effect the fuzzer
cannot observe, and the task names the fuzz operation that covers the rest.

Do not run the E2E suite (`script/e2e.sh` or `pytest test/e2e`), the configuration suite (`script/configuration.sh`) or
`devenv test` in this change, with no exceptions.

## 1. Validate the specification deltas

- [x] 1.1 Run `openspec validate specify-series --strict` and verify it passes; fix any description-length or
      scenario-format finding without changing the stated behavior.
- [x] 1.2 Verify `specs/series/spec.md` carries a `## Purpose`, a contiguous `REQ-SERIES-001`–`013` sequence, at least
      one `SCN-SERIES-*` scenario per requirement, and four hashtags on every scenario heading; record the requirement
      and scenario totals.
- [x] 1.3 Verify `specs/library/series/spec.md` removes exactly the fifteen requirements `specify-metadata-enrichment`
      leaves, each with a **Reason** and **Migration**, and that neither removed header names an external-metadata apply
      requirement; `openspec validate` reports no unmatched requirement.
- [x] 1.4 Verify the action map: list `Discover`, read `View`, show create `CreateChild` on the catalogue, show update
      `View`+`EditMetadata`, season and episode write `CreateChild` on the show with the season or episode child kind,
      season and episode update `View`+`EditMetadata`; confirm each against the use-case call sites.

## 2. System suite: show

- [x] 2.1 Add `test/system/series.py` with `@allure.epic("Series")` on every case and the `feature`/`story` labels for
      the requirement and scenario each proves; verify `pytest --collect-only -q test/system/series.py` lists it without
      an entry in `pytest.ini`.
- [x] 2.2 Prove `REQ-SERIES-008` first: the root creates a show and asserts `201` with a `Location`; a repeated name
      creates a second show; an unknown genre is stored; `422` for an empty name, empty genre, malformed date and
      non-two-letter country; `409` for a non-library cover; `403` for a Standard user. Verify the cases pass against
      the current image.
- [x] 2.3 Prove `REQ-SERIES-001` and `REQ-SERIES-002`: with two created shows, the collection embeds partial items in
      ascending identifier order carrying identifier, name, cover and links and no plot, genres, dates, seasons or
      counts; the `name` filter and page behave; a detail read carries the metadata, genres, cover and season summaries;
      nulls, unknown and malformed identifiers answer as specified.
- [x] 2.4 Correct the served doc comments: `post_series`, `get_series`, `get_series_list`, `patch_series` and the season
      and episode handlers stop naming `library.manage`/`library.read` and name the action instead; verify `cargo test`
      and the OpenAPI regeneration report no document change.

## 3. System suite: seasons and episodes

- [x] 3.1 Prove `REQ-SERIES-003`, `004`, `010` and `011`: write a season at a free index (`201`), replace it (`200`) and
      assert its episodes survive; read it and assert the fields, the index-as-`id` rule and the show scoping; patch the
      name and cover, clear each with `null`, and assert `404`/`400` for an unknown show, a foreign index and a
      malformed index; assert `409` for a non-library cover and `403` for a denied caller.
- [x] 3.2 Prove `REQ-SERIES-005`, `006`, `012` and `013`: upload a video, write an episode at a free index (`201`), read
      it and the season's episode collection, replace it (`200`), replace its video and assert the previous video is
      unattached; assert `409` for a video attached to another episode, `422` for an unknown video and invalid input,
      `404`/`400` for unknown parents and malformed indices; patch the metadata, clear the air date and assert the video
      is unchanged.
- [x] 3.3 Prove `REQ-SERIES-007`: a show with several seasons, each holding several episodes, lists seasons and episodes
      by ascending index independent of insertion order. If the largest 32-bit index cannot be staged cheaply, keep that
      half of `SCN-SERIES-003-B` in the application test and record the move in `design.md`.
- [x] 3.4 Add the glossary entries **Series**, **Season** and **Episode** to `docs/development/Glossary.md`, in the
      video-area order, and name `series.py` in `TechnicalDesign.md` § 5's system-suite narrative; verify
      `prettier --check docs/development/Glossary.md` and `markdownlint-cli2 '**/*.md'` pass.

## 4. E2E reduction and Rust labels

- [x] 4.1 Reduce `test/e2e/library/uc023_browse_series.py`, `uc027_write_series_metadata.py`,
      `uc028_write_season_metadata.py` and `uc029_write_episode_metadata.py` to their business flow: delete every case
      the system module proves and keep the flow that binds each file to its catalogued use case; verify by reading all
      four modules that no assertion they keep duplicates a system case.
- [x] 4.2 Label the series, season and episode application tests in
      `use_case/{list_series,get_series,create_series,     update_series,put_series_season,update_series_season,put_series_episode,update_series_episode,get_series_season,     list_series_episodes,get_series_episode}.rs`
      with the `allure.epic`/`feature`/`story` of the requirement each uniquely proves, and delete or reduce a case that
      only re-proves a system scenario per `TEST-003`; verify the affected `cargo test` filters pass.

## 5. Integration gates

- [x] 5.1 Run `cargo test` and verify the unit and integration suites pass.
- [x] 5.2 Run `cargo clippy --all-targets --all-features -- -D warnings`, `cargo clippy --all-targets -- -D warnings`
      and `cargo fmt --all -- --check`; verify all three are clean.
- [x] 5.3 Run
      `cargo llvm-cov --lib --ignore-filename-regex 'src/lib/infrastructure/inbound/rest/handler' --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80`
      and verify the coverage gate holds.
- [ ] 5.4 Run `script/rule_enforcer.sh check` and verify every registered rule passes, including the `TEST-039` E2E
      mapping after the four-module reduction.

## Workflow follow-up

- Archive the change after review, then verify the `series` main spec carries its `## Purpose` and `library/series` no
  longer exists.
