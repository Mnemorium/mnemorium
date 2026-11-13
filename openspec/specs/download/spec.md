# download Specification

## Purpose

Serves the bytes of a stored asset to an authenticated caller who may perform the route's action, whole or in part, with
the media type, validators, range semantics and attachment name a client needs to render, play or save it.

## Requirements

### Requirement: REQ-DOWNLOAD-001 — Content requires the session credential

Every content route SHALL authenticate its request with `Authorization: Bearer` and SHALL decide that request as it
arrives. A request without a valid session token SHALL answer `401 Unauthorized` and read no bytes. The system SHALL NOT
accept a credential supplied in the query string, and no representation SHALL publish one.

#### Scenario: SCN-DOWNLOAD-001-A — A bearer request reads content

- **WHEN** a caller requests content with a valid bearer token and an allowed action
- **THEN** the system serves the bytes with the stored media type

#### Scenario: SCN-DOWNLOAD-001-B — A missing token is refused

- **WHEN** a content request carries no `Authorization` header
- **THEN** the system answers `401 Unauthorized` and reads no bytes

#### Scenario: SCN-DOWNLOAD-001-C — A query value is not a credential

- **WHEN** a content request carries a token or a signature in the query string and no bearer header
- **THEN** the system answers `401 Unauthorized`

### Requirement: REQ-DOWNLOAD-002 — Each content route decides one action

The image content route SHALL decide `View`, the audio and video content routes SHALL decide `Stream`, and the file
content route SHALL decide `Download`, against the file resource the addressed asset resolves to, on every request. Who
holds an action is not stated here.

#### Scenario: SCN-DOWNLOAD-002-A — Image content is rendered

- **WHEN** a caller requests image content
- **THEN** the decision is `View` on the image's file

#### Scenario: SCN-DOWNLOAD-002-B — Audio and video content is played

- **WHEN** a caller requests audio content or video content
- **THEN** the decision is `Stream` on the addressed asset's file

#### Scenario: SCN-DOWNLOAD-002-C — File content is downloaded

- **WHEN** a caller requests file content
- **THEN** the decision is `Download` on that file

#### Scenario: SCN-DOWNLOAD-002-D — A changed decision applies to the next request

- **WHEN** a grant or a deny changes while a client is reading an asset
- **THEN** the client's next content request is decided with the new state

### Requirement: REQ-DOWNLOAD-003 — Content is served with its stored media type

The system SHALL serve the bytes of the addressed asset's file with the media type stored for that file, and SHALL NOT
reclassify it.

#### Scenario: SCN-DOWNLOAD-003-A — Image content carries the stored type

- **WHEN** a caller reads the content of a readable image
- **THEN** the response carries the bytes and the stored media type of its file

#### Scenario: SCN-DOWNLOAD-003-B — Audio and video content carry the stored type

- **WHEN** a caller reads the content of a readable audio or video
- **THEN** the response carries the bytes and the stored media type of its file

#### Scenario: SCN-DOWNLOAD-003-C — File content carries the stored type

- **WHEN** a caller reads the content of a readable file
- **THEN** the response carries the bytes and the stored media type of that file

### Requirement: REQ-DOWNLOAD-004 — Content is streamed in bounded chunks

The system SHALL serve content in bounded chunks and SHALL NOT require the whole content in memory. A `HEAD` request
SHALL answer with the same headers as `GET` and no body.

#### Scenario: SCN-DOWNLOAD-004-A — Content streams in bounded chunks

- **WHEN** a caller reads content
- **THEN** the system streams the bytes from the stored file in bounded chunks

#### Scenario: SCN-DOWNLOAD-004-B — A HEAD request answers without content

- **WHEN** a caller sends `HEAD` to a content route it may read
- **THEN** the system answers with the response headers and no body

### Requirement: REQ-DOWNLOAD-005 — Content supports byte ranges

The system SHALL advertise `Accept-Ranges: bytes` on every content response and SHALL answer a request carrying one
`Range: bytes=` header with `206 Partial Content`, a `Content-Range` header and only the requested bytes. A closed
range, an open-ended range and a suffix range SHALL be supported. A range the content cannot satisfy SHALL answer
`416 Range Not Satisfiable` with `Content-Range: bytes */<size>`. A request naming more than one range SHALL be answered
with the whole content.

#### Scenario: SCN-DOWNLOAD-005-A — A closed range

- **WHEN** a caller requests `Range: bytes=0-99` from content of at least 100 bytes
- **THEN** the system answers `206 Partial Content` with 100 bytes and `Content-Range: bytes 0-99/<size>`

#### Scenario: SCN-DOWNLOAD-005-B — An open-ended range

- **WHEN** a caller requests `Range: bytes=<offset>-`
- **THEN** the system answers the bytes from that offset to the end

