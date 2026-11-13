# music Specification

## Purpose

Lets an authenticated caller browse and curate the music catalogue — albums, their mediums and recordings, and groups —
and own playlists whose ordered tracks reference shared recordings.

## Requirements

### Requirement: REQ-MUSIC-001 — List the album collection

The system SHALL return the albums a **Caller** may `Discover`, filterable by `name`. Each collection item SHALL be a
**Partial representation** carrying the album's identifier, name, cover and links.

#### Scenario: SCN-MUSIC-001-A — A caller lists albums

- **WHEN** a caller requests the album collection
- **THEN** the system returns the albums it may discover, each as a partial representation

#### Scenario: SCN-MUSIC-001-B — Name filter

- **WHEN** the caller requests the collection with `name`
- **THEN** the system returns only the albums whose name matches

#### Scenario: SCN-MUSIC-001-C — An invisible album is filtered out

- **WHEN** the caller may not `Discover` an album
- **THEN** it is absent from the collection and the request is not an error

### Requirement: REQ-MUSIC-002 — Read one album

The system SHALL return one album the caller may `View`, carrying its identifier, name, group link, release date,
production type, genres, track count, cover and its mediums in ascending index order. An album the caller may not view
SHALL answer `404 Not Found`, indistinguishable from an unknown album.

#### Scenario: SCN-MUSIC-002-A — An existing album

- **WHEN** a caller requests an album it may view
- **THEN** the system answers `200 OK` with the album's identifier, name, group link, genres, track count, cover and
  mediums

#### Scenario: SCN-MUSIC-002-B — Unreported values are absent

- **WHEN** the album stores no release date, production type or cover
- **THEN** the representation carries `null` for each and no `_links.cover`

#### Scenario: SCN-MUSIC-002-C — Unknown or invisible album

- **WHEN** the album identifier is unknown, or the caller may not view the album
- **THEN** the system answers `404 Not Found`

#### Scenario: SCN-MUSIC-002-D — Invalid identifier

- **WHEN** the requested album identifier is not valid
- **THEN** the system answers `400 Bad Request` with a stable message

### Requirement: REQ-MUSIC-003 — A medium's type is CD or DIGITAL

A medium's type SHALL be `CD` or `DIGITAL`. A release whose mediums are all compact discs and number more than one SHALL
be represented as separate `CD` mediums indexed from zero; every other release SHALL be represented as one `DIGITAL`
medium at index zero.

#### Scenario: SCN-MUSIC-003-A — A multi-disc release

- **WHEN** an album holds two or more `CD` mediums
- **THEN** each disc is a `CD` medium at its own ascending index

#### Scenario: SCN-MUSIC-003-B — Every other release

- **WHEN** an album is not a multi-disc compact-disc release
- **THEN** it holds one `DIGITAL` medium at index zero

### Requirement: REQ-MUSIC-004 — Read one medium

The system SHALL return one medium of an album the caller may `View`, carrying its album-scoped index, type, track count
and links. An unknown album or medium SHALL answer `404 Not Found`.

#### Scenario: SCN-MUSIC-004-A — An existing medium

- **WHEN** the caller requests a medium an album it may view holds
- **THEN** the system answers `200 OK` with the medium's index, type, track count and links

#### Scenario: SCN-MUSIC-004-B — Unknown album or medium

- **WHEN** the album is unknown, or it holds no medium at that index
- **THEN** the system answers `404 Not Found`

#### Scenario: SCN-MUSIC-004-C — Malformed or out-of-range index

- **WHEN** the medium index is non-numeric
- **THEN** the system answers `400 Bad Request`
- **AND** when it is numeric but outside the 8-bit range, the system answers `422 Unprocessable Content`

### Requirement: REQ-MUSIC-005 — List a medium's tracks

The system SHALL return the tracks of one medium of an album the caller may `View`, in ascending track-index order.

#### Scenario: SCN-MUSIC-005-A — Tracks of a medium

- **WHEN** the caller requests the tracks of a medium it may view
- **THEN** the system answers `200 OK` with the medium's tracks in ascending index order

#### Scenario: SCN-MUSIC-005-B — Unknown album or medium

- **WHEN** the album is unknown, or it holds no medium at that index
- **THEN** the system answers `404 Not Found`

### Requirement: REQ-MUSIC-006 — Read one track

The system SHALL return one track addressed by its position in the medium. A position the medium does not sequence, or
an unknown album or medium, SHALL answer `404 Not Found`.

#### Scenario: SCN-MUSIC-006-A — An existing track

