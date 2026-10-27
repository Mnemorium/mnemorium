# Design

## Context

See `proposal.md` for motivation; the behavior contract is in `proposal.md`'s two capability deltas.

The Library context already carries the read patterns this change reuses:

- `GalleryRepository` exposes `search` and `search_items`, both returning domain objects, and the use cases assemble the
  HAL representations.
- `MusicAlbumRepository::search` hydrates an aggregate whole — album, genres, mediums and their recordings — and the
  album use cases project what each endpoint returns.
- `LibraryUseCaseFactory`, `LibraryUnitOfWork`, the SQLx unit of work, the runtime factory and the `library` handler
  folder are the wiring points.
- `Video` exists in `domain/model/video.rs`, and `VideoRepository` already reads and writes it; `Movie` has no Rust
  type, port, repository, use case or handler.

The datastore is asymmetric:

```text
movie    complete: movie, movie_genre, video, stream, audio_stream, subtitle_stream
series   absent:   no series, season or episode table, and no series genre join
```

`movie` is `movie_id, name, plot, country_of_origin, release_date, video_id` with one `movie_genre` join per genre and a
unique `video_id`. Its `country_of_origin` is uppercased by a trigger. The schema is otherwise sufficient for this
change, so the migration work is series only.

The API stays read-only: an ingestion change will populate the tables later. Manual rows must therefore satisfy the
existing foreign keys — a movie or episode is unreadable until its `video` row, and that video's `file` row, exist.

## Goals / Non-Goals

**Goals:**

- Deliver movie and series browsing as a Library sub-context, not a new bounded context.
- Reuse one aggregate-returning `search` per repository and project in the use cases, so the ports stay free of read
  models (`STY-RUST-081`).
- Keep the series list body minimal (`id` + `name`) while keeping the detail body self-navigable.
- Model season and episode positions as 32-bit unsigned indexes.
- Leave the existing `movie` schema untouched, and keep every new table inside the Library context's cascade rules.

**Non-Goals:**

- Any write endpoint, ingestion, scanning, probing or external metadata provider.
- Poster images, cast/crew joins, playback, streaming, and audio/subtitle stream exposure.
- Ownership or visibility rules for video titles; the catalog is Library-wide.
- A performance-optimised series summary query. The list may hydrate more than it returns; the use case owns the
  projection.
- Changing the album, playlist or gallery surface.

## Decisions

### 1. Video is a sub-context of Library

`Movie`, `Series`, `Season`, `Episode` and their ports live in `domain/model/` and `domain/port/`, join
`LibraryUnitOfWork`, `LibraryUseCaseFactory` and the `library` OpenAPI tag, and sit beside the existing `Video` model.
`ARCH-002` requires each model and port to belong to exactly one context; Library already owns `video`, `gallery` and
the music sub-context.

- **Alternative (rejected):** a Video bounded context with its own unit-of-work view, factory, tag and handler folder.
  Cleaner ownership for a large sub-context, but it duplicates the Library wiring for seven read endpoints and splits
  `video` from the gallery that already references it. Revisit if video gains a mutation surface.

### 2. One aggregate-returning `search` per repository; the use case projects

```text
MovieRepository::search(&MovieFilter)   -> Vec<Movie>
SeriesRepository::search(&SeriesFilter) -> Vec<Series>
```

`Movie` carries its genres and `Video`. `Series` carries its genres and its `Season`s; each `Season` carries its
`Episode`s; each `Episode` carries its `Video`. Both repositories hydrate the aggregate whole, and the use cases decide
what leaves the API:

| Use case             | Reads                                   | Returns                            |
| -------------------- | --------------------------------------- | ---------------------------------- |
| `ListMovies`         | `MovieRepository::search`               | full movie representation per item |
| `GetMovie`           | `MovieRepository::search` scoped by id  | full movie representation          |
| `ListSeries`         | `SeriesRepository::search`              | `id` + `name` per item             |
| `GetSeries`          | `SeriesRepository::search` scoped by id | metadata + season summaries        |
| `GetSeriesSeason`    | same                                    | one season summary                 |
| `ListSeriesEpisodes` | same                                    | full episode representations       |
| `GetSeriesEpisode`   | same                                    | one full episode representation    |

The series list therefore hydrates seasons, episodes and videos and then drops them. That is deliberate: it keeps one
query shape, avoids a port-level projection, and keeps the list contract independent of the read cost.

- **Alternative (rejected):** split the series port into `search` and `search_seasons`, or return summary read models,
  so the list reads only series rows. Rejected: `STY-RUST-081` requires the classic query to return the aggregate, and
  the use case is the right place to project.
- **Alternative (rejected):** one combined repository for movies and series. They are separate aggregate roots with
  different shapes; combining them would force one filter and one hydration into a union.

### 3. Series aggregate and parent-scoped identity

```text
Series
  id, name, plot, country_of_origin, first_air_date, genre_ids
  seasons: Vec<Season>

Season
  index: SeasonIndex(u32), name: Option<String>, episodes: Vec<Episode>

Episode
  index: EpisodeIndex(u32), name, plot, air_date, video: Video
```

