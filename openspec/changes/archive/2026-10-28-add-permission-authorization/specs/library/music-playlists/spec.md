# Spec Delta

## MODIFIED Requirements

### Requirement: Playlist reads follow visibility

Any authenticated caller SHALL read a playlist its `playlist.read` permission covers. The default scope SHALL be
`PUBLIC_OR_OWNED`: public playlists and the caller's own. A per-user override MAY widen the scope to `ALL` or `LISTED`
or narrow it to `OWNED` or `NONE`. A playlist the caller cannot read SHALL answer `404 Not Found`.

#### Scenario: Authenticated caller reads a public playlist

- **WHEN** an authenticated caller with the default permission requests a public playlist
- **THEN** the system returns `200 OK` with the playlist representation and never `403 Forbidden`

#### Scenario: Owner reads a private playlist

- **WHEN** the owner requests its private playlist
- **THEN** the system returns `200 OK` with the playlist representation

#### Scenario: Root Admin reads a private playlist

- **WHEN** the Root Admin requests another user's private playlist
- **THEN** the system returns `200 OK` with the playlist representation

#### Scenario: Another caller cannot see a private playlist

- **WHEN** an authenticated caller who is neither the owner nor the Root Admin requests a private playlist it cannot
  read
- **THEN** the system returns `404 Not Found` and does not distinguish the playlist from a missing one

#### Scenario: A listed override shares a private playlist

- **WHEN** a caller whose `playlist.read` override is `LISTED` naming a private playlist requests that playlist
- **THEN** the system returns `200 OK` with the playlist representation

#### Scenario: A none override hides a public playlist

- **WHEN** a caller whose `playlist.read` override is `NONE` requests a public playlist
- **THEN** the system returns `404 Not Found`

#### Scenario: An owned override hides another user's public playlist

- **WHEN** a caller whose `playlist.read` override is `OWNED` requests another user's public playlist
- **THEN** the system returns `404 Not Found`

### Requirement: Track moderation follows playlist visibility

Adding, removing, and reordering tracks SHALL be allowed to a caller whose `playlist.moderate` permission covers the
playlist. The default scope SHALL be `PUBLIC_OR_OWNED`, so any caller who can read a public or owned playlist may
moderate it. A caller who cannot read the playlist SHALL receive `404 Not Found`; one who can read it but cannot
moderate it SHALL receive `403 Forbidden`.

#### Scenario: Any authenticated caller moderates a public playlist

- **WHEN** an authenticated caller with the default permission who is not the owner adds, removes, or reorders a track
  of a public playlist
- **THEN** the system applies the change and returns the documented success response

#### Scenario: Owner or Root Admin moderates a private playlist

- **WHEN** the owner or the Root Admin adds, removes, or reorders a track of a private playlist
- **THEN** the system applies the change

#### Scenario: Another caller cannot moderate a private playlist

- **WHEN** an authenticated caller who is neither the owner nor the Root Admin tries to add, remove, or reorder a track
  of a private playlist it cannot read
- **THEN** the system returns `404 Not Found` and applies nothing

#### Scenario: A readable caller without moderation is forbidden

- **WHEN** a caller whose `playlist.read` permission covers a playlist but whose `playlist.moderate` permission does not
  tries to add, remove, or reorder a track
- **THEN** the system returns `403 Forbidden` and applies nothing

### Requirement: Playlist metadata and deletion are owner-only

Only the owner or the Root Admin SHALL rename a playlist, change its visibility, or delete it. A caller who can read a
public playlist but is neither its owner nor the Root Admin SHALL receive `403 Forbidden`.

#### Scenario: Owner changes metadata or deletes

- **WHEN** the owner renames or changes the visibility of a playlist, or deletes it
- **THEN** the system applies the change

#### Scenario: Non-owner cannot change metadata or delete

- **WHEN** a caller who can read a public playlist but is neither its owner nor the Root Admin renames it, changes its
  visibility, or deletes it
- **THEN** the system returns `403 Forbidden` and applies nothing

#### Scenario: Root Admin is not the owner

- **WHEN** the Root Admin tries to rename, change the visibility of, or delete another user's playlist
- **THEN** the system applies the change

### Requirement: List music playlists

The system SHALL return the playlists the caller's `playlist.read` permission covers through
`GET /api/v1/library/music/playlist`. The default scope SHALL include public playlists and the caller's own; the Root
Admin SHALL see every playlist; an override MAY change the set. The collection SHALL accept optional `name`,
`is_public`, and `owner_id` filters and optional `limit` and `offset` pagination. `limit` SHALL default to 20 and SHALL
be between 1 and 100.

#### Scenario: Authenticated caller lists visible playlists

- **WHEN** an authenticated caller with the default permission requests `GET /api/v1/library/music/playlist`
- **THEN** the system returns `200 OK` with a HAL collection of the public playlists and the caller's own playlists

#### Scenario: A restricted caller lists only permitted playlists

- **WHEN** an authenticated caller whose `playlist.read` permission covers only some playlists requests the collection
- **THEN** the system returns only the playlists that permission covers

#### Scenario: A caller with no playlist access lists no playlists

- **WHEN** an authenticated caller whose `playlist.read` permission is `NONE` requests the collection
- **THEN** the system returns `200 OK` with an empty collection

#### Scenario: Playlist representation content

- **WHEN** the system returns a playlist, in a listing or on a detail read
- **THEN** the representation carries the playlist's identifier, name, visibility, owner identifier, creation and last
  modification timestamps, track count, its cover, and links to itself and its tracks
- **AND** the cover is the identifier of the file holding the playlist's artwork, or `null` when the playlist stores
  none

#### Scenario: Filter and page

