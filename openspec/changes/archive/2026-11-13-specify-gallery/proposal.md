# Proposal

## Why

Gallery is implemented end to end — creation, browse, read, deletion and an ordered image/video item collection — and
`authorization` already models `GALLERY` and `GALLERY_ITEM`, yet no capability describes it. The rules live in
`docs/development/UseCases.md` and handler comments, and three of them have drifted:

- `create_gallery` performs no `CreateChild` check, so a `Deny` on `GALLERY_COLLECTION` does not prevent creation; the
  seeded grant admits every Standard user, which hides the omission.
- The create route documents `is_public` as "read and moderate", but `Public` visibility grants the `Viewer` actions
  only; moderation needs a grant.
- The removal use cases promise the backing file's record _and_ stored content disappear, while `library/media-ingest`
  and the code keep the bytes and remove the file row only when the caller may view the file.
- An item read is described as carrying "media metadata", but it carries only the medium identifier, its type, a display
  name and the backing file identifier; the client follows the typed route.

Gallery carries no artwork. Adding an optional cover would need a new column, trigger pairs, a unique index and a PATCH
route the server does not have, so it is explicitly out of scope.

## What Changes

- Add a `gallery` capability with `REQ-GALLERY-*`/`SCN-GALLERY-*` identifiers, owning the collection, the seeded default
  gallery, creation, browse, read, deletion, item addition and removal, ordering and the last-modified time.
- Require `CreateChild` on `GALLERY_COLLECTION` when a gallery is created, so `authorization` owns who may create and a
  denied caller answers `403 Forbidden`. **BREAKING** only for a caller whose explicit `Deny` previously went ignored.
- State what `is_public` means: it selects `Public` or `Private` visibility, and `Public` grants the `Viewer` actions,
  not moderation.
- Align removal with `library/media-ingest`: the item and its media row are deleted; the backing file row is deleted
  only when the caller may `View` it; stored bytes are never touched. Update UC-010 and UC-021 to match.
- State the item representation as the medium identifier plus type, display name and backing file identifier; the typed
  characteristics stay with the `image` and `video` capabilities.
- No route, payload or status change for an allowed caller. **BREAKING**: none for the default permission.

## Capabilities

### New Capabilities

- `gallery`: an owned, ordered collection of image and video items — the seeded default gallery, creation, browse, read,
  deletion, item addition and removal, ordering, the last-modified time and the file/byte removal policy.

### Modified Capabilities

None. The `CreateChild` fix makes the code obey `authorization`, whose requirements already state the action; no
authorization delta is needed.

## Impact

- **Specs**: one capability is created.
- **API**: no route, payload or status change for an allowed caller. A caller denied `CreateChild` now receives `403`
  where creation previously succeeded.
- **Code**: `create_gallery` gains the `CreateChild` check on `GALLERY_COLLECTION`; `CreateGalleryError` gains a
  `Forbidden` variant and the handler maps it to `403`; the gallery handler doc comments stop naming the removed
  `gallery.manage` model and stop promising moderation from `Public`.
- **Tests**: `test/system/gallery.py` becomes the black-box proof; the seven E2E modules UC-007 to UC-011, UC-020 and
  UC-021 keep only their business flow; the gallery application tests gain the three Allure labels and a denied-create
  case. No new fixture is needed.
- **Docs**: `docs/development/UseCases.md` UC-010 and UC-021 stop promising content deletion; `TechnicalDesign.md` § 5
  names `gallery.py`; the glossary gains **Gallery**, **Gallery item** and **Default gallery**.
- **Sequencing**: independent. The capability cites the `image` and `video` routes without requiring
  `specify-typed-assets`, and it has no cover-art delta.

## Out of Scope

- Gallery artwork: no cover column, no `PATCH /gallery/{id}`, no `cover-art` delta.
- Renaming a gallery or changing its visibility through a gallery route; visibility stays with the permission resource
  route.
- Paginating the gallery collection or its items.
