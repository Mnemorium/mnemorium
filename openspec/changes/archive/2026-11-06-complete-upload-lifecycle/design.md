# Design

## Context

The Asset context already has four upload use cases (`begin_upload`, `write_upload_chunk`, `get_upload`,
`complete_upload`) over one `upload`/`upload_chunk` schema, a staging root (`data/uploads`) and a fixed library root
(`media`). Completion is a two-phase unit of work: phase 1 resolves and validates the session, the staged file is hashed
and probed with no transaction open, and phase 2 re-resolves, promotes and persists. Image and video are probed and get
a media row; audio is neither probed nor registered, and the ingest watcher backfills the missing row later
(`complete_upload.rs:331-334`). The library root is the operator's mount and is never created
(`ensure_storage_directories.rs:60-64`); a missing destination is currently reported as a storage failure and reaches
the client as `500`. The upload API has no spec; `library/assets` covers reads only. See `proposal.md` for motivation
and `specs/library/assets/spec.md` for the required behaviour.

## Goals / Non-Goals

**Goals:**

- Make audio completion as strict and as truthful as image and video completion.
- Remove the unreachable `try_new` error branches by proving the media values before the row is built.
- Close the hash/promote race without a second full read of the file.
- Give the session a lifecycle end: cancel, expiry, reaping, and a retained identity.
- Make the library-root failure a deliberate `503` instead of an accidental `500`.
- Keep the single-writer rule: no hashing, probing or filesystem work inside a write transaction.

**Non-Goals:**

- Per-user quotas, concurrent-session limits and upload rate limiting.
- Content-type sniffing: the declared type stays authoritative (option A).
- Audio and video asset read endpoints (`#346`).
- The pre-existing `#290`, `#299`, `#301` handler-classification fixes.
- A collection endpoint for resuming: the client keeps the session identifier.

## Decisions

### 1. Audio is probed at completion, strictly

Reuse `MediaProbe::probe_audio` (`media_probe.rs:194`) in the same slot as the image and video probes: after phase 1
rolls back and after the whole-file hash, with no unit of work open. `MediaKind` gains `Audio`; `Probed` gains
`Audio(ProbedAudio)`.

```
probe_audio(staged)
  Ok(Some)          -> Probed::Audio
  Ok(None)          -> 404 NoSuchUpload
  UnsupportedMedia  -> 422 UnsupportedMedia
  Timeout / IO      -> 500
```

Any probe error aborts before phase 2, so no file row is written and the staged file stays for a retry or the reaper.
Alternatives: keep the watcher as the authority (the `200` would claim a media row that does not exist yet), probe in a
background job (adds an async state machine), probe at begin (the bytes are not there yet).

### 2. Media rows are built infallibly from validated probe values

Add two scalar newtypes to `domain/alias.rs`:

```
NonNegativeF64   rejects NaN, infinities and negatives
PositiveI64      rejects zero and negatives
```

`ProbedAudio`, `ProbedImage` and `ProbedVideo` carry these types (audio duration/sample rate as
`Option<NonNegativeF64>`), so a `Probed*` value is valid by construction. The probe maps a constructor failure to
`MediaProbeError::UnsupportedMedia`, which is the correct classification for content the server cannot represent.
`Audio::from_probed`, `Image::from_probed` and `Video::from_probed` then convert without an error branch. `try_new`
stays for repository rehydration, where the database constraints are the proof.

This removes the three unreachable branches in `complete_upload` and, in the same move, fixes `#334`: an invalid video
metadata value is now rejected by the probe as unsupported content (`422`, a validation event) instead of reaching the
row constructor and becoming an `application_error` (`500`). Alternatives: an unchecked `from_probed` (the domain would
trust the adapter), or keeping the defensive branch (dead code the user asked to delete).

### 3. Completion seals the session before hashing

Add `is_verifying BOOLEAN NOT NULL DEFAULT 0` and `sealed_at TEXT` to `upload`.

```
phase 1   BEGIN WRITE
          resolve (owner, expiry, finished, can_finish)
          UPDATE upload SET is_verifying = 1, sealed_at = now
             WHERE upload_id = ? AND is_finished = 0 AND is_verifying = 0
             -> 0 rows: re-read
                  is_finished = 1 -> 200 idempotent
                  is_verifying = 1 -> 409 completion in progress
          COMMIT                                   (the seal is durable)
hash + probe                                        (no transaction)
phase 2   BEGIN WRITE
          resolve, requiring is_verifying = 1
          digest compare, dedupe, promote, file row + media row
          UPDATE upload SET is_finished = 1, is_verifying = 0, sealed_at = NULL
             WHERE is_verifying = 1
          COMMIT
```

