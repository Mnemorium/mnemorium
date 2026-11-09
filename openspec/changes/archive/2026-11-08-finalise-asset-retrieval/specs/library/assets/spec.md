# Spec Delta

## MODIFIED Requirements

### Requirement: A file is readable by its identifier

The system SHALL return the file identified by `GET /api/v1/asset/file/{file_id}` with its identifier, path, media type,
size, upload date, modification instant, origin and public flag. The representation SHALL carry `_links.self` and
`_links.owner`, `_links.content`, and `_links.image`, `_links.audio` or `_links.video` when the corresponding media row
exists.

#### Scenario: File of an existing identifier

- **WHEN** an authenticated caller requests a file it may read
- **THEN** the system returns `200 OK` with the file representation and its owner link

#### Scenario: An image-backed file links its image

- **WHEN** the file has an image row
- **THEN** the representation carries `_links.image` naming the image resource

#### Scenario: A typed media-backed file links its typed view

- **WHEN** the file has an audio row or a video row
- **THEN** the representation carries `_links.audio` or `_links.video` naming the typed resource

#### Scenario: The file representation links its content

- **WHEN** an authenticated caller reads a file it may read
- **THEN** the representation carries `_links.content` naming the file's content route

#### Scenario: Unknown or unreadable file

- **WHEN** the file does not exist, or the caller may not read it
- **THEN** the system returns `404 Not Found`, indistinguishable from an unknown file

### Requirement: A typed image is readable

The system SHALL return the image identified by `GET /api/v1/asset/image/{image_id}` with its identifier, name, width,
height, orientation, creation date and backing file identifier. The representation SHALL carry `_links.self`,
`_links.file` and `_links.content`.

#### Scenario: Image of an existing identifier

- **WHEN** an authenticated caller requests an image it may read
- **THEN** the system returns `200 OK` with the image representation and its file link

#### Scenario: The image representation links its content

- **WHEN** an authenticated caller reads an image it may read
- **THEN** the representation carries `_links.content` naming the image's content route

#### Scenario: Unknown or unreadable image

- **WHEN** the image does not exist, or the caller may not read its file
- **THEN** the system returns `404 Not Found`

### Requirement: Asset content is readable

The system SHALL serve the bytes of a readable file at `GET /api/v1/asset/file/{file_id}/content`, of a readable image
at `GET /api/v1/asset/image/{image_id}/content`, of a readable audio at `GET /api/v1/asset/audio/{audio_id}/content` and
of a readable video at `GET /api/v1/asset/video/{video_id}/content`, with the stored media type. The content SHALL be
served in bounded chunks and the system SHALL NOT require the whole content in memory. A `HEAD` request SHALL answer
with the same headers and no body.

#### Scenario: Content of a readable image

- **WHEN** an authenticated caller requests the content of an image it may read
- **THEN** the system returns `200 OK` with the image bytes and the image media type

#### Scenario: Content of a readable audio or video

- **WHEN** an authenticated caller requests the content of an audio or a video it may read
- **THEN** the system returns `200 OK` with the stored bytes and the stored media type

#### Scenario: A HEAD request answers without content

- **WHEN** an authenticated caller sends `HEAD` to a content route it may read
- **THEN** the system returns the response headers and no body

#### Scenario: Content of an unreadable asset

- **WHEN** the caller may not read the asset
- **THEN** the system returns `404 Not Found` and no bytes

## ADDED Requirements

### Requirement: A typed audio is readable

The system SHALL return the audio identified by `GET /api/v1/asset/audio/{audio_id}` with its identifier, name,
duration, codec, sample rate and backing file identifier, and its bit depth and block size when the content reports
them. The representation SHALL carry `_links.self`, `_links.file` and `_links.content`.
`GET /api/v1/asset/audio/{audio_id}/file` SHALL answer `303 See Other` with a `Location` naming the file.

#### Scenario: Audio of an existing identifier

- **WHEN** an authenticated caller requests an audio it may read
- **THEN** the system returns `200 OK` with the audio representation and its file link

#### Scenario: A measurement the content does not report

- **WHEN** the audio's content reports no bit depth or block size
- **THEN** the representation carries that measurement as absent rather than a fabricated value

#### Scenario: The audio addresses its file

- **WHEN** an authenticated caller requests the file alias of an audio it may read
- **THEN** the system returns `303 See Other` with the file resource in `Location`

#### Scenario: Unknown or unreadable audio

- **WHEN** the audio does not exist, or the caller may not read its file
- **THEN** the system returns `404 Not Found`

### Requirement: A typed video is readable

The system SHALL return the video identified by `GET /api/v1/asset/video/{video_id}` with its identifier, codec, colour
identifier, duration, frame count, height, width, scan type and backing file identifier. The representation SHALL carry
`_links.self`, `_links.file` and `_links.content`. `GET /api/v1/asset/video/{video_id}/file` SHALL answer
`303 See Other` with a `Location` naming the file.

