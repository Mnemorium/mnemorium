# Design

## Context

The gallery slice under `src/lib/infrastructure/inbound/rest/handler/library/` is the working template: a
`GalleryRepository` port, a `LibraryUnitOfWork` view, one `port/<use_case>.rs` + `use_case/<use_case>.rs` pair per
operation, a `LibraryUseCaseFactory`, a runtime factory, and one handler file per endpoint with shared payloads in
`handler/library.rs`. See `proposal.md` — Why for the motivation.

Music is half-built and its layers disagree:

```text
                      DB schema    Rust domain     ports/repos   use cases   handlers
  gallery             complete     complete        complete      complete    complete
  music               complete     partial (*)     none          none        none

  (*) MusicAlbum / MusicMedium / MusicRecording / MusicPlaylist exist;
      there is no MusicTrack, and no ordering is represented.
```

**"Music" is neither a bounded context nor one aggregate:** it is a **sub-context of Library**, and it holds two
separate aggregate roots, `MusicAlbum` and `MusicPlaylist`. This change touches only the **`MusicAlbum` aggregate**:

```text
  Library (bounded context)
    `-- Music (sub-context)
          |-- MusicAlbum        AGGREGATE ROOT   <- this change
          |     `-- MusicMedium   entity
          |           `-- MusicTrack  entity, positioned by its medium
          |                 `-- MusicRecording (referenced)
          `-- MusicPlaylist     AGGREGATE ROOT   (out of scope)
                `-- MusicTrack  entity, positioned by its playlist
```

The `music_track` table backs both aggregates — `music_medium_id` for an album track, `music_playlist_id` for a playlist
track — and gives every row exactly one of the two. One `MusicTrack` entity models both sides, carrying the container it
is positioned by as a `MusicTrackPosition` (`Medium` or `Playlist`); this change reads only the album side through the
`MusicAlbumRepository`, which joins through the medium and so never returns a playlist row.

`docs/development/Overview.md` lists a `MusicTrack` class, and the `music_track` table is a first-class entity:

- `music_track(music_track_id PK, track_index, music_playlist_id NULL, music_medium_id NULL, music_recording_id NN)`
- The Rust models instead collapse tracks into `MusicPlaylist.tracks: Vec<MusicRecording>` /
  `MusicMedium.tracks: Vec<MusicRecording>`, losing both the track identity and its position.

Two schema/domain mismatches constrain the read model:

- `music_album` has no genre column: genres are a many-to-many join (`music_album_genre` → `genre`, `genre_id TEXT`),
  while `MusicAlbum.genre` is `Option<String>`.
- `music_medium` carries `UNIQUE (music_album_id)`, so an album holds at most one medium, while
  `MusicAlbum.mediums: Vec<MusicMedium>` implies many.

The project's existing precedent for reading data that is not a clean domain entity is the port-level projection
`GalleryItemDetail` (`domain/port/gallery_repository.rs`), returned by `search_items` and mapped to the response in the
handler.

## Goals / Non-Goals

**Goals:**

- Deliver the three album read endpoints on the existing Library slice, consistent with the gallery endpoints.
- Give music tracks a first-class identity and position in the domain so the endpoints are built on a correct model.
- Keep the change read-only: no schema migration, no write path.

**Non-Goals:**

- Hydrating the full `MusicAlbum` aggregate for responses (its mediums, tracks, genres, music groups). The read models
  carry only what the endpoints expose; reconciling the aggregate itself belongs to the change that first writes albums.
- Playlist endpoints, album detail by identifier, album/track upload, playlist track curation, visibility changes.
- Paginating the album's track collection (see Decisions).

## Decisions

### 1. The capability lives in Library, under its Music sub-context

Add to `LibraryUnitOfWork`, `LibraryUseCaseFactory`, `handler/library.rs`, `handler/library/`, and the `library` OpenAPI
tag. `Overview.md` defines Library as _"User's collection of media"_; Music is a sub-context of Library, not a bounded
context of its own, and the music models already sit in `domain/model/` beside gallery/media. `ARCH-002` requires each
model and port to belong to exactly one context — which today is none, and which this change settles as Library.

- **Alternative (rejected for now):** promote Music to its own bounded context with its own unit-of-work view, factory,
  tag and handler folder. Cleaner ownership for a large sub-context, but it adds a context and a factory to serve nine
  total endpoints, three of them in scope. Revisit when the playlist/mutation surface lands.

### 2. Add a `MusicTrack` entity with a container position

