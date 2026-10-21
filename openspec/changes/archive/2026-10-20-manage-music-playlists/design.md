# Design

## Context

See `proposal.md` — Why. The playlist table and `MusicPlaylist` model already exist from the earlier data-model work.
The `music_track` table already distinguishes a medium placement from a playlist placement, and the unique index on
`(music_playlist_id, track_index)` already exists. What is missing is the repository, application, and REST layers, plus
the ownership, naming, duplicate-recording, cascade, and timestamp rules the inherited model does not enforce.

The implemented album API is medium-scoped (`/album/{album_id}/medium/{medium_index}/track`), even though the main
`library/music-albums` spec still describes the older album-level paths. The unarchived `refactor-music-track-identity`
change owns that sync; this design assumes the implemented medium-scoped surface and does not touch albums.

The gallery is the closest existing mutable, owner-scoped resource. It supplies the public/private read rule, the
owner-only deletion rule, the `(user_id, name)` unique key, and the `last_modified_at` touch pattern. It also exposes a
known drift: a private gallery the caller cannot read answers `403`, which discloses its existence despite `API-044`.
The playlist surface keeps the gallery's visibility semantics but not that error behavior.

Constraints from the rulebook: the hexagonal direction (`AGENTS.md`), `STY-RUST-080` (filter fields), `STY-RUST-081`
(the classic `search` returns the aggregate), `STY-RUST-084` (domain names, not columns), `API-032`/`API-033`/`API-034`
(HAL shape), `API-042` (canonical `self`), `API-044` (no existence disclosure), `API-045` (declared constraints are
enforced), `PERS-011` (ER diagram), `PERS-014` (migrations mutable until release), `PERS-020`/`ARCH-004` (cascade
scope), `OBS-002`/`OBS-003`/`OBS-006` (logging), and `TEST-019`/`TEST-046` (domain through application, shared
fixtures).

## Goals / Non-Goals

**Goals:**

- Give an authenticated caller the complete playlist lifecycle: list, create, read, update metadata, add a recording,
  remove a track, reorder tracks, and delete.
- Keep the playlist a true aggregate: one repository read hydrates its ordered recording occurrences.
- Make visibility, ownership, and error disclosure explicit and testable.
- Enforce name normalization/uniqueness and the no-duplicate-recording rule in both the domain and the schema.
- Share the name policy with galleries and make the gallery behavior match its documented trimming.
- Land HAL representations and the generated OpenAPI spec alongside the code.

**Non-Goals:**

- Deleting a user's playlists; the user-to-playlist foreign key crosses bounded contexts, so UC-005 must orchestrate it.
- Sorting or filtering by `last_modified_at`, ETags, or conditional requests.
- A canonical recording read with no placement; the add path resolves a recording inside the placement insert.
- A recording write path, playlist import/export, or collaborative editing beyond the public flag.
- Changing album reads, including the existing allowance for one medium to sequence the same recording twice.
- Fixing the gallery's `403`-on-invisible behavior; it is noted as a follow-up, not changed here.

## Decisions

### 1. Playlist as the second container; one aggregate read

```text
                     music_recording
                      /           \
             music_track          music_track
      (playlist_id, track_index)  (medium_id, track_index)
             |                       |
       music_playlist            music_medium
```

`MusicPlaylistRepository::search` reads the playlist, its track placements, the shared recording, the backing audio, and
the file in one query:

```sql
SELECT music_playlist..., music_track.track_index,
       music_recording..., file.file_id
FROM music_playlist
LEFT JOIN music_track       ON music_track.music_playlist_id = music_playlist.music_playlist_id
LEFT JOIN music_recording   ON music_recording.music_recording_id = music_track.music_recording_id
LEFT JOIN audio             ON audio.audio_id = music_recording.audio_id
LEFT JOIN file              ON file.file_id = audio.file_id
ORDER BY music_playlist.music_playlist_id, music_track.track_index
```

