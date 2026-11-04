# Proposal

## Why

The music catalogue is read-only: nothing can create an album, place a medium, attach a recording or apply external
metadata. The read endpoints therefore return nothing in practice. This change adds the write surface the preview,
manual and auto-apply modes need.

## What Changes

- `POST /api/v1/library/music/album` creates an album for `(name, group_id)`, returning the existing album when the
  identity is already present.
- `PATCH /api/v1/library/music/album/{album_id}` updates album metadata; a `null` clears an optional value.
- `PUT /api/v1/library/music/album/{album_id}/medium/{medium_index}` creates or retypes a medium.
- `PUT /api/v1/library/music/album/{album_id}/medium/{medium_index}/track/{track_index}` places or moves a recording.
- `PATCH /api/v1/library/music/album/{album_id}/medium/{medium_index}/track/{track_index}` edits the shared recording's
  name and ISRC.
- `POST /api/v1/library/music/album/{album_id}/apply` resolves external metadata and writes the album and its medium
  structure, with optional track placements.
- A release with more than one compact-disc medium becomes separate `CD` mediums; every other release becomes one
  `DIGITAL` medium.
- Album apply is album-and-medium only; a recording moves only when the request names its placement.
- Guard every write with `library.manage`.
- Out of scope: the full library scan and the creation of default recordings at ingest (change 10).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library/music-albums`: adds the album, medium and track write contracts and the apply contract.

## Impact

- New handlers and use cases for album, medium and track writes, plus a repository mutation path.
- `docs/openapi.json` is regenerated.
- The album and track representations gain the write responses already used by the read endpoints.
