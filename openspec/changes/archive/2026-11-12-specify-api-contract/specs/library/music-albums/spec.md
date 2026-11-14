# Spec Delta

## REMOVED Requirements

### Requirement: Album and track responses are HAL representations

**Reason**: The `api` capability now owns the response media type, the payload shape and `_links.self` for every
representation (`REQ-API-003`, `REQ-API-004`). Restating the rule here duplicated it and let the copies drift.
**Migration**: No client action; the served behavior is unchanged. The rule is read from `openspec/specs/api/spec.md`.

## MODIFIED Requirements

### Requirement: List music albums

The system SHALL return the music albums the caller's `library.read` permission covers through
`GET /api/v1/library/music/album`. The default permission covers every album for every authenticated caller; a per-user
override may narrow the set. The collection SHALL embed each page item's partial representation under `_embedded.items`;
the item carries the album's identifier, name and cover, and the detail read returns the full representation.

#### Scenario: Authenticated caller lists albums

- **WHEN** an authenticated caller with the default permission requests `GET /api/v1/library/music/album`
- **THEN** the system returns `200 OK` with a collection whose `_embedded.items` carries the page's partial album
  representations

#### Scenario: A restricted caller lists only permitted albums

- **WHEN** an authenticated caller whose `library.read` permission covers only some albums requests the collection
- **THEN** the system returns only the albums that permission covers

#### Scenario: A caller with no catalog access lists no albums

- **WHEN** an authenticated caller whose `library.read` permission is `NONE` requests the collection
- **THEN** the system returns `200 OK` with an empty collection

#### Scenario: Album representation content

- **WHEN** the system returns an album on a detail read
- **THEN** the representation carries the album's identifier, name, release date or `null`, production type, genres,
  total track count, its cover and its mediums
- **AND** the cover is the identifier of the image holding the album's artwork, or `null` when the album stores none
- **AND** each medium carries its album-scoped index, its type, its track count, and links to the medium and its track
  collection
- **AND** the album's total track count is the sum of its mediums' track counts

#### Scenario: A listing item is partial

- **WHEN** an album appears in the catalog listing
- **THEN** the item carries its identifier, name, cover image identifier, `self` link and `cover` link when artwork is
  set
- **AND** it carries no genres, total track count or mediums

### Requirement: Album listing is filterable and paginated

The album collection SHALL accept an optional `name` filter and SHALL be paginated as `api` defines. A `name` filter
SHALL return only the albums whose name matches the supplied value.

#### Scenario: Filter and page

- **WHEN** an authenticated caller requests the album collection with `name`, `limit` and `offset`
- **THEN** the system returns only the albums whose name matches, starting at `offset` and holding at most `limit` items

#### Scenario: Out-of-range limit

- **WHEN** the caller requests the album collection with a `limit` of 0 or greater than 100
- **THEN** the system returns `422 Unprocessable Content` with a stable message

### Requirement: Read the tracks of an album

The system SHALL return the tracks of one medium of one album through
`GET /api/v1/library/music/album/{album_id}/medium/{medium_index}/track`.

#### Scenario: Tracks of an existing album

- **WHEN** an authenticated caller requests the tracks of a medium that belongs to an album that exists
- **THEN** the system returns `200 OK` with a collection of the medium's tracks

#### Scenario: Album representation of the track collection

- **WHEN** the system returns the tracks of an album medium
- **THEN** each track representation carries its index as `id`, the recording's identifier as `recording_id`, the medium
  index, the recording name, the backing file identifier, and the links to its album and medium

#### Scenario: Unknown album

- **WHEN** the caller requests the tracks of an album or medium that does not exist, or the medium does not belong to
  the album
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed album identifier

- **WHEN** the caller supplies an `album_id` that is not a valid identifier, or a `medium_index` that is not a valid
  8-bit unsigned integer
- **THEN** the system returns `400 Bad Request` with a stable message
