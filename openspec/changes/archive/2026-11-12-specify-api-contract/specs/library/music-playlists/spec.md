# Spec Delta

## REMOVED Requirements

### Requirement: Playlist and track responses are HAL representations

**Reason**: The `api` capability now owns the response media type, the payload shape, `_links.self`, collection
navigation and the templated `search` link (`REQ-API-003`, `REQ-API-004`, `REQ-API-006`, `REQ-API-007`). Restating the
rules here duplicated them and let the copies drift. **Migration**: No client action; the served behavior is unchanged.
The rules are read from `openspec/specs/api/spec.md`.

## MODIFIED Requirements

### Requirement: List music playlists

The system SHALL return the playlists the caller's `playlist.read` permission covers through
`GET /api/v1/library/music/playlist`. The default scope SHALL include public playlists and the caller's own; the Root
Admin SHALL see every playlist; an override MAY change the set. The collection SHALL accept optional `name`,
`is_public`, and `owner_id` filters and SHALL be paginated as `api` defines.

#### Scenario: Authenticated caller lists visible playlists

- **WHEN** an authenticated caller with the default permission requests `GET /api/v1/library/music/playlist`
- **THEN** the system returns `200 OK` with a collection of the public playlists and the caller's own playlists

#### Scenario: A restricted caller lists only permitted playlists

- **WHEN** an authenticated caller whose `playlist.read` permission covers only some playlists requests the collection
- **THEN** the system returns only the playlists that permission covers

#### Scenario: A caller with no playlist access lists no playlists

- **WHEN** an authenticated caller whose `playlist.read` permission is `NONE` requests the collection
- **THEN** the system returns `200 OK` with an empty collection

#### Scenario: Playlist representation content

- **WHEN** the system returns a playlist, in a listing or on a detail read
- **THEN** the representation carries the playlist's identifier, name, visibility, owner identifier, creation and last
  modification timestamps, track count, its cover, and a `tracks` link to its tracks
- **AND** the cover is the identifier of the image holding the playlist's artwork, or `null` when the playlist stores
  none

#### Scenario: Filter and page

- **WHEN** an authenticated caller requests the collection with `name`, `is_public`, `owner_id`, `limit`, and `offset`
- **THEN** the system returns only the playlists matching the visible filters, starting at `offset` and holding at most
  `limit` items

#### Scenario: Out-of-range limit

- **WHEN** the caller requests the collection with a `limit` of 0 or greater than 100
- **THEN** the system returns `422 Unprocessable Content` with a stable message

### Requirement: Read a playlist

The system SHALL return one playlist through `GET /api/v1/library/music/playlist/{playlist_id}` with its metadata and
tracks ordered by ascending track index. `GET /api/v1/library/music/playlist/{playlist_id}/track` SHALL be an alias that
returns the same representation as the canonical playlist route.

#### Scenario: Read an existing playlist

- **WHEN** an authenticated caller who can read the playlist requests it
- **THEN** the system returns `200 OK` with the playlist metadata and its tracks in ascending index order

#### Scenario: The track alias returns the same representation

- **WHEN** a caller requests `GET /api/v1/library/music/playlist/{playlist_id}/track`
- **THEN** the system returns the same representation as `GET /api/v1/library/music/playlist/{playlist_id}`

#### Scenario: Unknown playlist

- **WHEN** the requested playlist identifier does not exist
- **THEN** the system returns `404 Not Found`

### Requirement: Read one playlist track

The system SHALL return one track through `GET /api/v1/library/music/playlist/{playlist_id}/track/{track_index}`, scoped
to the playlist named in the path. The track representation SHALL carry its index as `id`, the shared `recording_id`,
the backing `file_id`, the recording metadata, and a `playlist` link to the playlist.

#### Scenario: Track of the playlist

- **WHEN** an authenticated caller who can read the playlist requests an existing track index
- **THEN** the system returns `200 OK` with the track representation

#### Scenario: Track not in the playlist, or unknown playlist

- **WHEN** the requested track index does not exist in the playlist, or the playlist is unknown or invisible
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed track index

- **WHEN** the caller supplies a `track_index` that is not an unsigned integer
- **THEN** the system returns `400 Bad Request` with a stable message
