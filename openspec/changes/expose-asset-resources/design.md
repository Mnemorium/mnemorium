# Design

## Context

The Asset routes currently serve only upload sessions. `Image` already carries `file_id`, `name`, dimensions,
orientation and creation date; `File` carries the path, media type, size, timestamps and origin. Change 4 supplies the
delegated authorization. See `proposal.md` for motivation.

## Goals / Non-Goals

**Goals:**

- Resolve a file or image by identifier, with links in both directions.
- Serve the bytes behind a cover link.
- Make the upload-complete response lead to the image id without a new upload field.

**Non-Goals:**

- Audio and video endpoints, and asset write/delete.
- Range requests, transcoding or caching. Content is served whole.

## Decisions

### One file resource, typed views over it

`/asset/file/{id}` is canonical. `/asset/image/{id}` is a typed view carrying `file_id`, and `/asset/image/{id}/file` is
a `303 See Other` alias. Returning a 200 copy of the file at the alias would give one resource two identities.

### Reverse links on the file

The file representation reports `_links.image` (and later `_links.audio`/`_links.video`) when a media row exists for the
file. `ImageFilter` already supports filtering by `file_id`, so this is one lookup. It turns `upload -> file -> image`
into a navigable chain without changing the upload request schema.

### Content is a sub-resource

`/asset/file/{id}/content` and `/asset/image/{id}/content` serve the bytes with the stored media type. The metadata
routes stay JSON, so `_links.cover` can point at the content route a browser or image client can consume directly.

### Authorization is the delegated asset read

Every route resolves the file, then the asset decision from change 4. A denial is `404 Not Found` and no bytes. The
image route resolves its `file_id` first; the alias resolves the file and redirects so the client continues under the
canonical resource.

### Upload completion links the file

`PostUploadCompleteResponse` replaces `file_id` with `_links.file`. The client follows the link to `/asset/file/{id}`,
then `_links.image` for an image upload. No `image_id` is added to the upload schema.

## Risks / Trade-offs

- **A broken `_links.image` for a non-image file** → the link is omitted unless the media row exists.
- **Content is served whole** → acceptable for artwork; a later streaming change can add range support.
- **A file path is relative to the library root** → the storage port resolves it; the representation never exposes an
  absolute path.
- **The alias adds a redirect hop** → clients follow it; the alternative duplicates a representation.

## Migration Plan

No datastore change. Ship the handlers and the upload-response link; regenerate `docs/openapi.json`.

## Open Questions

None.
