# Tasks

## 1. Schema

- [x] 1.1 Add `image.scope` and `video.scope` with the `LIBRARY`/`USER` check to their create migrations, and verify a
      row with another value is rejected.
- [x] 1.2 Change `cover_file_id` to `cover_image_id` referencing `image` on `music_album`, `music_playlist`, `movie`,
      `series` and `season`, and verify a cover can reference an image but not a missing image.
- [x] 1.3 Delete the five `require_image_cover_*` migration pairs, and verify `rg "require_image_cover" migrations`
      returns nothing.
- [x] 1.4 Add the scope triggers: a gallery item accepts only `NULL` or `USER`, a library cover accepts only `NULL` or
      `LIBRARY`, a playlist cover accepts only `NULL` or `USER`, and a mismatch aborts. Verify each mismatch maps to
      `Conflict`.
- [x] 1.5 Add the single-referrer constraint for `USER` assets and verify a second gallery item or playlist cover for
      the same image is rejected.

## 2. Domain and adapters

- [x] 2.1 Rename `cover_file_id` to `cover_image_id` in the album, playlist, movie, series and season models and their
      repositories, and verify the focused repository tests pass.
- [x] 2.2 Add `scope` to the `Image` and `Video` models and adapters, and verify reading a stored scope round-trips.
- [x] 2.3 Update the ERD and the trigger notes in `docs/development/TechnicalDesign.md`, and verify `PERS-011` is
      satisfied by the diagram matching the migrations.

## 3. HTTP representations

- [x] 3.1 Rename the request and response field to `cover_image_id`, and verify each affected handler test asserts the
      new field.
- [x] 3.2 Point `_links.cover` at the Asset image route with `TODO(image-link)`, and verify a representation with a
      cover exposes the link and one without exposes none.

## 4. Specifications and documentation

- [x] 4.1 Update `library/cover-art` and the four read specifications to the image identifier and the new link, and
      verify `rg "cover_file_id" openspec/specs` returns nothing.
- [x] 4.2 Run the Markdown gates over the changed documents and verify they pass.

## 5. Integration verification

- [x] 5.1 Verify a cover write with a `USER` image and a gallery add with a `LIBRARY` image both return `409 Conflict`.
- [x] 5.2 Verify a library cover may be shared by two albums and a `USER` image may not have two referrers.
- [x] 5.3 Run the full gate (`cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`,
      `cargo fmt --all -- --check`, `script/rule_enforcer.sh check`) and verify it passes.
