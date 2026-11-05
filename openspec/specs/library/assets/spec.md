# library/assets Specification

## Purpose

Serves the stored files and their typed image view over HTTP, so a client can resolve an identifier it holds, follow a
cover or track link, and fetch the backing bytes under one authorization rule.

## Requirements

### Requirement: A file is readable by its identifier

The system SHALL return the file identified by `GET /api/v1/asset/file/{file_id}` with its identifier, path, media type,
size, upload date, modification instant, origin and public flag. The representation SHALL carry `_links.self` and
`_links.owner`, and `_links.image`, `_links.audio` or `_links.video` when the corresponding media row exists.

#### Scenario: File of an existing identifier

- **WHEN** an authenticated caller requests a file it may read
- **THEN** the system returns `200 OK` with the file representation and its owner link

#### Scenario: An image-backed file links its image

- **WHEN** the file has an image row
- **THEN** the representation carries `_links.image` naming the image resource

#### Scenario: Unknown or unreadable file

- **WHEN** the file does not exist, or the caller may not read it
- **THEN** the system returns `404 Not Found`, indistinguishable from an unknown file

### Requirement: A typed image is readable

The system SHALL return the image identified by `GET /api/v1/asset/image/{image_id}` with its identifier, name, width,
height, orientation, creation date and backing file identifier. The representation SHALL carry `_links.self` and
`_links.file`.

#### Scenario: Image of an existing identifier

- **WHEN** an authenticated caller requests an image it may read
- **THEN** the system returns `200 OK` with the image representation and its file link

#### Scenario: Unknown or unreadable image

- **WHEN** the image does not exist, or the caller may not read its file
- **THEN** the system returns `404 Not Found`

### Requirement: A typed image addresses its file

The system SHALL serve `GET /api/v1/asset/image/{image_id}/file` as an alias of the backing file resource, answering
`303 See Other` with a `Location` naming the file.

#### Scenario: Alias redirects

- **WHEN** an authenticated caller requests the file alias of an image it may read
- **THEN** the system returns `303 See Other` with the file resource in `Location`

### Requirement: Asset content is readable

The system SHALL serve the bytes of a readable file at `GET /api/v1/asset/file/{file_id}/content` and the bytes of a
readable image at `GET /api/v1/asset/image/{image_id}/content`, with the stored media type.

#### Scenario: Content of a readable image

- **WHEN** an authenticated caller requests the content of an image it may read
- **THEN** the system returns `200 OK` with the image bytes and the image media type

#### Scenario: Content of an unreadable asset

- **WHEN** the caller may not read the asset
- **THEN** the system returns `404 Not Found` and no bytes
