# library/music-playlists Specification

## Purpose

Lets an authenticated caller create, browse, update, delete, and curate owner-scoped music playlists whose ordered
tracks reference shared recordings.

## Requirements

### Requirement: Playlist endpoints require an authenticated account

Every music playlist endpoint SHALL require a valid bearer token, and a token that names an account which no longer
exists SHALL be rejected.

#### Scenario: Missing or invalid credentials

- **WHEN** a request reaches a music playlist endpoint without a valid bearer token
- **THEN** the system returns `401 Unauthorized`

#### Scenario: Account no longer exists

- **WHEN** an authenticated request reaches a music playlist endpoint but the caller's account no longer exists
- **THEN** the system returns `401 Unauthorized`

### Requirement: Playlist path identifiers are validated

A `playlist_id` path segment that is not a valid identifier SHALL be rejected with `400 Bad Request` before any use case
runs, and the rejection SHALL carry a stable message.

#### Scenario: Malformed playlist identifier

- **WHEN** a caller supplies a `playlist_id` that is not a valid identifier on any playlist route
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: Playlist reads follow visibility

Any authenticated caller SHALL read a public playlist. A private playlist SHALL be readable only by its owner and the
Root Admin. A playlist the caller cannot read SHALL answer `404 Not Found`.

#### Scenario: Authenticated caller reads a public playlist

- **WHEN** an authenticated caller requests a public playlist
- **THEN** the system returns `200 OK` with the playlist representation and never `403 Forbidden`

#### Scenario: Owner reads a private playlist

- **WHEN** the owner requests its private playlist
- **THEN** the system returns `200 OK` with the playlist representation

#### Scenario: Root Admin reads a private playlist

- **WHEN** the Root Admin requests another user's private playlist
- **THEN** the system returns `200 OK` with the playlist representation

#### Scenario: Another caller cannot see a private playlist

- **WHEN** an authenticated caller who is neither the owner nor the Root Admin requests a private playlist
- **THEN** the system returns `404 Not Found` and does not distinguish the playlist from a missing one

### Requirement: Track moderation follows playlist visibility

Adding, removing, and reordering tracks SHALL be allowed to any caller who can read the playlist. On a private playlist
this means the owner and the Root Admin. A caller who cannot read the playlist SHALL receive `404 Not Found`.

#### Scenario: Any authenticated caller moderates a public playlist

- **WHEN** an authenticated caller who is not the owner adds, removes, or reorders a track of a public playlist
- **THEN** the system applies the change and returns the documented success response

#### Scenario: Owner or Root Admin moderates a private playlist

- **WHEN** the owner or the Root Admin adds, removes, or reorders a track of a private playlist
- **THEN** the system applies the change

#### Scenario: Another caller cannot moderate a private playlist

- **WHEN** an authenticated caller who is neither the owner nor the Root Admin tries to add, remove, or reorder a track
  of a private playlist
- **THEN** the system returns `404 Not Found` and applies nothing

### Requirement: Playlist metadata and deletion are owner-only

Only the owner SHALL rename a playlist, change its visibility, or delete it. A caller who can read a public playlist but
is not its owner SHALL receive `403 Forbidden`, including the Root Admin.

#### Scenario: Owner changes metadata or deletes

- **WHEN** the owner renames or changes the visibility of a playlist, or deletes it
- **THEN** the system applies the change

#### Scenario: Non-owner cannot change metadata or delete

- **WHEN** a caller who can read a public playlist but is not its owner renames it, changes its visibility, or deletes
  it
- **THEN** the system returns `403 Forbidden` and applies nothing

#### Scenario: Root Admin is not the owner

- **WHEN** the Root Admin tries to rename, change the visibility of, or delete another user's playlist
- **THEN** the system returns `403 Forbidden`

### Requirement: List music playlists

The system SHALL return the playlists the caller can see through `GET /api/v1/library/music/playlist`. The collection
SHALL include public playlists and the caller's own playlists; the Root Admin SHALL see every playlist. The collection
SHALL accept optional `name`, `is_public`, and `owner_id` filters and optional `limit` and `offset` pagination. `limit`
SHALL default to 20 and SHALL be between 1 and 100.

