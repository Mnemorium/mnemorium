# Tasks

## 1. File resource

- [x] 1.1 Add `GET /api/v1/asset/file/{file_id}` returning the file representation with `_links.self`, `_links.owner`
      and the media link, and verify the handler test covers a file with and without an image row.
- [x] 1.2 Add the file read to the repository path and resolve `_links.image` through `ImageFilter`, and verify a
      file-backed image reports the link and a plain file does not.

## 2. Image resource

- [x] 2.1 Add `GET /api/v1/asset/image/{image_id}` returning the image representation with `file_id`, `_links.self` and
      `_links.file`, and verify the handler test covers an existing and an unknown image.
- [x] 2.2 Add `GET /api/v1/asset/image/{image_id}/file` as a `303 See Other` to the file resource, and verify the
      `Location` header names the canonical file URI.

## 3. Content

- [x] 3.1 Add `GET /api/v1/asset/file/{file_id}/content` and `GET /api/v1/asset/image/{image_id}/content`, streaming the
      stored bytes with the stored media type, and verify the response body matches the file and the `Content-Type` is
      the image type.
- [x] 3.2 Verify a content request for an unreadable asset returns `404` with no bytes.

## 4. Authorization

- [x] 4.1 Route every asset route through the delegated decision from change 4, and verify a private gallery image is
      `404` for a non-owner and `200` for the owner.
- [x] 4.2 Verify the alias and the content route apply the same decision as the metadata route.

## 5. Upload link and documentation

- [x] 5.1 Replace `file_id` with `_links.file` in the upload-completion response, and verify the handler test asserts
      the link and the absence of the raw identifier.
- [x] 5.2 Regenerate `docs/openapi.json` with `cargo run --bin openapi_gen` and commit the changed asset and upload
      responses.
- [x] 5.3 Run the Markdown and OpenAPI checks and verify they pass.

## 6. Integration verification

- [x] 6.1 Verify `upload -> file -> image -> image_id` resolves for an uploaded image and that the image id can then be
      used as `cover_image_id`.
- [x] 6.2 Run the full gate (`cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`,
      `cargo fmt --all -- --check`, `script/rule_enforcer.sh check`) and verify it passes.
