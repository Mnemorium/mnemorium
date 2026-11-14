# upload Specification

## Purpose

Accepts caller-supplied content into the library through a resumable, chunked **Upload session**, so a client can send a
file larger than one request, resume after an interruption, and receive one registered asset. It owns the session
lifecycle, the chunk protocol, integrity and type verification, expiry, cancellation, reaping and promotion; serving a
stored asset and deciding who may address a session belong to their own capabilities.

## Requirements

### Requirement: REQ-UPLOAD-001 — A session begins from a declared file

The system SHALL create an **Upload session** from a begin request carrying a declared **Content Type**, file name, file
size and whole-file **Integrity hash**, owned by the **Caller**. It SHALL reject an unsafe name, a zero or oversized
file, a malformed hash and an unsupported type, and SHALL answer `201 Created` with the identifier, the fixed chunk
size, the expiry instant and the session, chunk and completion links.

#### Scenario: SCN-UPLOAD-001-A — A valid request creates a session

- **WHEN** an authenticated caller posts a supported content type, a safe file name, a positive size within the maximum
  and a valid integrity hash
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

- **WHEN** the declared file size exceeds the configured maximum
- **THEN** the system answers `413 Content Too Large` and creates no session

#### Scenario: SCN-UPLOAD-001-F — The integrity hash is malformed

- **WHEN** the declared integrity hash is not a 64-character lowercase hexadecimal digest
- **THEN** the system answers `400 Bad Request` and creates no session

### Requirement: REQ-UPLOAD-002 — A session and its staging file begin together

The system SHALL create the session and its **Staged content** in one unit of work, so a failed begin leaves neither. A
**Staged content** file that already exists for a newly created session SHALL be reclaimed as a leftover from an
interrupted begin rather than fail the retry.

#### Scenario: SCN-UPLOAD-002-A — A failed begin leaves nothing

- **WHEN** a begin fails after its session was to be created
- **THEN** neither the session nor its staged content exists

#### Scenario: SCN-UPLOAD-002-B — A leftover staging file is reclaimed

- **WHEN** staged content already exists for the identifier of a newly created session
- **THEN** the system reclaims that content and the begin succeeds

### Requirement: REQ-UPLOAD-003 — The chunk size is fixed for the session

The chunk size returned when a session begins SHALL be the value of the `asset.upload.chunk_size_bytes` setting and
SHALL govern every **Chunk** of that session for its whole life. A configuration change SHALL NOT alter an existing
session, and a new value SHALL apply to sessions created after a restart.

#### Scenario: SCN-UPLOAD-003-A — The configuration changes mid-session

- **WHEN** the configured chunk size changes while a session is in progress
- **THEN** the session keeps the chunk size it was created with and its chunks are accepted at that size

#### Scenario: SCN-UPLOAD-003-B — A new value applies after a restart

- **WHEN** the configured chunk size changes and the server restarts
- **THEN** sessions created after the restart use the new chunk size

### Requirement: REQ-UPLOAD-004 — A chunk is written at its declared offset

The system SHALL store one **Chunk** received for a session after verifying the chunk number, the declared byte range,
the body length and the content digest. It SHALL write the bytes at the chunk's offset, record the chunk and return the
updated received-chunk bitmap; re-sending a recorded chunk SHALL change no stored content.

#### Scenario: SCN-UPLOAD-004-A — A valid chunk is stored

- **WHEN** the owner sends a chunk whose number, range, length and digest are valid
- **THEN** the system stores the bytes at the chunk's offset, records the chunk, and answers `200 OK` with the updated
  bitmap, the total chunk count, the expiry instant and the finished flag

#### Scenario: SCN-UPLOAD-004-B — A chunk is re-sent

- **WHEN** the owner re-sends a chunk that was already recorded, with the same digest
- **THEN** the system answers `200 OK` and the stored content is unchanged

#### Scenario: SCN-UPLOAD-004-C — The digest does not match

