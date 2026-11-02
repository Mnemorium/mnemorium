# Design

## Context

Change 3 gives every image and video a `scope` (`LIBRARY` or `USER`) and makes a `USER` asset have exactly one referrer.
The permission system decides an action by evaluating its scope against a `Protected` resource. `file.read` currently
uses the file's own owner and `is_public`, which does not match the referrer. See `proposal.md` for motivation.

## Goals / Non-Goals

**Goals:**

- One decision per asset, taken on the resource that owns it.
- No visibility flag to maintain.
- A private gallery or playlist hides its artwork from a caller who knows the id.

**Non-Goals:**

- The Asset endpoints (change 5).
- Audio and video scopes (change 3 already adds the columns).
- Changing `gallery.read`, `playlist.read` or `library.read`.

## Decisions

### Delegate rather than copy the scope

`FileRead` becomes derived: `is_configurable() == false`, no `role_scope`, no `resource_kinds`, and
`Permission::try_new` rejects an override naming it. The asset use case resolves:

```
scope LIBRARY -> Authorization::decide(caller, permissions, LibraryRead, &Global)
scope USER    -> gallery item  -> GalleryRead on the gallery
                 playlist cover -> PlaylistRead on the playlist
```

Alternative considered: give `file.read` a `PUBLIC_OR_OWNED` scope and maintain `file.is_public`. Rejected: it is the
duplicate rule this change removes, and it cannot express "the owner of the gallery" when the file owner differs.

### `Global` for library art

Library art is public, and the catalogue read defaults to `ALL`. A caller whose `library.read` is `NONE` cannot read
art; a `LISTED` caller can, because art is public and `LISTED` narrows catalogue metadata. This is documented rather
than resolved through the owning album, which would reintroduce a per-referrer lookup for shared art.

### No Admin bypass

An Admin sees assets exactly where its `library.read`, `gallery.read` and `playlist.read` allow. Only the Root Admin
short-circuits. This keeps the delegation exact.

### `file.use` remains the write action

Asset writes are owner-or-admin. `file.use` already has `OWNED` for both roles; change it to `ALL` for Admin so an admin
can delete or modify any file. The asset write endpoints arrive later and will consume it.

## Risks / Trade-offs

- **A `USER` asset with no referrer** → only its file owner and the Root Admin can read it; this is the safe default.
- **A caller with `library.read` `LISTED`** can read unlisted artwork → accepted; artwork is public and the catalogue
  metadata is what `LISTED` narrows.
- **A file owner that differs from the gallery owner** → the gallery owner may not read the bytes. This is an
  over-restriction, not a leak; it follows from delegating to the gallery and is documented.
- **`file.is_public` still exists** → it is no longer an authorization input; leaving it avoids a schema change now.

## Migration Plan

No datastore change. Ship the derived action with the code; existing overrides naming `file.read` are rejected and must
be removed (the schema is unreleased).

## Open Questions

None.
