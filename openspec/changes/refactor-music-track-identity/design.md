# Design

## Context

See `proposal.md` — Why. The current album read issues `search` (album, medium, track, recording, audio, file) and then
`search_tracks` per medium, and the use case resolves each track's recording against the medium by `recording_id`. The
archived album change accepted that because `uq_music_medium_music_album_id` caps an album at one medium, but the two
queries are two projections of the same `music_track` row: the first drops `music_track_id` and `track_index`, the
second drops the recording columns.

The playlist model already repeats the mistake (`MusicPlaylist.tracks: Vec<MusicRecording>`), and the shared
`music_track` table has two nullable container FKs with no XOR `CHECK` and no unique position. The playlist session is
separate; this change fixes the model it will inherit.

Constraints from the rulebook: the hexagonal direction (`AGENTS.md`), `STY-RUST-080` (filter fields), `STY-RUST-081`
(the classic `search` returns the aggregate), `STY-RUST-084` (domain names, not columns), `PERS-011` (ER diagram),
`PERS-014` (migrations mutable until release), `API-032`/`API-042` (HAL self links), `TEST-019` (domain models are
tested through the application layer), and `TEST-046`/`TEST-047` (shared test helpers).

## Goals / Non-Goals

**Goals:**

- Make the recording the item a container yields; make its placement a typed index on the recording.
- Preserve the placement identity and position in the aggregate read without a second query or an in-memory re-join.
- Address mediums and album tracks by their indexes, with the recording identifier exposed alongside.
- Enforce the identity and one-container invariants in the schema.
- Leave the playlist aggregate with the correct shape for the playlist session.

**Non-Goals:**

- Playlist endpoints, curation, ownership, or visibility; the playlist repository and use cases.
- A recording write path, or a canonical recording read with no placement.
- Index-based identity for albums or playlists themselves; they keep their surrogate identifiers.
- Multi-medium ingestion or a medium write path. The read and schema support multiple mediums; no endpoint writes them.
- Reordering semantics beyond leaving gaps in the stored indexes.

## Decisions

### 1. The recording is the item; the index is the placement

```rust
// domain/model/music_recording.rs
pub enum TrackIndex {
    MediumIndex(u8),
    PlaylistIndex(u32),
}

pub struct MusicRecording {
    id: NumericID,
    name: Option<String>,
    first_release_date: NaiveDate,
    isrc_code: Option<String>,
    audio_id: NumericID,
    file_id: NumericID,
    music_group_ids: Option<Vec<NumericID>>,
    index: TrackIndex,
}

// domain/model/music_medium.rs
pub enum MediumIndex {
    Cd(u8),
}

pub struct MusicMedium {
    index: MediumIndex,
    tracks: Vec<MusicRecording>,
}

pub struct MusicPlaylist {
    // ...
    tracks: Vec<MusicRecording>,
}
```

`MusicTrack`, `MusicTrackPosition`, and `MusicTrackError` are deleted; `domain/model/music_track.rs` is removed from
`model.rs`. `TrackIndex` lives beside `MusicRecording`; `MediumIndex` beside `MusicMedium`.

The value inside a container is an **occurrence**: the shared recording fields plus the placement of this occurrence. It
is not the canonical recording entity. Consequences:

- Equality includes the index, so two occurrences of one recording are distinct. Look up a placement by its index; the
  recording id is shared.
- The model does not own the recording's lifecycle. A future write path must update the recording columns and the
  placement row separately; the repository must not offer a whole-occurrence upsert that rewrites shared fields when
  only the placement changed.
- `MusicRecording` does not get a mutable recording accessor used for writes through a placement.

Alternatives rejected:

- A `PlacedRecording { recording, placement }` wrapper. The user explicitly wants the recording itself to carry the
  index; the wrapper only renames the placement.
- `MusicTrack { recording_id }` plus `MusicAlbum { recordings }`. It keeps the reference direction clean but
  reintroduces an in-memory lookup and a second collection in the aggregate.
- A port-level `MusicTrackDetail`. `STY-RUST-081` keeps the classic `search` on the aggregate; the archived design
  already rejected the projection.
- A parallel track-id/index list. Two sources of truth for the same placement.

### 2. Medium type and album-scoped index

`MediumIndex::Cd(u8)` for now. The adapter maps stored `type = 'CD'` and `medium_index` to `Cd(index)`; any other type
is a data-integrity failure until a variant is added. `medium_type` and `medium_index` collapse into one domain value.

The medium identity is `(music_album_id, medium_index)`; the type is descriptive. Two albums can both hold `Cd(2)`; one
album cannot hold `Cd(2)` and `Vinyl(2)`. The unique constraint replaces `uq_music_medium_music_album_id`:

```sql
CONSTRAINT uq_music_medium_album_medium_index UNIQUE (music_album_id, medium_index)
```

Adding a medium type later means adding an enum variant, extending the stored-type `CHECK`, and adding tests; the API
path does not change because the path carries only the `u8`.