- **WHEN** an authenticated caller requests the collection with `name`, `is_public`, `owner_id`, `limit`, and `offset`
- **THEN** the system returns only the playlists matching the visible filters, starting at `offset` and holding at most
  `limit` items
- **AND** the collection's `_links.self` preserves the active filters and page

#### Scenario: Out-of-range limit

- **WHEN** the caller requests the collection with a `limit` of 0 or greater than 100
- **THEN** the system returns `422 Unprocessable Content` with a stable message

### Requirement: Update playlist metadata

`PATCH /api/v1/library/music/playlist/{playlist_id}` SHALL accept an optional `name` and an optional `is_public` and
return `200 OK` with the updated playlist. An omitted field SHALL remain unchanged. A request without any field SHALL
change no metadata and SHALL return `200 OK` with the playlist. A successful update SHALL advance `last_modified_at`,
including a request that changes no metadata. Only the owner or the Root Admin MAY send a metadata field; a visible
non-owner SHALL receive `403 Forbidden`.

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

- **WHEN** a caller who can read a public playlist but is neither its owner nor the Root Admin sends `name` or
  `is_public`
- **THEN** the system returns `403 Forbidden` and applies nothing

#### Scenario: Root Admin updates another user's metadata

- **WHEN** the Root Admin sends `name` or `is_public` for another user's playlist
- **THEN** the system applies the change and returns `200 OK`

### Requirement: Add a recording through PATCH

`PATCH /api/v1/library/music/playlist/{playlist_id}` SHALL accept an optional `add_track` object carrying a
`recording_id` and SHALL append one occurrence of that recording. The recording SHALL exist. The response SHALL be the
updated playlist including the new track. The whole request SHALL be atomic. Adding SHALL require the
`playlist.moderate` permission.

#### Scenario: Add a recording

- **WHEN** a caller whose `playlist.moderate` permission covers the playlist sends `add_track` with an existing
  recording that is not already in the playlist
- **THEN** the system appends the track and returns `200 OK` with the updated playlist

#### Scenario: Unknown recording

- **WHEN** `add_track.recording_id` names a recording that does not exist
- **THEN** the system returns `422 Unprocessable Content` and applies nothing

#### Scenario: Duplicate recording

- **WHEN** `add_track.recording_id` names a recording already present in the playlist
- **THEN** the system returns `409 Conflict` and applies nothing

#### Scenario: Mixed authorization is atomic

- **WHEN** a caller who can moderate a public playlist but is not its owner or the Root Admin sends `add_track` together
  with `name` or `is_public`
- **THEN** the system returns `403 Forbidden` and applies neither change

### Requirement: Remove a track from a playlist

`DELETE /api/v1/library/music/playlist/{playlist_id}/track/{track_index}` SHALL remove the addressed occurrence and
return `204 No Content`, leaving the remaining indices unchanged. The shared recording and its media SHALL survive. A
successful removal SHALL advance `last_modified_at`. Removing SHALL require the `playlist.moderate` permission.

#### Scenario: Remove an existing track

- **WHEN** a caller whose `playlist.moderate` permission covers the playlist deletes an existing track index
- **THEN** the system returns `204 No Content`
- **AND** the remaining tracks keep their indices, leaving a gap

#### Scenario: Unknown track or invisible playlist

- **WHEN** the track index does not exist in the playlist, or the playlist is unknown or invisible
- **THEN** the system returns `404 Not Found`

#### Scenario: Readable but not moderatable playlist

- **WHEN** a caller whose `playlist.read` permission covers the playlist but whose `playlist.moderate` permission does
  not deletes a track
- **THEN** the system returns `403 Forbidden` and applies nothing

#### Scenario: Malformed track index

- **WHEN** the caller supplies a `track_index` that is not an unsigned integer
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: Reorder playlist tracks

`PUT /api/v1/library/music/playlist/{playlist_id}/track` SHALL accept a complete permutation of the playlist's current
track indices and SHALL reindex the tracks to `0..n-1` in the requested order, returning `200 OK` with the updated
playlist. An incomplete, duplicate, or unknown set SHALL return `422 Unprocessable Content`. A successful reorder SHALL
advance `last_modified_at`. Reordering SHALL require the `playlist.moderate` permission.

#### Scenario: Reorder a playlist

- **WHEN** a caller whose `playlist.moderate` permission covers the playlist sends every current track index exactly
  once in a new order
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

#### Scenario: Readable but not moderatable playlist

- **WHEN** a caller whose `playlist.read` permission covers the playlist but whose `playlist.moderate` permission does
  not sends an order
- **THEN** the system returns `403 Forbidden` and leaves the order unchanged

### Requirement: Delete a playlist

`DELETE /api/v1/library/music/playlist/{playlist_id}` SHALL delete the playlist and its track placements and return
`204 No Content`. The shared recordings, audio, and files SHALL survive. Only the owner or the Root Admin SHALL delete a
playlist.

#### Scenario: Owner deletes a playlist

- **WHEN** the owner deletes the playlist
- **THEN** the system returns `204 No Content`
- **AND** the playlist and its track placements no longer exist
- **AND** the recordings, audio, and files it referenced still exist

#### Scenario: Visible non-owner deletes a playlist

- **WHEN** a caller who can read a public playlist but is neither its owner nor the Root Admin deletes it
- **THEN** the system returns `403 Forbidden` and deletes nothing

#### Scenario: Root Admin deletes another user's playlist

- **WHEN** the Root Admin deletes another user's playlist
- **THEN** the system returns `204 No Content` and deletes it

#### Scenario: Invisible playlist

- **WHEN** the caller cannot read the playlist
- **THEN** the system returns `404 Not Found`
