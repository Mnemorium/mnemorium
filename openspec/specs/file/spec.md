# file Specification

## Purpose

Lets a client resolve one **Stored file** by its identifier: the content it records and the measurements the server
holds for it, the account that owns it, and the typed image, audio and video views and content route it links.

## Requirements

### Requirement: REQ-FILE-001 — A stored file records one stored content

A **Stored file** SHALL record one content stored under the **Library root**, addressed by a path relative to that root,
carrying its media type, its size in bytes, the instant its content was last modified, the date it was registered, its
origin and whether it is public. Origin SHALL be `UPLOADED` when the server received the content through upload and
`SCANNED` when it found the content under the root.

#### Scenario: SCN-FILE-001-A — Content the server received

- **WHEN** the server promotes content a caller uploaded
- **THEN** the record's origin is `UPLOADED`
- **AND** its path is relative to the **Library root**

#### Scenario: SCN-FILE-001-B — Content the server found

- **WHEN** the ingest registers content it found under the **Library root**
- **THEN** the record's origin is `SCANNED`

#### Scenario: SCN-FILE-001-C — The record carries the content's measurements

- **WHEN** a file is registered
- **THEN** its record carries the content's media type, size in bytes and last modification instant
- **AND** its registration date is the date it was recorded

### Requirement: REQ-FILE-002 — A file is readable by its identifier

The system SHALL return the **Stored file** a **Caller** may `View`, carrying its identifier, root-relative path, media
type, size in bytes, registration date, modification instant, origin and public flag. An unknown or unviewable file
SHALL answer `404 Not Found`, indistinguishable from a missing one, and an identifier that is not valid SHALL answer
`400 Bad Request`.

#### Scenario: SCN-FILE-002-A — A file of an existing identifier

- **WHEN** a caller requests a file it may view
- **THEN** the system answers `200 OK` with the file's identifier, path, media type, size, dates, origin and public flag

#### Scenario: SCN-FILE-002-B — Unknown or unviewable file

- **WHEN** the file does not exist, or the caller may not view it
- **THEN** the system answers `404 Not Found`

#### Scenario: SCN-FILE-002-C — An identifier that is not valid

- **WHEN** the requested identifier is not a valid identifier
- **THEN** the system answers `400 Bad Request`

### Requirement: REQ-FILE-003 — A file publishes its links

The file representation SHALL carry `_links.owner` naming the account that owns it and `_links.content` naming its
content route, whose delivery `download` owns. It SHALL carry `_links.image`, `_links.audio` or `_links.video` naming
the typed asset backed by the file when that row exists, and SHALL omit that link when it does not.

#### Scenario: SCN-FILE-003-A — The representation links its owner and content

- **WHEN** a caller reads a file it may view
- **THEN** `_links.owner` names the account that owns it
- **AND** `_links.content` names the file's content route

#### Scenario: SCN-FILE-003-B — An image-backed file links its image

- **WHEN** the file has an image row
- **THEN** the representation carries `_links.image` naming the image resource

#### Scenario: SCN-FILE-003-C — A typed media-backed file links its typed view

- **WHEN** the file has an audio row or a video row
- **THEN** the representation carries `_links.audio` or `_links.video` naming the typed resource

#### Scenario: SCN-FILE-003-D — A file with no media row

- **WHEN** the file backs no typed asset
- **THEN** the representation carries none of `_links.image`, `_links.audio` and `_links.video`