Hydration groups rows by playlist and builds `MusicRecording` occurrences with `TrackIndex::PlaylistIndex(u32)`. The
join naturally excludes medium placements because their `music_playlist_id` is `NULL`. A playlist with no placement
yields one row with null track columns and hydrates an empty `tracks` vector. A stored index outside `u32` is a
`DataIntegrityViolation`, like an out-of-range medium index in the album adapter. `music_group_ids` is `None`, matching
the album read, because no group join is exposed by either representation.

Alternatives rejected: a per-playlist track query (the N+1 the album change removed); a repository read model
(`STY-RUST-081`); hydrating tracks from `music_recording` alone (loses the placement order and identity).

### 2. Visibility, ownership, and the error surface

```text
+---------------------------+-------------------------+--------------------------+
| Action                    | Public playlist         | Private playlist         |
+---------------------------+-------------------------+--------------------------+
| Read detail / alias       | any authenticated       | owner or Root Admin      |
| Add / remove / reorder    | any authenticated       | owner or Root Admin      |
| Rename / set visibility   | owner only              | owner only               |
| Delete playlist           | owner only              | owner only               |
+---------------------------+-------------------------+--------------------------+
```

The `LibraryAccess` domain service gains `can_read_playlist`, the direct analogue of `can_read_gallery`. `MusicPlaylist`
gains `is_owned_by`, the analogue of `Gallery::is_owned_by`. The caller lookup stays in each use case through
`UserUnitOfWork`: it is application orchestration, not part of the domain service.

The error surface follows `API-044`, which the gallery does not:

- A caller who cannot read the playlist receives `404 Not Found` for every operation, including owner-only ones. The use
  case still emits `authorization_failed` internally; only the response is indistinguishable from a missing playlist.
- A caller who can read a public playlist but is not its owner receives `403 Forbidden` when the request carries `name`,
  `is_public`, or a delete. The playlist's existence is already visible to that caller, so `403` discloses nothing new.
- The Root Admin can read and moderate every playlist, but cannot rename, change visibility of, or delete another user's
  playlist; those are owner-only, mirroring `DeleteGallery`.
- The list applies the repository filters, then retains the playlists the caller can read. The Root Admin retains all.

Alternatives rejected: gallery-style `403` for an unreadable private playlist (discloses existence, `API-044`); public
playlists as read-only (the user chose gallery semantics); a separate collaborator model (out of scope).

### 3. REST surface, PATCH curation, and the alias

```text
GET    /api/v1/library/music/playlist
POST   /api/v1/library/music/playlist
GET    /api/v1/library/music/playlist/{playlist_id}
GET    /api/v1/library/music/playlist/{playlist_id}/track          alias of the detail
GET    /api/v1/library/music/playlist/{playlist_id}/track/{index}
PATCH  /api/v1/library/music/playlist/{playlist_id}                metadata + add_track
DELETE /api/v1/library/music/playlist/{playlist_id}
DELETE /api/v1/library/music/playlist/{playlist_id}/track/{index}
PUT    /api/v1/library/music/playlist/{playlist_id}/track          reorder
```

`PATCH` is the only add path. Its body is:

```json
{
  "name": "Late night",
  "is_public": true,
  "add_track": { "recording_id": 30 }
}
```

Every field is optional. The authorization is per field: `name` and `is_public` require ownership; `add_track` requires
readability. A non-owner who sends an owner-only field alongside `add_track` is rejected as a whole, and nothing is
applied. The use case validates and mutates in one unit of work, so a duplicate name, a duplicate recording, or an
unknown recording leaves the playlist untouched.

`add_track` carries a `recording_id` rather than an album track address:

- The album track representation already exposes `recording_id`; it is the shared entity a playlist references.
- A playlist does not care which album, if any, a recording came from. An album address would couple curation to album
  structure and fail for a recording with no medium placement.
- The placement insert can validate the recording and compute the next index atomically:

