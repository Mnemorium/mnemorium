# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-MUSIC-008 — Create an album as an upsert by name and group

The system SHALL create an album from a name, a group identifier and optional release date, production type, genres and
cover, requiring `CreateChild` for a music album on the **Catalogue**. The name SHALL be non-empty, at most 100
characters, and SHALL NOT begin or end with whitespace; the submitted value is stored unchanged. An album whose
`(name, group)` already exists SHALL be returned unchanged with `200 OK`; a new album SHALL answer `201 Created`. A
request that names no genre SHALL store the `UNKNOWN` genre.

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

- **WHEN** a create names a name that is empty, too long, or begins or ends with whitespace, a group that does not
  exist, or a genre that is empty
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

#### Scenario: SCN-MUSIC-008-E — Concurrent duplicate

- **WHEN** a concurrent request wins the identity
- **THEN** the system answers `409 Conflict` and creates nothing

### Requirement: REQ-MUSIC-014 — Create a group

The system SHALL create a group from a name and optional cover, requiring `CreateChild` for a music group on the
catalogue. The name SHALL be non-empty, at most 100 characters, SHALL NOT begin or end with whitespace, and SHALL be
unique; the submitted value is stored unchanged. The cover SHALL reference a `LIBRARY` image. A created group SHALL
answer `201 Created` with its representation.

#### Scenario: SCN-MUSIC-014-A — A new group

- **WHEN** a caller permitted to create groups submits a valid name and no cover
- **THEN** the system answers `201 Created` with the group

#### Scenario: SCN-MUSIC-014-B — A group with artwork

- **WHEN** the request names an existing `LIBRARY` image
- **THEN** the group stores it as its cover

#### Scenario: SCN-MUSIC-014-C — A duplicate name

- **WHEN** a group already holds the name
- **THEN** the system answers `409 Conflict` and creates nothing

#### Scenario: SCN-MUSIC-014-D — An invalid name or cover

- **WHEN** the name is empty, too long, or begins or ends with whitespace
- **THEN** the system answers `422 Unprocessable Content`
- **AND** when the cover is not a usable `LIBRARY` image, the system answers `409 Conflict`

### Requirement: REQ-MUSIC-015 — Rename a group or change its artwork

A group update SHALL require `EditMetadata` on the group. An absent name or cover SHALL keep its value, a `null` cover
SHALL clear it, and a supplied cover SHALL replace it. The name SHALL stay non-empty, at most 100 characters, SHALL NOT
begin or end with whitespace, and SHALL stay unique; the submitted value is stored unchanged. The cover SHALL reference
a `LIBRARY` image.

#### Scenario: SCN-MUSIC-015-A — A group is renamed

- **WHEN** the request carries a new valid name
- **THEN** the name is replaced and the cover is unchanged

#### Scenario: SCN-MUSIC-015-B — Artwork is replaced

- **WHEN** the request carries an existing `LIBRARY` image
- **THEN** the cover is replaced

#### Scenario: SCN-MUSIC-015-C — Artwork is cleared

- **WHEN** the request carries `null` for the cover
- **THEN** the stored cover is cleared and the representation reports no `_links.cover`

#### Scenario: SCN-MUSIC-015-D — A duplicate name or unusable cover

- **WHEN** another group holds the name, or the cover is not a usable `LIBRARY` image
- **THEN** the system answers `409 Conflict` and applies nothing

#### Scenario: SCN-MUSIC-015-E — An invalid name

- **WHEN** the name is empty, too long, or begins or ends with whitespace
- **THEN** the system answers `422 Unprocessable Content` and applies nothing

### Requirement: REQ-MUSIC-019 — Create a playlist

The system SHALL create a playlist owned by the caller and visible to it, requiring `CreateChild` for a music playlist
on the playlist collection. The request SHALL carry a name satisfying the playlist naming rule, and MAY carry
`is_public`, which SHALL default to `false`. A created playlist SHALL answer `201 Created` with its representation.

#### Scenario: SCN-MUSIC-019-A — A private playlist

- **WHEN** a caller permitted to create playlists submits a valid name and no visibility
- **THEN** the playlist is created, private, owned by the caller, and the system answers `201 Created`

#### Scenario: SCN-MUSIC-019-B — A public playlist

- **WHEN** the request carries `is_public: true`
- **THEN** the playlist's **Visibility** is `Public`