A season is addressed by its index within its series, and an episode by its index within its season. Neither carries a
surrogate identifier into the URL or the representation; the representation exposes the index as `id`, as the album
track does.

`u32` is chosen over `u8` because a season can exceed 255 episodes, and over `i64` because the domain has no negative
positions; it matches the playlist side of `TrackIndex`, which is already `u32`. SQLite stores the value in a 64-bit
`INTEGER` and the schema check bounds it to `0..=4294967295`.

- **Alternative (rejected):** surrogate `season_id` / `episode_id` in the URL. Stable under reordering, but it breaks
  the album-scoped identity convention and forces clients to discover opaque identifiers before addressing a level.
- **Alternative (rejected):** a single `season_number` on a flat episode table. It loses season-level metadata and makes
  `/season/{n}` unaddressable.

### 4. Datastore additions

Five migrations, all Library-owned:

```sql
CREATE TABLE series (
    series_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    plot TEXT NOT NULL,
    country_of_origin VARCHAR(30) NOT NULL,
    first_air_date DATE NOT NULL DEFAULT (date('now')),
    CONSTRAINT pk_series_series_id PRIMARY KEY (series_id)
);

CREATE TABLE series_genre (
    series_id INTEGER NOT NULL,
    genre_id TEXT NOT NULL,
    CONSTRAINT pk_series_genre_series_genre PRIMARY KEY (series_id, genre_id),
    CONSTRAINT fk_series_genre_series FOREIGN KEY (series_id) REFERENCES series (series_id),
    CONSTRAINT fk_series_genre_genre FOREIGN KEY (genre_id) REFERENCES genre (genre_id)
);

CREATE TABLE season (
    season_id INTEGER NOT NULL,
    series_id INTEGER NOT NULL,
    season_index INTEGER NOT NULL DEFAULT 0,
    name TEXT,
    CONSTRAINT pk_season_season_id PRIMARY KEY (season_id),
    CONSTRAINT uq_season_series_id_season_index UNIQUE (series_id, season_index),
    CONSTRAINT chk_season_season_index CHECK (season_index BETWEEN 0 AND 4294967295),
    CONSTRAINT fk_season_series FOREIGN KEY (series_id) REFERENCES series (series_id) ON DELETE CASCADE
);

CREATE TABLE episode (
    episode_id INTEGER NOT NULL,
    season_id INTEGER NOT NULL,
    episode_index INTEGER NOT NULL,
    name TEXT NOT NULL,
    plot TEXT NOT NULL,
    air_date DATE NOT NULL,
    video_id INTEGER NOT NULL,
    CONSTRAINT pk_episode_episode_id PRIMARY KEY (episode_id),
    CONSTRAINT uq_episode_season_id_episode_index UNIQUE (season_id, episode_index),
    CONSTRAINT uq_episode_video_id UNIQUE (video_id),
    CONSTRAINT chk_episode_episode_index CHECK (episode_index BETWEEN 0 AND 4294967295),
    CONSTRAINT fk_episode_season FOREIGN KEY (season_id) REFERENCES season (season_id) ON DELETE CASCADE,
    CONSTRAINT fk_episode_video FOREIGN KEY (video_id) REFERENCES video (video_id)
);
```

The uppercase `country_of_origin` trigger mirrors `tg_movie_upper_country_of_origin_insert/update` (`PERS-008`).
`series_genre` inherits the `genre` table's uppercase trigger through its foreign key.

`ON DELETE CASCADE` is used only inside Library, series → season → episode (`ARCH-004`, `PERS-020`). The episode → video
link does not cascade, mirroring movie → video: a video is a Library media record that a later deletion path
orchestrates. `season.name` is optional; the response omits it when absent, like an album's optional production type.

- **Alternative (rejected):** alter `movie` to add `series_id` or reuse it for episodes. Movies and episodes are
  different domain concepts and `movie` already has a complete read model; mixing them would make every future query
  branch on a discriminator.
- **Alternative (rejected):** `u8` indexes for consistency with `medium_index`. Rejected: a season's episode count is
  not a physical-medium count and can exceed 255.

### 5. Representations and links

Business data stays at the payload root, and every representation carries `_links.self` (`API-032`):

```text
GET /library/movie/{id}
  id, name, plot, country_of_origin, release_date, genres,
  video: { id, file_id, duration_ms, codec, width, height, scan_type }

GET /library/series
  items: { id, name, _links.self }

GET /library/series/{id}
  id, name, plot, country_of_origin, first_air_date, genres,
  seasons: { index, name?, episode_count,
             _links: { self, series, episodes } }

GET /library/series/{id}/season/{n}
  series_id, index, name?, episode_count,
  _links: { self, series, episodes }

GET /library/series/{id}/season/{n}/episode[/{m}]
  id, series_id, season_index, name, plot, air_date,
  video: { id, file_id, duration_ms, codec, width, height, scan_type },
  _links: { self, series, season }
```