```sql
INSERT INTO music_track (track_index, music_playlist_id, music_recording_id)
SELECT COALESCE((SELECT MAX(track_index) + 1
                 FROM music_track
                 WHERE music_playlist_id = ?1), 0),
       ?1,
       music_recording_id
FROM music_recording
WHERE music_recording_id = ?2
```

Zero inserted rows means the recording does not exist, which the use case reports as `422` because the identifier is a
request-body value (`API-041`, `API-044`). The repository returns the hydrated occurrence so the PATCH response can
include the new track without a second aggregate read. The partial unique index from decision 5 catches a concurrent
second add of the same recording as `AlreadyExist`, which the use case maps to `409`.

Track indices are zero-based: `0` for the first track, `MAX + 1` for an append, and `0..n-1` after a reorder. This
matches the gallery's zero-based item index and keeps the playlist independently addressable from the album's one-based
track positions.

`GET /{id}/track` is a literal alias for `GET /{id}`: it returns the same playlist representation, and `_links.self` is
the canonical playlist URI (`API-042`). The `tracks` link on a playlist points to this alias. That link is deliberately
a route convenience, not a second collection representation; a client following it receives the playlist with its
embedded tracks. `PUT /track` returns the updated playlist, whose `tracks` field is the reordered collection.
`DELETE /track/{index}` returns `204`.

Alternatives rejected: `POST /track` (the user removed it in favour of PATCH); an album track address (couples to
albums); a full `tracks` replacement via PATCH (duplicates the remove and reorder endpoints); a separate track
collection representation (the user chose the alias).

### 4. Names: shared policy and per-owner uniqueness

```text
domain/service/name_policy.rs

MAX_NAME_LENGTH = 100

NamePolicy::validate(name) -> Result<String, NamePolicyError>
    trim
      +-- empty after trim?       -> Empty
      +-- more than 100 chars?    -> TooLong
      +-- otherwise               -> Ok(trimmed)
```

`Gallery::try_new`/`set_name` and `MusicPlaylist::try_new`/`set_name` both call it. Each model maps `Empty`/`TooLong`
onto its own stable error message, so the API keeps saying "gallery name..." or "playlist name...". The extraction is
intentional: the gallery currently trims only for the emptiness check but stores the original whitespace, while its
create documentation already promises a trimmed name. Adopting the policy corrects that behavior.

Uniqueness is not part of the policy because it needs the repository. The playlist schema adds `UNIQUE (user_id, name)`;
`CreatePlaylist` searches the caller's names before inserting and `UpdatePlaylist` searches them before saving,
excluding the target playlist. Both map a unique-constraint race to `409`. Trimming makes the constraint meaningful:
`"Mix"` and `" Mix "` are the same name.

Existing gallery rows with surrounding whitespace are an edge case: a later create/rename with the trimmed equivalent
will not collide with the untrimmed legacy row. No migration is required; an optional one-time normalization can be a
follow-up.

Alternatives rejected: keeping gallery validation inline (the user asked for the shared policy); a value object instead
of a service (more type-safe, but the project already uses stateless policy services such as `PasswordPolicy`);
case-insensitive uniqueness (the gallery is case-sensitive, and changing it is a separate decision).

### 5. Playlist curation rules and duplicate recordings

`MusicPlaylist` owns the rules that span the aggregate and the requested change:

- `ensure_recording_absent(recording_id)` returns `RecordingAlreadyPresent` when the aggregate already holds that
  recording. The PATCH use case maps it to `409`.
- `validate_order(order)` requires the requested indices to be a complete permutation of the playlist's current indices
  and produces the new `0..n-1` sequence as `(index, recording_id)` pairs. A missing, repeated, or unknown index returns
  `InvalidOrder`, mapped to `422`.

The rules live on the aggregate rather than in a separate stateless policy service: both read and constrain one
playlist's own tracks, so they have no dependency beyond the model they describe.

