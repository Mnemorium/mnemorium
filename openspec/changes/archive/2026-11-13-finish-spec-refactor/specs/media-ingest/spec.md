# Spec Delta

## Purpose

Lets the library populate itself from a mounted media root, so content the operator already holds becomes registered
media without passing through upload: the ingest identifies each file by its content, registers the ones it accepts, and
keeps what it registered reconciled with the directory, at startup and as that directory changes.

## ADDED Requirements

### Requirement: REQ-MEDIA-INGEST-001 — Registered files share one fixed root

Every **Stored file** SHALL be addressed by a path relative to a single **Library root** that is a fixed location, and
SHALL NOT be addressed by an absolute path. A file the server stored and a file it found already present SHALL be
addressed the same way, so one root resolves every registered file.

#### Scenario: SCN-MEDIA-INGEST-001-A — A stored file is addressed under the root

- **WHEN** the server stores a file it received through the upload flow
- **THEN** the recorded location is relative to the **Library root**
- **AND** it resolves to the stored file under that root

#### Scenario: SCN-MEDIA-INGEST-001-B — A file found under the root is addressed the same way

- **WHEN** the scan registers a file already present under the **Library root**
- **THEN** the recorded location is relative to the same root
- **AND** it resolves to that file

### Requirement: REQ-MEDIA-INGEST-002 — An absent library root is decided at startup

The server SHALL NOT create the **Library root**. When it is absent at startup the server SHALL wait up to the
configured window for it to appear and SHALL run the scan if it does. When it is still absent after the window, the
server SHALL refuse startup if the configuration requires the root, and otherwise start and reconcile the library as
empty.

#### Scenario: SCN-MEDIA-INGEST-002-A — The root is present at startup

- **WHEN** the server starts and the **Library root** exists
- **THEN** the scan examines the files under it without waiting

#### Scenario: SCN-MEDIA-INGEST-002-B — The root appears within the window

- **WHEN** the server starts without the **Library root** and it appears before the window elapses
- **THEN** the scan examines the files under it
- **AND** no record is reconciled as removed for its absence

#### Scenario: SCN-MEDIA-INGEST-002-C — The root never appears and is required

- **WHEN** the root is still absent after the window and the configuration requires it
- **THEN** startup is refused with a message naming the absent root
- **AND** no record is reconciled as removed

#### Scenario: SCN-MEDIA-INGEST-002-D — The root never appears and is not required

- **WHEN** the root is still absent after the window and the configuration does not require it
- **THEN** the server starts
- **AND** every registered file is reconciled as removed

### Requirement: REQ-MEDIA-INGEST-003 — Ingest does not delay readiness

The scan SHALL run without blocking the server from accepting requests, and a scan in progress SHALL NOT delay readiness
or delay a response to a request. The bounded startup wait for an absent **Library root** SHALL delay readiness only
while the configuration requires that root.

#### Scenario: SCN-MEDIA-INGEST-003-A — A request arrives during a scan

- **WHEN** a request reaches the server while the scan is examining the **Library root**
- **THEN** the server answers it normally
- **AND** the scan continues

#### Scenario: SCN-MEDIA-INGEST-003-B — The root is not required and absent at startup

- **WHEN** the server starts without the **Library root** and the configuration does not require it
- **THEN** the server accepts requests immediately
- **AND** the reconciliation runs without blocking them

#### Scenario: SCN-MEDIA-INGEST-003-C — The root is required and absent at startup

- **WHEN** the server starts without the **Library root** and the configuration requires it
- **THEN** the server accepts no request until the wait decides
- **AND** it refuses startup when the root never appears

### Requirement: REQ-MEDIA-INGEST-004 — Media type is determined from content

The system SHALL determine a candidate file's media type from its content, and SHALL NOT rely on the file's name or
extension.

#### Scenario: SCN-MEDIA-INGEST-004-A — The extension contradicts the content

- **WHEN** the scan examines a file whose name suggests one type while its content is an accepted media type
- **THEN** the file is classified by its content and registered as that type

#### Scenario: SCN-MEDIA-INGEST-004-B — The content type cannot be determined

- **WHEN** the scan examines a file whose content yields no media type
- **THEN** the file is skipped

### Requirement: REQ-MEDIA-INGEST-005 — Only supported image, video and audio files are registered

The system SHALL register only a file whose determined media type is an image, a video or an audio type and is a type
the supported set contains. Any other file SHALL be skipped, SHALL NOT produce an error the scan reports as a failure,
and SHALL NOT stop the scan.