#### Scenario: Video of an existing identifier

- **WHEN** an authenticated caller requests a video it may read
- **THEN** the system returns `200 OK` with the video representation and its file link

#### Scenario: The video addresses its file

- **WHEN** an authenticated caller requests the file alias of a video it may read
- **THEN** the system returns `303 See Other` with the file resource in `Location`

#### Scenario: Unknown or unreadable video

- **WHEN** the video does not exist, or the caller may not read its file
- **THEN** the system returns `404 Not Found`

### Requirement: Content supports byte ranges

The system SHALL advertise `Accept-Ranges: bytes` on every content response and SHALL answer a request carrying one
`Range: bytes=` header with `206 Partial Content`, a `Content-Range` header and only the requested bytes. A closed
range, an open-ended range and a suffix range SHALL be supported. A range the content cannot satisfy SHALL answer
`416 Range Not Satisfiable` with `Content-Range: bytes */<size>`. A request naming more than one range SHALL be answered
with the whole content.

#### Scenario: A closed range

- **WHEN** a caller requests `Range: bytes=0-99` from content of at least 100 bytes
- **THEN** the system returns `206 Partial Content` with 100 bytes and `Content-Range: bytes 0-99/<size>`

#### Scenario: An open-ended range

- **WHEN** a caller requests `Range: bytes=<offset>-` from content
- **THEN** the system returns the bytes from that offset to the end

#### Scenario: A suffix range

- **WHEN** a caller requests `Range: bytes=-500` from content of at least 500 bytes
- **THEN** the system returns the final 500 bytes

#### Scenario: An unsatisfiable range

- **WHEN** the requested range begins past the end of the content
- **THEN** the system returns `416 Range Not Satisfiable` with `Content-Range: bytes */<size>`

#### Scenario: More than one range

- **WHEN** a request names several ranges in one `Range` header
- **THEN** the system returns `200 OK` with the whole content

### Requirement: Content carries validators

The system SHALL answer a content response with an `ETag` derived from the stored whole-file digest and a
`Last-Modified` naming the content's modification instant. A request whose `If-None-Match` matches the content SHALL
answer `304 Not Modified` with no body. A request whose `If-Range` does not match the content SHALL answer the whole
content instead of a partial one.

#### Scenario: A content response carries its validators

- **WHEN** a caller reads content
- **THEN** the response carries the file's `ETag` and `Last-Modified`

#### Scenario: The content is unchanged

- **WHEN** a caller repeats a content request with a matching `If-None-Match`
- **THEN** the system returns `304 Not Modified` and no body

#### Scenario: A range is conditional on the content

- **WHEN** a caller sends a `Range` with an `If-Range` naming the current content
- **THEN** the system returns the requested range

#### Scenario: The validator no longer matches

- **WHEN** a caller sends a `Range` with an `If-Range` naming different content
- **THEN** the system returns the whole content with `200 OK`

### Requirement: A content link carries its own credential

Every file, image, audio and video representation SHALL carry `_links.content` naming the content route. The link SHALL
carry a credential scoped to that one file that expires, so a client that cannot set an authorization header can read
the content. A content request SHALL accept either the bearer header or that credential, and SHALL NOT accept the
session token in the URL.

#### Scenario: A representation links playable content

- **WHEN** an authenticated caller reads a file, image, audio or video it may read
- **THEN** `_links.content` names the content route and carries a credential scoped to that file

#### Scenario: Content is read without an authorization header

- **WHEN** a client requests the content link without an `Authorization` header
- **THEN** the system returns the content

#### Scenario: The credential has expired

- **WHEN** a client requests content with an expired credential and no bearer header
- **THEN** the system returns `401 Unauthorized`

#### Scenario: The credential addresses one file

- **WHEN** a client presents a content credential minted for another file
- **THEN** the system returns `401 Unauthorized`

#### Scenario: The bearer header still works

- **WHEN** a client requests content with a valid bearer header and no credential
- **THEN** the system returns the content

### Requirement: A content outcome is distinguishable

Content SHALL distinguish an asset that does not exist or is unreadable (`404 Not Found`), a library root that is
unavailable (`503 Service Unavailable`) and a registered file whose bytes are absent (`500 Internal Server Error`). An
unreadable asset's bytes SHALL NOT be read.

#### Scenario: The asset is unknown or unreadable

- **WHEN** the asset does not exist, or the caller may not read it
- **THEN** the system returns `404 Not Found` and reads no bytes

#### Scenario: The library root is unavailable

- **WHEN** a readable asset's content is requested while the library root is absent
- **THEN** the system returns `503 Service Unavailable`

#### Scenario: A registered file has no bytes

- **WHEN** a readable file's record exists but its bytes are absent
- **THEN** the system returns `500 Internal Server Error`
