# Tasks

Schemathesis already proves a requirement's declarative contract against the served document: every declared status,
media type, response header, envelope shape and `_links`/`_embedded` field belongs to the API fuzz test
(`script/api_fuzz.sh`). Do not add a system case that restates it — a system case proves only the effect the fuzzer
cannot observe, and the task names the fuzz operation that covers the rest.

Do not run the E2E suite (`script/e2e.sh` or `pytest test/e2e`), the configuration suite (`script/configuration.sh`) or
`devenv test` in this change, with no exceptions.

## 1. Validate the specification delta

- [x] 1.1 Run `openspec validate specify-gallery --strict` and verify it passes; fix any description-length or
      scenario-format finding without changing the stated behavior.
- [x] 1.2 Verify `specs/gallery/spec.md` carries a `## Purpose`, the `REQ-GALLERY-*` sequence (`001`–`008`, `010`,
      `011`), at least one `SCN-GALLERY-*` scenario per requirement, and four hashtags on every scenario heading; record
      the requirement and scenario totals in the change.
- [x] 1.3 Verify every removal, browse, create, item and ordering rule names the action it requires and leaves the
      who-may rule to `authorization`; record the action map (create `CreateChild` on `GALLERY_COLLECTION`, read `View`,
      browse `Discover`, delete `Delete`, add item `CreateChild` on the gallery).

## 2. Creation authorization and the handler contract

- [x] 2.1 Write the failing application test in `use_case/create_gallery.rs` for `SCN-GALLERY-003-E`: a resource
      repository that returns the `GALLERY_COLLECTION` surface owned by another principal with no allow answers
      `CreateGalleryError::Forbidden`, stores nothing and rolls back; verify the test fails for the expected reason
      (creation currently succeeds).
- [x] 2.2 Add the `CreateChild` authorization call on `ResourceKind::GalleryCollection` with `ChildKind::Gallery`, add
      `CreateGalleryError::Forbidden` and map it to `403 Forbidden` in the handler; verify the new test and the existing
      create tests pass with `cargo test create_gallery`.
- [x] 2.3 Correct the stale handler contracts without changing a route or payload: `post_gallery`'s `is_public` comment
      says read, not "read and moderate"; `delete_gallery` and `delete_gallery_item` stop naming the removed
      `gallery.manage` model; verify `cargo test` and the OpenAPI regeneration report no document change.

## 3. System suite: create, browse and read

- [x] 3.1 Add `test/system/gallery.py` with `@allure.epic("Gallery")` on every case and the `feature`/`story` labels for
      the requirement and scenario each proves; verify `pytest --collect-only -q test/system/gallery.py` lists it
      without an entry in `pytest.ini`.
- [x] 3.2 Prove `REQ-GALLERY-003` first: a created gallery answers `201` with a `Location`, is owned by its creator and
      private; the public flag yields a gallery another caller may read; a duplicate name answers `409`; an empty,
      whitespace-only and overlong name answer `422`; each refusal stores nothing. Verify the cases pass against the
      current image.
- [x] 3.3 Prove `REQ-GALLERY-004`: the collection lists what the caller may discover, filters by `name`, `is_public` and
      `owner_id`, carries `item_count`, and answers `400` for a malformed query; a caller with no discoverable gallery
      receives an empty collection.
- [x] 3.4 Prove `REQ-GALLERY-002` and `REQ-GALLERY-005`: the default gallery is ownerless, public and empty; it answers
      `409` on deletion; a gallery read returns its items ordered by position; an unknown gallery answers `404`.
- [x] 3.5 Add the glossary entries **Gallery**, **Gallery item** and **Default gallery** to
      `docs/development/Glossary.md`, and name `gallery.py` in `TechnicalDesign.md` § 5's system-suite narrative; verify
      `prettier --check docs/development/Glossary.md` and `markdownlint-cli2 '**/*.md'` pass.

