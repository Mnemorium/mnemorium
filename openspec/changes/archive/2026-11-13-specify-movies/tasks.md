# Tasks

Schemathesis already proves a requirement's declarative contract against the served document: every declared status,
media type, response header, envelope shape and `_links`/`_embedded` field belongs to the API fuzz test
(`script/api_fuzz.sh`). Do not add a system case that restates it — a system case proves only the effect the fuzzer
cannot observe, and the task names the fuzz operation that covers the rest.

Do not run the E2E suite (`script/e2e.sh` or `pytest test/e2e`), the configuration suite (`script/configuration.sh`) or
`devenv test` in this change, with no exceptions.

## 1. Validate the specification deltas

- [x] 1.1 Run `openspec validate specify-movies --strict` and verify it passes; fix any description-length or
      scenario-format finding without changing the stated behavior.
- [x] 1.2 Verify `specs/movies/spec.md` carries a `## Purpose`, a contiguous `REQ-MOVIES-001`–`005` sequence, at least
      one `SCN-MOVIES-*` scenario per requirement, and four hashtags on every scenario heading; record the requirement
      and scenario totals.
- [x] 1.3 Verify `specs/library/movies/spec.md` removes exactly the seven requirements `specify-metadata-enrichment`
      leaves, each with a **Reason** and **Migration**, and that no removed header names the external-metadata apply
      requirement; `openspec validate` reports no unmatched requirement.
- [x] 1.4 Verify the mapping is complete: every removed read, list, order and write requirement names its `REQ-MOVIES-*`
      successor, and every whose subject is authentication or authorization names the owning capability instead.

## 2. System suite: film creation

- [x] 2.1 Add `test/system/movies.py` with `@allure.epic("Movies")` on every case and the `feature`/`story` labels for
      the requirement and scenario each proves; verify `pytest --collect-only -q test/system/movies.py` lists it without
      an entry in `pytest.ini`.
- [x] 2.2 Prove `REQ-MOVIES-004` first: the root uploads a video, reads its `_links.video`, creates a film and asserts
      `201` with a `Location`; repeats the same request and asserts `200` with the same film; creates a film with an
      unknown genre and asserts the genre is linked; asserts `422` for an unknown video, empty name, empty genre,
      malformed date and non-two-letter country; asserts `409` for a cover that is not a library image; asserts `403`
      when a Standard user attempts creation. Verify the cases pass against the current image.
- [x] 2.3 Prove the attached-video case of `SCN-MOVIES-004-C`: create an episode that takes a video through the series
      route, then attempt a film on the same video and assert `409` with nothing created. If the series route cannot be
      driven from this module, keep the case in the `series` system module and record the move in `design.md`.
- [x] 2.4 Correct the served doc comments: `post_movie`, `get_movie`, `patch_movie` and the collection handler stop
      naming `library.manage`/`library.read` and name the action instead; verify `cargo test` and the OpenAPI
      regeneration report no document change.

## 3. System suite: list, read and update

- [x] 3.1 Prove `REQ-MOVIES-001` and `REQ-MOVIES-002`: with two created films, the collection embeds partial items in
      ascending identifier order, each carrying identifier, name, cover and links and no plot, genre list or video
      summary; the `name` filter and `limit`/`offset` page behave; an out-of-range limit answers `422` and a malformed
      query `400`; a caller that discovers no film gets an empty collection.
- [x] 3.2 Prove `REQ-MOVIES-003`: the detail read carries name, plot, country, release date, genres ordered by
      identifier, the video summary and the cover; a film without a date or country reports `null`; an unknown film
      answers `404` and a malformed identifier `400`.
- [x] 3.3 Prove `REQ-MOVIES-005`: patch changes each field, `null` clears the optional ones, the video identifier is
      unchanged, a Standard user who may read but not edit answers `403`, an unknown film `404` and a malformed
      identifier `400`; each refusal leaves the stored film unchanged.
- [x] 3.4 Add the glossary entry **Movie** with its film synonym resolved, in the video-area order, and name `movies.py`
      in `TechnicalDesign.md` § 5's system-suite narrative; verify `prettier --check docs/development/Glossary.md` and
      `markdownlint-cli2 '**/*.md'` pass.

## 4. E2E reduction and Rust labels

- [x] 4.1 Reduce `test/e2e/library/uc022_browse_movies.py` and `uc026_write_movie_metadata.py` to their business flow:
      delete every case the system module proves, correct the stale "no endpoint creates a film" docstring, and verify
      by reading both modules that no assertion they keep duplicates a system case.
- [x] 4.2 Label the `list_movies`, `get_movie`, `create_movie` and `update_movie` application tests with the
      `allure.epic`/`feature`/`story` of the requirement each uniquely proves, and delete or reduce a case that only
      re-proves a system scenario per `TEST-003`; verify the four `cargo test` filters pass.

## 5. Integration gates

- [x] 5.1 Run `cargo test` and verify the unit and integration suites pass.
- [x] 5.2 Run `cargo clippy --all-targets --all-features -- -D warnings`, `cargo clippy --all-targets -- -D warnings`
      and `cargo fmt --all -- --check`; verify all three are clean.
- [x] 5.3 Run
      `cargo llvm-cov --lib --ignore-filename-regex 'src/lib/infrastructure/inbound/rest/handler' --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80`
      and verify the coverage gate holds.
- [ ] 5.4 Run `script/rule_enforcer.sh check` and verify every registered rule passes, including the `TEST-039` E2E
      mapping after the two-module reduction.

## Workflow follow-up

- Archive the change after review, then verify the `movies` main spec carries its `## Purpose` and `library/movies` no
  longer exists.
