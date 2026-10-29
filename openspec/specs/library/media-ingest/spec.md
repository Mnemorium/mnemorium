# library/media-ingest Specification

## Purpose

Lets the library populate itself from a mounted media root, so files the operator already holds become registered media
without passing through the upload API: the ingest identifies each file by content, registers the ones it accepts, and
keeps what it registered earlier reconciled with what is on disk, at startup and as the directory changes.

## Requirements

### Requirement: Registered files share one fixed root

Every registered file SHALL be addressed relative to a single library root that is a fixed location, and SHALL NOT be
addressed by an absolute path. A file the server stored and a file it found already present SHALL be addressed the same
way, so one root resolves every registered file.

#### Scenario: A stored file is addressed under the root

- **WHEN** the server stores a file it received through the upload flow
- **THEN** the recorded location is relative to the library root
- **AND** it resolves to the stored file under that root

#### Scenario: A file found under the root is addressed the same way

- **WHEN** the scan registers a file already present under the library root
- **THEN** the recorded location is relative to the same root
- **AND** it resolves to that file

### Requirement: An absent library root is a warning

The server SHALL NOT create the library root. When the root is absent at startup the server SHALL warn, SHALL continue
running and SHALL keep serving requests, and the scan SHALL register nothing.

#### Scenario: The root is absent at startup

- **WHEN** the server starts and the library root does not exist
- **THEN** the server records a warning naming the absent root
- **AND** the server accepts requests and answers the health route
- **AND** no file is registered

#### Scenario: The root is present at startup

- **WHEN** the server starts and the library root exists
- **THEN** the scan examines the files under it

### Requirement: Ingest does not delay readiness

The scan SHALL run without blocking the server from accepting requests. A scan in progress SHALL NOT delay readiness,
and SHALL NOT delay a response to a request.

#### Scenario: A request arrives during a scan

- **WHEN** a request reaches the server while the scan is examining the library root
- **THEN** the server answers it normally
- **AND** the scan continues

### Requirement: Media type is determined from content

The system SHALL determine a candidate file's media type from its content, and SHALL NOT rely on the file's name or
extension.

#### Scenario: The extension contradicts the content

- **WHEN** the scan examines a file whose name suggests one type while its content is an accepted media type
- **THEN** the file is classified by its content and registered as that type

#### Scenario: The content type cannot be determined

- **WHEN** the scan examines a file whose content yields no media type
- **THEN** the file is skipped

### Requirement: Only supported image, video and audio files are registered

The system SHALL register only a file whose determined media type is an image, a video or an audio type **and** is a
type the server supports. The seeded `mime_type` table is the supported set. Any other file SHALL be skipped, SHALL NOT
produce an error the scan reports as a failure, and SHALL NOT stop the scan.

#### Scenario: A file that is not media is skipped

- **WHEN** the scan examines a file whose content is not an image, a video or an audio type
- **THEN** nothing is registered for it
- **AND** the scan continues with the remaining files

#### Scenario: A media type the server does not support is skipped

- **WHEN** the scan examines a file whose content is an image, video or audio type the supported set does not contain
- **THEN** nothing is registered for it
- **AND** the scan continues with the remaining files

#### Scenario: Unsuitable files do not abort the scan

- **WHEN** the scan meets several unsuitable files among suitable ones
- **THEN** each suitable file is still registered

### Requirement: Registration records the file and its media row together

The system SHALL register an accepted file by recording its location, size, last modification instant, media type and
whether the server stored it, together with the media row its type requires. The file record and the media row SHALL
commit together or neither SHALL be recorded.

#### Scenario: An image is registered

- **WHEN** the scan accepts a file whose content is an image
- **THEN** its file record and its image row are recorded
- **AND** the image row carries the probed dimensions and orientation

#### Scenario: A video is registered

- **WHEN** the scan accepts a file whose content is a video
- **THEN** its file record and its video row are recorded
- **AND** the video row carries the probed duration, codec, dimensions and scan type

#### Scenario: An audio file is registered

- **WHEN** the scan accepts a file whose content is audio
- **THEN** its file record and its audio row are recorded
- **AND** the audio row carries the probed duration, codec and sample rate
- **AND** a bit depth or block size the content does not report is recorded as absent rather than fabricated

#### Scenario: A probe fails

- **WHEN** an accepted file's media row cannot be built
- **THEN** neither the file record nor the media row is recorded

### Requirement: A registered file carries the size it has on disk

A registered file SHALL record the size the stored content has and the instant it was last modified, so a later scan can
recognise the same content without reading it.

