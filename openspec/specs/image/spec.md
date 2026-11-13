# image Specification

## Purpose

Lets a client read one stored image as a typed entity: the file it represents, the measurements the content reports, and
the link to its content.

## Requirements

### Requirement: REQ-IMAGE-001 — An image is a typed visual file

An image SHALL be created only from content the intake identifies as an image, and SHALL reference exactly one stored
file. A stored file SHALL back at most one image.

#### Scenario: SCN-IMAGE-001-A — Image content creates one image

- **WHEN** an image file completes intake
- **THEN** one image exists, backed by the file the intake registered
- **AND** the file representation links that image

#### Scenario: SCN-IMAGE-001-B — A file backs one image

- **WHEN** the same stored file is registered again
- **THEN** no second image is created for it

#### Scenario: SCN-IMAGE-001-C — Content of another media type is not an image

- **WHEN** audio or video content completes intake
- **THEN** no image is created

### Requirement: REQ-IMAGE-002 — An image reports its characteristics

An image SHALL report a non-empty name, a positive width and height in pixels, an orientation derived from those
dimensions, and the date it was created. Orientation SHALL be `LANDSCAPE` when width exceeds height, `PORTRAIT` when
height exceeds width, and `SQUARE` when they are equal. A value the content does not report SHALL NOT be fabricated.

#### Scenario: SCN-IMAGE-002-A — The image is wider than it is tall

- **WHEN** the system returns an image whose width exceeds its height
- **THEN** its orientation is `LANDSCAPE`

#### Scenario: SCN-IMAGE-002-B — The image is taller than it is wide

- **WHEN** the system returns an image whose height exceeds its width
- **THEN** its orientation is `PORTRAIT`

#### Scenario: SCN-IMAGE-002-C — The image is square

- **WHEN** the system returns an image whose width equals its height
- **THEN** its orientation is `SQUARE`

#### Scenario: SCN-IMAGE-002-D — The name and creation date are reported

- **WHEN** the system returns an image
- **THEN** its name is the non-empty name the intake assigned
- **AND** its creation date is the date the image was registered

### Requirement: REQ-IMAGE-003 — An image is readable

A caller who may read an image SHALL read it. The representation SHALL carry the image identifier, name, dimensions,
orientation, creation date and backing file identifier. An unknown or unreadable image SHALL answer `404 Not Found`,
indistinguishable from a missing one.

#### Scenario: SCN-IMAGE-003-A — An image of an existing identifier

- **WHEN** an authenticated caller requests an image it may read
- **THEN** the system returns `200 OK` with the image representation

#### Scenario: SCN-IMAGE-003-B — Unknown or unreadable image

- **WHEN** the image does not exist, or the caller may not read it
- **THEN** the system returns `404 Not Found`

### Requirement: REQ-IMAGE-004 — An image addresses its file

The system SHALL serve the file alias of an image as an alias of the backing file resource, answering `303 See Other`
with a `Location` naming the file.

#### Scenario: SCN-IMAGE-004-A — The alias redirects

- **WHEN** an authenticated caller requests the file alias of an image it may read
- **THEN** the system returns `303 See Other` with the file resource in `Location`
