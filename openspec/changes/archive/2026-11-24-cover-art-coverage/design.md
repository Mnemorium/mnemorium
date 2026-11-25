# Design

## Removing `SCN-COVER-ART-004-C`

The scenario asks that a representation keep `_links.cover` when the image route "is not served", and that following the
link answer the error envelope. Every image a cover names exists — `fk_<table>_cover_image` forbids the alternative —
and the image route is registered on every build. The only caller for whom the route is not served is one that may not
view the image, which is exactly the state `REQ-AUTHORIZATION-025-A` ("catalogue artwork follows its resource") says
must not occur. A black-box case written against today's behaviour would therefore enshrine a defect: an uploaded cover
asset is never re-parented to the container that references it, so every non-owner reader of a public album receives a
link that answers `404`.

The link-exposure half of `REQ-COVER-ART-004` is already proven by `004-A` (present when set) and `004-B` (absent when
null). The unserved-route half is dropped rather than moved: it is a consequence of the authorization model, not a
cover-art rule, and the authorization capability owns that model.

## One referrer: `USER` only

`REQ-COVER-ART-003` states a universal "at most one referrer". The datastore does not enforce it for `LIBRARY` images
and a committed case relies on the opposite: `test/system/music.py:781` reads an album whose cover is image `12603`, and
`:957` attaches that same image to a group and expects `200`. The authorization backfill in
`20261111090000_create_authorization_resource.up.sql` also assumes a `LIBRARY` image is reachable through a single
container, so sharing artwork across catalogue rows is coherent with the model.

The one-referrer rule that carries meaning is the `USER` one: a `USER` image belongs to exactly one gallery item or
playlist cover, and the schema already enforces it (`uq_gallery_item_image_id`, `uq_music_playlist_cover_image_id`, and
the cross-table triggers). The requirement is narrowed to that rule and a new scenario proves the second-playlist
direction the suite never exercised.

## Target-side readability

The gallery capability already models the rule: `add_gallery_item` resolves the referenced media, resolves its backing
file and requires `View` on it (`src/lib/application/use_case/add_gallery_item.rs`). Cover attachment had no equivalent,
so a `USER` image with no scope yet could be claimed by any authenticated playlist creator who guessed its identifier,
and a catalogue writer could claim it as artwork.

The check is implemented once, as an application service helper, and called by every use case that writes a cover. It
resolves the image by identifier, then its backing file, then authorizes `View` on the file resource — the same check
`get_image` uses, so "may attach" and "may read the artwork" cannot drift. A missing image and an unreadable image both
fail the check.

`View` (not `ViewMetadata`) is chosen because the client must be able to render the artwork; a caller who may only read
metadata could not display what it attaches.

### Status

The refusal is `409 Conflict`, reusing the existing cover-conflict variant of every use case. `422` was rejected: an
unknown cover already answers `409` through the foreign key, and answering `422` for an image that exists but is
unreadable would let a caller distinguish the two, violating `REQ-API-013` ("a refusal does not disclose existence").
`403` was rejected because the addressed resource is writable; only the reference is refused, and the cover conflict is
already the declared outcome on all twelve endpoints.

### Where it runs

In the application layer, after the container authorization and before the row is written, in the caller's unit of work.
The handler layer stays a translation layer; the repository layer has no caller identity.

## Playlist cover uniqueness

Attaching an image that another playlist already backs currently surfaces through `uq_music_playlist_cover_image_id` as
`RepositoryError::AlreadyExist`, which both playlist use cases map to `NameAlreadyExists` — a conflict with the wrong
cause and a misleading message. The use case searches the playlist collection by `cover_image_id` before writing and
returns `InvalidCover`, so the refusal is honest and the unique index stays as the concurrent-write guard.

## Rejected alternatives

- **Enforce the universal one-referrer rule with a migration.** Rejected: it breaks a deliberate shared-artwork
  behaviour and would make a show unable to reuse its own poster.
- **Write `SCN-COVER-ART-004-C` against today's non-owner `404`.** Rejected: it contradicts `REQ-AUTHORIZATION-025-A`
  and would have to be deleted when that gap is fixed.
- **Check readability in the handler.** Rejected: authorization belongs to the application layer, and the handler does
  not own the caller's unit of work.
- **Answer `422` for an unreadable image.** Rejected: it creates an existence oracle against the `409` an unknown cover
  answers.