The aggregate excludes duplicate recordings only within one playlist. Album mediums keep the existing occurrence model,
including two placements of one recording, as the album test asserts.

The schema is the race guard:

```sql
CREATE UNIQUE INDEX uq_music_track_playlist_recording
ON music_track (music_playlist_id, music_recording_id)
WHERE music_playlist_id IS NOT NULL;
```

The use case's pre-check gives the friendly `409`; the partial index catches a concurrent second add. A concurrent
append of a different recording can still collide on `(music_playlist_id, track_index)`, which also maps to `409`; the
message is the generic conflict, not a false "already present" claim.

Alternatives rejected: a repository-only check (no domain rule, and still needs the schema guard); a database-only
constraint (no friendly `409`); a separate stateless policy service (both rules read one aggregate's own tracks, so the
aggregate is their natural home).

### 6. Schema changes in the initial migrations

The schema is edited in place in the two initial migrations. No new migration file and no `ALTER TABLE` are introduced.
This relies on `PERS-014`: the migrations must still be mutable, so the database engineer confirms the schema is
unreleased before the edit lands.

`migrations/20260904012848_create_music_playlist.up.sql`:

```sql
CREATE TABLE music_playlist (
    music_playlist_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_modified_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    user_id INTEGER NOT NULL,
    is_public BOOLEAN NOT NULL DEFAULT 0,
    CONSTRAINT pk_music_playlist_music_playlist_id PRIMARY KEY (music_playlist_id),
    CONSTRAINT fk_music_playlist_user FOREIGN KEY (user_id) REFERENCES user (user_id),
    CONSTRAINT chk_music_playlist_is_public CHECK (is_public IN (0, 1)),
    CONSTRAINT uq_music_playlist_user_id_name UNIQUE (user_id, name)
);
```

`migrations/20260904012849_create_music_track.up.sql`:

- add `ON DELETE CASCADE` to `fk_music_track_playlist`;
- add the partial unique index from decision 5;
- add `chk_music_track_playlist_track_index CHECK (music_playlist_id IS NULL OR track_index <= 4294967295)` so the
  stored playlist index cannot exceed the domain's `u32`.

The `last_modified_at` default keeps the existing `seed_playlist` fixture and any legacy rows valid; the create use case
still writes an explicit timestamp. `is_public` moves to `0` so the schema fallback matches the API default. The down
migrations remain `DROP TABLE`, so no extra down work is needed.

The ER diagram (`PERS-011`) and the paragraph describing the partial unique indexes in `TechnicalDesign.md` gain the new
column, constraint, cascade, and third index.

Deleting a playlist can now be a single `DELETE FROM music_playlist`, because the placement rows cascade. The shared
recording, audio, and file rows are untouched. The user-to-playlist foreign key remains non-cascading because it crosses
the User and Library contexts (`ARCH-004`, `PERS-020`); UC-005's future delete-user orchestration must remove playlists
explicitly.

### 7. Repository port and adapter

`MusicPlaylistRepository`:

```rust
fn search(&mut self, filter: &MusicPlaylistFilter)
    -> impl Future<Output = Result<Vec<MusicPlaylist>, RepositoryError>> + Send;
fn create(&mut self, playlist: MusicPlaylist)
    -> impl Future<Output = Result<MusicPlaylist, RepositoryError>> + Send;
fn save(&mut self, playlist: MusicPlaylist)
    -> impl Future<Output = Result<MusicPlaylist, RepositoryError>> + Send;
fn delete(&mut self, id: NumericID)
    -> impl Future<Output = Result<bool, RepositoryError>> + Send;
fn add_track(&mut self, playlist_id: NumericID, recording_id: NumericID)
    -> impl Future<Output = Result<Option<MusicRecording>, RepositoryError>> + Send;
fn remove_track(&mut self, playlist_id: NumericID, track_index: u32)
    -> impl Future<Output = Result<bool, RepositoryError>> + Send;
fn replace_tracks(&mut self, playlist_id: NumericID, order: &[(u32, NumericID)])
    -> impl Future<Output = Result<(), RepositoryError>> + Send;
```