#### Scenario: SCN-MEDIA-INGEST-005-A — A file that is not media is skipped

- **WHEN** the scan examines a file whose content is not an image, a video or an audio type
- **THEN** nothing is registered for it
- **AND** the scan continues with the remaining files

#### Scenario: SCN-MEDIA-INGEST-005-B — A media type the server does not support is skipped

- **WHEN** the scan examines a file whose content is an image, video or audio type the supported set does not contain
- **THEN** nothing is registered for it
- **AND** the scan continues with the remaining files

#### Scenario: SCN-MEDIA-INGEST-005-C — Unsuitable files do not abort the scan

- **WHEN** the scan meets several unsuitable files among suitable ones
- **THEN** each suitable file is still registered

### Requirement: REQ-MEDIA-INGEST-006 — Registration records the file and its media row together

The system SHALL register an accepted file by recording its location, size, last modification instant, media type and
whether the server stored it, together with the media row its type requires. The file record and the media row SHALL be
recorded together, or neither SHALL be.

#### Scenario: SCN-MEDIA-INGEST-006-A — An image is registered

- **WHEN** the scan accepts a file whose content is an image
- **THEN** its file record and its image row are recorded
- **AND** the image row carries the probed dimensions and orientation

#### Scenario: SCN-MEDIA-INGEST-006-B — A video is registered

- **WHEN** the scan accepts a file whose content is a video
- **THEN** its file record and its video row are recorded
- **AND** the video row carries the probed duration, codec, dimensions and scan type

#### Scenario: SCN-MEDIA-INGEST-006-C — An audio file is registered

- **WHEN** the scan accepts a file whose content is audio
- **THEN** its file record and its audio row are recorded
- **AND** the audio row carries the probed duration, codec and sample rate
- **AND** a bit depth or block size the content does not report is recorded as absent rather than fabricated

#### Scenario: SCN-MEDIA-INGEST-006-D — A probe fails

- **WHEN** an accepted file's media row cannot be built
- **THEN** neither the file record nor the media row is recorded

### Requirement: REQ-MEDIA-INGEST-007 — A registered file carries the size it has on disk

A registered file SHALL record the size its stored content has and the instant it was last modified, so a later scan can
recognise the same content without reading it.

#### Scenario: SCN-MEDIA-INGEST-007-A — The size and modification instant are recorded

- **WHEN** a file is registered
- **THEN** its record carries the content's size and last modification instant

### Requirement: REQ-MEDIA-INGEST-008 — A file the scan did not store belongs to the Root Admin

A file the server did not store itself SHALL be attributed to the **Root Admin** when it is registered, because no
**Caller** owns it.

#### Scenario: SCN-MEDIA-INGEST-008-A — A found file belongs to the Root Admin

- **WHEN** the scan registers a file it found under the **Library root**
- **THEN** the file is attributed to the **Root Admin**
- **AND** no other account owns it

### Requirement: REQ-MEDIA-INGEST-009 — A registered file is verified rather than re-registered

When the scan meets a file it has already registered, it SHALL compare the recorded size and last modification instant
with the file and SHALL treat the file as untouched, changed or removed. It SHALL NOT record a second file for a
location it already recorded.

#### Scenario: SCN-MEDIA-INGEST-009-A — The file is unchanged

- **WHEN** the scan meets a registered file whose size and last modification instant match its record
- **THEN** the file is reported as untouched
- **AND** nothing is written

#### Scenario: SCN-MEDIA-INGEST-009-B — The file changed

- **WHEN** the scan meets a registered file whose size or last modification instant differs from its record
- **THEN** the file is reported as changed
- **AND** its record and content are left as they are

#### Scenario: SCN-MEDIA-INGEST-009-C — The file is gone

- **WHEN** the scan no longer finds a file it registered
- **THEN** the file is reconciled as removed
- **AND** it is not verified as untouched or changed

### Requirement: REQ-MEDIA-INGEST-010 — A media row the scan can complete is completed

The scan SHALL complete a registered file that carries no media row for its media type, by probing the file and
recording the missing row.

#### Scenario: SCN-MEDIA-INGEST-010-A — A file record without its media row

- **WHEN** the scan meets a registered audio file that carries no audio row
- **THEN** it probes the file and records the audio row
- **AND** reports the file as completed rather than untouched

### Requirement: REQ-MEDIA-INGEST-011 — The ingest never changes the content it examines

The ingest SHALL only read the content under the **Library root**. It SHALL NOT create, modify, move or delete that
content.

#### Scenario: SCN-MEDIA-INGEST-011-A — Content is left byte-identical