- **WHEN** the recomputed digest of the chunk body differs from the declared content digest
- **THEN** the system answers `422 Unprocessable Content` and stores no bytes

#### Scenario: SCN-UPLOAD-004-D — The range does not match the chunk

- **WHEN** the declared range does not start at the chunk's offset, does not match the body length, or the chunk number
  is outside the session's chunk count
- **THEN** the system answers `422 Unprocessable Content` and stores no bytes

#### Scenario: SCN-UPLOAD-004-E — A header is malformed

- **WHEN** `Content-Range` or `Content-Digest` is present but malformed
- **THEN** the system answers `400 Bad Request` and stores no bytes

#### Scenario: SCN-UPLOAD-004-F — A required header is missing or its digest algorithm is unsupported

- **WHEN** the request omits `Content-Range` or `Content-Digest`, or the digest names an algorithm other than SHA-256
- **THEN** the system answers `422 Unprocessable Content` and stores no bytes

### Requirement: REQ-UPLOAD-005 — A chunk is refused when the session is not writable

The system SHALL refuse a **Chunk** for a session that is expired, finished or sealed against writes: an expired session
SHALL answer `410 Gone`, and a finished or sealed session SHALL answer `409 Conflict` and store no bytes.

#### Scenario: SCN-UPLOAD-005-A — The session is expired

- **WHEN** the owner sends a chunk after the session's expiry instant
- **THEN** the system answers `410 Gone` and stores no bytes

#### Scenario: SCN-UPLOAD-005-B — The session is finished

- **WHEN** the owner sends a chunk for a finished session
- **THEN** the system answers `409 Conflict` and stores no bytes

#### Scenario: SCN-UPLOAD-005-C — The session is being verified

- **WHEN** the owner sends a chunk while a completion of the same session is verifying its content
- **THEN** the system answers `409 Conflict` and stores no bytes

### Requirement: REQ-UPLOAD-006 — The session state is readable

The system SHALL return the addressed session with its received-chunk bitmap, total chunk count, expiry instant,
finished flag, the caller's file identifier when one exists, and links. An unknown or expired session SHALL answer
`404 Not Found`, and the system SHALL NOT expose a collection of sessions.

#### Scenario: SCN-UPLOAD-006-A — A known session is read

- **WHEN** the owner reads a session it owns
- **THEN** the system answers `200 OK` with the bitmap, total chunk count, expiry instant, finished flag and links

#### Scenario: SCN-UPLOAD-006-B — An unknown or expired session is read

- **WHEN** the addressed session does not exist or has expired
- **THEN** the system answers `404 Not Found`

#### Scenario: SCN-UPLOAD-006-C — A finished session is read

- **WHEN** the owner reads a finished session
- **THEN** the bitmap reports every chunk as received

#### Scenario: SCN-UPLOAD-006-D — No collection exists

- **WHEN** a caller asks for the upload collection rather than one session
- **THEN** the system does not list the caller's sessions

### Requirement: REQ-UPLOAD-007 — Completion verifies the whole-file hash

On a completion request the system SHALL recompute the whole-file **Integrity hash** of the **Staged content** and SHALL
reject a mismatch or a session missing a chunk with `422 Unprocessable Content`, registering nothing.

#### Scenario: SCN-UPLOAD-007-A — The hash matches

- **WHEN** the owner completes a session whose staged content matches the declared integrity hash
- **THEN** the completion proceeds

#### Scenario: SCN-UPLOAD-007-B — The hash does not match

- **WHEN** the recomputed hash differs from the declared hash
- **THEN** the system answers `422 Unprocessable Content` and registers nothing

#### Scenario: SCN-UPLOAD-007-C — The session is incomplete

- **WHEN** the owner completes a session that has not received every chunk
- **THEN** the system answers `422 Unprocessable Content` and registers nothing

### Requirement: REQ-UPLOAD-008 — Completion is sealed against concurrent writes

Before recomputing the hash, completion SHALL seal the session so no **Chunk** can be written. A completion that fails
after sealing SHALL unseal the session so it can be retried, and a concurrent completion of the same session SHALL be
rejected with `409 Conflict`.

