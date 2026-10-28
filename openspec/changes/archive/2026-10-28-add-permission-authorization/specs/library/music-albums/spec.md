# Spec Delta

## MODIFIED Requirements

### Requirement: Album reads require an authenticated account

Every music album endpoint SHALL require a valid bearer token, and a token that names an account which no longer exists
SHALL be rejected. A caller whose `catalog.read` permission does not cover the album SHALL receive `404 Not Found` for
the album, its mediums and its tracks.

#### Scenario: Missing or invalid credentials

- **WHEN** a request reaches a music album endpoint without a valid bearer token
- **THEN** the system returns `401 Unauthorized`

#### Scenario: Account no longer exists

- **WHEN** an authenticated request reaches a music album endpoint but the caller's account no longer exists
- **THEN** the system returns `401 Unauthorized`

#### Scenario: An authenticated caller is never forbidden

- **WHEN** an authenticated caller with the default permission reads any album, a medium, a medium's tracks, or one
  track
- **THEN** the system returns the representation and never a `403 Forbidden`

#### Scenario: A restricted caller cannot read an album

- **WHEN** an authenticated caller whose `catalog.read` permission does not cover an album requests the album, a medium
  or a track
- **THEN** the system returns `404 Not Found`, indistinguishable from an unknown album

### Requirement: List music albums

The system SHALL return the music albums the caller's `catalog.read` permission covers through
`GET /api/v1/library/music/album`. The default permission covers every album for every authenticated caller; a per-user
override may narrow the set.

#### Scenario: Authenticated caller lists albums

- **WHEN** an authenticated caller with the default permission requests `GET /api/v1/library/music/album`
- **THEN** the system returns `200 OK` with a HAL collection of album representations

#### Scenario: A restricted caller lists only permitted albums

- **WHEN** an authenticated caller whose `catalog.read` permission covers only some albums requests the collection
- **THEN** the system returns only the albums that permission covers

#### Scenario: A caller with no catalog access lists no albums

- **WHEN** an authenticated caller whose `catalog.read` permission is `NONE` requests the collection
- **THEN** the system returns `200 OK` with an empty collection

#### Scenario: Album representation content

- **WHEN** the system returns an album in a listing
- **THEN** the representation carries the album's identifier, name, release date, production type, genres, total track
  count, its cover and its mediums
- **AND** the cover is the identifier of the file holding the album's artwork, or `null` when the album stores none
- **AND** each medium carries its album-scoped index, its type, its track count, and links to the medium and its track
  collection
- **AND** the album's total track count is the sum of its mediums' track counts
