# video Specification

## Purpose

Lets a client read one stored video file as a typed entity: the file it represents, the measurements the content
reports, the summary shape other resources embed, and the link to its content.

## Requirements

### Requirement: REQ-VIDEO-001 — A video is a typed moving-image file

A video SHALL be created only from content the intake identifies as video, and SHALL reference exactly one stored file.
A stored file SHALL back at most one video.

#### Scenario: SCN-VIDEO-001-A — Video content creates one video

- **WHEN** a video file completes intake
- **THEN** one video exists, backed by the file the intake registered
- **AND** the file representation links that video

#### Scenario: SCN-VIDEO-001-B — A file backs one video

- **WHEN** the same stored file is registered again
- **THEN** no second video is created for it

#### Scenario: SCN-VIDEO-001-C — Content of another media type is not a video

- **WHEN** image or audio content completes intake
- **THEN** no video is created

### Requirement: REQ-VIDEO-002 — A video reports its characteristics

A video SHALL report a codec, a colour identifier from the stored vocabulary, a non-negative duration in milliseconds, a
positive frame count, a positive width and height in pixels, and a scan type of `PROGRESSIVE`, `INTERLACED`, `MBAFF` or
`PAFF`. A video SHALL NOT carry a name. A measurement the content does not report SHALL NOT be fabricated.

#### Scenario: SCN-VIDEO-002-A — The measurements are reported

- **WHEN** the system returns a video
- **THEN** the representation carries its codec, colour identifier, duration, frame count, width, height and scan type

#### Scenario: SCN-VIDEO-002-B — Scan types are the discrete set

- **WHEN** the content reports an interlaced, macroblock-adaptive or progressive scan
- **THEN** the representation carries `INTERLACED`, `MBAFF` or `PROGRESSIVE` respectively

#### Scenario: SCN-VIDEO-002-C — A video carries no name

- **WHEN** the system returns a video
- **THEN** the representation carries no name field

### Requirement: REQ-VIDEO-003 — A video is readable

A caller who may read a video SHALL read it. The representation SHALL carry the video identifier, codec, colour
identifier, duration, frame count, width, height, scan type and backing file identifier. An unknown or unreadable video
SHALL answer `404 Not Found`, indistinguishable from a missing one.

#### Scenario: SCN-VIDEO-003-A — A video of an existing identifier

- **WHEN** an authenticated caller requests a video it may read
- **THEN** the system returns `200 OK` with the video representation

#### Scenario: SCN-VIDEO-003-B — Unknown or unreadable video

- **WHEN** the video does not exist, or the caller may not read it
- **THEN** the system returns `404 Not Found`

### Requirement: REQ-VIDEO-004 — A video addresses its file

The system SHALL serve the file alias of a video as an alias of the backing file resource, answering `303 See Other`
with a `Location` naming the file.

#### Scenario: SCN-VIDEO-004-A — The alias redirects

- **WHEN** an authenticated caller requests the file alias of a video it may read
- **THEN** the system returns `303 See Other` with the file resource in `Location`

### Requirement: REQ-VIDEO-005 — A resource embeds the video summary shape

A **resource** that embeds a video SHALL carry the video identifier, backing file identifier, codec, duration, width,
height and scan type.

#### Scenario: SCN-VIDEO-005-A — A film embeds the summary

- **WHEN** the system returns a film backed by a video
- **THEN** the film representation carries the video summary
