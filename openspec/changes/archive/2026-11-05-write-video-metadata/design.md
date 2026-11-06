# Design

## Context

See `proposal.md` for motivation and the three delta specs for the required behaviour.

Current constraints that shape the approach:

- The music catalogue already has the write shape this change copies: `POST /music/album` (upsert by `(name, group)`),
  `PATCH /music/album/{id}`, and `POST /music/album/{id}/apply`. Its use cases, handlers, error mapping and tests are
  the template; `write-music-metadata` is the archived precedent.
- `MovieRepository` and `SeriesRepository` expose only `search`. Nothing in production creates a film, show, season or
  episode; the `INSERT` statements in `sqlx/sqlite3.rs` are test fixtures. Ingest writes `file`, `video`, `image` and
  `audio` rows only.
- `IdentifyVideo` exists and is wired into `LibraryUseCaseFactory`, but no handler or route references it. The domain
  `VideoKind` is `Movie | Series`; an episode is a `Target::Episode { reference, season, episode }` whose `reference` is
  the **show's** reference.
- `movie`, `series` and `episode` store their dates and country as `NOT NULL`, with `DEFAULT (date('now'))` on the two
  first-air columns. `MediaMetadata` reports both as optional. This is the contract break the proposal authorises.
- `library.manage` already targets `Movie` and `Series`; `docs/Permissions.md` states that a season or episode is
  authorized through its series. Cover-scope triggers already exist for `movie`, `series` and `season`.
- The schema is pre-release, so migrations are mutable in place (`PERS-014`); changing an applied migration requires
  deleting the development database.

## Goals / Non-Goals

**Goals:**

- One write path per catalogue entity, with the same absent-keeps / `null`-clears semantics as the album.
- Identification exposed as resource-scoped reads that return only the kind the route names.
- Absence representable end to end: provider → datastore → representation.
- Four reviewable delivery stages, each leaving `cargo test`, Clippy and the rule enforcer green.

**Non-Goals:**

- Video discovery (a client still needs an externally known `video_id`, exactly as it needs a `recording_id` for a track
  placement).
- Persisting provider artwork on apply (album apply does not either).
- Closing the video genre vocabulary (recorded as a follow-up issue).
- Deletion, season apply, or creating seasons and episodes from a series apply.

## Decisions

### D1 — Identification is resource-scoped, not a `video` namespace

```
GET /api/v1/library/movie/identify?name=...
GET /api/v1/library/movie/identify/{provider}/{reference}
GET /api/v1/library/series/identify?name=...
GET /api/v1/library/series/identify/{provider}/{reference}?season=&episode=
```

`/library/music` is a real namespace because albums, groups and playlists live under it. Video has no such aggregate:
`/library/video` would be a path with nothing to read. Scoping identification to `movie` and `series` keeps every
segment a real resource and mirrors the write side (`GET /movie/identify/...` → `POST /movie/{id}/apply`).

`IdentifyVideoCommand` gains a `VideoKind`. The use case filters the interpreter's ordered queries **before** the search
loop, so a name that yields both hypotheses cannot stop on a kind the route did not ask for. The handler maps
`Unavailable` to an `available: false` document, matching the music identify route.

Alternatives rejected:

- `/library/video/identify` — no `video` resource exists.
- A single `/library/identify?kind=` — introduces a new top-level noun and diverges from `<namespace>/identify`.
- Filtering candidates after the use case returns — a series query could win the "first query that hits" policy and
  starve a later film query.

The episode position lives on the series route because `Target::Episode` carries the show reference. Both `season` and
`episode` are required together; one alone is `422`. The handler verifies the provider's returned position matches the
request and rejects a mismatch instead of writing another episode's metadata.

`ProviderId` gains `serves_video` as the sibling of `serves_music`, so the route rejects a music provider with `404` the
same way the music route rejects a video provider.

Authorization uses `Authorization::decide_library_surface`, the rule the music identify routes use: a caller whose
`library.read` is `NONE` or `OWNED` is concealed with `404`; every other scope, including `LISTED`, reaches the
provider.

### D2 — Absence is nullable in the domain, the schema and the read contract

