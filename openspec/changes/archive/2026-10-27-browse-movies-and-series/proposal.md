# Proposal

## Why

The datastore already carries `movie`, `movie_genre`, `video`, `stream`, `audio_stream` and `subtitle_stream`, yet no
client can read any of it: the Library context exposes galleries and music only, and there is no `movie` type in the
Rust domain at all. Series has no schema either. Clients can browse albums and playlists but cannot browse the films and
shows the server will hold.

This change delivers the read-only API surface for movies and series. Populating it — scanning a library or fetching
metadata from an external service — is deliberately out of scope and will land as a later change. Until then rows are
inserted directly into the datastore, so the endpoints are specified against the data model rather than an ingestion
path.

## What Changes

- Add two authenticated read endpoints for movies:
  - `GET /api/v1/library/movie` — list movies, paginated and filterable by name.
  - `GET /api/v1/library/movie/{movie_id}` — read one movie.
- Add five authenticated read endpoints for series, navigate series → season → episode:
  - `GET /api/v1/library/series` — list series as `id` + `name` summaries, paginated and filterable by name.
  - `GET /api/v1/library/series/{series_id}` — read one series with its season summaries.
  - `GET /api/v1/library/series/{series_id}/season/{season_index}` — read one season.
  - `GET /api/v1/library/series/{series_id}/season/{season_index}/episode` — list a season's episodes.
  - `GET /api/v1/library/series/{series_id}/season/{season_index}/episode/{episode_index}` — read one episode.
- Add the missing datastore schema: `series`, `series_genre`, `season` and `episode`, plus the uppercase
  `series.country_of_origin` trigger. `movie` and `movie_genre` already exist and are unchanged.
- Add the `Movie`, `Series`, `Season` and `Episode` domain models, with season and episode positions as `u32`.
- Add `MovieRepository` and `SeriesRepository` ports, their `LibraryUnitOfWork` accessors and the SQLx adapters. Each
  port exposes one `search` returning the aggregate whole; the use cases project only the fields their representation
  needs.
- Expose a video summary (identifier, backing file, duration, codec, resolution and scan type) on movie and episode
  representations, reusing the existing `Video` domain model.
- Add the HAL representations, handlers, route entries and OpenAPI paths/schemas, then regenerate `docs/openapi.json`.
- Record the capability as two business use cases in `docs/development/UseCases.md`, add the classes to
  `docs/development/Overview.md`, and extend the ERD in `docs/development/TechnicalDesign.md`.

**Out of scope** (explicitly not in this change):

- Any write endpoint for movies, series, seasons or episodes; the API stays read-only.
- Metadata gathering: disk scanning, probing, external providers, and populating rows.
- Poster images and cast/crew links; both are deferred, together with the equivalent music surface.
- Playback, streaming, subtitle/audio-stream exposure and file-byte access.
- Ownership or visibility for video titles; like albums, every authenticated caller sees the same catalog.
- Changes to the gallery, album and playlist requirements.

## Capabilities

### New Capabilities

- `library/movies`: browsing the films the server holds, with their genres and video summary.
- `library/series`: browsing the shows the server holds through the series → season → episode hierarchy.

### Modified Capabilities

- None. Existing gallery, album and playlist requirements are unchanged.

## Impact

- **Domain**: new `domain/model/{movie,series,season,episode}.rs`; new
  `domain/port/{movie_repository,series_repository}.rs`; extended `domain/port/library_unit_of_work.rs`.
- **Application**: new `application/port/*` and `application/use_case/*` for the seven read operations; extended
  `application/port/library_use_case_factory.rs`; extended fixtures in `src/lib/lib.rs`.
- **Infrastructure**: new SQLx row models and repositories; extended `infrastructure/outbound/sqlx/unit_of_work.rs` and
  `infrastructure/use_case_factory/library.rs`; seven new handler files under
  `infrastructure/inbound/rest/handler/library/`; extended `handler/library.rs` and `infrastructure/inbound/rest.rs`.
- **Persistence**: five new migrations under `migrations/` (four tables and one trigger).
- **API**: seven new paths and their schemas in `docs/openapi.json`, regenerated from source.
- **Tests**: unit and SQLx integration tests per layer; new `test/e2e/library/uc022_browse_movies.py` and
  `test/e2e/library/uc023_browse_series.py`.
- **Docs**: `docs/development/UseCases.md`, `docs/development/Overview.md` and `docs/development/TechnicalDesign.md`
  (ERD and `PERS-008`).
- **Dependencies**: none added; the change reuses `sqlx`, `axum`, `utoipa`, `serde` and `chrono`.