#### Scenario: Authenticated caller lists visible playlists

- **WHEN** an authenticated caller requests `GET /api/v1/library/music/playlist`
- **THEN** the system returns `200 OK` with a HAL collection of the public playlists and the caller's own playlists

#### Scenario: Playlist representation content

- **WHEN** the system returns a playlist in a listing
- **THEN** the representation carries the playlist's identifier, name, visibility, owner identifier, creation and last
  modification timestamps, track count, and links to itself and its tracks

#### Scenario: Filter and page

- **WHEN** an authenticated caller requests the collection with `name`, `is_public`, `owner_id`, `limit`, and `offset`
- **THEN** the system returns only the playlists matching the visible filters, starting at `offset` and holding at most
  `limit` items
- **AND** the collection's `_links.self` preserves the active filters and page

#### Scenario: Out-of-range limit

- **WHEN** the caller requests the collection with a `limit` of 0 or greater than 100
- **THEN** the system returns `422 Unprocessable Content` with a stable message

### Requirement: Create a playlist

The system SHALL create a playlist owned by the caller through `POST /api/v1/library/music/playlist`. The request SHALL
carry a name and an optional `is_public` flag that defaults to `false`. A successful creation SHALL return
`201 Created`, a `Location` header, and the playlist representation.

#### Scenario: Create a private playlist

- **WHEN** an authenticated caller creates a playlist without `is_public`
- **THEN** the system returns `201 Created` with the playlist representation, which is private
- **AND** the `Location` header names the created playlist

#### Scenario: Create a public playlist

- **WHEN** an authenticated caller creates a playlist with `is_public` set to `true`
- **THEN** the stored playlist is public

#### Scenario: Duplicate name

- **WHEN** the caller already owns a playlist with the requested normalized name
- **THEN** the system returns `409 Conflict` and creates nothing

#### Scenario: Invalid name

- **WHEN** the requested name is empty, whitespace only, or longer than 100 characters
- **THEN** the system returns `422 Unprocessable Content` and creates nothing

### Requirement: Playlist names are normalized and unique per owner

A playlist name SHALL be trimmed, non-empty, and at most 100 characters. It SHALL be unique among the playlists owned by
one user; different users MAY use the same name.

#### Scenario: Surrounding whitespace is trimmed

- **WHEN** a caller creates or renames a playlist with a name surrounded by whitespace
- **THEN** the stored and returned name is the trimmed value

#### Scenario: Two owners share a name

- **WHEN** two different users each create a playlist with the same name
- **THEN** both creations succeed

#### Scenario: The same owner repeats a name

- **WHEN** a caller creates or renames a playlist to a name already used by another playlist it owns
- **THEN** the system returns `409 Conflict`

### Requirement: Read a playlist

The system SHALL return one playlist through `GET /api/v1/library/music/playlist/{playlist_id}` with its metadata and
tracks ordered by ascending track index. `GET /api/v1/library/music/playlist/{playlist_id}/track` SHALL be an alias that
returns the same representation with the canonical playlist `_links.self`.

#### Scenario: Read an existing playlist

- **WHEN** an authenticated caller who can read the playlist requests it
- **THEN** the system returns `200 OK` with the playlist metadata and its tracks in ascending index order

#### Scenario: The track alias returns the same representation

- **WHEN** a caller requests `GET /api/v1/library/music/playlist/{playlist_id}/track`
- **THEN** the system returns the same representation as `GET /api/v1/library/music/playlist/{playlist_id}`, whose
  `_links.self` is the canonical playlist URI

#### Scenario: Unknown playlist

- **WHEN** the requested playlist identifier does not exist
- **THEN** the system returns `404 Not Found`

### Requirement: Read one playlist track

The system SHALL return one track through `GET /api/v1/library/music/playlist/{playlist_id}/track/{track_index}`, scoped
to the playlist named in the path. The track representation SHALL carry its index as `id`, the shared `recording_id`,
the backing `file_id`, the recording metadata, and links to itself and the playlist.

