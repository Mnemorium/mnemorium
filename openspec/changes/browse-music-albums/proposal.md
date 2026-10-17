# Proposal

## Why

Mnemorium stores music — the schema has `music_album`, `music_medium`, `music_track`, `music_recording` and `audio`, and
the domain layer already carries `MusicAlbum`, `MusicMedium` and `MusicRecording` models — but no client can read any of
it: there are no music ports, repositories, use cases, handlers or routes. The Library context exposes only galleries.
This change delivers the first read slice of the music surface so clients can browse albums and their tracks.

## What Changes

- Add three authenticated read endpoints in the Library bounded context:
  - `GET /api/v1/library/music/album` — list albums, paginated and filterable by name.
  - `GET /api/v1/library/music/album/{album_id}/track` — list the tracks of one album, ordered by medium index then
    track index.
  - `GET /api/v1/library/music/album/{album_id}/track/{track_id}` — read one track of one album.
- Add a `MusicTrack` entity inside the `MusicAlbum` aggregate, carrying its identity, position and recording reference,
  closing the gap between `docs/development/Overview.md` (which lists `MusicTrack`) and the Rust domain model (which
  does not). A playlist track belongs to the separate `MusicPlaylist` aggregate and stays out of scope.
- Add a `MusicAlbumRepository` outbound port (Library-scoped, named after the aggregate) with album and track read
  models, its `SQLx` adapter, row models and a `LibraryUnitOfWork` view.
- Add three application use cases, their factory accessors on `LibraryUseCaseFactory`, and the runtime factory wiring.
- Add the HAL representations, endpoint handlers, route entries and OpenAPI paths/schemas, then regenerate
  `docs/openapi.json`.
- Record the capability as a business use case in `docs/development/UseCases.md`.
- Normalise the endpoint shapes requested for this change: drop the trailing slashes from collection paths, keep the
  project's singular resource naming, and name the path parameters `album_id` / `track_id` so the two identifiers are
  unambiguous.

**Out of scope** (explicitly not in this change): playlist endpoints, an album-detail endpoint (`GET .../album/{id}`),
any write or upload of albums or tracks, adding or removing tracks from a playlist, and changing a playlist's
visibility.

## Capabilities

### New Capabilities

- `library/music-albums`: browsing the music albums a client may read and the tracks each album contains.

### Modified Capabilities

- None. No spec exists yet, and this change touches no requirement of another capability.

## Impact

- **Domain**: `src/lib/domain/model.rs` and a new `music_track.rs`; new `domain/port/music_album_repository.rs`;
  extended `domain/port/library_unit_of_work.rs`.
- **Application**: new `application/port/{list_albums,list_album_tracks,get_album_track}.rs` and matching
  `application/use_case/*`; extended `application/port/library_use_case_factory.rs`.
- **Infrastructure**: new `infrastructure/outbound/sqlx/music_album_repository.rs` and row models; extended
  `infrastructure/outbound/sqlx/unit_of_work.rs`; extended `infrastructure/use_case_factory/library.rs`; new
  `infrastructure/inbound/rest/handler/library/{get_album_list,get_album_track_list,get_album_track}.rs`; extended
  `handler/library.rs` and `inbound/rest.rs`.
- **API**: three new paths and their schemas in `docs/openapi.json` (regenerated from source).
- **Docs**: a new business use case in `docs/development/UseCases.md`.
- **Dependencies**: none added; the change reuses `sqlx`, `axum`, `utoipa`, `serde` and `chrono` already in use.