`domain/model/music_track.rs` carries the track identity, its `track_index`, its recording reference and a
`MusicTrackPosition` naming the container it is positioned by — a `MusicMedium` of a `MusicAlbum`, or a `MusicPlaylist`.
It needs no container of its own: the container owns its tracks. This closes the gap between `Overview.md` and the Rust
model, and it is what the two `/track/{track_id}` requirements need: the database gives a track its own
`music_track_id`, distinct from `music_recording_id`. Naming the position is also what lets the album-scoped track
filter declare a field the entity really has (`STY-RUST-080`).

- **Scope:** one entity models the shared `music_track` table's row, whose two nullable foreign keys give it exactly one
  container. This change reads only the album side: the `MusicAlbumRepository` joins through `music_medium`, so a
  playlist-positioned row is never returned. The playlist-side variant is exercised when the playlist endpoints land.
- **Alternative (rejected):** treat a track as its recording, ordered by result-set order. Loses the addressable
  `track_id` the endpoint path exposes and makes ordering an accident of the query, not a property of the model.
- **Alternative (rejected):** a distinct entity per aggregate. Two types over one table duplicate the identity, position
  and recording reference, and the shared row still gives each track exactly one container.

### 3. Query the `MusicAlbum` aggregate through port read models

The port is named after the aggregate — `MusicAlbumRepository`, in `domain/port/music_album_repository.rs`, adapted as
`SqlxMusicAlbumRepository` (§ 8.1–8.2, `STY-RUST-079`) — and it queries only that aggregate. Its methods follow the
canonical repository names: `search` for the album query, and `search_tracks` for the album's tracks, mirroring
`GalleryRepository::search` and `search_items` (§ 8.1). Both return dedicated read models (`MusicAlbumSummary`,
`MusicTrackDetail`) built by the SQLx adapter from joins (`STY-RUST-081`), mirroring `GalleryItemDetail`: a result
carrying data the aggregate does not declare — aggregated genres, a track count, the Asset-owned file identity, and the
identifier of the medium the album's tracks are positioned by — returns a read model rather than a partly-hydrated
aggregate. The port file declares the filters first, then the trait, then the read models (§ 8.3).

- **Alternative (considered):** rehydrate `MusicAlbum` with its `MusicMedium`s, each holding `MusicTrack`s. Faithful to
  the boundary, but the aggregate's `genre: Option<String>` cannot represent `music_album_genre`, and a listing needs an
  aggregate-wide track count; rehydrating drags a domain/schema reconciliation into a read-only change. Promote the
  aggregate when a write path needs it.
- **Alternative (rejected):** return raw rows from the repository. Leaks persistence shape outward and breaks the
  hexagonal direction.

### 4. Album access mirrors the rest of the Library context

Authorization follows the Library context's existing shape rather than a new scheme:

- The decision lives in the application layer, in `application/use_case/library_access.rs` — the module that already
  owns `load_caller` and `can_read_gallery` — never in the handler.
- Each album use case loads the authenticated caller through `library_access::load_caller` before reading, exactly as
  `ListGalleries` and `GetGallery` do, so a token naming a deleted account is rejected with `401` (`UC-005`).
- An album is always public-equivalent: it has no owner column and no visibility flag, so the predicate admits every
  authenticated caller. Read access is never denied, so the use cases carry no `Forbidden` variant and no `authz_failed`
  event is emitted; the only authorization outcome is `NoSuchCaller` → `401`.
- A track follows its album, mirroring how a gallery item follows its gallery.

- **Alternative (rejected):** a dedicated permission scheme (a permissions module, a policy engine, per-album grants).
  The Library context authorises through `library_access`; a second mechanism would duplicate it and drift.
- **Alternative (rejected):** skip the caller load. Saves one query but lets a deleted user's token keep reading, which
  contradicts the token-invalidation rule.

### 5. Normalise the endpoint shapes

Requested shapes are made consistent with `/library/gallery`:

| Requested                              | Adopted                                            |
| -------------------------------------- | -------------------------------------------------- |
| `/library/music/album/{id}/track/`     | `/library/music/album/{album_id}/track`            |
| `/library/music/album/{id}/track/{id}` | `/library/music/album/{album_id}/track/{track_id}` |

Singular resource names and no trailing slash follow the existing library routes; distinct parameter names (`album_id`,
`track_id`) remove the double-`{id}` ambiguity. Handler `path =` values omit the `/api/v1` prefix, as the gallery paths
do (`servers` carries it).

### 6. Ordering and pagination

