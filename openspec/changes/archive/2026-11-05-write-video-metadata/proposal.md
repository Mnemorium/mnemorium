# Proposal

## Why

The music catalogue has a complete write surface — create, patch and apply — while the video catalogue is read-only.
Nothing in production creates a film, show, season or episode, video identification is not exposed over HTTP, and a
client that finds a work through a provider cannot write it into the catalogue. The read endpoints therefore serve only
rows inserted directly into the datastore.

## What Changes

- Expose video identification as resource-scoped reads:
  - `GET /api/v1/library/movie/identify?name=...`
  - `GET /api/v1/library/movie/identify/{provider}/{reference}`
  - `GET /api/v1/library/series/identify?name=...`
  - `GET /api/v1/library/series/identify/{provider}/{reference}?season=&episode=`, where both query parameters are
    required together and resolve one episode through its show's reference.
- Add the create surface:
  - `POST /api/v1/library/movie`, an upsert by `video_id`.
  - `POST /api/v1/library/series`, insert-only.
  - `PUT /api/v1/library/series/{series_id}/season/{season_index}`, idempotent by index.
  - `PUT /api/v1/library/series/{series_id}/season/{season_index}/episode/{episode_index}`, idempotent by index.
- Add partial edits through `PATCH` on a film, a show, a season and an episode. An absent field keeps its value and a
  `null` clears an optional one.
- Add provider application through `POST .../apply` on a film, a show and an episode. Apply is authoritative: an absent
  provider date, country or genre list clears the stored value, mirroring album apply. It never touches the cover or the
  backing video.
- **BREAKING** make a catalogue row able to store absence:
  - `movie.release_date`, `series.first_air_date` and `episode.air_date` become nullable and lose their
    `DEFAULT (date('now'))`.
  - `movie.country_of_origin` and `series.country_of_origin` become nullable.
  - The movie and series read representations report `null` for an absent date or country.
- Accept genres as free-form non-empty identifiers, inserting an unknown one on demand, matching album genres. Validate
  that a video backs at most one film or episode.
- Deliver the work in four reviewable stages: schema and read contract, identification routes, create and edit, then
  apply.
- Out of scope: video discovery (a client still learns a `video_id` from an existing representation, as it learns a
  `recording_id` today), persisting provider artwork on apply, a closed video genre vocabulary, and deletion of any
  video catalogue entity.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library/movies`: adds film creation, partial update and provider application; makes the release date and country of
  origin nullable in reads and writes.
- `library/series`: adds show, season and episode creation, partial update and provider application; makes the first-air
  date, air date and country of origin nullable in reads and writes.
- `library/video-identification`: adds the HTTP identification routes and the optional episode position on the series
  reference route.

## Impact

- New handlers, use cases, ports and use-case-factory entries for identification and the four catalogue entities.
- `MovieRepository` and `SeriesRepository` gain create, update, lookup-by-video and parent-scoped write methods; the
  `SQLx` adapters and their tests grow accordingly.
- `Movie`, `Series` and `Episode` carry optional dates and country; every fixture, projection and handler test follows.
- Migrations `20260904012851_create_movie`, `20261026090000_create_series` and `20261026090003_create_episode` are
  edited in place (`PERS-014`), and the entity-relationship diagram follows (`PERS-011`).
- `docs/openapi.json` is regenerated; `docs/development/UseCases.md`, `Overview.md` and the event catalogue follow.
- No new crate is required.