- **WHEN** the caller requests a position the medium sequences
- **THEN** the system answers `200 OK` with the track representation

#### Scenario: SCN-MUSIC-006-B — Position not sequenced

- **WHEN** the requested position is not held by the named album medium
- **THEN** the system answers `404 Not Found`

#### Scenario: SCN-MUSIC-006-C — Malformed position

- **WHEN** the medium index or track index is non-numeric
- **THEN** the system answers `400 Bad Request`

### Requirement: REQ-MUSIC-007 — Address a track by position

A track SHALL be identified by its medium index and its index within that medium, not by a separate identifier. Its
representation SHALL carry that index as `id`, the shared recording as `recording_id`, the backing file as `file_id`,
the medium index, the recording's name and ISRC, and links to its album and medium. A track SHALL carry no artwork of
its own (`REQ-COVER-ART-006`); a client reads the album's cover.

#### Scenario: SCN-MUSIC-007-A — The track representation

- **WHEN** the system returns a track
- **THEN** it carries the track index as `id`, the recording as `recording_id`, the backing file as `file_id`, the
  medium index, the recording's name and ISRC, and the album and medium links

#### Scenario: SCN-MUSIC-007-B — The track carries no artwork

- **WHEN** the system returns a track whose album stores a cover
- **THEN** the track carries no `cover_image_id` and no `_links.cover`
- **AND** the album representation carries that cover

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

- **WHEN** the name is empty or too long, the group does not exist, or a genre is empty
- **THEN** the system answers `422 Unprocessable Content` and creates nothing

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

- **WHEN** the type is outside `CD` and `DIGITAL`, or the album is the seeded default album
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

- **WHEN** the destination already holds another recording, or the album is the seeded default album
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

#### Scenario: SCN-MUSIC-011-D — Unknown track or protected album

- **WHEN** the album, medium or position does not exist
- **THEN** the system answers `404 Not Found`
- **AND** when the album is the seeded default album, the system answers `422 Unprocessable Content`

### Requirement: REQ-MUSIC-012 — List the group collection

The system SHALL return the groups a caller may `Discover`, filterable by `name`. Each collection item SHALL be a
partial representation carrying the group's identifier, name, cover and links.

#### Scenario: SCN-MUSIC-012-A — A caller lists groups

- **WHEN** a caller requests the group collection
- **THEN** the system returns the groups it may discover, each as a partial representation

#### Scenario: SCN-MUSIC-012-B — Name filter

- **WHEN** the caller requests the collection with `name`
- **THEN** the system returns only the groups whose name matches

### Requirement: REQ-MUSIC-013 — Read one group

The system SHALL return one group the caller may `View`, carrying its identifier, name, cover and cover link. An unknown
or invisible group SHALL answer `404 Not Found`.

#### Scenario: SCN-MUSIC-013-A — An existing group

- **WHEN** the caller requests a group it may view
- **THEN** the system answers `200 OK` with the group's identifier, name, cover and cover link

#### Scenario: SCN-MUSIC-013-B — Unknown or invisible group

- **WHEN** the group identifier is unknown, or the caller may not view the group
- **THEN** the system answers `404 Not Found`

### Requirement: REQ-MUSIC-014 — Create a group

The system SHALL create a group from a name and optional cover, requiring `CreateChild` for a music group on the
catalogue. The name SHALL be trimmed, non-empty, at most 100 characters and unique. The cover SHALL reference a
`LIBRARY` image. A created group SHALL answer `201 Created` with its representation.

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

- **WHEN** the name is empty after trimming or too long
- **THEN** the system answers `422 Unprocessable Content`
- **AND** when the cover is not a usable `LIBRARY` image, the system answers `409 Conflict`

### Requirement: REQ-MUSIC-015 — Rename a group or change its artwork

A group update SHALL require `EditMetadata` on the group. An absent name or cover SHALL keep its value, a `null` cover
SHALL clear it, and a supplied cover SHALL replace it. The name SHALL stay trimmed, non-empty, at most 100 characters
and unique; the cover SHALL reference a `LIBRARY` image.

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

- **WHEN** the name is empty after trimming or too long
- **THEN** the system answers `422 Unprocessable Content` and applies nothing

### Requirement: REQ-MUSIC-016 — Protect the default album and group

The seeded album `0` and group `0` (`Unknown`) SHALL be readable like any other. A write that would modify either SHALL
be refused and SHALL apply nothing.

#### Scenario: SCN-MUSIC-016-A — The default album and group are readable

- **WHEN** a caller reads album `0` or group `0`
- **THEN** the system answers `200 OK` with its representation

#### Scenario: SCN-MUSIC-016-B — A group write is refused

