# Spec Delta

## MODIFIED Requirements

### Requirement: Ingest does not delay readiness

The scan SHALL run without blocking the server from accepting requests. A scan in progress SHALL NOT delay readiness,
and SHALL NOT delay a response to a request. The bounded startup wait for an absent library root SHALL delay readiness
only while the configuration requires the root, and SHALL never delay it otherwise.

#### Scenario: A request arrives during a scan

- **WHEN** a request reaches the server while the scan is examining the library root
- **THEN** the server answers it normally
- **AND** the scan continues

#### Scenario: The root is not required and absent at startup

- **WHEN** the server starts without the library root and the configuration does not require it
- **THEN** the server accepts requests immediately
- **AND** the reconciliation runs without blocking them

#### Scenario: The root is required and absent at startup

- **WHEN** the server starts without the library root and the configuration requires it
- **THEN** the server accepts no request until the wait decides
- **AND** it refuses startup when the root never appears

### Requirement: A registered file is verified rather than re-registered

When the scan meets a file it has already registered, it SHALL compare the recorded size and last modification instant
with the file and SHALL treat the file as untouched, changed or removed. It SHALL NOT register a second record for a
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
- **THEN** the file is reconciled as removed
- **AND** it is not verified as untouched or changed

### Requirement: Ingest reports aggregate counts only

Ingest SHALL report its outcome as aggregate counts of registered, completed, untouched, changed, removed, missing and
skipped files. It SHALL NOT record an individual file's name or path.

#### Scenario: A completed ingest is reported

- **WHEN** a scan finishes or an observed change is handled
- **THEN** one record carries the counts of registered, completed, untouched, changed, removed, missing and skipped
  files
- **AND** it carries no file name and no file path

#### Scenario: A removal is reported

- **WHEN** the scan removes a registered file whose content is absent
- **THEN** its count is reported as removed
- **AND** no file name or path is recorded

## ADDED Requirements

### Requirement: An absent library root is decided at startup

The server SHALL NOT create the library root. When the root is absent at startup the server SHALL wait up to the
configured window for it to appear and SHALL run the scan if it appears. When it is still absent after the window, the
server SHALL either refuse startup when the configuration requires the root, or start and reconcile the library as
empty.

#### Scenario: The root is present at startup

- **WHEN** the server starts and the library root exists
- **THEN** the scan examines the files under it without waiting

#### Scenario: The root appears within the window

- **WHEN** the server starts without the library root and it appears before the window elapses
- **THEN** the scan examines the files under it
- **AND** no record is reconciled as removed for its absence

#### Scenario: The root never appears and is required

- **WHEN** the root is still absent after the window and the configuration requires it
- **THEN** startup is refused with a message naming the absent root
- **AND** no record is reconciled as removed

#### Scenario: The root never appears and is not required

- **WHEN** the root is still absent after the window and the configuration does not require it
- **THEN** the server starts
- **AND** every registered file is reconciled as removed

### Requirement: A removed file is reconciled

Removing a file from the library root SHALL be reported and SHALL remove the file's record together with everything that
depends on it, in one unit of work. When the removal fails, the record SHALL be retained, the file SHALL be reported as
missing, and the scan SHALL continue with the remaining files.

#### Scenario: A registered file is removed

- **WHEN** a registered file is removed from the library root while the server is running
- **THEN** its record is removed together with everything that depends on it

#### Scenario: The removal fails

- **WHEN** the removal of a registered file's record fails
- **THEN** the record is retained
- **AND** the file is reported as missing
- **AND** the scan continues with the remaining files

#### Scenario: An identifier is not reused

- **WHEN** a removed file's record is deleted and the file later reappears
- **THEN** it is registered as a new record rather than restoring the removed one

### Requirement: A removal removes what depends on it

Removing an image record SHALL clear every cover reference that names it and SHALL remove any gallery item that uses it
as media. Removing a video record SHALL remove the movie, episode, stream and gallery item that require it. Removing an
audio record SHALL remove its recording and the placements of that recording in a medium or a playlist. Content under
the library root SHALL NOT be changed.

#### Scenario: A cover image is removed

- **WHEN** the record of an image that backs cover art is removed
- **THEN** the resource carrying that cover reports no artwork
- **AND** the resource itself is retained

#### Scenario: A video is removed

- **WHEN** the record of a video is removed
- **THEN** the movie or episode that requires it is removed
- **AND** any gallery item that uses it is removed

#### Scenario: An audio is removed

- **WHEN** the record of an audio is removed
- **THEN** its recording is removed
- **AND** the track placements of that recording are removed

## REMOVED Requirements

### Requirement: An absent library root is a warning

**Reason**: A warning alone leaves every registered record stale while the root is absent, so availability cannot be
reported truthfully. Startup now decides the outcome within a bounded window and may refuse to start.

**Migration**: Leave the root requirement disabled in the configuration to keep the previous "warn and continue"
behaviour; the server then starts and reconciles the library as empty instead of retaining its records.

### Requirement: A removed file is reported without losing its record

**Reason**: A retained record made a missing file indistinguishable from available content, and forced the content route
to answer a single status for two different situations.

**Migration**: Restore the root and run the scan to rebuild file and media records from the files. Identification, cover
art, galleries and playlists are restored by the operator, either by hand or by repeating the identification calls.