`MusicPlaylistFilter` uses domain names (`id`, `name`, `user_id`, `is_public`) per `STY-RUST-080`. `search` returns the
aggregate, ordered by playlist identifier for deterministic pagination. `create` inserts and returns the stored row;
`save` updates metadata and `last_modified_at`; `delete` relies on the cascade; `add_track` performs the validating
insert from decision 3 and returns the hydrated occurrence; `remove_track` deletes one placement; `replace_tracks`
deletes all placements for the playlist and reinserts the supplied `(index, recording_id)` sequence in one transaction.

`LibraryUnitOfWork` gains `music_playlists() -> impl MusicPlaylistRepository + '_`. The SQLx adapter lives in
`infrastructure/outbound/sqlx/music_playlist_repository.rs` with a `sqlx/model/music_playlist.rs` row. Stored
conversions and malformed rows use `stored_data_invalid` so the adapter logs `output_validation_failed` once and returns
`DataIntegrityViolation` (`OBS-002`, `OBS-003`).

Alternatives rejected: saving the whole aggregate on every track change (rewrites placements and invalidates internal
identifiers for no benefit); separate `search_tracks` (the two-projection mistake the album change removed); a
read-model port (`STY-RUST-081`).

### 8. Application use cases

| Use case                | Behaviour                                                                                                 |
| ----------------------- | --------------------------------------------------------------------------------------------------------- |
| `ListPlaylists`         | Load caller, search with filters, retain readable playlists, page in memory, return total and items.      |
| `CreatePlaylist`        | Load caller, validate name, check per-owner uniqueness, create, return the representation.                |
| `GetPlaylist`           | Load caller, search by id, reject an invisible/unknown playlist as `NoSuchPlaylist`, return its tracks.   |
| `GetPlaylistTrack`      | Use the aggregate read, resolve the track by index, return it or `NoSuchTrack`.                           |
| `UpdatePlaylist`        | Metadata and/or `add_track` in one unit of work; per-field authorization; duplicate checks; touch; save.  |
| `DeletePlaylist`        | Load caller and playlist, require ownership, delete (placements cascade).                                 |
| `RemovePlaylistTrack`   | Load caller and playlist, require readability, remove the placement, touch, save.                         |
| `ReorderPlaylistTracks` | Load caller and playlist, require readability, validate the permutation, replace placements, touch, save. |

`UpdatePlaylist` is the largest command: `caller_id`, `playlist_id`, `name: Option<String>`, `is_public: Option<bool>`,
`add_track: Option<NumericID>`. It loads the caller, loads the playlist, checks readability, then checks ownership if an
owner-only field is present, validates the name and its uniqueness, checks the recording for duplication, performs the
placement add, touches `last_modified_at` once, and saves. Any failure rolls the unit of work back, so the mixed request
is atomic.

The list pages in memory after the visibility retain, exactly as `ListGalleries` does; repository-level pagination would
apply the limit before the visibility predicate. `total` is the count after visibility filtering.

Each use case gets a port, a `LibraryUseCaseFactory` accessor, and a mock, following the existing album and gallery
pattern. No separate `AddPlaylistTrack` use case exists because PATCH owns the add.

### 9. REST representations, HAL, and errors

New representations in `handler/library.rs`, new endpoint files under `handler/library/` (`STY-RUST-003`,
`STY-RUST-082`):

- `PlaylistLinks { self, tracks }`: `self` is the canonical playlist URI; `tracks` is the alias URI.
- `PlaylistResponse`: `id`, `name`, `is_public`, `owner_id`, `created_at`, `last_modified_at`, `track_count`,
  `tracks: Vec<PlaylistTrackResponse>`, `_links`.