#### Scenario: Track of the playlist

- **WHEN** an authenticated caller who can read the playlist requests an existing track index
- **THEN** the system returns `200 OK` with the track representation

#### Scenario: Track not in the playlist, or unknown playlist

- **WHEN** the requested track index does not exist in the playlist, or the playlist is unknown or invisible
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed track index

- **WHEN** the caller supplies a `track_index` that is not an unsigned integer
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: Playlist tracks are 0-based and ordered by index

A playlist track SHALL be addressed by a zero-based `track_index`. The first added track SHALL receive index `0`; adding
SHALL append at the next index; the collection SHALL be ordered by ascending index so repeated reads return a stable
order.

#### Scenario: The first track has index zero

- **WHEN** a track is added to an empty playlist
- **THEN** its `track_index` and representation `id` are `0`

#### Scenario: Adding appends at the end

- **WHEN** a track is added to a playlist whose highest index is `n`
- **THEN** the new track receives index `n + 1`

#### Scenario: Ordering is stable

- **WHEN** an authenticated caller reads the same playlist more than once
- **THEN** its tracks appear in ascending index order every time

### Requirement: Update playlist metadata

`PATCH /api/v1/library/music/playlist/{playlist_id}` SHALL accept an optional `name` and an optional `is_public` and
return `200 OK` with the updated playlist. An omitted field SHALL remain unchanged. A request without any field SHALL
change no metadata and SHALL return `200 OK` with the playlist. A successful update SHALL advance `last_modified_at`,
including a request that changes no metadata.

#### Scenario: Rename a playlist

- **WHEN** the owner sends a new valid and unused name
- **THEN** the system stores the trimmed name and returns `200 OK` with the updated playlist

#### Scenario: Change visibility

- **WHEN** the owner sends a new `is_public` value
- **THEN** the system stores the value and returns `200 OK`

#### Scenario: Empty update

- **WHEN** the owner sends a body with no fields
- **THEN** the system returns `200 OK` with the playlist, whose metadata is unchanged and whose `last_modified_at` is
  advanced

#### Scenario: Duplicate or invalid name

- **WHEN** the owner sends a name already used by another playlist it owns, or an empty or over-long name
- **THEN** the system returns `409 Conflict` or `422 Unprocessable Content` respectively and applies nothing

#### Scenario: Non-owner updates metadata

- **WHEN** a caller who can read a public playlist but is not its owner sends `name` or `is_public`
- **THEN** the system returns `403 Forbidden` and applies nothing

### Requirement: Add a recording through PATCH

`PATCH /api/v1/library/music/playlist/{playlist_id}` SHALL accept an optional `add_track` object carrying a
`recording_id` and SHALL append one occurrence of that recording. The recording SHALL exist. The response SHALL be the
updated playlist including the new track. The whole request SHALL be atomic.

#### Scenario: Add a recording

- **WHEN** a caller who can read the playlist sends `add_track` with an existing recording that is not already in the
  playlist
- **THEN** the system appends the track and returns `200 OK` with the updated playlist

#### Scenario: Unknown recording

- **WHEN** `add_track.recording_id` names a recording that does not exist
- **THEN** the system returns `422 Unprocessable Content` and applies nothing

#### Scenario: Duplicate recording

- **WHEN** `add_track.recording_id` names a recording already present in the playlist
- **THEN** the system returns `409 Conflict` and applies nothing

#### Scenario: Mixed authorization is atomic

- **WHEN** a caller who can read a public playlist but is not its owner sends `add_track` together with `name` or
  `is_public`
- **THEN** the system returns `403 Forbidden` and applies neither change

### Requirement: No duplicate recordings within a playlist

A recording SHALL appear at most once in one playlist. A second add of the same recording SHALL return `409 Conflict`.
Album mediums MAY still sequence the same recording more than once.

#### Scenario: The same recording in two playlists

- **WHEN** a recording already present in one playlist is added to a different playlist
- **THEN** the add succeeds

