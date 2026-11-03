# Design

## Context

Change 6 adds `music_group.cover_image_id` and `music_album.music_group_id` and seeds group 0. Change 5 serves the image
resource a cover link needs. Change 2 supplies `library.manage`. See `proposal.md` for motivation.

## Goals / Non-Goals

**Goals:**

- A group is addressable and an album can name it.
- An administrator can create a group and attach an image.
- The default group is always present and cannot be changed.

**Non-Goals:**

- Automatic metadata or Wikipedia artwork (change 12).
- Deleting a group; an album references it.
- Person membership management.

## Decisions

### A group is a top-level library resource

`/library/music/group` sits beside album and playlist. Reads use `library.read`; writes use `library.manage`, the same
admin-only action the album write will use.

### The album links its group

The album representation exposes `_links.group` and no `group_id`, following the link rule. The album input takes
`group_id`, because the client creates or finds the group first; the server does not guess a group from a name.

### Name is the identity

`music_group.stage_name` is unique. The write trims the name, rejects an empty value and a duplicate, and leaves the
stored value as the display name. The default group 0 is protected by the trigger from change 6, so it cannot be renamed
or deleted.

### Cover is an image identifier

`cover_image_id` follows the cover contract from change 3: an image with the `LIBRARY` scope, a `409` on a `USER` image,
and `_links.cover` to the Asset image route.

## Risks / Trade-offs

- **Case-sensitive uniqueness** → "Miles Davis" and "miles davis" can both exist. A case-insensitive unique index is a
  follow-up if the product wants it.
- **No delete** → a group with albums cannot be removed; that is intentional for now.
- **The default group is listed** → the client can resolve group 0 like any other; it cannot edit it.

## Migration Plan

No new schema beyond change 6. Ship the handlers and regenerate `docs/openapi.json`.

## Open Questions

- Should the group name be unique case-insensitively? The design leaves it case-sensitive.