- `PlaylistListLinks`: `self`, `next`, `prev`, templated `search`
  (`/api/v1/library/music/playlist{?name,is_public,owner_id,limit,offset}`).
- `PlaylistTrackResponse`: `id` (the `u32` index), `recording_id`, `file_id`, `name`, `isrc_code`, `first_release_date`,
  `_links { self, playlist }`.

Collection items reuse `PlaylistResponse` with an empty `tracks` array: only the detail, alias, create, PATCH, and
reorder responses hydrate the aggregate, and a listed playlist's tracks are reached through its `tracks` link.

Requests: `PostPlaylistRequest { name, is_public = false }`,
`PatchPlaylistRequest { name?, is_public?, add_track?: AddPlaylistTrackRequest { recording_id } }`,
`ReorderPlaylistTracksRequest { order: Vec<u32> }`.

Path parsers: `playlist_id` as `NumericID`, `track_index` as `u32`, each rejecting malformed values with a stable `400`
and a single `input_validation_failed` event (`OBS-006`). The `track_index` parser accepts `0`.

Status mapping:

| Outcome                                          | Status             |
| ------------------------------------------------ | ------------------ |
| List, read, alias, get track, PATCH, PUT         | `200`              |
| Create                                           | `201` + `Location` |
| Delete playlist, remove track                    | `204`              |
| Malformed identifier/index/query/body            | `400`              |
| Missing or invalid credentials                   | `401`              |
| Owner-only action on a visible playlist          | `403`              |
| Unknown or invisible playlist/track              | `404`              |
| Duplicate name or recording                      | `409`              |
| Invalid name/order, unknown recording, bad limit | `422`              |
| Unexpected failure                               | `500`              |

`POST` and `PATCH` success responses are HAL playlist representations. The alias handler calls the same `GetPlaylist`
use case and returns the same body, so `_links.self` is canonical. The one-track handler returns a track representation
whose `self` is the track URI and whose `playlist` link is canonical.

Error mappings live beside the representations, matching the album handlers. The use cases return stable error variants;
the handlers map them without exposing repository or framework detail (`API-040`). `docs/openapi.json` is regenerated
after the handlers land.

### 10. Validation, errors, and logging

- Name rejection: `input_validation_failed` with `field=name`, `reason=empty` or `reason=too_long`, once at the use
  case, never echoing the supplied name (`OBS-006`, `OBS-003`).
- Invisible playlist: the use case emits `authorization_failed` with the action (`read`, `update`, `delete`,
  `add_track`, `remove_track`, `reorder`) and target `music_playlist`, then returns `NoSuchPlaylist` so the client sees
  `404`.
- Visible non-owner owner-only action: `authorization_failed` with the action, then `403`.
- Duplicate name or recording: the schema's unique violation is logged once by `error_mapping` as `constraint_rejected`
  (`kind=unique_violation`); the use case maps it to `409`.
- Malformed stored rows: the adapter logs `output_validation_failed` with a stable reason and returns
  `DataIntegrityViolation`, surfacing as `500`.
- Known lookups (missing playlist, missing track) are normal outcomes and are not logged as errors.
- `last_modified_at` is advanced in the use case by `MusicPlaylist::touch`, like `Gallery::touch` on item changes; the
  repository's `save` persists it.

### 11. Tests and coverage

- Domain rules through the application (`TEST-019`): name trimming/emptiness/length via gallery and playlist
  create/update; duplicate recording via PATCH; reorder permutation via PUT. No domain test module.
- Adapter: aggregate read with one/several tracks, empty playlist, ordering, filters, a medium-attached row excluded,
  `u32` overflow, malformed recording, unique name, duplicate recording index, cascade delete, add/remove/replace, and
  `last_modified_at` persistence.
- Application: list visibility and filters, create uniqueness/validation, get/alias, get one track, PATCH mixed
  authorization and atomicity, unknown/duplicate recording, remove gap, reorder valid/invalid, delete, commit/rollback,
  and the Root Admin cases.