- **WHEN** a caller renames group `0` or replaces its cover
- **THEN** the system answers `409 Conflict` and applies nothing

#### Scenario: SCN-MUSIC-016-C — An album write is refused

- **WHEN** a caller updates album `0`, or writes a medium or track of it
- **THEN** the system answers `409 Conflict` or `422 Unprocessable Content` and applies nothing

### Requirement: REQ-MUSIC-017 — Name the group an album belongs to

An album representation SHALL carry a named group link, and album creation and update SHALL accept the group identifier.
An album's unique identity SHALL be its name and group.

#### Scenario: SCN-MUSIC-017-A — The album representation

- **WHEN** the system returns an album
- **THEN** its group link names the album's group

#### Scenario: SCN-MUSIC-017-B — An album moves group

- **WHEN** an update names another group and no colliding album holds the name there
- **THEN** the album is stored under that group

### Requirement: REQ-MUSIC-018 — List the playlist collection

The system SHALL return the playlists a caller may `Discover`, filterable by `name`, `is_public` and `owner_id`. A
playlist the caller may not `Discover` SHALL be absent from the collection.

#### Scenario: SCN-MUSIC-018-A — Public and owned playlists

- **WHEN** a caller requests the playlist collection
- **THEN** it carries the playlists whose **Visibility** is `Public` and the caller's own

#### Scenario: SCN-MUSIC-018-B — Filters

- **WHEN** the caller requests the collection with `name`, `is_public` and `owner_id`
- **THEN** only the playlists matching every filter are returned

#### Scenario: SCN-MUSIC-018-C — An invisible playlist is filtered out

- **WHEN** the caller may not `Discover` another caller's private playlist
- **THEN** it is absent from the collection and the request is not an error

### Requirement: REQ-MUSIC-019 — Create a playlist

The system SHALL create a playlist owned by the caller and visible to it, requiring `CreateChild` for a music playlist
on the playlist collection. The request SHALL carry a name and MAY carry `is_public`, which SHALL default to `false`. A
created playlist SHALL answer `201 Created` with its representation.

#### Scenario: SCN-MUSIC-019-A — A private playlist

- **WHEN** a caller permitted to create playlists submits a valid name and no visibility
- **THEN** the playlist is created, private, owned by the caller, and the system answers `201 Created`

#### Scenario: SCN-MUSIC-019-B — A public playlist

- **WHEN** the request carries `is_public: true`
- **THEN** the playlist's **Visibility** is `Public`

#### Scenario: SCN-MUSIC-019-C — A malformed request

- **WHEN** the name is empty, only whitespace, or longer than 100 characters
- **THEN** the system answers `422 Unprocessable Content` and creates nothing

### Requirement: REQ-MUSIC-020 — Normalize a playlist name and keep it unique per owner

A playlist name SHALL be trimmed and SHALL be at most 100 characters. Two playlists owned by the same caller SHALL NOT
share a name; two callers MAY each hold the same name.

#### Scenario: SCN-MUSIC-020-A — Names are trimmed

- **WHEN** a caller creates or renames a playlist with surrounding whitespace
- **THEN** the stored name is the trimmed value

#### Scenario: SCN-MUSIC-020-B — A duplicate name for one owner

- **WHEN** the caller already owns a playlist with the name
- **THEN** the system answers `409 Conflict` and creates or applies nothing

#### Scenario: SCN-MUSIC-020-C — The same name for two owners

- **WHEN** two callers each create a playlist with the same name
- **THEN** both playlists exist

### Requirement: REQ-MUSIC-021 — Read one playlist

The system SHALL return one playlist the caller may `View`, carrying its identifier, owner, name, **Visibility**, cover,
timestamps, track count and its tracks in ascending index order. A playlist's track collection SHALL carry the same
representation as the playlist. A playlist the caller may not view SHALL answer `404 Not Found`.

#### Scenario: SCN-MUSIC-021-A — An existing playlist

- **WHEN** the caller requests a playlist it may view
- **THEN** the system answers `200 OK` with its metadata and tracks in ascending index order

#### Scenario: SCN-MUSIC-021-B — The track collection

- **WHEN** the caller requests the collection of a playlist's tracks
- **THEN** the system answers the same representation as the playlist read

#### Scenario: SCN-MUSIC-021-C — Unknown, invisible or invalid

- **WHEN** the playlist is unknown or the caller may not view it
- **THEN** the system answers `404 Not Found`
- **AND** when the identifier is not valid, the system answers `400 Bad Request`

### Requirement: REQ-MUSIC-022 — Read one playlist track

