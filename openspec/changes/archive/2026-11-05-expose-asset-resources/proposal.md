# Proposal

## Why

Every album cover and track representation will expose a link, but no Asset endpoint serves the file or the typed image
behind it. An upload also returns a `file_id` without a way to reach the image row, so a client cannot name a cover even
when it has just uploaded one.

## What Changes

- Add `GET /api/v1/asset/file/{file_id}` and `GET /api/v1/asset/file/{file_id}/content`.
- Add `GET /api/v1/asset/image/{image_id}` and `GET /api/v1/asset/image/{image_id}/content`.
- Add `GET /api/v1/asset/image/{image_id}/file` as a `303 See Other` alias to the file resource.
- Report `_links.image`, `_links.audio` and `_links.video` on the file representation when the corresponding media row
  exists, so `upload -> file -> image` resolves the typed asset.
- Report `file_id` and `_links.file` on the image representation.
- Replace the upload-completion response's `file_id` with `_links.file`.
- Authorize every asset read through the delegation from change 4; a denied read is `404 Not Found`.
- Out of scope: `audio` and `video` asset endpoints, and asset write or delete endpoints.

## Capabilities

### New Capabilities

- `library/assets`: serves stored files, their typed image view and their content, and links an upload to its file.

### Modified Capabilities

None.

## Impact

- New handlers under `src/lib/infrastructure/inbound/rest/handler/asset/` and their routes.
- `File`, `Image` and their repositories gain the read path they need.
- The upload-completion response and its tests.
- `docs/openapi.json` is regenerated.
