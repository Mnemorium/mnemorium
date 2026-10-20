# Spec Delta

## MODIFIED Requirements

### Requirement: Album reads require an authenticated account

Every music album endpoint SHALL require a valid bearer token, and a token that names an account which no longer exists
SHALL be rejected.

#### Scenario: Missing or invalid credentials

- **WHEN** a request reaches a music album endpoint without a valid bearer token
- **THEN** the system returns `401 Unauthorized`

#### Scenario: Account no longer exists

- **WHEN** an authenticated request reaches a music album endpoint but the caller's account no longer exists
- **THEN** the system returns `401 Unauthorized`

#### Scenario: An authenticated caller is never forbidden

- **WHEN** an authenticated caller reads any album, a medium, a medium's tracks, or one track
- **THEN** the system returns the representation and never a `403 Forbidden`

### Requirement: List music albums

The system SHALL return the music albums it holds through `GET /api/v1/library/music/album`. Albums SHALL NOT be scoped
to an owner: every authenticated caller sees the same albums.

#### Scenario: Authenticated caller lists albums

- **WHEN** an authenticated caller requests `GET /api/v1/library/music/album`
- **THEN** the system returns `200 OK` with a HAL collection of album representations

#### Scenario: Album representation content

- **WHEN** the system returns an album in a listing
- **THEN** the representation carries the album's identifier, name, release date, production type, genres, total track
  count, and its mediums
- **AND** each medium carries its album-scoped index, its type, its track count, and links to the medium and its track
  collection
- **AND** the album's total track count is the sum of its mediums' track counts

### Requirement: Read the tracks of an album

The system SHALL return the tracks of one medium of one album through
`GET /api/v1/library/music/album/{album_id}/medium/{medium_index}/track`.

#### Scenario: Tracks of an existing album

- **WHEN** an authenticated caller requests the tracks of a medium that belongs to an album that exists
- **THEN** the system returns `200 OK` with a HAL collection of the medium's tracks

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

### Requirement: Read one track of an album

The system SHALL return one track through
`GET /api/v1/library/music/album/{album_id}/medium/{medium_index}/track/{track_index}`, scoped to the album and medium
named in the path.

#### Scenario: Track of the album

- **WHEN** an authenticated caller requests a track index that exists in the named album medium
- **THEN** the system returns `200 OK` with the track representation

#### Scenario: Track not in the album, or unknown album

- **WHEN** the requested `track_index` does not belong to the named album medium, or the album or medium identifier is
  unknown
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed track identifier

- **WHEN** the caller supplies a `medium_index` or `track_index` that is not a valid 8-bit unsigned integer
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: Tracks are ordered by position

The system SHALL order the mediums of an album by ascending medium index, and the tracks of each medium by ascending
track index, so repeated reads return a stable order.

#### Scenario: Ordering across mediums

- **WHEN** an album has more than one medium, each holding tracks
- **THEN** the album representation lists the mediums by ascending medium index
- **AND** each medium's track collection lists its tracks by ascending track index

## ADDED Requirements

### Requirement: Read a music medium

The system SHALL return one medium of an album through
`GET /api/v1/library/music/album/{album_id}/medium/{medium_index}`.

#### Scenario: Medium of an existing album

- **WHEN** an authenticated caller requests a medium index that belongs to an album that exists
- **THEN** the system returns `200 OK` with the medium representation

#### Scenario: Medium representation content

- **WHEN** the system returns a medium
- **THEN** the representation carries its album-scoped index, its type, its track count, and links to its album and its
  track collection

#### Scenario: Medium representation is HAL

- **WHEN** the system returns a medium representation
- **THEN** the response is served as `application/hal+json` with business data at the root
- **AND** the representation carries `_links.self` naming the medium's canonical URI

#### Scenario: Unknown album or medium

- **WHEN** the requested medium index does not belong to the album, or the album identifier is unknown
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed medium index

- **WHEN** the caller supplies a `medium_index` that is not a valid 8-bit unsigned integer
- **THEN** the system returns `400 Bad Request` with a stable message

#### Scenario: Medium indexes are scoped to the album

- **WHEN** two different albums each hold a medium with index 2
- **THEN** each album resolves its own medium, and the two mediums are independent

### Requirement: Album track identity

An album track SHALL be identified by its medium index and its index within that medium, not by a separate track
identifier. The track representation SHALL expose the track index as `id` and the shared recording's identifier as
`recording_id`.

#### Scenario: Track address

- **WHEN** an authenticated caller requests
  `/api/v1/library/music/album/{album_id}/medium/{medium_index}/track/{track_index}`
- **THEN** the path addresses the track at that position in the medium

#### Scenario: Two tracks of the same recording

- **WHEN** two tracks of an album reference the same recording
- **THEN** each track has its own index and its own representation
- **AND** both representations carry the same `recording_id` with different `id` values