#### Scenario: SCN-MUSIC-019-C — A malformed request

- **WHEN** the name is empty, only whitespace, longer than 100 characters, or begins or ends with whitespace
- **THEN** the system answers `422 Unprocessable Content` and creates nothing

### Requirement: REQ-MUSIC-024 — Update playlist metadata

A playlist metadata update SHALL require `EditMetadata` on the playlist. An absent `name` or `is_public` SHALL keep its
value. A successful change SHALL advance the playlist's **Last-modified time**, including a change that stores a new
value equal to the old one.

#### Scenario: SCN-MUSIC-024-A — Metadata is replaced

- **WHEN** the request carries a new name or visibility
- **THEN** the value is replaced and the last-modified time advances

#### Scenario: SCN-MUSIC-024-B — No metadata field

- **WHEN** the request carries neither `name` nor `is_public`
- **THEN** the metadata is unchanged

#### Scenario: SCN-MUSIC-024-C — A duplicate or invalid name

- **WHEN** the name is held by another playlist of the same owner
- **THEN** the system answers `409 Conflict`
- **AND** when it is empty, too long, or begins or ends with whitespace, the system answers `422 Unprocessable Content`

### Requirement: REQ-MUSIC-029 — An album's metadata is updated partially

The system SHALL update an album a **Caller** may `View`, requiring `EditMetadata` on it. An absent field SHALL keep its
stored value and a `null` SHALL clear an optional value. A supplied name SHALL be non-empty, at most 100 characters, and
SHALL NOT begin or end with whitespace; the submitted value is stored unchanged. A name and group that duplicate another
album SHALL answer `409 Conflict` and apply nothing; a value that fails validation SHALL answer
`422 Unprocessable Content` and apply nothing.

#### Scenario: SCN-MUSIC-029-A — Change the release date and genres

- **WHEN** a caller submits a new release date and genre list
- **THEN** the system stores them and returns the album representation

#### Scenario: SCN-MUSIC-029-B — Clear an optional value

- **WHEN** the caller sends `release_date: null`
- **THEN** the album stores no release date and reports `null`

#### Scenario: SCN-MUSIC-029-C — The group change collides

- **WHEN** the requested group makes the name and group duplicate another album
- **THEN** the system answers `409 Conflict` and applies nothing

#### Scenario: SCN-MUSIC-029-D — An absent field keeps its value

- **WHEN** the request names no production type
- **THEN** the stored production type is unchanged

#### Scenario: SCN-MUSIC-029-E — An invalid value applies nothing

- **WHEN** the name is empty, too long, or begins or ends with whitespace, a genre is empty, or the group does not exist
- **THEN** the system answers `422 Unprocessable Content` and applies nothing

## REMOVED Requirements

### Requirement: REQ-MUSIC-020 — Normalize a playlist name and keep it unique per owner

**Reason**: The rule changes from normalizing a name to rejecting one that surrounds itself with whitespace, and its
first scenario flips from "names are trimmed" to "surrounding whitespace is rejected". OpenSpec refuses a scenario
rename inside a `MODIFIED` requirement, so the rule is replaced by `REQ-MUSIC-030` rather than modified in place.

**Migration**: No stored name changes, because every stored playlist name is already trimmed. A client that relied on
the server trimming a submitted name must send the exact value, or accept the `422`.

## ADDED Requirements

### Requirement: REQ-MUSIC-030 — A playlist name is stored unchanged and unique per owner

A playlist name SHALL be non-empty, at most 100 characters, and SHALL NOT begin or end with whitespace; such a name
answers `422` and stores nothing. The submitted value SHALL be stored unchanged, never trimmed, and SHALL be unique per
owner, compared exactly. Two callers MAY each hold the same name.

#### Scenario: SCN-MUSIC-030-A — Surrounding whitespace is rejected

- **WHEN** a caller creates or renames a playlist with a name that begins or ends with whitespace
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

#### Scenario: SCN-MUSIC-030-B — A duplicate name for one owner

- **WHEN** the caller already owns a playlist with the name
- **THEN** the system answers `409 Conflict` and creates or applies nothing

#### Scenario: SCN-MUSIC-030-C — The same name for two owners

- **WHEN** two callers each create a playlist with the same name
- **THEN** both playlists exist
