# Spec Delta

## ADDED Requirements

### Requirement: Album creation is an upsert by name and group

The system SHALL create an album through `POST /api/v1/library/music/album` from a name, a group identifier and optional
release date, production type, genres and cover image. The write SHALL require `library.manage`. An album whose
`(name, group)` already exists SHALL be returned unchanged with `200 OK`; a new album SHALL return `201 Created`.

#### Scenario: A new album

- **WHEN** an administrator submits a name that does not exist for its group
- **THEN** the system creates the album with the `UNKNOWN` genre when none is supplied and returns `201 Created`

#### Scenario: An existing album

- **WHEN** an administrator submits a name already used by its group
- **THEN** the system returns the existing album with `200 OK` and applies no change

#### Scenario: Missing group or name

- **WHEN** the name is empty or the group does not exist
- **THEN** the system returns `422 Unprocessable Content` and creates nothing

### Requirement: Album metadata is updated partially

The system SHALL update an album through `PATCH /api/v1/library/music/album/{album_id}`. An absent field SHALL keep its
value, and a `null` SHALL clear an optional value. The write SHALL require `library.manage`.

#### Scenario: Change the release date and genres

- **WHEN** an administrator submits a new release date and genre list
- **THEN** the system stores them and returns the album representation

#### Scenario: Clear an optional value

- **WHEN** an administrator sends `release_date: null`
- **THEN** the album stores no release date and reports `null`

#### Scenario: Group change collides

- **WHEN** the requested group makes `(name, group)` duplicate another album
- **THEN** the system returns `409 Conflict` and applies nothing

### Requirement: A medium is created or retyped

The system SHALL create or retype a medium through `PUT /api/v1/library/music/album/{album_id}/medium/{medium_index}`
with type `CD` or `DIGITAL`. The operation SHALL be idempotent and SHALL require `library.manage`.

#### Scenario: Create a medium

- **WHEN** an administrator puts a `CD` medium at a free index
- **THEN** the system creates it with no tracks and returns `201 Created`

#### Scenario: Retype a medium

- **WHEN** an administrator puts a `DIGITAL` medium at an existing index
- **THEN** the system changes the type and keeps its tracks

#### Scenario: An unknown type or index

- **WHEN** the type is not `CD` or `DIGITAL`, or the index is outside the 8-bit range
- **THEN** the system returns `422 Unprocessable Content`

### Requirement: A recording is placed or moved by destination

The system SHALL place a recording at
`PUT /api/v1/library/music/album/{album_id}/medium/{medium_index}/track/{track_index}` from a `recording_id`. It SHALL
delete the recording's previous medium placement and insert the new one in one transaction. The write SHALL require
`library.manage`.

#### Scenario: Place a recording

- **WHEN** an administrator puts a recording at a free track index
- **THEN** the system places it and returns `201 Created` with the new track address

#### Scenario: Move a recording

- **WHEN** the recording is already placed in another medium
- **THEN** the system removes the old placement, inserts the new one and returns `200 OK`

#### Scenario: The destination is occupied

- **WHEN** another recording already holds the destination track index
- **THEN** the system returns `409 Conflict` and moves nothing

#### Scenario: Unknown recording or destination

- **WHEN** the recording does not exist, or the album or medium does not exist
- **THEN** the system returns `422 Unprocessable Content` or `404 Not Found` and applies nothing

### Requirement: Recording metadata is edited through the track path

The system SHALL update a recording's name and ISRC through
`PATCH /api/v1/library/music/album/{album_id}/medium/{medium_index}/track/{track_index}`. The recording is shared, so
the change SHALL be visible wherever it is placed. The write SHALL require `library.manage`.

#### Scenario: Rename a recording

- **WHEN** an administrator submits a new name
- **THEN** the system stores it and returns the track representation

#### Scenario: Clear the ISRC

- **WHEN** an administrator sends `isrc_code: null`
- **THEN** the recording stores no ISRC

#### Scenario: Duplicate or invalid ISRC

- **WHEN** the ISRC is used by another recording or is not twelve characters
- **THEN** the system returns `409 Conflict` or `422 Unprocessable Content` and applies nothing

### Requirement: External metadata is applied to an album

The system SHALL resolve a provider reference and write the album and its medium structure through
`POST /api/v1/library/music/album/{album_id}/apply`. It SHALL move recordings only when the request names their
placements, SHALL reject the default album, and SHALL require `library.manage`.

#### Scenario: Apply album metadata without placements

- **WHEN** an administrator applies a resolved album reference without placements
- **THEN** the system updates the album fields and the medium structure and leaves every track placement unchanged

#### Scenario: Apply with placements

- **WHEN** the request names a `recording_id` for each destination position
- **THEN** the system moves the named recordings in the same transaction

#### Scenario: The default album

- **WHEN** the target album is the seeded `Unknown` album
- **THEN** the system rejects the write with `422 Unprocessable Content`

#### Scenario: Provider failure

- **WHEN** the provider cannot resolve the reference
- **THEN** the system reports the failure and writes nothing