- **WHEN** the ingest examines the files under the **Library root**
- **THEN** the content of every file is byte-identical afterwards
- **AND** no file has been removed, added or moved by the ingest

#### Scenario: SCN-MEDIA-INGEST-011-B — An unreadable file does not damage the ingest

- **WHEN** the ingest meets a file it cannot read
- **THEN** it skips that file without altering it
- **AND** it continues with the remaining files

### Requirement: REQ-MEDIA-INGEST-012 — Removing a gallery reference never deletes content

Removing a gallery or a gallery item SHALL delete the recorded reference and SHALL NOT delete the content under the
**Library root**.

#### Scenario: SCN-MEDIA-INGEST-012-A — An item is removed from a gallery

- **WHEN** a caller removes an item from a gallery
- **THEN** the item and its media row are removed
- **AND** the content under the **Library root** is unchanged

#### Scenario: SCN-MEDIA-INGEST-012-B — A gallery is removed

- **WHEN** a caller removes a gallery
- **THEN** the gallery and its items are removed
- **AND** the content under the **Library root** is unchanged

### Requirement: REQ-MEDIA-INGEST-013 — A file added while the server runs is registered without a restart

The server SHALL observe the **Library root** while it runs and SHALL register a file added there, without requiring a
restart and without waiting for a later scan.

#### Scenario: SCN-MEDIA-INGEST-013-A — A file appears while the server is running

- **WHEN** an accepted file is added under the **Library root** while the server is running
- **THEN** it is registered as if the scan had found it
- **AND** the server is not restarted

#### Scenario: SCN-MEDIA-INGEST-013-B — The file is still being written

- **WHEN** a file is added and its content is still changing
- **THEN** it is not registered until its content stops changing
- **AND** it is registered once it is complete, with the size it finally has

#### Scenario: SCN-MEDIA-INGEST-013-C — Content is modified in place

- **WHEN** the content of a registered file is modified while the server is running
- **THEN** the file is not registered again
- **AND** its record is left as it is

### Requirement: REQ-MEDIA-INGEST-014 — A renamed file keeps its record

Renaming a file under the **Library root** SHALL update the recorded location of the file it names and SHALL preserve
the media row that location already carried. It SHALL NOT be treated as the removal of one file and the arrival of
another.

#### Scenario: SCN-MEDIA-INGEST-014-A — A registered file is renamed

- **WHEN** a registered file is renamed while the server is running
- **THEN** the recorded location follows the file
- **AND** its media row is preserved
- **AND** no second record is created for the same content

### Requirement: REQ-MEDIA-INGEST-015 — A removed file is reconciled

Removing a file from the **Library root** SHALL be reported and SHALL remove the file's record together with everything
that depends on it, in one unit of work. When that removal fails, the record SHALL be retained, the file SHALL be
reported as missing, and the scan SHALL continue.

#### Scenario: SCN-MEDIA-INGEST-015-A — A registered file is removed

- **WHEN** a registered file is removed from the **Library root** while the server is running
- **THEN** its record is removed together with everything that depends on it

#### Scenario: SCN-MEDIA-INGEST-015-B — The removal fails

- **WHEN** the removal of a registered file's record fails
- **THEN** the record is retained
- **AND** the file is reported as missing
- **AND** the scan continues with the remaining files

#### Scenario: SCN-MEDIA-INGEST-015-C — An identifier is not reused

- **WHEN** a removed file's record is deleted and the file later reappears
- **THEN** it is registered as a new record rather than restoring the removed one

### Requirement: REQ-MEDIA-INGEST-016 — A removal removes what depends on it

Removing an image record SHALL clear every cover reference that names it and SHALL remove any gallery item that uses it
as media. Removing a video record SHALL remove the film, episode, stream and gallery item that require it. Removing an
audio record SHALL remove its recording and that recording's placements. Content under the **Library root** SHALL NOT be
changed.

#### Scenario: SCN-MEDIA-INGEST-016-A — A cover image is removed

- **WHEN** the record of an image that backs cover art is removed
- **THEN** the resource carrying that cover reports no artwork
- **AND** the resource itself is retained

#### Scenario: SCN-MEDIA-INGEST-016-B — A video is removed

- **WHEN** the record of a video is removed
- **THEN** the film or episode that requires it is removed
- **AND** any gallery item that uses it is removed

#### Scenario: SCN-MEDIA-INGEST-016-C — An audio is removed

- **WHEN** the record of an audio is removed
- **THEN** its recording is removed
- **AND** the track placements of that recording are removed
