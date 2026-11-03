# Tasks

## 1. Repository and model

- [x] 1.1 Add the group repository with search (name, limit, offset), get, create and update, and verify the focused
      repository tests round-trip name and cover.
- [x] 1.2 Enforce the trimmed non-empty name and the unique name in the create and update paths, and verify a duplicate
      maps to `Conflict`.

## 2. Use cases

- [x] 2.1 Add list, get, create and update group use cases with `library.read` and `library.manage`, and verify the
      authorization and validation tests.
- [x] 2.2 Add the factory methods and verify the mock factory builds each use case.

## 3. HTTP

- [x] 3.1 Add `GET /library/music/group`, `GET /library/music/group/{group_id}`, `POST /library/music/group` and
      `PATCH /library/music/group/{group_id}`, and verify the handler tests cover list, read, create, rename, duplicate
      and forbidden.
- [x] 3.2 Add `_links.group` to the album representation, and verify an album with a group exposes it and one without a
      group cannot exist.
- [x] 3.3 Verify the cover link uses `cover_image_id` and a `USER` image is rejected with `409`.

## 4. Documentation

- [x] 4.1 Verify the generated OpenAPI document declares the group routes and the album group link (the static
      `docs/openapi.json` was removed in #386; the document is generated from the handlers at runtime).
- [x] 4.2 Run the Markdown and OpenAPI checks and verify they pass.

## 5. Integration verification

- [x] 5.1 Verify group 0 is readable and cannot be renamed, and that a created group can be used as an album's group.
- [x] 5.2 Run the full gate (`cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`,
      `cargo fmt --all -- --check`, `script/rule_enforcer.sh check`) and verify it passes.