The system SHALL return one track addressed by its index in the playlist. An index the playlist does not sequence, or a
playlist the caller may not view, SHALL answer `404 Not Found`.

#### Scenario: SCN-MUSIC-022-A — An existing track

- **WHEN** the caller requests an index the playlist sequences
- **THEN** the system answers `200 OK` with the track representation

#### Scenario: SCN-MUSIC-022-B — Position not sequenced

- **WHEN** the requested index is not held by the playlist
- **THEN** the system answers `404 Not Found`

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
- **AND** it carries no `cover_image_id` and no `_links.cover`

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
- **AND** when it is empty or too long, the system answers `422 Unprocessable Content`

### Requirement: REQ-MUSIC-025 — Add a track to a playlist

A playlist update carrying `add_track` SHALL append one occurrence of the named recording at the next index, requiring
`CreateChild` for a music recording on the playlist and `ViewMetadata` on the recording. The recording SHALL exist and
SHALL NOT already appear in the playlist.

#### Scenario: SCN-MUSIC-025-A — A recording is appended

- **WHEN** the caller holds both actions and the recording is absent from the playlist
- **THEN** one occurrence is appended at the next index

#### Scenario: SCN-MUSIC-025-B — The target recording is unreadable

- **WHEN** the caller may add to the playlist but may not read the recording's metadata
- **THEN** the system answers `403 Forbidden` and adds nothing

#### Scenario: SCN-MUSIC-025-C — Unknown or duplicate recording

- **WHEN** the recording does not exist
- **THEN** the system answers `422 Unprocessable Content`
- **AND** when it already appears in the playlist, the system answers `409 Conflict`

### Requirement: REQ-MUSIC-026 — Remove a track from a playlist

The system SHALL require `Delete` on the playlist to remove an addressed occurrence, remove it, and answer
`204 No Content`. Removing a track SHALL NOT delete the recording, its audio or its file.

#### Scenario: SCN-MUSIC-026-A — A track is removed

- **WHEN** the caller removes an index the playlist sequences
- **THEN** the occurrence is removed and the system answers `204 No Content`

#### Scenario: SCN-MUSIC-026-B — The recording survives

- **WHEN** a playlist track is removed
- **THEN** the recording and its backing audio and file still exist

#### Scenario: SCN-MUSIC-026-C — Position not sequenced

- **WHEN** the requested index is not held by the playlist
- **THEN** the system answers `404 Not Found`

### Requirement: REQ-MUSIC-027 — Reorder the tracks of a playlist

Reorder SHALL require `EditMetadata` on the playlist and accept a complete permutation of the playlist's current
indices. The tracks SHALL be reindexed to `0..n-1` in the requested order. An incomplete, duplicate or unknown set SHALL
answer `422 Unprocessable Content`.

#### Scenario: SCN-MUSIC-027-A — A complete permutation

- **WHEN** the request carries every current index exactly once
- **THEN** the tracks are reindexed in that order and the system answers `200 OK`

#### Scenario: SCN-MUSIC-027-B — An incomplete or invalid permutation

- **WHEN** the request omits an index, repeats one, or names an unknown one
- **THEN** the system answers `422 Unprocessable Content` and leaves the order unchanged

### Requirement: REQ-MUSIC-028 — Delete a playlist

Playlist deletion SHALL require `Delete` on the playlist, delete the playlist and its track placements, and answer
`204 No Content`. The recordings, audio and files it referenced SHALL survive.

#### Scenario: SCN-MUSIC-028-A — A playlist is deleted

- **WHEN** the caller deletes a playlist it may delete
- **THEN** the playlist and its placements are removed and the system answers `204 No Content`

#### Scenario: SCN-MUSIC-028-B — The recordings survive

- **WHEN** the playlist is deleted
- **THEN** its recordings, audio and files still exist

#### Scenario: SCN-MUSIC-028-C — Unknown or invisible playlist

- **WHEN** the playlist is unknown, or the caller may not view it
- **THEN** the system answers `404 Not Found`

### Requirement: REQ-MUSIC-029 — An album's metadata is updated partially

The system SHALL update an album a **Caller** may `View`, requiring `EditMetadata` on it. An absent field SHALL keep its
stored value and a `null` SHALL clear an optional value. A name and group that duplicate another album SHALL answer
`409 Conflict` and apply nothing; a value that fails validation SHALL answer `422 Unprocessable Content` and apply
nothing.

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

- **WHEN** the name is empty or too long, a genre is empty, or the group does not exist
- **THEN** the system answers `422 Unprocessable Content` and applies nothing
