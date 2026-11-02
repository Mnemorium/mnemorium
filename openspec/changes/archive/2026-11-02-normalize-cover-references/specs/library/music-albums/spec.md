# Spec Delta

## MODIFIED Requirements

### Requirement: List music albums

The system SHALL return the music albums the caller's `library.read` permission covers through
`GET /api/v1/library/music/album`. The default permission covers every album for every authenticated caller; a per-user
override may narrow the set.

#### Scenario: Authenticated caller lists albums

- **WHEN** an authenticated caller with the default permission requests `GET /api/v1/library/music/album`
- **THEN** the system returns `200 OK` with a HAL collection of album representations

#### Scenario: A restricted caller lists only permitted albums

- **WHEN** an authenticated caller whose `library.read` permission covers only some albums requests the collection
- **THEN** the system returns only the albums that permission covers

#### Scenario: A caller with no catalog access lists no albums

- **WHEN** an authenticated caller whose `library.read` permission is `NONE` requests the collection
- **THEN** the system returns `200 OK` with an empty collection

#### Scenario: Album representation content

- **WHEN** the system returns an album in a listing
- **THEN** the representation carries the album's identifier, name, release date, production type, genres, total track
  count, its cover and its mediums
- **AND** the cover is the identifier of the image holding the album's artwork, or `null` when the album stores none
- **AND** each medium carries its album-scoped index, its type, its track count, and links to the medium and its track
  collection
- **AND** the album's total track count is the sum of its mediums' track counts

### Requirement: Read one music album

The system SHALL return one music album through `GET /api/v1/library/music/album/{album_id}`.

#### Scenario: Album of an existing identifier

- **WHEN** an authenticated caller requests an album identifier that exists
- **THEN** the system returns `200 OK` with the album representation

#### Scenario: Album representation content

- **WHEN** the system returns one album
- **THEN** the representation carries the album's identifier, name, release date, production type, genres, total track
  count, its cover and its mediums
- **AND** the cover is the identifier of the image holding the album's artwork, or `null` when the album stores none
- **AND** each medium carries its album-scoped index, its type, its track count, and links to the medium and its track
  collection
- **AND** the album's total track count is the sum of its mediums' track counts

#### Scenario: Album representation is HAL

- **WHEN** the system returns an album representation
- **THEN** the response is served as `application/hal+json` with business data at the root
- **AND** the representation carries `_links.self` naming the album's canonical URI

#### Scenario: Unknown album

- **WHEN** the requested album identifier is unknown
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed album identifier

- **WHEN** the caller supplies an `album_id` that is not a valid identifier
- **THEN** the system returns `400 Bad Request` with a stable message
