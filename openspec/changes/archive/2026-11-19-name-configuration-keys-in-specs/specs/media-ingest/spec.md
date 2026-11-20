# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-MEDIA-INGEST-002 — An absent library root is decided at startup

The server SHALL NOT create the **Library root**. When it is absent at startup the server SHALL wait up to the
`library.startup.media_root_wait_seconds` window for it to appear and SHALL run the scan if it does. When it is still
absent after the window, the server SHALL refuse startup when `library.startup.fail_without_media_root` is true, and
otherwise start and reconcile the library as empty.

#### Scenario: SCN-MEDIA-INGEST-002-A — The root is present at startup

- **WHEN** the server starts and the **Library root** exists
- **THEN** the scan examines the files under it without waiting

#### Scenario: SCN-MEDIA-INGEST-002-B — The root appears within the window

- **WHEN** the server starts without the **Library root** and it appears before the
  `library.startup.media_root_wait_seconds` window elapses
- **THEN** the scan examines the files under it
- **AND** no record is reconciled as removed for its absence

#### Scenario: SCN-MEDIA-INGEST-002-C — The root never appears and is required

- **WHEN** the root is still absent after the window and `library.startup.fail_without_media_root` is true
- **THEN** startup is refused with a message naming the absent root
- **AND** no record is reconciled as removed

#### Scenario: SCN-MEDIA-INGEST-002-D — The root never appears and is not required

- **WHEN** the root is still absent after the window and `library.startup.fail_without_media_root` is false
- **THEN** the server starts
- **AND** every registered file is reconciled as removed

### Requirement: REQ-MEDIA-INGEST-003 — Ingest does not delay readiness

The scan SHALL run without blocking the server from accepting requests, and a scan in progress SHALL NOT delay readiness
or delay a response to a request. The bounded startup wait for an absent **Library root** SHALL delay readiness only
while `library.startup.fail_without_media_root` is true.

#### Scenario: SCN-MEDIA-INGEST-003-A — A request arrives during a scan

- **WHEN** a request reaches the server while the scan is examining the **Library root**
- **THEN** the server answers it normally
- **AND** the scan continues

#### Scenario: SCN-MEDIA-INGEST-003-B — The root is not required and absent at startup

- **WHEN** the server starts without the **Library root** and `library.startup.fail_without_media_root` is false
- **THEN** the server accepts requests immediately
- **AND** the reconciliation runs without blocking them

#### Scenario: SCN-MEDIA-INGEST-003-C — The root is required and absent at startup

- **WHEN** the server starts without the **Library root** and `library.startup.fail_without_media_root` is true
- **THEN** the server accepts no request until the wait decides
- **AND** it refuses startup when the root never appears