#### Scenario: Album mediums are unaffected

- **WHEN** an album medium sequences the same recording twice
- **THEN** the album read still returns both occurrences

### Requirement: Remove a track from a playlist

`DELETE /api/v1/library/music/playlist/{playlist_id}/track/{track_index}` SHALL remove the addressed occurrence and
return `204 No Content`, leaving the remaining indices unchanged. The shared recording and its media SHALL survive. A
successful removal SHALL advance `last_modified_at`.

#### Scenario: Remove an existing track

- **WHEN** a caller who can read the playlist deletes an existing track index
- **THEN** the system returns `204 No Content`
- **AND** the remaining tracks keep their indices, leaving a gap

#### Scenario: Unknown track or invisible playlist

- **WHEN** the track index does not exist in the playlist, or the playlist is unknown or invisible
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed track index

- **WHEN** the caller supplies a `track_index` that is not an unsigned integer
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: Reorder playlist tracks

`PUT /api/v1/library/music/playlist/{playlist_id}/track` SHALL accept a complete permutation of the playlist's current
track indices and SHALL reindex the tracks to `0..n-1` in the requested order, returning `200 OK` with the updated
playlist. An incomplete, duplicate, or unknown set SHALL return `422 Unprocessable Content`. A successful reorder SHALL
advance `last_modified_at`.

#### Scenario: Reorder a playlist

- **WHEN** a caller who can read the playlist sends every current track index exactly once in a new order
- **THEN** the system stores the new order, reindexes the tracks to `0..n-1`, and returns `200 OK` with the updated
  playlist

#### Scenario: Invalid order

- **WHEN** the body omits a current index, repeats an index, or names an index the playlist does not hold
- **THEN** the system returns `422 Unprocessable Content` and leaves the order unchanged

#### Scenario: Empty playlist

- **WHEN** the playlist has no tracks and the body carries an empty order
- **THEN** the system returns `200 OK` with the unchanged empty playlist

#### Scenario: Invisible playlist

- **WHEN** the caller cannot read the playlist
- **THEN** the system returns `404 Not Found`

### Requirement: Delete a playlist

`DELETE /api/v1/library/music/playlist/{playlist_id}` SHALL delete the playlist and its track placements and return
`204 No Content`. The shared recordings, audio, and files SHALL survive.

#### Scenario: Owner deletes a playlist

- **WHEN** the owner deletes the playlist
- **THEN** the system returns `204 No Content`
- **AND** the playlist and its track placements no longer exist
- **AND** the recordings, audio, and files it referenced still exist

#### Scenario: Visible non-owner deletes a playlist

- **WHEN** a caller who can read a public playlist but is not its owner deletes it
- **THEN** the system returns `403 Forbidden` and deletes nothing

#### Scenario: Invisible playlist

- **WHEN** the caller cannot read the playlist
- **THEN** the system returns `404 Not Found`

### Requirement: Playlist and track responses are HAL representations

Playlist and track success responses SHALL be served as `application/hal+json`, keep business data at the root of the
payload, and carry a `_links.self` naming the resource's canonical URI. A playlist SHALL expose `self` and `tracks`
links; the playlist collection SHALL expose `self`, `next`, `prev`, and a templated `search` link; a track SHALL expose
`self` and `playlist`.

#### Scenario: Playlist self link

- **WHEN** the system returns a playlist representation
- **THEN** `_links.self` names the canonical playlist URI
- **AND** `_links.tracks` names the playlist's track alias URI

#### Scenario: Collection navigation and search

- **WHEN** the system returns a playlist collection page
- **THEN** `_links.self` preserves the active filters and page
- **AND** `_links.search` is templated with the supported filters
- **AND** `_links.next` and `_links.prev` appear when another page exists in that direction

#### Scenario: Track links

- **WHEN** the system returns a track representation
- **THEN** `_links.self` names the track's canonical URI and `_links.playlist` names its playlist

#### Scenario: Alias uses the canonical self

- **WHEN** the system answers the track-alias route
- **THEN** `_links.self` names the canonical playlist URI, not the alias path
