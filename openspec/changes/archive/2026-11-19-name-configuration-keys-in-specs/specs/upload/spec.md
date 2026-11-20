# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-UPLOAD-001 — A session begins from a declared file

The system SHALL create an **Upload session** from a begin request carrying a declared **Content Type**, file name, file
size and whole-file **Integrity hash**, owned by the **Caller**. It SHALL reject an unsafe name, a zero or oversized
file — one larger than `asset.upload.max_file_size_bytes` — a malformed hash and an unsupported type, and SHALL answer
`201 Created` with the identifier, the fixed chunk size, the expiry instant and the session, chunk and completion links.

#### Scenario: SCN-UPLOAD-001-A — A valid request creates a session

- **WHEN** an authenticated caller posts a supported content type, a safe file name, a positive size within
  `asset.upload.max_file_size_bytes` and a valid integrity hash
- **THEN** the system answers `201 Created` with the session identifier, the fixed chunk size, the expiry instant and
  the session, chunk and completion links
- **AND** the `Location` header names the session

#### Scenario: SCN-UPLOAD-001-B — The file name is unsafe

- **WHEN** the declared file name is empty, starts with a dot, or contains a path separator or `..`
- **THEN** the system answers `422 Unprocessable Content` and creates neither a session nor its staged content

#### Scenario: SCN-UPLOAD-001-C — The content type is not supported

- **WHEN** the declared content type is not in the supported set
- **THEN** the system answers `415 Unsupported Media Type` and creates no session

#### Scenario: SCN-UPLOAD-001-D — The file size is zero

- **WHEN** the declared file size is zero
- **THEN** the system answers `422 Unprocessable Content` and creates no session

#### Scenario: SCN-UPLOAD-001-E — The file size exceeds the maximum

- **WHEN** the declared file size exceeds `asset.upload.max_file_size_bytes`
- **THEN** the system answers `413 Content Too Large` and creates no session

#### Scenario: SCN-UPLOAD-001-F — The integrity hash is malformed

- **WHEN** the declared integrity hash is not a 64-character lowercase hexadecimal digest
- **THEN** the system answers `400 Bad Request` and creates no session

### Requirement: REQ-UPLOAD-012 — A session expires at a fixed instant

An **Upload session** SHALL expire at an instant derived from its creation and the `asset.upload.expiry_seconds`
lifetime and SHALL NOT be renewed by activity. An expired session SHALL answer `410 Gone` to a chunk or completion and
`404 Not Found` to a status read, and its **Staged content** SHALL be removed.

#### Scenario: SCN-UPLOAD-012-A — A chunk is sent after expiry

- **WHEN** the owner sends a chunk after the expiry instant
- **THEN** the system answers `410 Gone` and the session's staged content is removed

#### Scenario: SCN-UPLOAD-012-B — Completion is attempted after expiry

- **WHEN** the owner completes after the expiry instant
- **THEN** the system answers `410 Gone` and registers nothing

#### Scenario: SCN-UPLOAD-012-C — The status is read after expiry

- **WHEN** the owner reads the session after the expiry instant
- **THEN** the system answers `404 Not Found`

#### Scenario: SCN-UPLOAD-012-D — Activity does not extend the session

- **WHEN** the owner writes chunks up to the expiry instant
- **THEN** the session still expires at the instant fixed when it was created

### Requirement: REQ-UPLOAD-015 — Expired, stale and orphaned sessions are reaped

The system SHALL periodically — every `asset.upload.reap_interval_seconds` — remove expired pending sessions and their
**Staged content**, unseal a verification that has outlived its bound, and remove **Staged content** no session owns,
without delaying readiness. Each phase SHALL handle at most `asset.upload.reap_batch_size` items, so one run never holds
the reaper past the next tick. It SHALL report aggregate counts only, never a file name or a path.

#### Scenario: SCN-UPLOAD-015-A — An expired session is reaped

- **WHEN** the reaper runs and a pending session is past its expiry
- **THEN** its staged content and its session record are removed

#### Scenario: SCN-UPLOAD-015-B — A stale verification is unsealed

- **WHEN** the reaper runs and a session has been sealed past the verification bound
- **THEN** the session is unsealed and may be completed again

#### Scenario: SCN-UPLOAD-015-C — Orphaned staging content is removed

- **WHEN** the reaper runs and staging content exists that no session owns
- **THEN** the content is removed

#### Scenario: SCN-UPLOAD-015-D — Reaping does not delay a request

- **WHEN** a request arrives while the reaper is running
- **THEN** the server answers it normally

#### Scenario: SCN-UPLOAD-015-E — Reaping reports counts only

- **WHEN** a reaping run finishes
- **THEN** one record carries the aggregate counts and no file name or path
