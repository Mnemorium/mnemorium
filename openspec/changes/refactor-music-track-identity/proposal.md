# Proposal

## Why

The album-track read splits one `music_track` row into two half-models. `search` hydrates the recording into
`MusicMedium.tracks`, dropping the track identity and index; `search_tracks` re-reads the same row without its
recording; the use cases then re-join the halves in memory by `recording_id`. The playlist aggregate repeats the same
shape (`Vec<MusicRecording>`), so the playlist work will inherit a model that cannot distinguish two placements of one
recording, hides an N+1 behind the one-medium constraint, and classifies the same integrity failure two different ways.

The recording is the shared entity. A track is not an entity: it is the placement of a recording in a medium or a
playlist. The domain should say that directly.

## What Changes

- **Domain model.** `MusicRecording` carries its placement as a typed `TrackIndex`: `MediumIndex(u8)` or
  `PlaylistIndex(u32)`. The shared recording fields stay on the same type; there is no `MusicTrack`, no
  `MusicTrackPosition`, and no wrapper type.
- **Containers.** `MusicMedium` carries a typed `MediumIndex` (`Cd(u8)` for now, other medium types later) and
  `tracks: Vec<MusicRecording>`. `MusicPlaylist.tracks` becomes `Vec<MusicRecording>`.
- **Identity.** A medium is addressed by its album-scoped `medium_index`; a track by its container-scoped `track_index`.
  `music_medium_id` and `music_track_id` become internal surrogate keys. Medium uniqueness is
  `(album_id, medium_index)`; track uniqueness is `(medium_id, track_index)` and `(playlist_id, track_index)`.
- **BREAKING API.** Album track paths gain the medium: `GET /album/{album_id}/medium/{medium_index}/track` and
  `.../track/{track_index}`. The old `/album/{album_id}/track[/{track_id}]` paths are removed. A track representation's
  `id` is the track index, and `recording_id` is added. The album representation exposes its mediums.
  `docs/openapi.json` and the `library/music-albums` spec change.
- **Repository.** `MusicAlbumRepository::search_tracks`, `MusicTrackFilter`, and `MusicTrackRow` are removed. `search`
  hydrates mediums and recording occurrences in one query, so no caller stitches a track from per-medium lookups.
- **Schema.** Replace the one-medium unique constraint with `UNIQUE (music_album_id, medium_index)`, add partial unique
  indexes on `(music_medium_id, track_index)` and `(music_playlist_id, track_index)`, add the exactly-one-container
  `CHECK`, and constrain the medium index, the medium track index, and the medium type to the modeled values.
- **Use cases.** `ListAlbumTracks` and `GetAlbumTrack` become medium-scoped; `ListAlbums` exposes mediums. The in-memory
  `find` by `recording_id` and the `Unknown(anyhow!(...))` miss disappear.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library/music-albums`: album, medium, and track representations and paths change; track identity becomes the track
  index and the recording identifier is exposed; the track collection is medium-scoped; medium uniqueness and ordering
  are specified.

## Impact

- **Domain models:** `music_recording`, `music_medium`, `music_album`, `music_playlist`; delete `music_track` and
  `MusicTrackPosition`.
- **Port and adapter:** `MusicAlbumRepository`, `SqlxMusicAlbumRepository`, `MusicAlbumRow`; delete `MusicTrackRow`.
- **Application:** `ListAlbums`, `ListAlbumTracks`, `GetAlbumTrack` and their ports; `ListAlbumsItem` gains mediums; the
  track items gain `recording_id` and index-based identity.
- **REST:** handlers, payloads, path parsers, HAL links, OpenAPI.
- **Schema:** `create_music_medium` and `create_music_track` migrations; ER diagram (`PERS-011`).
- **Tests:** adapter, use cases, handlers, E2E UC-012; 80% coverage gate.
- **Out of scope:** playlist endpoints, playlist curation, ownership and visibility; a recording write path; a canonical
  recording read with no placement. The playlist session consumes `TrackIndex::PlaylistIndex` and adds its own
  repository.
- **Coordination:** `MusicPlaylist.tracks` is updated here so the playlist session inherits the corrected shape.
