# library/assets Specification

## Purpose

Accepts a media file through a resumable chunked upload, then serves the stored files and their typed image view over
HTTP, so a client can populate the library, resolve an identifier it holds, follow a cover or track link, and fetch the
backing bytes under one authorization rule.

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

### Requirement: An upload session is initialized

The system SHALL create an upload session from `POST /api/v1/asset/upload` with a declared media type, file name, file
size and whole-file integrity hash. It SHALL reject a zero or oversized file, an unsafe file name, a malformed hash and
a media type outside the supported set. It SHALL create the session and its staging file together or neither.

#### Scenario: A valid request creates a session

- **WHEN** an authenticated caller posts a supported media type, a safe file name, a positive size within the maximum
  and a valid integrity hash
- **THEN** the system answers `201 Created` with the session identifier, the fixed chunk size, the expiry instant and
  links to the session, its chunk endpoint and its completion
- **AND** the `Location` header names the session

#### Scenario: The file name is unsafe

- **WHEN** the declared file name is empty, starts with a dot, or contains a path separator or `..`
- **THEN** the system rejects the request and creates neither a session nor a staging file

#### Scenario: The media type is not supported

- **WHEN** the declared media type is not in the supported set
- **THEN** the system answers `415 Unsupported Media Type` and creates no session

#### Scenario: The file size is invalid

- **WHEN** the declared size is zero or exceeds the configured maximum
- **THEN** the system rejects the request and creates no session

#### Scenario: The integrity hash is malformed

- **WHEN** the declared hash is not a 64-character lowercase hexadecimal digest
- **THEN** the system rejects the request and creates no session

### Requirement: An upload session belongs to one caller

The system SHALL allow any authenticated caller to begin an upload. An existing session SHALL be readable, writable,
completable and cancellable only by its owner, including the Root Admin, and a caller who is not the owner SHALL receive
`404 Not Found`, indistinguishable from an unknown session.

#### Scenario: Any authenticated caller may begin

- **WHEN** an authenticated caller with no upload permission override begins an upload
- **THEN** the session is created and owned by that caller

#### Scenario: Another caller addresses the session

- **WHEN** a caller who does not own the session reads, writes, completes or cancels it
- **THEN** the system answers `404 Not Found`

#### Scenario: The Root Admin addresses another caller's session

- **WHEN** the Root Admin addresses a session it does not own
- **THEN** the system answers `404 Not Found`

### Requirement: A chunk is stored at its declared offset

The system SHALL store one chunk at `PUT /api/v1/asset/upload/{upload_id}/chunk/{chunk_number}` after verifying the
chunk number, the declared byte range, the body length and the content digest. It SHALL write the bytes at the chunk's
offset, record the chunk as received, and return the updated received-chunk bitmap. Re-sending a recorded chunk SHALL
succeed without changing the stored content.

#### Scenario: A valid chunk is stored

- **WHEN** an authenticated owner sends a chunk whose number, range, length and digest are valid
- **THEN** the system stores the bytes at the chunk's offset, records the chunk, and answers `200 OK` with the updated
  bitmap, the total chunk count, the expiry instant and the finished flag

#### Scenario: A chunk is re-sent

- **WHEN** the owner sends a chunk that was already recorded, with the same digest
- **THEN** the system answers `200 OK` and the stored content is unchanged

#### Scenario: The digest does not match

- **WHEN** the recomputed digest of the chunk body differs from the declared content digest
- **THEN** the system answers `422 Unprocessable Content` and stores no bytes

#### Scenario: The range does not match the chunk

- **WHEN** the declared range does not start at the chunk's offset, does not match the body length, or the chunk number
  is outside the session's chunk count
- **THEN** the system answers `422 Unprocessable Content` and stores no bytes

#### Scenario: A required header is missing

- **WHEN** the request omits `Content-Range` or `Content-Digest`, or the digest uses an unsupported algorithm
- **THEN** the system rejects the request and stores no bytes

### Requirement: A chunk is rejected when the session is not writable

The system SHALL reject a chunk for a session that is expired, finished, or being verified. An expired session SHALL
answer `410 Gone`; a finished or sealed session SHALL answer `409 Conflict`.

#### Scenario: The session is expired

- **WHEN** the owner sends a chunk after the session's expiry instant
- **THEN** the system answers `410 Gone` and stores no bytes

#### Scenario: The session is finished

- **WHEN** the owner sends a chunk for a finished session
- **THEN** the system answers `409 Conflict`

#### Scenario: The session is being verified

- **WHEN** the owner sends a chunk while a completion of the same session is verifying its content
- **THEN** the system answers `409 Conflict` and stores no bytes

### Requirement: The session state is readable

The system SHALL return the session identified by `GET /api/v1/asset/upload/{upload_id}` with the received-chunk bitmap,
the total chunk count, the expiry instant, the finished flag, the caller's file identifier when one exists, and links.
An unknown or expired session SHALL answer `404 Not Found`. The system SHALL NOT provide a collection route: a client
resumes by retaining the session identifier and reading its bitmap.