#### Scenario: SCN-DOWNLOAD-005-C — A suffix range

- **WHEN** a caller requests `Range: bytes=-500` from content of at least 500 bytes
- **THEN** the system answers the final 500 bytes

#### Scenario: SCN-DOWNLOAD-005-D — An unsatisfiable range

- **WHEN** the requested range begins past the end of the content
- **THEN** the system answers `416 Range Not Satisfiable` with `Content-Range: bytes */<size>`

#### Scenario: SCN-DOWNLOAD-005-E — More than one range

- **WHEN** a request names several ranges in one `Range` header
- **THEN** the system answers `200 OK` with the whole content

### Requirement: REQ-DOWNLOAD-006 — Content carries validators

The system SHALL answer a content response with an `ETag` derived from the stored whole-file digest and a
`Last-Modified` naming the content's modification instant. A request whose `If-None-Match` matches the content SHALL
answer `304 Not Modified` with no body. A request whose `If-Range` does not match the content SHALL answer the whole
content instead of a partial one.

#### Scenario: SCN-DOWNLOAD-006-A — A content response carries its validators

- **WHEN** a caller reads content
- **THEN** the response carries the file's `ETag` and `Last-Modified`

#### Scenario: SCN-DOWNLOAD-006-B — The content is unchanged

- **WHEN** a caller repeats a content request with a matching `If-None-Match`
- **THEN** the system answers `304 Not Modified` with no body

#### Scenario: SCN-DOWNLOAD-006-C — A range is conditional on the content

- **WHEN** a caller sends a `Range` with an `If-Range` naming the current content
- **THEN** the system answers the requested range

#### Scenario: SCN-DOWNLOAD-006-D — The validator no longer matches

- **WHEN** a caller sends a `Range` with an `If-Range` naming different content
- **THEN** the system answers the whole content with `200 OK`

### Requirement: REQ-DOWNLOAD-007 — File content is served as an attachment

The file content route SHALL answer with `Content-Disposition: attachment`, naming the addressed asset's original name —
the name its typed media row carries when the file has one, the final segment of the stored path otherwise — and SHALL
encode a name that cannot be written as a plain quoted token. Every other content route SHALL NOT carry an attachment
disposition.

#### Scenario: SCN-DOWNLOAD-007-A — A typed file keeps its original name

- **WHEN** a caller downloads the file of an image, an audio or a video
- **THEN** the disposition names the typed media row's original name

#### Scenario: SCN-DOWNLOAD-007-B — A bare file keeps its stored name

- **WHEN** a caller downloads a file no typed media row backs
- **THEN** the disposition names the final segment of the file's stored path

#### Scenario: SCN-DOWNLOAD-007-C — A display route is not an attachment

- **WHEN** a caller reads image, audio or video content
- **THEN** the response carries no `Content-Disposition`

#### Scenario: SCN-DOWNLOAD-007-D — An awkward name is encoded

- **WHEN** the original name carries a quote, a separator or a non-ASCII character
- **THEN** the disposition is encoded so the name round-trips intact

### Requirement: REQ-DOWNLOAD-008 — A content outcome is distinguishable

Content SHALL distinguish an asset the caller cannot discover (`404 Not Found`), an asset it can discover whose action
refuses (`403 Forbidden`), an absent library root (`503 Service Unavailable`) and a registered file whose bytes are
absent (`500 Internal Server Error`). The denial rule's concealment is not restated here.

#### Scenario: SCN-DOWNLOAD-008-A — The asset is unknown or undiscoverable

- **WHEN** the addressed asset does not exist, or the caller cannot discover it
- **THEN** the system answers `404 Not Found` and reads no bytes

#### Scenario: SCN-DOWNLOAD-008-B — The asset is visible but the action refuses

- **WHEN** the caller can discover the addressed asset and its action refuses
- **THEN** the system answers `403 Forbidden` and reads no bytes

#### Scenario: SCN-DOWNLOAD-008-C — The library root is unavailable

- **WHEN** a readable asset's content is requested while the library root is absent
- **THEN** the system answers `503 Service Unavailable`

#### Scenario: SCN-DOWNLOAD-008-D — A registered file has no bytes

- **WHEN** a readable file's record exists but its bytes are absent
- **THEN** the system answers `500 Internal Server Error`

### Requirement: REQ-DOWNLOAD-009 — A refused read touches no bytes

The system SHALL NOT read an asset's bytes for a request whose decision refuses, whose asset is unknown, or whose
library root is absent.

#### Scenario: SCN-DOWNLOAD-009-A — A refused request reads nothing

- **WHEN** a content request is refused
- **THEN** the response carries no content and the stored bytes are never opened
