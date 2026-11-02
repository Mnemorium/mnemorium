# Spec Delta

## MODIFIED Requirements

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
- **AND** the cover is the identifier of the image holding the playlist's artwork, or `null` when the playlist stores
  none

#### Scenario: Filter and page

- **WHEN** an authenticated caller requests the collection with `name`, `is_public`, `owner_id`, `limit`, and `offset`
- **THEN** the system returns only the playlists matching the visible filters, starting at `offset` and holding at most
  `limit` items
- **AND** the collection's `_links.self` preserves the active filters and page

#### Scenario: Out-of-range limit

- **WHEN** the caller requests the collection with a `limit` of 0 or greater than 100
- **THEN** the system returns `422 Unprocessable Content` with a stable message
