# Tasks

## 1. Domain scalar values and media rows

- [x] 1.1 Add `NonNegativeF64` and `PositiveI64` to `src/lib/domain/alias.rs`, rejecting NaN, infinities and negatives
      (and zero for the positive type); verify with unit tests covering each rejection.
- [x] 1.2 Change `ProbedAudio`, `ProbedImage` and `ProbedVideo` to carry the new scalar types and validate on
      construction; verify the media-probe unit tests and add cases where an invalid duration, dimension or frame count
      maps to `MediaProbeError::UnsupportedMedia`.
- [x] 1.3 Add infallible `Audio::from_probed`, `Image::from_probed` and `Video::from_probed` and route `ingest_media`
      through them, deleting its now-unreachable `try_new` error arms; verify the ingest unit tests and that
      `cargo clippy` reports no dead code.
- [x] 1.4 Make `Audio.duration_ms` and `Audio.sample_rate_hz` `Option<f64>`; `try_new` accepts `None` and validates
      `Some`; verify with unit tests for absent and negative values.

## 2. Persistence

- [x] 2.1 Edit `20260904012844_create_audio.up.sql` so `duration_ms` and `sample_rate_hz` are nullable with
      `IS NULL OR >= 0` checks; verify by deleting the development database and running `cargo test`.
- [x] 2.2 Edit `20260926093000_create_upload.up.sql` to add `is_verifying BOOLEAN NOT NULL DEFAULT 0` and
      `sealed_at TEXT`, and extend the chunk-insert trigger to reject a sealed upload; verify with a fresh database and
      the migration tests.
- [x] 2.3 Add `is_verifying` and `sealed_at` to the `Upload` aggregate, add `seal`, `unseal` and `delete_chunks` to
      `UploadRepository` and the SQLx adapter, and read the new columns in `search`/`domain_upload`; verify with
      repository unit tests for the guarded seal, the stale-seal unseal and chunk deletion.
- [x] 2.4 Bind `Option<f64>` for audio duration and sample rate in the SQLx audio repository; verify the repository
      tests round-trip absent and present values.
- [x] 2.5 Add `list_staged` to `FileStorage` and the filesystem adapter, returning the staging entries and an empty list
      for an absent directory; verify with adapter unit tests.

## 3. Completion

- [x] 3.1 Add `MediaKind::Audio` and `Probed::Audio`, call `probe_audio` in the completion probe slot, and map
      `Ok(None)` to `404`, `UnsupportedMedia` to `422` and timeout/IO to `500`; verify with completion unit tests for
      each outcome.
- [x] 3.2 Build the audio row with `Audio::from_probed` in the same unit of work as the file row, and switch image and
      video to `from_probed`; verify unit tests for each kind and that no file row is written when the probe fails.
- [x] 3.3 Seal the session in phase 1 with the guarded update, require the seal in phase 2, unseal on failure, and
      answer `409` for a concurrent completion; verify with unit tests for seal, unseal, concurrent completion and a
      probe failure after sealing.
- [x] 3.4 Clear the seal in `finish` and delete the session's chunk rows in the same transaction; report an all-ones
      bitmap for a finished session; verify with unit tests.
- [x] 3.5 Return `503` from begin when the library root is absent and from complete when promote finds the destination
      missing; verify with unit tests for both paths and that no session or file row is created.
- [x] 3.6 Update UC-006 in `docs/development/UseCases.md` to state the required fingerprint, the declared-type rule, the
      chunk, expiry and dedupe behaviour, and the audio row; verify with `lint:md`.

## 4. Chunk and status

- [x] 4.1 Reject a chunk for a sealed session in `write_upload_chunk` and map the trigger conflict to `409`; verify with
      unit tests.
- [x] 4.2 Make `get_upload` report a complete bitmap for a finished session and keep `404` for an unknown or expired
      session; verify with unit tests.
- [x] 4.3 Confirm no collection route exists and document the identifier-based resume in the handler doc comment; verify
      the route table test does not expose `GET /asset/upload`.

## 5. Cancel

- [x] 5.1 Add the `CancelUpload` port and use case: owner-only, `404` for an unknown or unowned session, `409` for a
      finished session, delete the staged file and the row; verify with unit tests.
- [x] 5.2 Add the `DELETE /api/v1/asset/upload/{upload_id}` handler, route and OpenAPI declaration, and update the
      existing `405` test; verify the handler tests and that the generated specification contains the operation.
- [x] 5.3 Add the cancel scenario to UC-006; verify with `lint:md`.

## 6. Reaper

- [x] 6.1 Add the `ReapUploads` use case with the three phases and aggregate counts; verify with unit tests for an
      expired session, a stale seal and orphaned staging content.
- [x] 6.2 Add `UploadReaperRunner` with the interval loop, the first tick after one period, `MissedTickBehavior::Delay`
      and the shutdown watch, and wire it in `src/lib/app.rs` beside the media ingest; verify with unit tests for the
      timer and shutdown and a smoke run of the binary.
- [x] 6.3 Add `asset.upload.reap_interval_seconds` and `asset.upload.reap_batch_size` to `AssetUpload`, the defaults,
      the YAML, the environment adapter and the startup-only registry; verify the configuration tests.
- [x] 6.4 Add the `upload_reap` security event and its catalog entry, carrying counts only; verify with a log-assertion
      test that no path or name is emitted.
- [x] 6.5 Update the startup-only registry and the security-event catalog in `docs/development/TechnicalDesign.md`;
      verify with `lint:md`.

## 7. Chunk size is startup-only

- [x] 7.1 Snapshot `chunk_size_bytes` in `RuntimeAssetUseCaseFactory` and use the snapshot in `begin_upload`; remove the
      `TODO(hot-reload)` comment; verify with a unit test that a runtime change does not alter a new session.
- [x] 7.2 Update the startup-only registry entry to say the setting applies after a restart; verify with `lint:md`.

## 8. Integration and gates

- [x] 8.1 Add a handcrafted WAV fixture and the audio E2E test: complete returns `200`, links the file, and the file
      exposes `_links.audio` immediately; verify with `script/e2e.sh`.
- [x] 8.2 Add E2E coverage for multi-chunk resume, expiry `410`, cancel `204`/`409` and per-caller dedupe; verify with
      `script/e2e.sh`.
- [x] 8.3 Run `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check`,
      `script/rule_enforcer.sh check`,
      `cargo llvm-cov --lib --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80` and `devenv test`;
      verify all pass.

## Workflow follow-up

- Run the review panel and address its findings.
- Archive the change after the project's review requirements are satisfied.
- Verify the archived `library/assets` spec keeps the updated purpose and the new upload requirements.