#### Scenario: The size and modification instant are recorded

- **WHEN** a file is registered
- **THEN** its record carries the content's size and last modification instant

### Requirement: A file the scan did not store belongs to the Root Admin

A file the server did not store itself SHALL be attributed to the Root Admin when it is registered, because no account
owns it.

#### Scenario: A found file is readable by every authenticated caller

- **WHEN** the scan registers a file it found under the library root
- **THEN** the file is attributed to the Root Admin
- **AND** an authenticated caller holding the default permission can read the media it backs

### Requirement: A registered file is verified rather than re-registered

When the scan meets a file it has already registered, it SHALL compare the recorded size and last modification instant
with the file and SHALL treat the file as untouched, changed or missing. It SHALL NOT register a second record for a
location it already recorded.

#### Scenario: The file is unchanged

- **WHEN** the scan meets a registered file whose size and last modification instant match its record
- **THEN** the file is reported as untouched
- **AND** nothing is written

#### Scenario: The file changed

- **WHEN** the scan meets a registered file whose size or last modification instant differs from its record
- **THEN** the file is reported as changed
- **AND** its record and content are left as they are

#### Scenario: The file is gone

- **WHEN** the scan no longer finds a file it registered
- **THEN** it is reported as missing
- **AND** its record is retained

### Requirement: A media row the scan can complete is completed

The scan SHALL complete a registered file that carries no media row for its media type by probing the file and recording
the missing row.

#### Scenario: A file record without its media row

- **WHEN** the scan meets a registered audio file that carries no audio row
- **THEN** it probes the file and records the audio row
- **AND** reports the file as completed rather than untouched

### Requirement: The ingest never changes the content it examines

Ingest SHALL only read the file content under the library root. It SHALL NOT create, modify, move or delete that
content.

#### Scenario: Content is left byte-identical

- **WHEN** the ingest examines the files under the library root
- **THEN** the content of every file is byte-identical afterwards
- **AND** no file has been removed, added or moved by the ingest

#### Scenario: An unreadable file does not damage the ingest

- **WHEN** the ingest meets a file it cannot read
- **THEN** it skips that file without altering it
- **AND** it continues with the remaining files

### Requirement: Removing a gallery reference never deletes content

Removing a gallery or a gallery item SHALL delete the recorded reference and SHALL NOT delete the file content under the
library root.

#### Scenario: An item is removed from a gallery

- **WHEN** a caller removes an item from a gallery
- **THEN** the item and its media row are removed as before
- **AND** the file content under the library root is unchanged

#### Scenario: A gallery is removed

- **WHEN** a caller removes a gallery
- **THEN** the gallery and its items are removed as before
- **AND** the file content under the library root is unchanged

### Requirement: A file added while the server runs is registered without a restart

The server SHALL observe the library root while it runs and SHALL register a file added there, without requiring a
restart and without waiting for a later scan.

#### Scenario: A file appears while the server is running

- **WHEN** an accepted file is added under the library root while the server is running
- **THEN** it is registered as if the scan had found it
- **AND** the server is not restarted

#### Scenario: The file is still being written

- **WHEN** a file is added and its content is still changing
- **THEN** it is not registered until its content stops changing
- **AND** it is registered once it is complete, with the size it finally has

#### Scenario: Content is modified in place

- **WHEN** the content of a registered file is modified while the server is running
- **THEN** the file is not registered again
- **AND** its record is left as it is

### Requirement: A renamed file keeps its record

Renaming a file under the library root SHALL update the recorded location of the file it names and SHALL preserve the
media row that location already carried. It SHALL NOT be treated as the removal of one file and the arrival of another.

#### Scenario: A registered file is renamed

- **WHEN** a registered file is renamed while the server is running
- **THEN** the recorded location follows the file
- **AND** its media row is preserved
- **AND** no second record is created for the same content

### Requirement: A removed file is reported without losing its record

Removing a file from the library root SHALL be reported as a file the server registered but no longer finds, and SHALL
retain its record.

#### Scenario: A registered file is removed

- **WHEN** a registered file is removed from the library root while the server is running
- **THEN** it is reported as missing
- **AND** its record is retained

### Requirement: Ingest reports aggregate counts only

Ingest SHALL report its outcome as aggregate counts of registered, completed, untouched, changed, missing and skipped
files. It SHALL NOT record an individual file's name or path.

#### Scenario: A completed ingest is reported

- **WHEN** a scan finishes or an observed change is handled
- **THEN** one record carries the counts of registered, completed, untouched, changed, missing and skipped files
- **AND** it carries no file name and no file path