```
domain        Movie.release_date       : NaiveDate  -> Option<NaiveDate>
              Series.first_air_date    : NaiveDate  -> Option<NaiveDate>
              Episode.air_date         : NaiveDate  -> Option<NaiveDate>
              country_of_origin        : String     -> Option<String>

migrations    movie.release_date       NOT NULL DEFAULT date('now')  -> nullable, no default
              series.first_air_date    NOT NULL DEFAULT date('now')  -> nullable, no default
              episode.air_date         NOT NULL                        -> nullable
              movie/series.country_of_origin NOT NULL                 -> nullable

reads         release_date / first_air_date / air_date / country_of_origin become nullable JSON
```

The migrations are edited in place (pre-release, `PERS-014`). The `date('now')` default is exactly the fabricated
current date the identification capability forbids, so it cannot survive a path that persists provider metadata. Keeping
the columns required and "keeping the stored value on absent" was rejected: it makes absence unrepresentable and leaves
a newly created row dated today.

The domain constructors and the `SQLx` projections change together. The read specs for `library/movies` and
`library/series` are modified in this change, which is why it is marked **BREAKING**.

### D3 — Creation keys and the cross-kind invariant

| Resource | Identity                                               | Operation                                 |
| -------- | ------------------------------------------------------ | ----------------------------------------- |
| Film     | `video_id` unique                                      | `POST`, upsert: `201` new, `200` existing |
| Show     | none (no unique name)                                  | `POST`, insert-only                       |
| Season   | `(series_id, season_index)` unique                     | `PUT` by index, `201` or `200`            |
| Episode  | `(season_id, episode_index)` unique, `video_id` unique | `PUT` by index, `201` or `200`            |

A film cannot exist without a video and an episode cannot either, so `POST /movie` and `PUT .../episode/{e}` take a
`video_id`. `POST` rather than `PUT` for a film because a film's public address is its surrogate `movie_id`; the video
is the natural idempotency key. Seasons and episodes have a client-chosen position, so `PUT` to that position is the
natural create-or-replace operation, mirroring `PUT .../medium/{m}` and `PUT .../track/{t}`.

A video must back at most one catalogue item. The two tables have separate `UNIQUE(video_id)` constraints, so nothing
today prevents one video backing both a film and an episode. Enforce the invariant in both places:

- a migration adds `BEFORE INSERT` / `BEFORE UPDATE OF video_id` trigger pairs on `movie` and `episode` that
  `RAISE (ABORT)` when the other table already holds the video; `PERS-017` maps that to `RepositoryError::Conflict` and
  the handler answers `409`;
- the use case checks before writing so the common path returns the clean `409` without relying on the trigger.

Replacing an episode's video is allowed by `PUT`: the old video becomes unattached and the new one must be free. `PATCH`
never changes a video.

### D4 — Repositories gain targeted writes, not aggregate saves

```
MovieRepository
  create(movie) -> Movie
  update(movie) -> Option<Movie>              // metadata + genre links, never the video
  search(filter)                              // filter gains video_id

SeriesRepository
  create(series) -> Series                    // show row + genre links, no seasons
  update(series) -> Option<Series>            // show metadata + genre links
  put_season(series_id, season) -> Option<Season>
  update_season(series_id, season) -> Option<Season>
  put_episode(series_id, season_index, episode) -> Option<Episode>
  update_episode(series_id, season_index, episode) -> Option<Episode>
  find_episode_by_video(video_id) -> Option<...>
```

The aggregate-returning `search` stays as it is. A targeted write by `(series_id, index)` avoids loading and rewriting
the whole show tree, and the existing read aggregation already proves the index is the stable address. A missing target
is `Ok(None)` and becomes `404`, consistent with every other port.

Genre replacement copies `SqlxMusicAlbumRepository::replace_genres`: delete the links, insert each genre the catalogue
does not hold, then link. Video genres stay open, as decided, so no enum validation is added on write.

### D5 — Write use cases and authorization

```
create   POST /movie, /series, PUT season, PUT episode
         Authorization::decide_library_manage_surface   // library.manage = ALL, or Root Admin

patch    PATCH /movie, /series, season, episode
apply    POST /movie/{id}/apply, /series/{id}/apply, episode apply
         load the parent (Movie or Series)
         decide LibraryRead   -> deny => 404 (concealed)
         decide LibraryManage -> deny => 404 when concealed, else 403
```

A season and an episode authorize against their `Series`, because `library.manage` never names one (Permissions.md). The
use case loads the parent aggregate it already needs, then decides on it; there is no new `ResourceKind`.

