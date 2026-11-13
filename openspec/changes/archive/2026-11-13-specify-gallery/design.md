# Design

## Context

See `proposal.md` — Why. The constraints that shape the approach:

- Gallery is served by
  `handler/library/{post_gallery,get_gallery_list,get_gallery,delete_gallery,post_gallery_item, get_gallery_item_list,get_gallery_item,delete_gallery_item}.rs`,
  implemented by the matching `use_case/` modules and persisted by `gallery` and `gallery_item`.
- `authorization` already models `GALLERY`, `GALLERY_ITEM` and the `GALLERY_COLLECTION` surface. The seeded `STANDARD`
  and `ADMIN` role grants admit `CreateChild` with child kind `GALLERY`; a per-user or per-principal `Deny` can
  override.
- `create_gallery` fetches the caller and discards the value (`let _caller`), so the decision is never asked.
- `delete_gallery_item` intentionally keeps the bytes (`TODO(ingest)`): it may delete the file row when the caller may
  `View` the file, then drops the path without removing content. `library/media-ingest` states the same absolute rule.
- There is no `PATCH /gallery/{id}`; visibility changes through the permission resource route. The gallery collection is
  returned without pagination, and the item collection route answers the gallery read.
- `test/system/conftest.py` already carries `create_gallery` and `add_to_gallery` helpers, so the system module needs no
  new fixture.

## Goals / Non-Goals

**Goals:**

- One capability owning the gallery collection and its item collection.
- Code compliance with the `authorization` action for creation.
- One consistent deletion contract across the gallery and the ingest.
- A proof that the default permission still creates galleries and that a `Deny` refuses.

**Non-Goals:**

- Gallery artwork or a `PATCH` route; see the proposal's out-of-scope list.
- Renaming the `/api/v1/library/gallery` routes or the `library` handler module.
- Paginating the item collection or changing any representation field.
- Re-authoring the `image`/`video` entity contracts; the gallery names the medium and links its route.

## Decisions

### 1. Top-level `gallery`, and the routes do not move

A gallery is one cohesive behavior — an owned, ordered collection of visual media — with its own repository, use cases
and tests. The route prefix stays `/api/v1/library/gallery`, the same way `upload` kept `/api/v1/asset/upload` and
`music` kept `/api/v1/library`.

**Alternative considered:** `library/gallery`. Rejected: it makes the collection a child of a browsing area rather than
a capability, and the redesign is flat for single-owner areas.

### 2. Creation asks `CreateChild`, and a `Deny` answers `403`

`create_gallery` gains the same
`authorize(..., ResourceKind::GalleryCollection, Action::CreateChild, Some(ChildKind::Gallery))` call the other creation
use cases make. `CreateGalleryError::Forbidden` maps to `403`, the same outcome a denied film or show creation produces.

The alternative — leave the code as is and specify the unenforced behavior — would make the gallery the only creation
surface whose `Deny` is ignored. The seeded grant means ordinary callers see no change; only an explicit denial now
refuses.

### 3. `is_public` selects visibility and never a second permission model

The create request maps `is_public: true` to `Visibility::Public`, false to `Private`. The spec states what the
`authorization` capability already means by those values: `Public` grants the viewer actions; it grants no edit, delete
or share. It is not "moderate".

**Alternative considered:** keep the handler comment's "read and moderate" wording and treat `is_public` as a legacy
grant. Rejected: it contradicts `REQ-AUTHORIZATION` and the system suite's public-resource case.

### 4. Removal deletes rows, never bytes

The gallery and item removals delete the item and its media row; the file row is deleted only when the caller may `View`
the file, and the stored bytes are never touched. This is the code's current behavior and the ingest's absolute rule.
The use cases' "record and content disappear" wording is corrected rather than the code.

**Alternative considered:** restore byte deletion for uploaded files while sparing scanned ones. Rejected: it needs a
deletion policy over `file.origin`, which is a separate change; inventing it here would make the gallery spec the policy
owner.

### 5. Item reads carry the medium reference, not its characteristics

The item representation carries the medium identifier and type, a display name, the backing file identifier and a `self`
link. A client follows the medium identifier to `/asset/image/{id}` or `/asset/video/{id}`. The display name is the
image name or the video file path, both already stored.

**Alternative considered:** embed the image or video representation in the item. Rejected: it duplicates two entity
contracts and makes the gallery depend on their fields.

### 6. The item collection route is covered by the gallery read

`GET /gallery/{id}/item` answers the gallery read. It is not a separate requirement: its only unique clauses were the
HAL self link and the no-pagination rule, and the capability states neither. `REQ-GALLERY-005` owns the representation
the route returns, and the route surface stays as it is.

**Alternative considered:** keep a separate alias requirement. Rejected: it restated the read and existed mainly to pin
a HAL detail the specification no longer owns.

## Risks / Trade-offs

- **The create fix can surface an existing test that relied on an ignored denial** → the application test for the denied
  case is written first, and the E2E create flow keeps asserting the allowed path.
- **The permission API may not address `GALLERY_COLLECTION` from the system suite** → prove the denial in the
  application test with the real authorization engine; the system module proves the allowed path and the `Public` read
  behavior.
- **The deletion wording change can be read as a downgrade** → the spec and the corrected use cases state the same rule
  as `library/media-ingest`; no bytes are lost that were not already kept.
- **The item collection route can be mistaken for a collection with its own rules** → the design records that it answers
  the gallery read and that `REQ-GALLERY-005` owns the representation it returns.

## Migration Plan

No data migration. The change is spec-first plus one application authorization call and its error mapping; rollback is
the reverse archive and the reversed `create_gallery` hunk.

## Open Questions

None. Gallery artwork and a gallery `PATCH` are deferred whole; they are not needed to specify the shipped behavior.
