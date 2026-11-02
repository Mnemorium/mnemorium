# Design

## Context

Covers are stored as `cover_file_id` on five tables, each guarded by a pair of triggers that reads the file's mime type.
The `image` table already holds the probed image row for a file, and `gallery_item` references it. The schema is
unreleased (`PERS-014`). See `proposal.md` for motivation.

## Goals / Non-Goals

**Goals:**

- Make "this is an image" a foreign-key guarantee, not a trigger.
- Keep cover art and gallery media disjoint.
- Make the visibility of a user-owned image unambiguous.

**Non-Goals:**

- The Asset API and the image content bytes (change 5).
- Automatic artwork collection.
- Renaming the generic noun "cover".

## Decisions

### A scope on the asset, not a separate cover table

`image.scope` and `video.scope` are `LIBRARY` or `USER`, nullable until first attachment and immutable afterwards. A
separate `cover_art` table would duplicate the probe columns and force a second asset endpoint for the same bytes.

Alternative considered: a discriminator on the referrer only. Rejected because the asset row is what the Asset endpoint
reads; without a scope it cannot tell which rule applies.

### Set the scope at first attachment

An uploaded image is unattached (`NULL`). The first attachment decides: a catalogue cover sets `LIBRARY`; a gallery item
or a playlist cover sets `USER`. A later attempt in the other direction is a conflict. This avoids changing the upload
API and makes "cover art never enters a gallery" a property of the data.

### One referrer for `USER`, many for `LIBRARY`

A `USER` image or video belongs to exactly one gallery item or one playlist cover; a unique index and a trigger enforce
it. `LIBRARY` art may be shared by several albums because the catalogue is global and the art is public. The single
referrer is what makes the later asset authorization (`file.read` delegation) deterministic.

### The FK to `image` replaces the mime triggers

`cover_image_id REFERENCES image(image_id)` is enough: an `image` row only exists for a probed image file. The five
`require_image_cover_*` pairs are deleted, and with them that `RAISE(ABORT)` path.

### Edit the create migrations in place

`PERS-014` allows it. The alternative, five `ALTER TABLE` migrations, would leave the create migrations lying about the
schema. A development database is reset after the edit.

## Risks / Trade-offs

- **Existing covers hold file ids** → the schema is unreleased; reset the development database. No backfill is needed.
- **Sharing library art across albums** → allowed, and the visibility is the catalogue's, not one album's. If a single
  album reference is required later, a unique index can be added then.
- **Deleting an image used as a cover** → the cross-context foreign key refuses it (`ARCH-004`); the application must
  clear the cover first.
- **`_links.cover` points at a route that does not exist yet** → the existing `TODO(add-cover-art)` becomes
  `TODO(image-link)`; change 5 serves it.

## Migration Plan

1. Edit the create migrations; reset the development database per `PERS-014`.
2. Verify a cover can only reference an image and that a gallery item refuses a `LIBRARY` image.
3. Rollback by reverting the migrations and resetting the database.

## Open Questions

- Should a `LIBRARY` image be reusable by an unlimited number of covers, or limited to one? The design allows reuse; the
  catalogue is global and the art is public.
