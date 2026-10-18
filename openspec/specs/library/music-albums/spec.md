# library/music-albums Specification

## Purpose

Lets a client browse the music albums the server holds and read the tracks each album contains.

## Requirements

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

- **WHEN** an authenticated caller reads any album, an album's tracks, or one track
- **THEN** the system returns the representation and never a `403 Forbidden`

### Requirement: List music albums

The system SHALL return the music albums it holds through `GET /api/v1/library/music/album`. Albums SHALL NOT be scoped
to an owner: every authenticated caller sees the same albums.

#### Scenario: Authenticated caller lists albums

- **WHEN** an authenticated caller requests `GET /api/v1/library/music/album`
- **THEN** the system returns `200 OK` with a HAL collection of album representations

#### Scenario: Album representation content

- **WHEN** the system returns an album in a listing
- **THEN** the representation carries the album's identifier, name, release date, production type, genres and track
  count, plus a link to the album's tracks

### Requirement: Album listing is filterable and paginated

The album collection SHALL accept an optional `name` filter and optional `limit` and `offset` pagination. `limit` SHALL
default to 20 and SHALL be between 1 and 100.

#### Scenario: Filter and page

- **WHEN** an authenticated caller requests the album collection with `name`, `limit` and `offset`
- **THEN** the system returns only the albums whose name matches, starting at `offset` and holding at most `limit` items
- **AND** the collection's `_links.self` preserves the active filter and page

#### Scenario: Out-of-range limit

- **WHEN** the caller requests the album collection with a `limit` of 0 or greater than 100
- **THEN** the system returns `422 Unprocessable Content` with a stable message

### Requirement: Read the tracks of an album

The system SHALL return the tracks of one album through `GET /api/v1/library/music/album/{album_id}/track`.

#### Scenario: Tracks of an existing album

- **WHEN** an authenticated caller requests the tracks of an album that exists
- **THEN** the system returns `200 OK` with a HAL collection of the album's tracks

#### Scenario: Album representation of the track collection

- **WHEN** the system returns the tracks of an album
- **THEN** each track representation carries the track's identifier, its position, the recording name, the backing file
  identifier and the link to its album

#### Scenario: Unknown album

- **WHEN** the caller requests the tracks of an album identifier that does not exist
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed album identifier

- **WHEN** the caller supplies an `album_id` that is not a valid identifier
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: Read one track of an album

The system SHALL return one track through `GET /api/v1/library/music/album/{album_id}/track/{track_id}`, scoped to the
album named in the path.

#### Scenario: Track of the album

- **WHEN** an authenticated caller requests a track that belongs to the named album
- **THEN** the system returns `200 OK` with the track representation

#### Scenario: Track not in the album, or unknown album

- **WHEN** the requested `track_id` does not belong to the named album, or either identifier is unknown
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed track identifier

- **WHEN** the caller supplies a `track_id` that is not a valid identifier
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: Tracks are ordered by position

The system SHALL order an album's tracks by ascending medium index, then by ascending track index, so repeated reads
return a stable order.

#### Scenario: Ordering across mediums

- **WHEN** an album has more than one medium, each holding tracks
- **THEN** the track collection lists the mediums by ascending medium index and, within a medium, the tracks by
  ascending track index

### Requirement: Album and track responses are HAL representations

Album and track success responses SHALL be served as `application/hal+json`, keep business data at the root of the
payload, and carry a `_links.self` naming the resource's canonical URI.

#### Scenario: Self link

- **WHEN** the system returns a music album or track representation
- **THEN** the representation carries `_links.self` with the canonical path of the resource it is about
