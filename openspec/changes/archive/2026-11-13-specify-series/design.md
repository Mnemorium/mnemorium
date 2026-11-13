# Design

## Context

See `proposal.md` — Why. The constraints that shape the approach:

- After `specify-metadata-enrichment`, `library/series` holds fifteen requirements: authenticated reads, list,
  filter/page, read one show, index identity, read one season, list and read episodes, order, create show, update show,
  put/patch season, put/patch episode. The two apply requirements already moved.
- The hierarchy is `series` → `season` → `episode`; a season's identity is its index within the show, an episode's is
  its index within the season, and both are exposed as `id`. There is no separate season or episode surrogate in the
  representation.
- The writes are `POST /series`, `PATCH /series/{id}`, `PUT`/`PATCH` on a season and `PUT`/`PATCH` on an episode. A
  `PUT` creates at a free index (`201`) or replaces at an existing one (`200`); a `PATCH` updates partially.
- Authorization actions in the code: `Discover` per show when listing, `View` to read, `CreateChild` on the catalogue
  with `ChildKind::Series` to create a show, `View` then `EditMetadata` to update a show, `CreateChild` on the **show**
  with `ChildKind::Season` or `ChildKind::Episode` to write a season or episode, and `View` then `EditMetadata` on the
  show to update either.
- `cover-art` (from `specify-music`) owns the `cover_image_id` and `_links.cover` fields; `video` (from
  `specify-typed-assets`) owns the summary shape.
- The E2E modules for UC-023 and UC-027–029 cover the hierarchy and its writes; the system suite has no series helper.

## Goals / Non-Goals

**Goals:**

- One capability owning the show, season and episode contract with stable identifiers.
- The permission, artwork and video clauses referenced rather than copied.
- One black-box proof of every scenario, including the create-or-replace distinction and the show-scoped child write.

**Non-Goals:**

- Identification and provider application, owned by `metadata-enrichment`.
- A delete route for a show, season or episode.
- Changing routes, payloads, statuses or DTOs.
- Re-authoring the film area; `specify-movies` owns it.

## Decisions

### 1. One `series` capability for the three levels

The show, season and episode are one hierarchy with one address scheme and one write model; splitting them into three
capabilities would duplicate the identity, order and cover rules. This follows `music`, which owns album, medium,
recording, group and playlist in one capability.

**Alternative considered:** `season` and `episode` capabilities. Rejected: each would restate parent-scoped identity and
would have to reference the show for every action.

### 2. Actions replace permission prose; child writes are checked on the show

The old requirements say `library.read` and `library.manage`. The new ones name `Discover`, `View`, `CreateChild` and
`EditMetadata`. Season and episode writes are checked against the **show**, with the season or episode child kind, which
is what the use cases do; the spec states that scope so a reader does not assume the season resource is addressed.

No `authorization` delta is needed; its requirements already own the actions.

### 3. Artwork and video are referenced, not restated

A show and a season carry the cover `cover-art` defines; an episode carries the video summary `video` defines. The spec
does not list those fields. A change to the summary or the cover rule has one owner.

### 4. Index identity and the create-or-replace writes stay

A season's `id` is its index in the show; an episode's `id` is its index in the season. A `PUT` at a free index creates
and answers `201`; at an existing index it replaces the metadata and answers `200`, keeping the children for a season
and replacing the video for an episode only when a different one is named. A `PATCH` never changes the children or the
video.

**Alternative considered:** expose a surrogate season or episode identifier. Rejected: the route and the representation
already address both by index, and the identity requirement is the hierarchy's contract.

### 5. No code change beyond served doc comments and one concealed denial

The use cases and DTOs implement the stated behavior. The handler doc comments that name `library.manage`/`library.read`
are corrected to actions; the route surface and the document's contract — its paths, schemas, statuses, media types,
headers and links — do not change. The eleven corrected operation descriptions follow, because `utoipa` derives a path's
description from its doc comment when the attribute declares none.

The one exception is the season and episode `PUT` on an unknown show. Both use cases authorized the write before loading
the show, so the repository's concealed denial became `403` where `REQ-SERIES-010` and `REQ-SERIES-012` require `404`;
every other child write (`add_gallery_item`) and both sibling season updates already map a concealed denial to "no such
parent". `put_series_season` and `put_series_episode` now map it the same way, so an unknown show answers `404` and a
visible show the caller may not write answers `403`. Nothing else changes.

## Risks / Trade-offs

- **The capability references three in-flight capabilities** → author it after `metadata-enrichment`, `music` and
  `typed-assets`; the proposal names the sequence.
- **The apply requirements may already be gone** → the removal lists only the fifteen requirements `metadata-enrichment`
  leaves; the two apply headers are not removed.
- **The 32-bit index upper bound is hard to stage from the system suite** → staged there after all: a `PUT` at index
  `4294967295` addresses a season and an episode without any fixture, so `test/system/series.py` proves
  `SCN-SERIES-003-B` and no half of it moved to an application test.
- **The child write checks the show, not the season** → the spec states it and the system case asserts a denied season
  write on a show whose `CreateChild` refuses.

## Migration Plan

No data migration. Apply after the three capabilities it references; `openspec archive` deletes `library/series` and
creates `series`. Rollback is the reverse archive.

## Open Questions

None.