## 4. System suite: items

- [x] 4.1 Prove `REQ-GALLERY-007`: add an image and a video by their backing file identifiers; assert `201` with a
      `Location`, the appended position, the advanced last-modified time, the item fields and the medium type; assert
      `409` for a medium already assigned and `422` for a missing or unowned medium.
- [x] 4.2 Prove `REQ-GALLERY-008`: an image item carries the image name, a video item carries the file path, and an item
      of another gallery answers `404`.
- [x] 4.3 Prove `REQ-GALLERY-010` and `REQ-GALLERY-011`: removal answers `204`, the item and its media row are gone (a
      re-add answers `422`), the bytes under the working directory are unchanged, a wrong-gallery item answers `404`,
      and a successful change advances the last-modified time.

## 5. E2E reduction, Rust labels and the use-case correction

- [x] 5.1 Reduce `test/e2e/library/uc007`–`uc011`, `uc020` and `uc021` to their business flow: delete every case the
      system module now proves, keep the flow that binds each file to its catalogued use case, and verify by reading the
      seven modules that no assertion they keep duplicates a system case.
- [x] 5.2 Correct `docs/development/UseCases.md` UC-010 and UC-021: the item and its medium are deleted, the backing
      file row only when the caller may use the file, and the stored content never; verify the wording matches
      `REQ-GALLERY-010` and `library/media-ingest`, and the Markdown gate passes.
- [x] 5.3 Label the gallery application tests in
      `use_case/{create_gallery,list_galleries,get_gallery,delete_gallery,     add_gallery_item,get_gallery_item,delete_gallery_item}.rs`
      with the `allure.epic`/`feature`/`story` of the requirement each uniquely proves, and delete or reduce a case that
      only re-proves a system scenario per `TEST-003`; verify `cargo test gallery` passes.

## 6. Integration gates

- [x] 6.1 Run `cargo test` and verify the unit and integration suites pass.
- [x] 6.2 Run `cargo clippy --all-targets --all-features -- -D warnings`, `cargo clippy --all-targets -- -D warnings`
      and `cargo fmt --all -- --check`; verify all three are clean. The two pre-existing violations in
      `handler/asset.rs` and `handler/asset/get_file_content.rs` were fixed when `download` landed.
- [x] 6.3 Run
      `cargo llvm-cov --lib --ignore-filename-regex 'src/lib/infrastructure/inbound/rest/handler' --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80`
      and verify the coverage gate holds.
- [ ] 6.4 Run `script/rule_enforcer.sh check` and verify every registered rule passes, including the `TEST-039` E2E
      mapping after the seven-module reduction. **Waived by the operator for this session.**

## Validation record

- `openspec validate specify-gallery --strict` passes.
- `specs/gallery/spec.md` carries 10 requirements (`REQ-GALLERY-001`–`008`, `010`, `011`) and 36 `SCN-GALLERY-*`
  scenarios; every requirement has at least one scenario and every scenario heading carries four hashtags.
- Action map: create `CreateChild` on `GALLERY_COLLECTION`; browse `Discover`; read `View`; delete `Delete`; add item
  `CreateChild` on the gallery. The who-may rule stays with the `authorization` capability.
- System module: 20 cases in `test/system/gallery.py` (collected twice only when the file is named directly, because
  `test/system/conftest.py` also collects it; the directory run collects it once).
- Verification: `cargo test` (1568 passed), `cargo llvm-cov --lib ...` (88.17 % functions, 88.69 % regions, 93.43 %
  lines), `cargo fmt --all -- --check` clean, `pytest -q test/system/gallery.py` (40 passed, 20 cases run twice by the
  direct-path collection), `prettier --check` and `markdownlint-cli2 '**/*.md'` clean.

## Workflow follow-up

- Archive the change after review, then verify the `gallery` main spec carries its `## Purpose` and the seven E2E
  modules still map to their catalogued use cases.