- Album tracks are ordered by ascending `medium_index`, then ascending `track_index`; ties fall back to `music_track_id`
  for a deterministic order. Album scope is applied by resolving the album's medium through the album projection
  (`MusicAlbumSummary.music_medium_id`) and joining `music_track` to it (`music_medium.music_medium_id = ?`);
  playlist-attached rows (`music_playlist_id`) have no medium, so the join keeps them out of the query.
- The album **list** mirrors the gallery list: optional `name` filter, `limit` default 20 / range 1..100, `offset`.
- The album **track collection** returns every track of that album, unpaginated — the same shape as
  `GET /library/gallery/{id}/item`, which returns the whole collection.

### 7. Wire representations

`ProductionType` gains `from_wire`/`as_wire` mirroring `Role`'s uppercase convention, matching the
`chk_music_album_production_type` values (`COMPILATION`, `DJMIX`, …). Genres are exposed as the uppercased `genre_id`
tokens the `upper_genre_id` migration stores. JSON keys stay snake_case.

Sketches (HAL, `application/hal+json`; business data at the root):

```text
GET /api/v1/library/music/album
{
  "_links": { "self": { "href": ".../music/album?limit=20&offset=0" },
              "search": { "href": ".../music/album{?name,limit,offset}", "templated": true } },
  "items": [
    { "id": 7, "name": "Kind of Blue", "release_date": "1959-08-17",
      "production_type": "STUDIO", "genres": ["JAZZ"], "track_count": 5,
      "_links": { "self": { "href": ".../music/album/7" },
                  "tracks": { "href": ".../music/album/7/track" } } }
  ]
}

GET /api/v1/library/music/album/7/track
{
  "_links": { "self": { "href": ".../music/album/7/track" } },
  "items": [
    { "id": 101, "album_id": 7, "medium_index": 1, "track_index": 1, "file_id": 42,
      "name": "So What", "isrc_code": null, "first_release_date": "1959-08-17",
      "_links": { "self": { "href": ".../music/album/7/track/101" },
                  "album": { "href": ".../music/album/7" } } }
  ]
}
```

The track representation mirrors a gallery item's: it carries the backing `file_id` and the display name alongside the
music-specific fields, as `GalleryItemResponse` carries `file_id` and `name`.

The track display name resolves from `music_recording.name`, falling back to the backing `audio.name` (the recording
name is nullable, the audio name is not).

## Risks / Trade-offs

- [Multi-medium albums are impossible today: `uq_music_medium_music_album_id` caps an album at one medium] → The read
  path still orders by `(medium_index, track_index)` and works with a single medium, so the endpoints are correct either
  way. Do not change the schema in this read-only change; PERS-014 makes migrations mutable until release, so a later
  change that needs multi-medium albums drops the constraint.
- [`MusicAlbum.genre: Option<String>` cannot represent `music_album_genre`] → The listing exposes a `genres` array from
  the join and leaves the unused aggregate untouched. Reconcile the aggregate in the change that writes albums.
- [`music_track` has no unique index on `(music_medium_id, track_index)`; duplicate positions are possible] →
  Deterministic ordering breaks ties on `music_track_id`. A uniqueness index is an optional follow-up.
- [The `music_track` table is shared by the `MusicAlbum` and `MusicPlaylist` aggregates] → One `MusicTrack` entity
  models both sides of the shared row through its `MusicTrackPosition`. This change reads only the album side: the
  repository joins through `music_medium`, so a playlist-positioned row is never returned. The playlist-side variant is
  exercised when the playlist endpoints land.
- [A recording whose backing audio/file row is missing would leave the track detail incomplete] →
  `music_recording.audio_id` and `audio.file_id` are `NOT NULL`, so a track always resolves a file; if a join still
  yields none, the field is omitted rather than failing the request.
- [The caller-existence check adds a user query per album read] → Accepted for consistency with galleries and the
  deleted-account rule; it is a single indexed lookup inside the same transaction.
- [Album reads are public-equivalent, so a track exposes the backing `file_id` of another user's private upload] → This
  mirrors the Library read model (`GalleryItemResponse.file_id`), where file exposure follows the resource's read
  visibility; for an unowned album that visibility is "every authenticated caller". No endpoint serves the file bytes
  today, so the exposure is metadata only; access control for the file itself belongs to the streaming endpoint when it
  lands.

## Migration Plan

No datastore migration: this change only reads. Regenerate `docs/openapi.json` (`cargo run --bin openapi_gen`) and
commit it with the code (`AGENTS.md`, "Special Rules"). Rollback is reverting the change; nothing persisted changes, so
no data migration is required.

## Open Questions

None that block the specs or the task breakdown.