`record_chunk` and the chunk-insert trigger reject a sealed session; `write_upload_chunk` maps that to `409`. A failure
after the seal calls `unseal` in its own transaction (best-effort); if that also fails, the reaper unseals after
`seal_timeout`, default 5 minutes (2x the 30 s probe plus margin). Only the request that won the guarded update reaches
phase 2, so no seal token is needed: a second completion re-reads and answers `409`.

Alternatives: re-hash the promoted file and compare (doubles the read, up to 100 GiB), hold the write lock across
hash+probe (violates `TechnicalDesign.md:767-769`), compare size/mtime around the hash (a heuristic, not a proof).

### 4. A reaper owns the lifecycle end

A new inbound runner mirrors `MediaIngestRunner` (`media_ingest.rs:213-261`):

```
app.rs
  setup_routes
  MediaIngestRunner::start()
  UploadReaperRunner::start()          -> tokio::spawn, watch shutdown
  axum::serve(...).with_graceful_shutdown(...)
        signal: media_ingest.shutdown(); upload_reaper.shutdown()
  media_ingest.wait().await
  upload_reaper.wait().await
```

The loop uses `tokio::time::interval` with the first tick after one full period and `MissedTickBehavior::Delay`, so it
never competes with startup or bursts after a pause. Each tick runs `ReapUploads`:

```
1  expired pending   is_finished = 0, is_verifying = 0, created_at + expiry < now
                     -> delete staged file, delete the row
2  stale sealed      is_verifying = 1, sealed_at + seal_timeout < now
                     -> unseal
3  orphan staging    data/uploads/{n} with no upload row
                     -> delete the file
```

Each phase is one bounded write unit of work; file deletion is best-effort and idempotent. Configuration:
`asset.upload.reap_interval_seconds` (default `900`, validated `1..=86400`, startup-only, added to the startup-only
registry) and `asset.upload.reap_batch_size` (default `100`, read live). A new `upload_reap` security event (`info`)
reports aggregate counts only, never a path or a name.

**Finished sessions are retained.** Deleting a finished row would let SQLite reuse its rowid, and the promoted path is
`media/{upload_id}_{file_name}`; a reused id with the same name could collide with a live file. So the tiny `upload` row
stays (idempotent completion keeps working) and the growth is removed at the other end: `finish` deletes the session's
`upload_chunk` rows in the same transaction, and a finished session reports an all-ones bitmap, which is true by
definition. Alternatives: delete finished rows after a retention window (reintroduces id reuse; would need
`AUTOINCREMENT`, which no table uses, or a different path key), or keep the chunk rows (20,480 rows for a 100 GiB file
at 5 MiB chunks).

The orphan sweep only touches the staging root, never `media/`: a leftover promoted file is the operator's content and
the ingest registers it as a found file, which is harmless.

### 5. Cancellation deletes the session, never the file

`DELETE /api/v1/asset/upload/{upload_id}` runs `CancelUpload`: owner-only through `upload.write`, `404` for an unknown
or unowned session (concealed), `409` for a finished session, `204` on success. It deletes the staged file and the
session row (chunks cascade); a completed file is untouched. A second cancel answers `404`, matching the unknown-session
rule. Alternative: `204` for an already-cancelled session (idempotent but indistinguishable from an unknown id).

### 6. An absent library root is `503`

`begin` checks `directory_exist(media_root())` and returns `BeginUploadError::LibraryUnavailable` -> `503` before
creating anything, so a client does not upload 100 GiB to discover the mount is missing. `complete` maps a promote whose
destination directory is missing to `StorageError::Unavailable` (the adapter already tells a missing source from a
missing destination in `classify_missing_promotion`) and then to `CompleteUploadError::LibraryUnavailable` -> `503`. The
server keeps serving every other route and never creates the root, matching the media-ingest rule. Alternatives: `409`
(used for a finished session), `500` (current; hides an operator error).

### 7. The chunk size is fixed at startup

`RuntimeAssetUseCaseFactory` snapshots `chunk_size_bytes` when it is built and `begin_upload` uses that snapshot; the
router body limit already uses the startup value (`asset.rs:229-231`). The `TODO(hot-reload)` is removed and the
startup-only registry entry says the setting applies after a restart. `expiry_seconds` and `max_file_size_bytes` stay
live because they do not constrain the request body. This closes a real inconsistency: today a runtime increase of
`chunk_size_bytes` makes new sessions accept chunks the router rejects with `413`.