Alternatives rejected: `(music_album_id, type, medium_index)` (the user confirmed album-only scoping); `medium_id` in
the path (the index is the identity); a global medium index (proposal says album scope).

### 3. Index identity; surrogate keys are internal

`music_medium_id` and `music_track_id` remain the physical keys for joins and updates but never reach the domain
representation or the API. The addressable identity is:

- medium: `(album_id, medium_index)` — unique by the constraint above;
- album track: `(album_id, medium_index, track_index)`;
- playlist track: `(playlist_id, track_index)`.

The schema enforces the track positions:

```sql
CREATE UNIQUE INDEX uq_music_track_medium_track_index
    ON music_track (music_medium_id, track_index)
    WHERE music_medium_id IS NOT NULL;

CREATE UNIQUE INDEX uq_music_track_playlist_track_index
    ON music_track (music_playlist_id, track_index)
    WHERE music_playlist_id IS NOT NULL;
```

Removing a track leaves a gap; reindexing would change the address. Gaps are the chosen semantics. Playlist reordering
is out of scope, but the unique index means a future reorder must preserve uniqueness at every statement.

The `track_index` is an unsigned value: `u8` for a medium placement, `u32` for a playlist placement. The DB column stays
`INTEGER`; the adapter converts and returns `DataIntegrityViolation` on overflow, and the path parser accepts `u8` for
album paths and `u32` for playlist paths.

Alternatives rejected: keeping `music_track_id` as the API id (the user chose index identity); addressing by index
without a unique constraint (ambiguous paths); reindexing on removal (unstable addresses).

### 4. One aggregate read; delete `search_tracks`

`MusicAlbumRepository` keeps only `search`. `MusicTrackFilter`, `MusicTrackRow`, and `search_tracks` are deleted;
`sqlx/model/music_track.rs` and its module entry go.

`MusicAlbumRow` gains the medium index/type and the track index, keeps the medium id only to group rows, and does not
select `music_track_id`:

```text
music_album_id, name, release_date, production_type, genres,
music_medium_id, medium_index, medium_type,
music_recording_id, recording_name, first_release_date, isrc_code, audio_id, file_id,
track_index
```

Hydration groups by album, then by `music_medium_id` (the domain identity is `MediumIndex`), and builds
`MusicRecording { .., index: TrackIndex::MediumIndex(track_index) }` for every track row. It validates the stored type,
the `u8` conversions, and every recording field; any failure is `RepositoryError::DataIntegrityViolation`. A medium with
no track yields an empty `tracks` vector; a playlist-attached row cannot join through `music_medium`.

The ordering is `music_album_id, medium_index, track_index`, so the album representation and each medium's collection
are stable. The use cases iterate; they never look a recording up by id.

Alternatives rejected: keeping `search_tracks` with a recording join (a second hydration path that can drift);
per-medium queries from the application (the archived N+1); a repository read model (`STY-RUST-081`).

### 5. API surface

```text
GET /api/v1/library/music/album
GET /api/v1/library/music/album/{album_id}/medium/{medium_index}
GET /api/v1/library/music/album/{album_id}/medium/{medium_index}/track
GET /api/v1/library/music/album/{album_id}/medium/{medium_index}/track/{track_index}
```

The old `/album/{album_id}/track[/{track_id}]` routes are removed.

- Album listing item: identifier, name, release date, production type, genres, total track count, and a `mediums` array.
  Each medium carries `index` (`u8`), `type` (`"CD"`), `track_count`, and `_links.self`/`_links.tracks`.
- Medium representation: `index`, `type`, `track_count`, `_links.self`, `_links.album`, `_links.tracks`. Served as
  `application/hal+json`.
- Track representation: `id` = track index, `recording_id`, `album_id`, `medium_index`, `file_id`, `name`, `isrc_code`,
  `first_release_date`, and `_links.self`/`_links.album`/`_links.medium`. `track_index` is no longer a separate field
  because `id` is the index.
- Path parsers parse `album_id` as `NumericID`, `medium_index`/album `track_index` as `u8`; malformed values return
  `400` with a stable message. Unknown album, medium, or track returns `404`.

New application surface: `GetAlbumMedium` (command, response, error, factory accessor, handler) for the medium resource.
`ListAlbumsItem` gains a mediums collection; `ListAlbumTracksItem` and `GetAlbumTrackResponse` gain `recording_id`,
`medium_index`, and index-based identity. `ListAlbumTracksCommand` and `GetAlbumTrackCommand` gain `medium_index`;
`GetAlbumTrackCommand` renames `track_id` to `track_index`.

Alternatives rejected: a medium query parameter on the old album-level track route (ambiguous and less desirable for
caching); retaining an album-level track collection (it cannot expose a unique track address without the medium).

### 6. Schema and migration

`migrations/20260904012838_create_music_medium.up.sql`:

- replace `uq_music_medium_music_album_id` with `UNIQUE (music_album_id, medium_index)`;
- add `CHECK (medium_index BETWEEN 0 AND 255)`;
- constrain `type` to the modeled value (`CHECK (type IN ('CD'))`) until another `MediumIndex` variant exists.

`migrations/20260904012849_create_music_track.up.sql`:

- add the XOR check: exactly one of `music_medium_id` and `music_playlist_id` is non-null;
- add `CHECK (music_medium_id IS NULL OR track_index BETWEEN 0 AND 255)` so a medium position fits the `u8` domain;
- add the two partial unique indexes from decision 3.

`PERS-014` makes migrations mutable until release. If the schema is already released, add a forward migration instead of
editing these; the database engineer confirms the release state. Either way, update the ER diagram in
`docs/development/TechnicalDesign.md` (`PERS-011`) in the same change. The down migrations only drop tables; no extra
down work is needed unless a named constraint is referenced.

There is no data migration: no endpoint writes music today, so the only rows are seeds and test fixtures. The unique
indexes must be added after the seed data is consistent; the repository tests seed fresh in-memory databases.

### 7. Playlist coordination

`MusicPlaylist.tracks` becomes `Vec<MusicRecording>`; each entry carries `TrackIndex::PlaylistIndex(u32)`. The playlist
session owns the playlist repository, endpoints, curation, ownership, and visibility. It inherits:

- the `TrackIndex` enum and the occurrence shape;
- `UNIQUE (music_playlist_id, track_index)` and the XOR check;
- the need to resolve a recording by `recording_id` when adding a placement, which is its own read/repository concern.

The playlist endpoint addresses its tracks by `track_index`; `music_playlist_id` remains the playlist's surrogate
identity.

### 8. Validation, errors, and logging

- Stored medium type, medium index, track index, and recording fields are validated by the adapter; failures map to
  `RepositoryError::DataIntegrityViolation` and surface as the existing `500` path.
- Missing album, medium, or track is a valid read outcome, not an error: `404`.
- Malformed path values are rejected by the handler before the use case: `400`.
- No new logging sites. The adapter's existing error mapping owns the data-integrity log; the use cases keep mapping
  repository failures to `Unknown` and do not log. The deleted
  `anyhow!("the track's recording is missing from its medium")` branch removes a second classification of the same
  integrity problem.

### 9. Tests and coverage

- Adapter: `search` with one and several mediums, track/medium ordering, two tracks pointing at one recording, a
  playlist-attached row excluded, a medium with no track, unknown stored type, out-of-range medium/track index,
  malformed recording. Delete the `search_tracks` tests.
- Application: list albums (mediums and total count), list a medium's tracks, get one track by index, unknown
  album/medium/track, repository failure, commit/rollback. `TEST-019` keeps domain validation tested here; no domain
  test module.
- Handlers: new paths and parsers, payload shapes and HAL links, `400`/`404`/`401`, the medium resource.
- E2E UC-012: update the paths and fixtures, add the medium and malformed-index cases; no seeded music, so the
  happy-path coverage stays in the adapter/application tests.
- Regenerate `docs/openapi.json`; run `cargo test`, clippy, fmt, coverage, and `devenv test`.

## Risks / Trade-offs

- **Breaking API** → The album track `id` becomes an index, the medium enters the path, and `recording_id` is added. The
  OpenAPI spec, handler tests, and E2E fixtures change together; no compatibility shim. This is deliberate.
- **The occurrence conflates the shared entity and its placement** → Document the distinction; keep the recording fields
  immutable from a placement; any write path splits recording updates from placement updates. A canonical read with no
  placement is deferred.
- **Index addresses are unstable under reindexing** → Gaps on removal; reordering is out of scope. A future reorder must
  respect the unique indexes.
- **The medium type check blocks future types** → Adding `Vinyl`/`Cassette` adds a variant, extends the check, and adds
  tests; the path stays the same.
- **Multi-medium becomes possible** → The read already iterates mediums and the schema now permits several; add an
  adapter test with two mediums. No medium write path exists, so ingestion remains out of scope.
- **Editing existing migrations** → Allowed by `PERS-014` only until release. If released, add a forward migration; the
  database engineer verifies.
- **The album listing now carries mediums** → It already hydrates every track for `track_count`; the mediums collection
  adds little. The `ListAlbumsItem` and handler payload grow.
- **The XOR and unique indexes are shared with the playlist** → The playlist session must not duplicate them; it
  consumes the corrected table.

## Migration Plan

1. Update `create_music_medium` and `create_music_track` (or add a forward migration if already released).
2. Update the ER diagram (`PERS-011`) in `docs/development/TechnicalDesign.md`.
3. Regenerate `docs/openapi.json`.
4. Run the adapter tests against a fresh in-memory database, then the full gates.
5. Rollback is reverting the change; nothing is persisted by the read path, and no data migration is required.

## Open Questions

None that change the specs, approach, or task breakdown.
