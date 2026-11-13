# Spec Delta

## MODIFIED Requirements

### Requirement: A file is readable by its identifier

The system SHALL return the file identified by `GET /api/v1/asset/file/{file_id}` with its identifier, path, media type,
size, upload date, modification instant, origin and public flag. The representation SHALL carry `_links.owner`,
`_links.content`, and `_links.image`, `_links.audio` or `_links.video` when the corresponding media row exists.

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
height, orientation, creation date and backing file identifier. The representation SHALL carry `_links.file` and
`_links.content`.

#### Scenario: Image of an existing identifier

- **WHEN** an authenticated caller requests an image it may read
- **THEN** the system returns `200 OK` with the image representation and its file link

#### Scenario: The image representation links its content

- **WHEN** an authenticated caller reads an image it may read
- **THEN** the representation carries `_links.content` naming the image's content route

#### Scenario: Unknown or unreadable image

- **WHEN** the image does not exist, or the caller may not read its file
- **THEN** the system returns `404 Not Found`

### Requirement: A typed audio is readable

The system SHALL return the audio identified by `GET /api/v1/asset/audio/{audio_id}` with its identifier, name,
duration, codec, sample rate and backing file identifier, and its bit depth and block size when the content reports
them. The representation SHALL carry `_links.file` and `_links.content`. `GET /api/v1/asset/audio/{audio_id}/file` SHALL
answer `303 See Other` with a `Location` naming the file.

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
`_links.file` and `_links.content`. `GET /api/v1/asset/video/{video_id}/file` SHALL answer `303 See Other` with a
`Location` naming the file.

#### Scenario: Video of an existing identifier

- **WHEN** an authenticated caller requests a video it may read
- **THEN** the system returns `200 OK` with the video representation and its file link

#### Scenario: The video addresses its file

- **WHEN** an authenticated caller requests the file alias of a video it may read
- **THEN** the system returns `303 See Other` with the file resource in `Location`

#### Scenario: Unknown or unreadable video

- **WHEN** the video does not exist, or the caller may not read its file
- **THEN** the system returns `404 Not Found`