- Handlers: every path and parser, payload shapes, HAL links, alias canonical `self`, and `200`/`201`/`204`/`400`/`401`/
  `403`/`404`/`409`/`422`.
- E2E: one file per UC-013..UC-019 for the metadata lifecycle, list visibility, duplicate name, malformed identifiers,
  and unauthenticated access. Music fixtures are absent on the E2E server, so the add/remove/reorder happy paths stay in
  adapter/application tests; the empty track collection and curation error paths are still exercised where possible.
- Regenerate `docs/openapi.json`; run `cargo test`, clippy, fmt, coverage, and `devenv test`.

## Risks / Trade-offs

- **Public playlists are collaborative** → Any authenticated caller can add, remove, or reorder tracks of a public
  playlist, including emptying it. This is the chosen gallery parity; rename, visibility, and delete stay owner-only. A
  future read-only sharing model needs a new flag or policy, not a silent behavior change.
- **The alias returns the parent representation** → `GET /{id}/track` and the playlist's `tracks` link return the
  playlist, not a separate `{ items: [...] }` collection. `API-042` keeps `self` canonical. If a client expects a track
  collection, it must read `tracks[]`. Document this in the Endpoint docs and OpenAPI description.
- **Zero-based playlist indices differ from one-based album positions** → Intentional, mirroring gallery item indices.
  The path parsers and representations must not mix them.
- **Index reuse after removing the last track** → Appending at `MAX + 1` can reuse the highest removed index. Reorder
  already rewrites addresses, so absolute address stability is not promised; a removed index can later name a new
  occurrence. A monotonic counter would avoid it but adds state for little benefit.
- **Unique names plus gallery trimming** → Existing gallery names with surrounding whitespace may not collide with their
  trimmed equivalents. No migration is required; a one-time normalization can follow if it matters.
- **PATCH carries mixed authorization** → The use case must check readability first, then ownership for metadata, and
  apply nothing on failure. Tests should cover the mixed request explicitly.
- **Concurrent appends** → Two appends can collide on the position index even when their recordings differ. The request
  fails with `409`; the client retries. This matches the gallery's `MAX + 1` approach.
- **In-memory pagination** → The list reads all matching playlists and pages after the visibility retain, as galleries
  do. Acceptable for a home server; revisit if the collection grows.
- **Editing the initial migrations** → Valid only while `PERS-014` allows it. The database engineer must confirm the
  schema is unreleased before the edit lands; an already-released schema would need a different plan. A database that
  already applied the old definitions will not re-run them, so local and development databases must be recreated before
  testing. A fresh in-memory test proves the edited definitions, but it cannot detect a production checksum divergence.
- **User deletion** → The playlist rows are not cascaded from `user`; UC-005 must orchestrate their removal. This change
  leaves that coordination explicit.

## Migration Plan

1. Edit `migrations/20260904012848_create_music_playlist.up.sql` and
   `migrations/20260904012849_create_music_track.up.sql` in place. No new migration file and no `ALTER TABLE`. Confirm
   `PERS-014`'s unreleased condition first, then verify the edited definitions on a fresh in-memory database.
2. Update the ER diagram (`PERS-011`) and the partial-index paragraph in `TechnicalDesign.md`.
3. Land the domain services and models, then the repository port/adapter and `LibraryUnitOfWork`.
4. Land the application ports/use cases/factory.
5. Land the REST representations, handlers, routes, and error mappings; regenerate `docs/openapi.json`.
6. Update the use case documentation and the overview model/glossary.
7. Run adapter tests on a fresh in-memory database, then the full unit, handler, E2E, coverage, lint, and format gates.
8. Rollback: revert the change. The existing down migrations drop the playlist and track tables, so the new surface is
   removed without touching recordings, audio, or files.

## Open Questions

None that change the specs, approach, or task breakdown.