#### Scenario: A known session is read

- **WHEN** the owner reads a session it owns
- **THEN** the system answers `200 OK` with the bitmap, total chunk count, expiry instant, finished flag and links

#### Scenario: An unknown or expired session is read

- **WHEN** the addressed session does not exist or has expired
- **THEN** the system answers `404 Not Found`

#### Scenario: A finished session is read

- **WHEN** the owner reads a finished session
- **THEN** the bitmap reports every chunk as received

#### Scenario: No collection route exists

- **WHEN** a caller requests the upload collection rather than one session
- **THEN** the system does not list the caller's sessions

### Requirement: Completion verifies the whole-file digest

The system SHALL recompute the whole-file digest of the staged content on
`POST /api/v1/asset/upload/{upload_id}/complete` and SHALL reject a mismatch with `422 Unprocessable Content`. It SHALL
reject a session that has not received every chunk with `422 Unprocessable Content`.

#### Scenario: The digest matches

- **WHEN** the owner completes a session whose staged content matches the declared integrity hash
- **THEN** the completion proceeds

#### Scenario: The digest does not match

- **WHEN** the recomputed digest differs from the declared hash
- **THEN** the system answers `422 Unprocessable Content` and registers nothing

#### Scenario: The session is incomplete

- **WHEN** the owner completes a session that has not received every chunk
- **THEN** the system answers `422 Unprocessable Content` and registers nothing

### Requirement: Completion registers the typed media

Completion SHALL promote the staged content under the library root and SHALL record the file and the media row its
declared type requires in one unit of work, together or neither: an image row, a video row or an audio row. An audio row
SHALL record duration, codec and sample rate, and SHALL record bit depth and block size only when the content reports
them. Content that does not belong to the declared family SHALL be rejected and nothing SHALL be stored.

#### Scenario: An image is completed

- **WHEN** the owner completes a valid image upload
- **THEN** the file row and the image row with its dimensions and orientation are recorded together
- **AND** the response links the file

#### Scenario: A video is completed

- **WHEN** the owner completes a valid video upload
- **THEN** the file row and the video row with its duration, codec, dimensions and scan type are recorded together
- **AND** the response links the file

#### Scenario: An audio file is completed

- **WHEN** the owner completes a valid audio upload
- **THEN** the file row and the audio row with its duration, codec and sample rate are recorded together
- **AND** the response links the file

#### Scenario: Audio values the content does not report

- **WHEN** the content reports no bit depth or no block size
- **THEN** the audio row records the missing value as absent rather than a fabricated number

#### Scenario: The content does not belong to the declared family

- **WHEN** the content has no stream of the declared family
- **THEN** the system rejects the completion and records neither the file row nor a media row

### Requirement: The declared media type is authoritative

The system SHALL determine the media family from the declared content type and SHALL use the probe to confirm that the
content belongs to that family. It SHALL NOT reclassify the content and SHALL NOT replace the declared media type with
one derived from the content.

#### Scenario: A declared audio container carries a video track

- **WHEN** the content has an audio stream and the caller declared an audio type
- **THEN** the completion is accepted and the declared audio type is stored

#### Scenario: The content has no stream of the declared family

- **WHEN** the caller declared an audio type and the content has no audio stream
- **THEN** the completion is rejected with `422 Unprocessable Content`

#### Scenario: The declared type is an image type

- **WHEN** the caller declared an image type and the content decodes as an image
- **THEN** the declared image type is stored unchanged

### Requirement: Completion is sealed against concurrent writes

Before recomputing the digest, completion SHALL seal the session so that no chunk can be written. A completion that
fails after sealing SHALL unseal the session so it can be retried, and a concurrent completion of the same session SHALL
be rejected with `409 Conflict`.

#### Scenario: A chunk arrives during verification

- **WHEN** a chunk is sent for a session whose completion is verifying its content
- **THEN** the chunk is rejected with `409 Conflict`

#### Scenario: Completion fails after sealing

- **WHEN** the digest, probe or persistence step fails after the session was sealed
- **THEN** the session is unsealed and the owner may retry the completion

#### Scenario: Two completions race

- **WHEN** two completions of the same session arrive at the same time
- **THEN** one proceeds and the other answers `409 Conflict`

### Requirement: An upload session expires at a fixed instant

An upload session SHALL expire at an instant derived from its creation and the configured lifetime, and SHALL NOT be
renewed by activity. An expired session SHALL answer `410 Gone` to a chunk or completion and `404 Not Found` to a status
read, and its staged content SHALL be removed.

#### Scenario: A chunk is sent after expiry

- **WHEN** the owner sends a chunk after the expiry instant
- **THEN** the system answers `410 Gone` and the session's staged content is removed

#### Scenario: Completion is attempted after expiry

- **WHEN** the owner completes after the expiry instant
- **THEN** the system answers `410 Gone` and registers nothing

#### Scenario: The status is read after expiry

- **WHEN** the owner reads the session after the expiry instant
- **THEN** the system answers `404 Not Found`

