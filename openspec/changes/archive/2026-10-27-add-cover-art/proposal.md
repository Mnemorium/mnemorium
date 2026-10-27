# Proposal

## Why

Every library resource the server exposes -- films, shows, seasons, music albums and music playlists -- is browsable by
name and metadata only. None of the five carries artwork, so no client can render the library grid or a detail page a
user recognises a title by, and the API has no field through which artwork could ever arrive.

The datastore also has no column for it: `movie`, `series`, `season`, `music_album` and `music_playlist` reference no
image. The `image` table cannot serve this need without cost -- it is gallery media, its `file_id` is unique, and a
cover that reused it could never also be a gallery photograph. A single nullable file reference per resource is the
smallest shape that gives every resource artwork without touching the gallery model.

## What Changes

- **Schema.** `movie`, `series`, `season`, `music_album` and `music_playlist` each gain a nullable `cover_file_id`
  referencing `file (file_id)`. Cover art is optional everywhere: a resource without artwork stores `NULL` and reports
  no cover.
- **Migrations edited in place.** Under `PERS-014` the five `create_*` migrations gain the column; no new migration file
  and no `ALTER TABLE` are introduced. The entity-relationship diagram is updated in the same change (`PERS-011`).
- **Cross-context reference.** `file` belongs to the Asset context, so the reference declares no `ON DELETE CASCADE` and
  the application orchestrates any deletion (`ARCH-004`, `PERS-020`).
- **Image-only invariant.** A cover SHALL reference a file whose `mime_type.type` is `IMAGE`. Insert and update triggers
  on each of the five tables raise `ABORT` otherwise; `PERS-017` already maps that extended result code to
  `RepositoryError::Conflict`, so no new error plumbing is needed.
- **Representations.** Movie, series, album and playlist list items and detail reads carry `cover_file_id`, as do the
  season representation and the season summaries embedded in a series read. The value is `null` when unset.
- **HAL.** A representation whose `cover_file_id` is set exposes `_links.cover` naming the Asset file endpoint,
  `/api/v1/asset/image/{file_id}`. The link is published before that route is served, because a client that has to build
  the URI itself violates `API-037`; a missing route is not a reason to omit the link.
- **No inherited cover.** The server reports the cover each resource stores and never resolves a season's cover from its
  series. Choosing a fallback is the client's decision, and the season representation already carries `series_id` and
  `_links.series` for that purpose.
- **Documentation policy.** The Technical Design HAL section stops qualifying the file link with "once a file endpoint
  exists" and states the positive policy: a representation carries a referenced file's `file_id` **and** links it.
- **OpenAPI.** `docs/openapi.json` is regenerated from the annotated handlers.

**BREAKING**: none. `cover_file_id` is a new optional field, and `_links.cover` appears only where a cover exists.

## Capabilities

### New Capabilities

- `library/cover-art`: the cross-resource rules that make cover art one behaviour rather than five unrelated fields --
  optional artwork on a library resource, the image-only datastore invariant, the file link a cover exposes, and the
  rule that the server never inherits a cover from a parent resource.

### Modified Capabilities

- `library/movies`: the movie representation, in a listing and on a detail read, carries the film's cover.
- `library/series`: the series representation carries the show's cover, the season representation carries the season's
  cover, and the series listing stops being identifier-and-name only.
- `library/music-albums`: the album representation, in a listing and on a detail read, carries the album's cover.
- `library/music-playlists`: the playlist representation, in a listing and on a detail read, carries the playlist's
  cover.

## Impact

- **Schema:** five `create_*` migrations edited in place, five trigger migrations, and the ER diagram and partial-index
  prose in `docs/development/TechnicalDesign.md`.
- **Domain models:** `Movie`, `Series`, `Season`, `MusicAlbum` and `MusicPlaylist` each carry an optional cover file
  identifier.
- **Ports and adapters:** the five read queries project the new column; no new port, repository or unit-of-work accessor
  is required, because the reference resolves through the existing `file` foreign key.
- **Application:** the list and detail use cases for movies, series, albums and playlists pass the cover through their
  response types; the season read carries it through the series aggregate.
- **REST:** five `_links` types gain an optional `cover` link, five response types gain `cover_file_id`, one shared href
  helper builds the Asset URI, and `docs/openapi.json` is regenerated.
- **Documentation:** the HAL file-link policy in `TechnicalDesign.md` and the ER diagram; no new business use case.
- **Tests:** handler assertions on exact payloads, adapter reads with and without a cover, and trigger behaviour on both
  paths. The 80% coverage gate still applies.

**Out of scope** (explicitly not in this change):

- The Asset fetch routes `/api/v1/asset/{audio,image,video}/{file_id}`. `_links.cover` names the image route before it
  is served; a client following it today receives the standard HAL `404` envelope.
- File deletion, orphaned cover files, and byte garbage collection. Cover references are cross-context, so deleting a
  `file` that is a cover stays the existing delete path's problem, which already tolerates an unknown reference.
- Every write path for a cover. No endpoint creates films, shows, seasons or albums, and playlist creation and update do
  not accept artwork, so covers are populated out of band until an ingestion path exists.
- Retro-fitting a file link onto the six `file_id` fields the rest of the API already carries (`GalleryItemResponse`,
  `GalleryItemDetailResponse`, `TrackResponse`, `VideoResponse`, `PlaylistTrackResponse`, `UploadSessionResponse`). They
  become non-conformant with the new Technical Design policy on the day it lands and are addressed separately;
  `MovieResponse` will therefore carry a linked cover beside an unlinked `video.file_id` until then.