### 8. Unknown audio values are absent, not zero

Edit `20260904012844_create_audio.up.sql` in place (`PERS-014`): `duration_ms` and `sample_rate_hz` become nullable with
`CHECK (col IS NULL OR col >= 0)`. `Audio` carries `Option<f64>`; `ProbedAudio` carries `Option<NonNegativeF64>`;
`probe_audio` returns `None` where it currently uses `unwrap_or(0.0)`. The repository binds `Option<f64>`. Editing an
applied migration changes its checksum, so a development database must be deleted and rebuilt (`TechnicalDesign.md`
records the reset procedure); the schema is unreleased.

### 9. The declared type stays authoritative

The media family comes from the declared `mime_type_id` prefix; the probe confirms the content belongs to that family
and nothing reclassifies it. A video container that carries an audio track, declared `audio/*`, is accepted and stored
under the declared type. This is deliberate (option A): content sniffing would change image and video behaviour too and
needs its own requirement. The supported set remains the seeded `mime_type` table.

### 10. Resumption stays identifier-based

No collection route. The client retains the session identifier and reads `GET /api/v1/asset/upload/{upload_id}` for the
bitmap. A listing endpoint would have to decide whether finished sessions appear, and the client already has to persist
the identifier to resume a chunked upload.

### 11. The uploaded name wins

The media row created by an upload uses `upload.file_name()`; the ingest names a row after the file's path
(`ingest_media.rs:270-273`). After this change, uploads always create their own row, so the path-derived name only
applies to files the ingest finds itself. No new column is needed; the difference is documented in the spec.

### 12. Expiry is absolute

`expires_at = created_at + expiry_seconds`, computed the same way by the use cases and the reaper. Activity does not
extend a session. Idle-based expiry would need a persisted activity or expiry column and a write on every chunk; it is
deferred.

### 13. Beginning needs authentication, not a permission

Any authenticated caller may begin; `upload.read` and `upload.write` remain fixed to `OWNED` for an existing session,
including for the Root Admin. No new action enters the catalog. Abuse control is a quota/rate-limit concern and is out
of scope.

### 14. Deduplication is unchanged

Completion still looks up the caller's file by digest and, on a hit, returns it and leaves the session unfinished. The
probe runs before that lookup, so a duplicate still pays one probe; moving the lookup into phase 1 would be an
optimization, not a correctness change.

## Risks / Trade-offs

- **A crash between seal and unseal leaves a session sealed** -> the reaper unseals after `seal_timeout`; a client that
  retries immediately receives `409` and retries later.
- **`from_probed` moves validation into the probe** -> a probe that under-validates could let an invalid value reach the
  database, where the constraint answers `DataIntegrityViolation` (`500`). The newtypes, the probe tests and the
  existing constraints are three layers against that.
- **The nullable audio migration is an in-place edit** -> an existing development database fails at startup with
  `VersionMismatch`; the reset procedure is documented and the database is disposable.
- **Audio completion now costs one `ffprobe`** -> bounded by the shared permit pool (5 s admission) and the 30 s wall
  clock, and it holds no database connection. A broken `ffprobe` makes audio completion fail rather than store an
  untyped file; that is the strict choice.
- **Retaining finished rows grows one row per upload** -> the row is tiny and the chunk rows are removed at finish; the
  trade-off buys a monotonic identifier and a collision-free path.
- **The seal adds a `409` for concurrent completion** -> a client that races itself must retry after the winner
  finishes; the alternative is two writers on one staged file.
- **A reaper tick deletes rows in bounded batches** -> a very large backlog takes several ticks; each tick holds the
  write lock briefly instead of once for a long time.

## Migration Plan

1. Edit the two migrations in place (`PERS-014`): `upload.is_verifying`/`sealed_at` and the chunk trigger; nullable
   `audio.duration_ms`/`sample_rate_hz`. Delete the development database and its `-wal`/`-shm` sidecars, then restart.
2. Add `asset.upload.reap_interval_seconds` and `asset.upload.reap_batch_size` to the defaults, the YAML, the
   environment adapter and the startup-only registry.
3. Land the domain scalar types and `from_probed` first, then the completion change, then the seal, then the reaper and
   cancel, then the tests. Each step keeps the previous one working.
4. Rollback: revert the change and rebuild the development database. The schema is unreleased, so there is no forward
   migration to preserve.
