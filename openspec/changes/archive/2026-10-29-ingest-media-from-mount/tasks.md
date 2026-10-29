# Tasks

## 1. Persistence

- [x] 1.1 Edit `migrations/20260904012843_create_file.up.sql` in place (`PERS-014`) to add
      `size_bytes INTEGER NOT NULL`, `modified_at TEXT NOT NULL` and
      `origin TEXT NOT NULL DEFAULT 'SCANNED' CHECK (origin IN ('UPLOADED','SCANNED'))`, and to make
      `integrity_hash CHAR(64)` nullable with `chk_file_integrity_hash` accepting `NULL`; verify the down migration
      still drops the table and that `cargo test` applies the schema cleanly against a fresh database.
- [x] 1.2 Edit `migrations/20260904012844_create_audio.up.sql` in place to drop `audio_channel_id` with
      `fk_audio_audio_channel`, and to make `bit_depth` and `block_size` nullable with
      `chk_audio_bit_depth`/`chk_audio_block_size` applying only when the value is present; verify a row with both
      absent inserts and a row with `bit_depth = 0` is rejected.
- [x] 1.3 Delete the `create_audio_channel` and `seed_audio_channel` migration pairs and confirm no migration references
      the table; verify `cargo test` migrates a fresh database without error.
- [x] 1.4 Update the entity-relationship diagram in `docs/development/TechnicalDesign.md` for the `file` and `audio`
      changes and drop `audio_channel` (`PERS-011`), and remove `audio_channel` from `PERS-005`'s seed list; verify no
      `audio_channel` reference remains under `migrations/` or in the diagram.

## 2. Domain

- [x] 2.1 Extend `domain/model/file.rs` with `size_bytes`, `modified_at` and an `origin` value object
      (`Uploaded`/`Scanned`) defaulting to `Scanned`, and make `integrity_hash` optional; verify the model tests cover a
      file with no digest and a file that omits its origin.
- [x] 2.2 Update `domain/model/audio.rs` to drop the channel and accept an absent `bit_depth` and `block_size`, moving
      `AudioError::InvalidBitDepth`/`InvalidBlockSize` to validate only a present value; verify tests cover
      present-and-valid, present-and-invalid and absent.
- [x] 2.3 Delete `domain/model/audio_channel.rs` and its module declaration; verify the crate builds and no reference to
      `AudioChannel` remains.
- [x] 2.4 Add `domain/port/content_type_sniffer.rs` reporting the media type of a file and nothing else, with no
      accept-list and no "unsupported" failure; verify its doc comment states that the caller owns the supported set and
      that the port has a `mockall` automock for the use-case tests.
- [x] 2.5 Add `domain/port/media_source.rs` yielding `{ relative_path, size_bytes, modified_at }` for the files under
      the media root, read-only; verify the port exposes no write or delete method and that a missing root is
      expressible as an outcome rather than an error.
- [x] 2.6 Add `probe_audio` to `domain/port/media_probe.rs` with
      `ProbedAudio { duration_ms, codec, sample_rate_hz, bit_depth, block_size }`, the last two optional; verify the
      `MediaProbe` automock compiles and existing probe tests still pass.

## 3. Storage layout

- [x] 3.1 Replace `STORAGE_ROOT`/`FILES_FOLDER` with `MEDIA_ROOT = "media"` in `application/use_case/upload_layout.rs`,
      drop `files_path()` and `stored_file_path()`, and keep `uploads_path()` composing `data/uploads`; verify layout
      tests assert a media-relative path for a stored file and a `data/uploads` path for a staged one.
- [x] 3.2 Change `application/use_case/ensure_storage_directories.rs` to create only the staging folder and to report an
      absent media root instead of creating it; verify tests assert no `create_directory` call for the media root and
      that a missing root is surfaced to the caller.
- [x] 3.3 Update `docs/development/TechnicalDesign.md` § 10 to record two fixed paths (the datastore and the media root)
      and drop the storage-root sentence, and update `README.md` to stop instructing operators to move files into
      `data/` and to document mounting the media root as writable at `/app/media`; verify the documented mount command
      matches `MEDIA_ROOT` and the container `WORKDIR`.

## 4. Outbound adapters

- [x] 4.1 Add the content-type sniffer adapter over `infer` under `infrastructure/outbound/`, returning the media type
      or nothing; verify adapter tests cover an image, a video, an audio file, a text file and a truncated header.
- [x] 4.2 Add the media-source adapter walking the media root and yielding the relative path, size and modification
      instant, skipping hidden entries, not following symbolic links and staying inside the root; verify adapter tests
      cover a nested tree, a hidden file, a symlink pointing outside the root and an absent root.
- [x] 4.3 Implement `probe_audio` in `infrastructure/outbound/media_probe.rs` over `ffprobe`, deriving `bit_depth` from
      the sample format and `block_size` from the frame size, leaving each absent when the content reports neither;
      verify tests cover a lossy file with no frame size, a lossless file reporting both, and a file the probe cannot
      decode.
- [x] 4.4 Update `infrastructure/outbound/sqlx/model/file.rs` and `audio.rs` for the new and nullable columns, drop
      `audio_channel_id` from the joins in `music_album_repository.rs` and `music_playlist_repository.rs`, and keep
      every other adapter in step with the domain; verify repository integration tests read back a file with no digest
      and an audio row with no bit depth or block size.

## 5. Upload path

- [x] 5.1 Change `application/use_case/complete_upload.rs` to promote into the media root, record `size_bytes` from the
      upload, set `origin` to uploaded and store the digest as before; verify completion tests assert the promoted path
      is media-relative and that the recorded origin is `UPLOADED` with a digest.