Validation mirrors the album patch:

- name through `NamePolicy` (empty and over-long rejected), because the domain constructors only reject empty;
- dates through `NaiveDate::parse_from_str(..., "YYYY-MM-DD")`, `422` otherwise;
- country as an uppercase ISO 3166-1 alpha-2 code, `422` for anything else (the DB trigger still uppercases);
- genres as non-empty identifiers, open on write;
- cover through the existing FK and `LIBRARY`-scope trigger; a conflict maps to `409`;
- plot and season name may be empty/absent; an absent plot is stored as an empty string.

Each success returns the existing full representation (`MovieResponse`, `SeriesResponse`, `SeasonResponse`,
`EpisodeResponse`), as album PATCH returns `AlbumResponse`.

### D6 — Apply is authoritative, and a series apply never invents a tree

```
film       Target::Movie(ref)                    writes name, plot, date, country, genres
show       Target::Series(ref)                   writes name, plot, first air date, country, genres
episode    Target::Episode { show ref, s, e }    writes name, plot, air date
```

Every field the provider reports replaces the stored value. Date, country and genres that the provider does not report
are cleared (`null` or empty list); name and plot are always present in the metadata. The cover and the backing video
are never in the apply body and are left untouched. Provider artwork is ignored, as `ApplyMusicAlbum` ignores it.

A show apply does not create seasons or episodes: `lookup(Series)` returns show-level metadata only, and the port has no
season tree. A season has no apply at all because there is no provider season target. An episode apply takes the show
reference plus the path position, and rejects a provider position that differs.

Error mapping copies `ApplyMusicAlbumError`'s: unknown provider and invalid reference `422`, unknown reference `404`,
provider unavailable or failed `500`, conflict `409`.

### D7 — Delivery stages

The change is one capability capture but four reviewable stages, ordered by dependency:

1. **Schema and read contract** — nullable columns, domain fields, projections, read specs/tests, ERD.
2. **Identification** — `VideoKind`-scoped use case, four routes, response types, OpenAPI.
3. **Create and edit** — repositories, seven use cases, seven handlers, cross-kind trigger, OpenAPI.
4. **Apply** — three use cases, three handlers, provider wiring, OpenAPI.

Each stage is independently green and independently reviewable; stage 1 is the only one that changes existing read
behaviour. The task list is grouped the same way.

## Risks / Trade-offs

- **The read contract break** → Pre-MVP and explicitly authorised. Edit the three migrations in place and reset the
  development database (delete `mnemorium.db`, `-wal`, `-shm`); no compatibility shim.
- **Authoritative apply wipes a manual correction** → Documented in the endpoint description; it matches the album, and
  the client can re-patch. The alternative (merge) was rejected to keep parity.
- **Open genres fragment the vocabulary** → Accepted for consistency with music; the closed-vocabulary question is
  recorded as a follow-up issue.
- **Creation is not reachable end to end** → `video_id` is not discoverable yet. This is the same gap as `recording_id`;
  seeded rows and the future scan close it. Recorded as a follow-up, not silently ignored.
- **Cross-kind trigger pair** → SQLite has no cross-table `CHECK`; two trigger pairs are the only declarative option.
  Test both directions (film→episode and episode→film) and the `PATCH` that leaves `video_id` unchanged.
- **Broad fixture churn from nullable dates** → Keep the change mechanical: touch only constructors, projections and
  assertions that name the field, and rely on `cargo test` to list the remaining call sites.
- **Series apply looks incomplete** because it writes only show-level fields → That is the provider contract, not a
  missing feature; the spec states it and the client creates seasons/episodes through `PUT`.

## Migration Plan

1. Edit `20260904012851_create_movie`, `20261026090000_create_series` and `20261026090003_create_episode` in place.
2. Add one new migration pair for the cross-kind video triggers.
3. Reset the development datastore (delete `mnemorium.db`, `-wal`, `-shm`) before the first run.
4. Regenerate `docs/openapi.json` after each stage that adds routes.
5. Rollback is the pre-release reset: drop the database and reverse the migration edits.

## Open Questions

None that change the specs, approach or task breakdown. The two deferred concerns — video discovery and the video genre
vocabulary — are recorded as follow-ups, not open questions for this change.