#### Scenario: SCN-UPLOAD-008-A — A chunk arrives during verification

- **WHEN** a chunk is sent for a session whose completion is verifying its content
- **THEN** the chunk is rejected with `409 Conflict`

#### Scenario: SCN-UPLOAD-008-B — Completion fails after sealing

- **WHEN** the hash, probe or persistence step fails after the session was sealed
- **THEN** the session is unsealed and the owner may retry the completion

#### Scenario: SCN-UPLOAD-008-C — Two completions race

- **WHEN** two completions of the same session arrive at the same time
- **THEN** one proceeds and the other answers `409 Conflict`

### Requirement: REQ-UPLOAD-009 — Completion promotes and registers the content together

Completion SHALL promote the **Staged content** under the **Library root** at a path relative to that root and SHALL
record the file and the media row its declared type requires in one unit of work, together or neither: an image, a video
or an audio row. Content that does not belong to the declared family SHALL be rejected and nothing SHALL be stored.

#### Scenario: SCN-UPLOAD-009-A — An image is completed

- **WHEN** the owner completes a valid image upload
- **THEN** the file row and the image row with its dimensions and orientation are recorded together
- **AND** the response links the file

#### Scenario: SCN-UPLOAD-009-B — A video is completed

- **WHEN** the owner completes a valid video upload
- **THEN** the file row and the video row with its duration, codec, dimensions and scan type are recorded together
- **AND** the response links the file

#### Scenario: SCN-UPLOAD-009-C — An audio file is completed

- **WHEN** the owner completes a valid audio upload
- **THEN** the file row and the audio row with its duration, codec and sample rate are recorded together
- **AND** the response links the file

#### Scenario: SCN-UPLOAD-009-D — Audio values the content does not report

- **WHEN** the content reports no bit depth or no block size
- **THEN** the audio row records the missing value as absent rather than a fabricated number

#### Scenario: SCN-UPLOAD-009-E — The content does not belong to the declared family

- **WHEN** the content has no stream of the declared family
- **THEN** the system answers `422 Unprocessable Content` and records neither the file row nor a media row

#### Scenario: SCN-UPLOAD-009-F — The content is not a supported media file

- **WHEN** the staged content cannot be probed as an image, a video or an audio file
- **THEN** the system answers `422 Unprocessable Content` and records neither the file row nor a media row

### Requirement: REQ-UPLOAD-010 — The declared content type is authoritative

The system SHALL determine the media family from the declared **Content Type** and SHALL use the probe only to confirm
the content belongs to that family. It SHALL NOT reclassify the content and SHALL NOT replace the declared type with one
derived from the content.

#### Scenario: SCN-UPLOAD-010-A — A declared audio container carries a video track

- **WHEN** the content has an audio stream and the caller declared an audio type
- **THEN** the completion is accepted and the declared audio type is stored

#### Scenario: SCN-UPLOAD-010-B — The content has no stream of the declared family

- **WHEN** the caller declared an audio type and the content has no audio stream
- **THEN** the completion is rejected with `422 Unprocessable Content`

#### Scenario: SCN-UPLOAD-010-C — The declared type is an image type

- **WHEN** the caller declared an image type and the content decodes as an image
- **THEN** the declared image type is stored unchanged

### Requirement: REQ-UPLOAD-011 — Completion reuses the caller's identical content

When the **Caller** already owns a file whose whole-file **Integrity hash** equals the session's, completion SHALL
answer `200 OK` with that file, store nothing new and leave the session unfinished.

#### Scenario: SCN-UPLOAD-011-A — The caller already owns the content

- **WHEN** the owner completes a session whose hash matches a file the owner already has
- **THEN** the system answers `200 OK` with the existing file and stores no new file or media row
- **AND** the session is left unfinished

#### Scenario: SCN-UPLOAD-011-B — Another caller owns the content

- **WHEN** a different caller owns a file with the same hash
- **THEN** the completion stores the caller's own copy