- [x] 5.2 Add a `TODO(ingest)` at the media-kind branch recording that an uploaded audio file is stored without its
      audio row while the scan creates one, and that the scan's verification pass repairs it; verify the comment names
      the scan as the repair path.
- [x] 5.3 Verify the upload flow still registers images and videos end to end and that `cargo test` covers the modified
      completion paths.

## 6. Deletion stops removing bytes

- [x] 6.1 Comment out the byte deletion in `application/use_case/delete_gallery_item.rs` and `delete_gallery.rs`,
      keeping the removed-path plumbing and adding a `TODO(ingest)` naming the media mount as the reason; verify tests
      assert no `delete_upload_file` call is made on either path and that the row deletion still commits.
- [x] 6.2 Verify `FileStorage` still exposes no method the two use cases call on removal, so the guarantee cannot
      regress silently.

## 7. Ingest use case

- [x] 7.1 Add `application/port/ingest_media.rs` and `application/use_case/ingest_media.rs` walking the media root,
      sniffing each file and registering the accepted ones as a file row plus the media row its type requires in one
      unit of work; verify tests cover an image, a video and an audio file each producing both rows.
- [x] 7.2 Make registration skip a file whose content type is not image, video or audio, skip a type the seeded
      `mime_type` table does not support, and skip a file whose probe cannot build its media row, without aborting the
      scan; verify tests cover a text file, an unsupported mime type (never probed), an image the image crate cannot
      decode and a file whose probe times out.
- [x] 7.3 Attribute a found file to the Root Admin and set its origin to scanned, and record its size and modification
      instant; verify tests assert the owner, the origin and both measurements.
- [x] 7.4 Implement verification of an already-registered file, reporting untouched, changed or missing from its size
      and modification instant without writing; verify tests cover each of the three verdicts and assert that a changed
      or missing file leaves its row untouched.
- [x] 7.5 Implement backfill: a registered file with no media row for its mime type is probed and completed; verify a
      test registers an audio file row without an audio row and asserts the row is added and reported as completed
      rather than untouched.
- [x] 7.6 Report aggregate counts of registered, completed, untouched, changed, missing and skipped files as one record
      carrying no file name and no path; verify a logging test asserts the counters are present and that no fixture file
      name appears in the output.

## 8. Incremental ingest

- [x] 8.1 Add `notify` with an exact pinned version (operator-owned), clear it against `deny.toml`, record it in
      `docs/development/TechnicalDesign.md` § 6, and confirm `THIRD_PARTY_NOTICES.txt` is regenerated by the commit
      hook; verify `cargo build`, `cargo deny check` and the notices diff are clean.
- [x] 8.2 Add an inbound change-source port and a `notify` adapter that delivers a created, renamed and removed relative
      path to the runtime across a channel without blocking the watcher thread; verify the adapter test observes a
      create, a rename and a remove in a temporary tree.
- [x] 8.3 Coalesce the raw events before dispatch and hold a path until its size and modification instant stop changing;
      verify a test that writes one file in two steps asserts exactly one registration, made after the second write.
- [x] 8.4 Dispatch a created path to the same single-path operation the scan drives, and ignore a content modification
      without re-registering; verify tests assert a created file is registered and a modified registered file leaves its
      row untouched.
- [x] 8.5 Update the recorded location on a rename while preserving the media row, and report a removal as missing while
      retaining the row; verify tests assert the path moved, the media row identifier is unchanged and no second record
      exists.
- [x] 8.6 Report a failed watch registration, a watcher error and a queue overflow without failing startup, leaving the
      scan as the mechanism in every case; verify tests assert one warning per condition and that startup completes.
- [x] 8.7 Drain the observer on shutdown alongside the scan worker; verify a shutdown test asserts both stop.

## 9. Composition root and lifecycle

- [x] 9.1 Wire the ingest use case through `infrastructure/use_case_factory/` with the media source, sniffer, probe and
      unit-of-work factory, and spawn the scan on a background task after `EnsureStorageDirectories` and after the
      router is built; verify the server answers `GET /health` while a scan of a populated fixture tree is still
      running.
- [x] 9.2 Signal the scan worker's cancellation from the graceful-shutdown path before axum is drained, and warn once at
      startup when the media root is absent; verify a shutdown test asserts the worker stops and that an absent root
      produces exactly one warning.
- [x] 9.3 Add the scan's and the observer's lifecycle and outcome events to the security-event catalog in
      `docs/development/TechnicalDesign.md` § 6 at the levels the catalog assigns (`OBS-005`, `OBS-006`), and emit them
      from the owning layer; verify the catalog entry exists and a logging test asserts the declared level and fields.

## 10. Integration verification

- [x] 10.1 Run `cargo clippy --all-targets --all-features -- -D warnings` and `cargo fmt --all -- --check` and resolve
      every finding.
- [x] 10.2 Run `cargo llvm-cov --lib --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80` and close
      any gap the new modules introduce.
- [x] 10.3 Run `cargo run --bin openapi_gen` and confirm `docs/openapi.json` is unchanged, since this change adds no
      route.
- [x] 10.4 Run `devenv test` and confirm every pre-commit gate passes.
- [x] 10.5 Verify by hand against a Docker container with a populated directory mounted at `/app/media`: the scan
      registers the library, the health route answers during the scan, an absent mount logs one warning, removing a
      gallery item leaves the content on the mount untouched, and a file copied into the mount while the server runs is
      registered without a restart.

## Workflow follow-up

- Archive the change once the project's review requirements are satisfied.