The series **detail** embeds season summaries because otherwise the season indexes are undiscoverable. The series
**list** stays `id` + `name` as specified. The video summary reuses the existing `Video` model's fields; it carries
`file_id` for the same reason an album track does, and no stream detail because streaming is out of scope.

Dates use `YYYY-MM-DD`; `scan_type` is the uppercase wire token the `video` schema stores; genres are the uppercase
`genre_id` tokens ordered by identifier.

### 6. Authorization, pagination and errors

- Every use case loads the authenticated caller and rejects a token whose account was deleted (`NoSuchCaller` → `401`),
  following `ListAlbums`. No title is owned, so no use case carries `Forbidden` and no `403` is ever emitted.
- Movie and series collections accept `name`, `limit` and `offset`, default to 20, and reject `limit` outside `1..=100`
  with `422`. Pagination happens in the use case after the repository read, and the total drives the `next` link.
- Season and episode collections are unpaginated, like an album's mediums and tracks.
- Stable ordering is part of the read: movies by ascending id, series by ascending id, seasons by ascending index,
  episodes by ascending index. Ties do not exist for the child levels because of the unique constraints.
- Error mapping is the Library convention: malformed path or query → `400`; `NoSuchCaller` → `401`; unknown movie,
  series, season or episode → `404`; out-of-range limit → `422`; unmapped → `500`.

### 7. Adapter strategy

- `SqlxMovieRepository::search` joins `movie`, `movie_genre` and `video`, groups genres, parses the stored scan type and
  builds `Movie` with `Video::try_new`. It orders by `movie_id`.
- `SqlxSeriesRepository::search` joins `series`, `series_genre`, `season`, `episode` and `video`, orders by
  `series_id, season_index, episode_index`, and hydrates one `Series` per group. Left joins keep a series with no season
  and a season with no episode readable.
- A stored value the domain cannot hold — an unknown scan type, a non-positive frame count, width or height, an empty
  name — fails the read with `RepositoryError::DataIntegrityViolation` rather than serving an invalid domain object, as
  the album adapter already does.
- `Movie.genre_ids` is aggregated from the join, ordered by `genre_id`; the adapter declares no summary type.

### 8. Documentation and verification

- Add `UC-022 - Browse Movies` and `UC-023 - Browse Series` to `docs/development/UseCases.md` under a new Video
  subsection of Library.
- Add `Series`, `Season` and `Episode` to the `Overview.md` domain model and glossary, and add the four entities plus
  relations to the `TechnicalDesign.md` ERD.
- Regenerate `docs/openapi.json` (`cargo run --bin openapi_gen`) and commit it.
- Cover the SQLx hydration with in-memory migrated-database tests, the use-case projections with mocked repositories,
  the handlers with mocked use cases, and E2E with `401`, empty collections, `400`, `404` and `422`. No E2E seeds
  populate the tables because no endpoint writes them; the populated paths are covered by integration tests.

## Risks / Trade-offs

- [The series list hydrates seasons, episodes and videos only to project `id` and `name`] → Accepted to keep one
  aggregate-returning port. If it becomes a measured problem, a later change can add a dedicated summary query without
  changing the list contract; that change would need to revisit `STY-RUST-081`, not this one's API.
- [The series list forces a detail request for genres, dates and counts] → Explicit requirement: the list is a
  navigation index. Clients that need more fetch the detail, which carries the season summaries.
- [Season and episode positions are `u32`; SQLite `INTEGER` is 64-bit] → The schema check bounds the stored value to
  `0..=4294967295`, and the domain constructor receives a `u32`, so an out-of-range row is a data-integrity failure, not
  a silent truncation.
- [`movie` and `episode` rows require a `video` row, which requires a `file` row] → This is a data-setup constraint of
  the read-only phase, documented for manual insertion; the future ingestion change owns creating the chain in one
  transaction.
- [Hydrating the aggregate validates stored data, so a malformed row fails the read] →
  `RepositoryError::DataIntegrityViolation`, consistent with the album read. The alternative — serving a value the
  domain cannot model — is worse.
- [`episode.name` and `plot` are `NOT NULL` but admit empty strings] → The domain rejects an empty name; an empty plot
  is served as stored. Manual inserts must supply a name.
- [The `movie` table has no unique name constraint] → Duplicate titles are possible and are listed in id order. Adding a
  uniqueness rule is a separate data-model decision.
- [No poster/cast fields] → Deferred with the equivalent music surface; adding them later extends the representation and
  adds joins without breaking the existing fields.

## Migration Plan

- Add the five migrations in `migrations/` (series, series_genre, season, episode, uppercase country trigger). No
  existing row changes; `movie` is untouched.
- Rollback drops the four new tables and the trigger in reverse order. Because the API is read-only, no data migration
  or backfill is required.
- Regenerate `docs/openapi.json` after the handlers land and commit it with the code.
- Deploy as one backend release; an older client that does not call the new paths is unaffected.

## Open Questions

None blocking. The movie list shape (full representation versus `id` + `name`) is settled as full for this change; if it
is later flattened, that is a new spec change, not a deferred decision here.
