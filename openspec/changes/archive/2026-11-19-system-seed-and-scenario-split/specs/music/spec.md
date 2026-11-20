# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-MUSIC-008 — Create an album as an upsert by name and group

The system SHALL create an album from a name, a group identifier and optional release date, production type, genres and
cover, requiring `CreateChild` for a music album on the **Catalogue**. An album whose `(name, group)` already exists
SHALL be returned unchanged with `200 OK`; a new album SHALL answer `201 Created`. A request that names no genre SHALL
store the `UNKNOWN` genre.

#### Scenario: SCN-MUSIC-008-A — A new album

- **WHEN** a caller permitted to create albums submits a valid name and group
- **THEN** the system answers `201 Created` with the album

#### Scenario: SCN-MUSIC-008-B — An existing album

- **WHEN** the `(name, group)` pair already exists
- **THEN** the system answers `200 OK` with the stored album unchanged

#### Scenario: SCN-MUSIC-008-C — An unnamed genre

- **WHEN** the request carries no genre
- **THEN** the album stores the `UNKNOWN` genre

#### Scenario: SCN-MUSIC-008-D — Invalid name, group or genre

- **WHEN** a create or an update names a name that is empty or too long, a group that does not exist, or a genre that is
  empty
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

#### Scenario: SCN-MUSIC-008-E — Concurrent duplicate

- **WHEN** a concurrent request wins the identity
- **THEN** the system answers `409 Conflict` and creates nothing

### Requirement: REQ-MUSIC-009 — Create or retype a medium

The system SHALL create or retype a medium as `CD` or `DIGITAL`, requiring `CreateChild` for a music medium on the
album. The operation SHALL be idempotent: repeating it with the stored type SHALL change nothing and answer `200 OK`.

#### Scenario: SCN-MUSIC-009-A — A medium is created

- **WHEN** the album holds no medium at the index and the request names a valid type
- **THEN** the medium is created and the system answers `200 OK`

#### Scenario: SCN-MUSIC-009-B — A medium is retyped

- **WHEN** the album holds a medium of another type at the index
- **THEN** the stored type is replaced and the tracks are kept

#### Scenario: SCN-MUSIC-009-C — Repeat is idempotent

- **WHEN** the request repeats the stored type
- **THEN** nothing changes and the system answers `200 OK`

#### Scenario: SCN-MUSIC-009-D — Invalid type or protected album

- **WHEN** the type is outside `CD` and `DIGITAL`, or the album is the default album
- **THEN** the system answers `422 Unprocessable Content` and applies nothing

### Requirement: REQ-MUSIC-010 — Place a recording at a destination

The system SHALL place the recording named by `recording_id` at a destination in an album medium, requiring
`CreateChild` for a music recording on the album. A recording SHALL occupy at most one medium position. The write SHALL
remove its previous placement and insert the new one in one transaction.

#### Scenario: SCN-MUSIC-010-A — A recording is placed

- **WHEN** the request names a recording and an unoccupied destination
- **THEN** the recording is placed there and the system answers `200 OK`

#### Scenario: SCN-MUSIC-010-B — A recording is moved

- **WHEN** the recording is already placed elsewhere in the catalogue
- **THEN** the previous placement is removed and the new one is stored in one transaction

#### Scenario: SCN-MUSIC-010-C — Unknown recording

- **WHEN** `recording_id` names no recording
- **THEN** the system answers `422 Unprocessable Content` and applies nothing

#### Scenario: SCN-MUSIC-010-D — Unknown album or medium

- **WHEN** the album or medium does not exist
- **THEN** the system answers `404 Not Found`

#### Scenario: SCN-MUSIC-010-E — Occupied destination or protected album

- **WHEN** the destination already holds another recording, or the album is the default album
- **THEN** the system answers `409 Conflict` or `422 Unprocessable Content` respectively and applies nothing

### Requirement: REQ-MUSIC-011 — Edit a recording through the track position

A track-position update SHALL update the recording's name and ISRC, requiring `EditMetadata` on the album. The recording
is shared, so the change SHALL be visible wherever it is placed. An absent field SHALL keep its value and a `null` SHALL
clear it.

#### Scenario: SCN-MUSIC-011-A — A rename is visible elsewhere

- **WHEN** the caller renames the recording behind a track
- **THEN** every playlist entry that references the recording reports the new name

#### Scenario: SCN-MUSIC-011-B — A field is cleared

- **WHEN** the request sends `null` for the name or the ISRC
- **THEN** the stored value is cleared

#### Scenario: SCN-MUSIC-011-C — A duplicate or invalid ISRC

- **WHEN** the ISRC belongs to another recording
- **THEN** the system answers `409 Conflict`
- **AND** when it is not 12 valid characters, the system answers `422 Unprocessable Content`

#### Scenario: SCN-MUSIC-011-D — Unknown track

- **WHEN** the album, medium or position does not exist
- **THEN** the system answers `404 Not Found`

#### Scenario: SCN-MUSIC-011-E — Protected album

- **WHEN** the album is the default album
- **THEN** the system answers `422 Unprocessable Content` and applies nothing

### Requirement: REQ-MUSIC-016 — Protect the default album and group

The **default album** and the **default group** SHALL be readable like any other. A write that would modify either SHALL
be refused and SHALL apply nothing.

#### Scenario: SCN-MUSIC-016-A — The default album and group are readable

- **WHEN** a caller reads the default album or the default group
- **THEN** the system answers `200 OK` with its representation

#### Scenario: SCN-MUSIC-016-B — A group write is refused

- **WHEN** a caller renames the default group or replaces its cover
- **THEN** the system answers `409 Conflict` and applies nothing

#### Scenario: SCN-MUSIC-016-C — An album write is refused

- **WHEN** a caller updates the default album, or writes a medium or track of it
- **THEN** the system answers `409 Conflict` or `422 Unprocessable Content` and applies nothing

### Requirement: REQ-MUSIC-023 — Index a playlist's tracks from zero

A playlist's tracks SHALL be indexed from zero and SHALL be returned in ascending index order. A track representation
SHALL carry its index as `id`, the shared recording as `recording_id`, the backing file as `file_id`, the recording's
name and ISRC, and a playlist link. A playlist track SHALL carry no artwork of its own (`REQ-COVER-ART-006`).

#### Scenario: SCN-MUSIC-023-A — Contiguous indices

- **WHEN** a playlist holds tracks
- **THEN** their indices are `0` to `n-1` with no gap

#### Scenario: SCN-MUSIC-023-B — The track representation

- **WHEN** the system returns a playlist track
- **THEN** it carries its index, `recording_id`, `file_id`, the recording's name and ISRC, and the playlist link