### Requirement: REQ-UPLOAD-012 — A session expires at a fixed instant

An **Upload session** SHALL expire at an instant derived from its creation and the configured lifetime and SHALL NOT be
renewed by activity. An expired session SHALL answer `410 Gone` to a chunk or completion and `404 Not Found` to a status
read, and its **Staged content** SHALL be removed.

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

### Requirement: REQ-UPLOAD-013 — A session can be cancelled

The system SHALL accept a cancellation request from the session's **Owner**, delete the session and its **Staged
content** and answer `204 No Content`. It SHALL answer `404 Not Found` for an unknown session and `409 Conflict` for a
finished or sealed one, and SHALL NOT delete a completed file.

#### Scenario: SCN-UPLOAD-013-A — A pending session is cancelled

- **WHEN** the owner cancels a pending session
- **THEN** the session and its staged content are removed and the system answers `204 No Content`

#### Scenario: SCN-UPLOAD-013-B — A finished or sealed session is cancelled

- **WHEN** the owner cancels a finished or sealed session
- **THEN** the system answers `409 Conflict` and the completed file is untouched

#### Scenario: SCN-UPLOAD-013-C — An unknown session is cancelled

- **WHEN** the owner cancels a session that does not exist
- **THEN** the system answers `404 Not Found`

### Requirement: REQ-UPLOAD-014 — A finished session keeps its identity

Completing a finished session SHALL return the caller's file with `200 OK`. A finished session SHALL report a complete
bitmap, and its identifier SHALL NOT be reused for another session.

#### Scenario: SCN-UPLOAD-014-A — Completion is retried

- **WHEN** the owner completes a session that is already finished
- **THEN** the system answers `200 OK` with the caller's file

#### Scenario: SCN-UPLOAD-014-B — An identifier is not reused

- **WHEN** a session is finished and later sessions are created
- **THEN** no later session carries the finished session's identifier

### Requirement: REQ-UPLOAD-015 — Expired, stale and orphaned sessions are reaped

The system SHALL periodically remove expired pending sessions and their **Staged content**, unseal a verification that
has outlived its bound, and remove **Staged content** no session owns, without delaying readiness. It SHALL report
aggregate counts only, never a file name or a path.

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

### Requirement: REQ-UPLOAD-016 — An absent library root refuses uploads

The system SHALL NOT create the **Library root**. While it is absent, beginning and completing a session SHALL answer
`503 Service Unavailable` and no session SHALL begin, while every other route SHALL keep serving.

#### Scenario: SCN-UPLOAD-016-A — A session is begun while the root is absent

- **WHEN** an authenticated caller begins an upload and the library root does not exist
- **THEN** the system answers `503 Service Unavailable` and creates no session

#### Scenario: SCN-UPLOAD-016-B — Completion is attempted while the root is absent

- **WHEN** the owner completes a session and the library root does not exist
- **THEN** the system answers `503 Service Unavailable` and registers nothing

#### Scenario: SCN-UPLOAD-016-C — The rest of the server keeps serving

- **WHEN** the library root is absent
- **THEN** the server answers every non-upload route normally

### Requirement: REQ-UPLOAD-017 — A completed file belongs to its uploader

Completion SHALL record the promoted file as owned by the **Caller**, `Private`, origin `UPLOADED`, with the content's
size and last modification instant, at a path unique under the **Library root** that resolves to the stored bytes.

#### Scenario: SCN-UPLOAD-017-A — The file is attributed to its uploader

- **WHEN** an upload completes
- **THEN** the file's owner is the caller, its visibility is `Private`, and its origin is `UPLOADED`

#### Scenario: SCN-UPLOAD-017-B — The content's size and instant are recorded

- **WHEN** an upload completes
- **THEN** the file record carries the promoted content's size and last modification instant

#### Scenario: SCN-UPLOAD-017-C — The path is unique under the root

- **WHEN** an upload completes
- **THEN** its path is relative to the library root, no earlier file record uses it, and it resolves to the stored bytes
