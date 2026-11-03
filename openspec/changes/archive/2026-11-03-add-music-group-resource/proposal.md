# Proposal

## Why

The album now references a music group, and the group carries a cover, but there is no way to read, create or rename
one. A client needs the group resource to build an album and to attach artwork; without it the album can only name an
identifier it cannot resolve.

## What Changes

- Add `GET /api/v1/library/music/group` with `name`, `limit` and `offset`, and
  `GET /api/v1/library/music/group/{group_id}`.
- Add `POST /api/v1/library/music/group` and `PATCH /api/v1/library/music/group/{group_id}` for administrators.
- Report `id`, `name`, `cover_image_id` and `_links.self` / `_links.cover` on the group representation.
- Report `_links.group` on an album representation, and accept `group_id` when an album is created (change 9).
- Guard reads with `library.read` and writes with `library.manage`.
- Keep the seeded group 0 (`Unknown`) readable and protected from writes.
- Out of scope: automatic group metadata or artwork collection (change 12), and deleting a group.

## Capabilities

### New Capabilities

- `library/music-groups`: reads, creates and updates music groups and lets an album name the group it belongs to.

### Modified Capabilities

None.

## Impact

- A group repository, use cases, handlers and routes.
- The album representation gains `_links.group`.
- `docs/openapi.json` is regenerated.