#### Scenario: Activity does not extend the session

- **WHEN** the owner writes chunks up to the expiry instant
- **THEN** the session still expires at the instant fixed when it was created

### Requirement: An upload session can be cancelled

The system SHALL accept `DELETE /api/v1/asset/upload/{upload_id}` from the session owner and SHALL delete the session
and its staged content, answering `204 No Content`. It SHALL answer `404 Not Found` for an unknown or unowned session
and `409 Conflict` for a finished session, and SHALL NOT delete a completed file.

#### Scenario: A pending session is cancelled

- **WHEN** the owner cancels a pending session
- **THEN** the session and its staged content are removed and the system answers `204 No Content`

#### Scenario: A finished session is cancelled

- **WHEN** the owner cancels a finished session
- **THEN** the system answers `409 Conflict` and the completed file is untouched

#### Scenario: An unknown session is cancelled

- **WHEN** a caller cancels a session that does not exist or that it does not own
- **THEN** the system answers `404 Not Found`

### Requirement: Expired, stale and orphaned sessions are reaped

The system SHALL periodically remove expired pending sessions and their staged content, unseal a verification that has
outlived its bound, and remove staged content that no session owns, without delaying readiness. It SHALL report
aggregate counts only, never a file name or a path.

#### Scenario: An expired session is reaped

- **WHEN** the reaper runs and a pending session is past its expiry
- **THEN** its staged content and its session record are removed

#### Scenario: A stale verification is unsealed

- **WHEN** the reaper runs and a session has been sealed past the verification bound
- **THEN** the session is unsealed and may be completed again

#### Scenario: Orphaned staging content is removed

- **WHEN** the reaper runs and staging content exists that no session owns
- **THEN** the content is removed

#### Scenario: Reaping does not delay a request

- **WHEN** a request arrives while the reaper is running
- **THEN** the server answers it normally

#### Scenario: Reaping reports counts only

- **WHEN** a reaping run finishes
- **THEN** one record carries the aggregate counts and no file name or path

### Requirement: A finished session keeps its identity

Completing a finished session SHALL return the caller's file with `200 OK`. A finished session SHALL report a complete
bitmap, and its identifier SHALL NOT be reused for another session.

#### Scenario: Completion is retried

- **WHEN** the owner completes a session that is already finished
- **THEN** the system answers `200 OK` with the caller's file

#### Scenario: A finished session is read

- **WHEN** the owner reads a finished session
- **THEN** the bitmap reports every chunk as received

#### Scenario: An identifier is not reused

- **WHEN** a session is finished and later sessions are created
- **THEN** no later session carries the finished session's identifier

### Requirement: Duplicate content is reused per caller

When the caller already owns a file whose whole-file digest equals the session's digest, completion SHALL return that
file and SHALL store nothing new, leaving the session unfinished.

#### Scenario: The caller already owns the content

- **WHEN** the owner completes a session whose digest matches a file the owner already has
- **THEN** the system answers `200 OK` with the existing file and stores no new file or media row
- **AND** the session is left unfinished

#### Scenario: Another caller owns the content

- **WHEN** a different caller owns a file with the same digest
- **THEN** the completion stores the caller's own copy

### Requirement: The chunk size is fixed for the session

The chunk size returned when a session is initialized SHALL govern every chunk of that session for its whole life, and a
runtime configuration change SHALL NOT alter an existing session. A new value SHALL apply to sessions started after a
restart.

#### Scenario: The configuration changes mid-session

- **WHEN** the configured chunk size changes while a session is in progress
- **THEN** the session keeps the chunk size it was created with and its chunks are accepted at that size

#### Scenario: A new value applies after a restart

- **WHEN** the configured chunk size changes and the server restarts
- **THEN** sessions created after the restart use the new chunk size

### Requirement: An absent library root is unavailable

The system SHALL NOT create the library root. When the root is absent, beginning a session and completing a session
SHALL answer `503 Service Unavailable`, the server SHALL keep serving every other route, and no session SHALL be
started.

#### Scenario: A session is begun while the root is absent

- **WHEN** an authenticated caller begins an upload and the library root does not exist
- **THEN** the system answers `503 Service Unavailable` and creates no session

#### Scenario: Completion is attempted while the root is absent

- **WHEN** the owner completes a session and the library root does not exist
- **THEN** the system answers `503 Service Unavailable` and registers nothing

#### Scenario: The rest of the server keeps serving

- **WHEN** the library root is absent
- **THEN** the server answers every non-upload route normally

### Requirement: An upload names its media row after the uploaded file

The media row created by an upload SHALL carry the uploaded file's original name. A file the ingest registers itself
SHALL carry the name derived from its path.

#### Scenario: An uploaded file names its media row

- **WHEN** an upload completes
- **THEN** the image, video or audio row carries the original uploaded file name

#### Scenario: A found file names its media row

- **WHEN** the ingest registers a file it found under the library root
- **THEN** the media row carries the name derived from the file's path
