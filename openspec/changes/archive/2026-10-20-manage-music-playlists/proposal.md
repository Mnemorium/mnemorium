# Proposal

## Why

The playlist domain model and database table already exist, but no endpoint can create, browse, or curate a playlist.
The previous track-identity change deliberately left the playlist aggregate ready: `MusicPlaylist.tracks` is a sequence
of recording occurrences, a `music_track` row belongs to a medium or a playlist, and each container has a unique
position. Clients still cannot use any of it.

This change adds the management surface and closes the gaps the inherited model leaves open: ownership and visibility,
unique per-owner names, no duplicate recordings within one playlist, cascade on playlist deletion, and a
`last_modified_at` timestamp. It also extracts the name validation shared with galleries so both resources enforce one
policy.

## What Changes

- **New playlist API.** `GET`/`POST /api/v1/library/music/playlist`,
  `GET`/`PATCH`/`DELETE /api/v1/library/music/playlist/{playlist_id}`,
  `GET /api/v1/library/music/playlist/{playlist_id}/track` (a literal alias of the detail read),
  `GET`/`DELETE /api/v1/library/music/playlist/{playlist_id}/track/{track_index}`, and
  `PUT /api/v1/library/music/playlist/{playlist_id}/track` (reorder).
- **PATCH adds tracks.** `PATCH /playlist/{playlist_id}` accepts optional `name`, `is_public`, and
  `add_track: { recording_id }`. Name and visibility are owner-only; adding a track is allowed to any caller who can
  read the playlist. The request is atomic: an owner-only field from a non-owner, a duplicate name, or a duplicate
  recording rejects the whole request and applies nothing.
- **0-based track indices.** A playlist track is addressed by its `track_index`; the first track is `0`. Adding appends
  at `MAX(track_index) + 1`; reordering reindexes to `0..n-1`.
- **Visibility like galleries.** Public playlists are readable and track-moderatable by any authenticated caller;
  private playlists are visible to the owner and the Root Admin. Rename, visibility change, and deletion are owner-only.
  A playlist the caller cannot see answers `404`; a visible public playlist the caller cannot rename answers `403`.
- **No duplicate recordings.** A recording appears at most once in one playlist; a second add is `409`. Album mediums
  may still sequence the same recording twice.
- **Domain rules.** `NamePolicy` is shared by `Gallery` and `MusicPlaylist` (names are trimmed, non-empty, and at most
  100 characters). `MusicPlaylist` owns the duplicate-recording and reorder-permutation rules.
- **Persistence.** The two initial migrations are edited in place under `PERS-014`: `music_playlist` gains
  `last_modified_at`, `is_public DEFAULT 0`, and `UNIQUE (user_id, name)`; `music_track` gains `ON DELETE CASCADE` on
  the playlist foreign key and a partial unique index on `(music_playlist_id, music_recording_id)`. No new migration
  file and no `ALTER TABLE` are introduced. The entity-relationship diagram is updated.
- **Read model.** One aggregate read hydrates a playlist and its ordered recording occurrences in a single query,
  mirroring the album read. A new playlist repository and `LibraryUnitOfWork` accessor back the list, create, get,
  get-one-track, update, delete, remove-track, and reorder use cases.
- **HAL and OpenAPI.** Playlist and track representations, collection links, error mappings, and `docs/openapi.json` are
  added or regenerated.
- **Gallery name behavior.** Gallery names are now trimmed by the shared policy, matching what the create-gallery
  documentation already promises. No gallery spec capability exists today; the adoption is recorded in the design and
  tasks rather than a spec delta.

## Capabilities

### New Capabilities

- `library/music-playlists`: an authenticated caller can list, create, read, update, delete, and curate owner-scoped
  music playlists whose ordered tracks reference shared recordings.

### Modified Capabilities

- None. The album capability is not changing, and the gallery has no existing spec capability to modify. The gallery
  name-policy adoption is an implementation-level behavior correction recorded in the design and tasks.

## Impact

- **Domain models:** `music_playlist`, `gallery` (name-policy adoption), and new `domain/service/name_policy.rs`.
- **Ports and adapters:** new `MusicPlaylistRepository` port, `LibraryUnitOfWork::music_playlists`,
  `SqlxMusicPlaylistRepository`, and a SQLx row model.
- **Application:** new ports, use cases, and factory accessors for list, create, get, get track, update, delete, remove,
  and reorder; `UpdatePlaylist` owns the PATCH add-track path.
- **REST:** new handler files and routes under `handler/library`, playlist and track representations and links, error
  mappings, and OpenAPI regeneration.
- **Schema:** the initial `create_music_playlist` and `create_music_track` migrations edited in place, and the ER
  diagram (`PERS-011`). No new migration file and no `ALTER TABLE`.
- **Documentation:** new use cases in `UseCases.md`, model and glossary alignment in `Overview.md`, and persistence
  notes (the ER diagram and partial-index paragraph) in `TechnicalDesign.md`.
- **Tests:** adapter, application, handler, and E2E coverage; the 80% coverage gate still applies.
- **Coordination:** the main `library/music-albums` spec is stale relative to the implemented medium-scoped album API.
  The unarchived `refactor-music-track-identity` change owns that sync; this change assumes the implemented
  medium-scoped album surface.

**Out of scope:** deleting a user's playlists (the UC-005 orchestration), sorting or filtering by `last_modified_at`,
ETags and conditional requests, a recording write path, playlist import/export, and any change to album reads. Duplicate
recordings remain allowed in album mediums.
