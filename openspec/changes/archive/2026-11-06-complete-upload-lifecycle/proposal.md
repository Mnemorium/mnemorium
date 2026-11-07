# Proposal

## Why

The chunked upload API already works for the happy path, but its lifecycle is unfinished and unspecified. Audio
completion stores a file with no audio row and leaves the typed media to a later ingest pass; there is no cancel and no
proactive reaper, so expired sessions and their staging files are only cleaned when someone touches them again; and the
completion path has two correctness holes: the declared media type is never checked against the content, and a chunk
written between the hash and the promote can make the stored file disagree with its persisted integrity hash. The
`library/assets` capability documents only reads, so none of the upload behaviour is contractual today.

## What Changes

- Specify the upload lifecycle in `library/assets`: begin, chunk, status, complete, cancel, expiry, reaping, resume and
  the rules that bind them.
- Complete an audio upload synchronously: probe the staged file, and write the audio row in the same unit of work as the
  file row. A probe failure aborts the completion, exactly as it does for image and video. **BREAKING**: a file declared
  as audio whose content has no audio stream is now rejected at completion (`422`) instead of being stored untyped.
- Make media row construction infallible for all three kinds: validate in the probe through non-negative/positive scalar
  types, then convert without an error branch. This removes the three unreachable `try_new` branches in completion and
  fixes the image/video invalid-metadata rejection that answers `500` today.
- Seal an upload before hashing it, so a concurrent chunk write cannot change the bytes after the hash. A sealed session
  rejects chunk writes; a failed completion unseals; a stale seal is unsealed by the reaper.
- Add `DELETE /api/v1/asset/upload/{upload_id}` to cancel a pending session: owner-only, deletes the staged file and the
  session, leaves any completed file untouched.
- Add a background reaper: delete expired pending sessions and their staged files, unseal stale verifications, and
  remove orphaned staging files. Keep the finished session row (idempotent completion) and delete its chunk rows at
  finish, so upload identifiers are never reused and a promoted path cannot collide.
- Answer `503 Service Unavailable` when the library root is absent, at begin (fail fast) and at completion (the root
  vanished mid-upload), instead of the current `500`.
- Make `asset.upload.chunk_size_bytes` startup-only for both the router body limit and the use cases, and remove the
  hot-reload TODO.
- Record unknown audio duration and sample rate as absent rather than `0`, with nullable columns.
- Keep the uploaded file's original name for the media row; document the path-derived name the ingest uses for files it
  finds itself.
- No collection endpoint: resumption requires the client to retain the session identifier; `GET` on a known session
  returns the received-chunk bitmap.

Out of scope: per-user quotas and upload rate limiting; the pre-existing `API-041`/`OBS-006` issues in `post_upload`,
`put_upload_chunk` and `complete_upload`; audio and video asset read endpoints; and content-type sniffing (the declared
type remains authoritative, and the probe proves the media family only).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library/assets`: adds the upload lifecycle requirements and the rules for completion, cancellation, expiry, reaping,
  resume, media-root absence and declared-type authority.

## Impact

- `src/lib/application/use_case/{begin_upload,write_upload_chunk,get_upload,complete_upload}.rs` and new
  `cancel_upload.rs` and `reap_uploads.rs`; `upload_session.rs` projections.
- `src/lib/domain/{alias,model/{upload,audio,image,video}}.rs`; `src/lib/domain/port/upload_repository.rs` and
  `file_storage.rs`.
- `src/lib/infrastructure/inbound/rest/handler/asset/` (cancel handler, route, OpenAPI) and a new `upload_reaper.rs`
  inbound runner wired in `src/lib/app.rs`.
- `src/lib/infrastructure/outbound/sqlx/upload_repository.rs`, `audio_repository.rs`, and `file_system/file_storage.rs`.
- Migrations: `upload` gains `is_verifying` and `sealed_at`; the chunk trigger rejects a sealed upload; `audio` duration
  and sample rate become nullable (edited in place, `PERS-014`).
- Configuration: `asset.upload.reap_interval_seconds` and `asset.upload.reap_batch_size`; `chunk_size_bytes` becomes
  startup-only.
- Documentation: the `library/assets` purpose, `docs/development/UseCases.md` UC-006, and the startup-only registry and
  security-event catalog in `docs/development/TechnicalDesign.md`.
- Tests: unit tests for the three completion kinds, seal/unseal, cancel and reap; E2E for audio completion, multi-chunk
  resume, expiry, cancel and dedupe.
