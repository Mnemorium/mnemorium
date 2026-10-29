# Proposal

## Why

A file can only enter Mnemorium through the chunked upload API, and its bytes land in `data/files/`, a store the server
owns. An operator who already holds a library on a NAS or an external disk has no way in but to re-upload every file
through a client, and the server cannot report on media it did not personally write.

Nothing has been released yet, so the storage layout and the `file` table can still be corrected in place (`PERS-014`)
rather than carried as a compatibility path. Doing it now is what makes the mount the library root instead of a second
root bolted beside a server-owned one.

## What Changes

- **BREAKING** The library root becomes the fixed `media/` directory, a path constant beside the existing storage-root
  constant; `data/files/` is removed. A file is addressed relative to the media root wherever it came from.
- **BREAKING** An uploaded file is promoted into the media root, so uploads and pre-existing files share one namespace
  and one path semantics.
- `data/` keeps only the upload staging area, which stays server-owned and recreated when absent. The media root is
  never created: a missing root means the volume did not mount, so startup warns and the server runs without it.
- Add a startup ingest scan that walks the media root on a background task, so a large library never delays readiness.
- Add change observation: the server watches the media root and registers a file added, renamed or removed while it
  runs, without a restart. The mechanism is filesystem change events, which assumes a local filesystem; a share mounted
  over a network filesystem is out of scope. The startup scan stays authoritative and repairs whatever the events
  missed, so no deployment depends on a watcher for correctness.
- Add a content-type sniffing port that reports a file's media type and nothing else; each calling use case decides
  which types it accepts and what an unsupported file means.
- Register an image, video or audio file the scan accepts by writing its `file` row and its media row in one unit of
  work, exactly as upload completion already does. Metadata enrichment — matching a file to a film, an episode, an album
  or a recording — stays out of scope.
- Verify files already registered when the scan meets them again, and report each one as untouched, changed, missing, or
  needing a media row: a `file` row whose mime type has no media row is probed and completed, which also repairs an
  uploaded audio file.
- Record on `file` the size, the last modification instant and an origin discriminator, and make the integrity hash
  optional: an upload still carries the digest that verifies its content and deduplicates it, while a scanned file
  carries none.
- Remove `audio_channel` — the reference table, its seed, its value object and the `audio.audio_channel_id` column — and
  relax the audio bit-depth and block-size checks to a sane positive bound, allowing each to be absent.
- **BREAKING** Deleting a gallery or a gallery item stops deleting bytes. The bytes now belong to the operator's mount,
  and a scanned file was never the server's to remove.
- Leave the upload path's media-row registration of audio unchanged; the scan's verification pass closes that gap, and
  the site carries a TODO until it does.

## Capabilities

### New Capabilities

- `library/media-ingest`: the library populating itself from the media root — sniffing a file's media type, registering
  an accepted file as a media row, verifying a file registered earlier, reporting the scan's aggregate outcome, and
  treating an absent media root as a warning rather than a failure.

### Modified Capabilities

None. The upload flow, gallery deletion and the audio schema have no specification today, so the storage-root move and
the byte-deletion removal change behavior that no existing requirement states. Every listed capability —
`library/movies`, `library/series`, `library/music-albums`, `library/music-playlists`, `library/cover-art`,
`identity/authorization`, `configuration/loading`, `instance/state`, `user/account-creation` — keeps its requirements
unchanged: no route is added and no representation changes.

## Impact

- **Application**: `use_case/upload_layout.rs` (the media-root constant replaces the file-folder path),
  `use_case/ensure_storage_directories.rs` (media root warned about, not created), `use_case/complete_upload.rs`
  (promote into the media root, record size and origin), `use_case/delete_gallery.rs` and
  `use_case/delete_gallery_item.rs` (byte deletion commented out with a TODO), a new ingest use case and its port,
  `use_case_factory/` for the wiring.
- **Domain**: new content-type sniffer port; `MediaProbe` gains `probe_audio` and `ProbedAudio`; `File` gains size,
  modification instant and origin and loses its mandatory integrity hash; `Audio` loses its channel and tolerates an
  absent bit depth and block size; `AudioChannel` is deleted.
- **Infrastructure**: new sniffer adapter over the already-declared `infer` crate; `LocalMediaProbe` gains an `ffprobe`
  audio path; a new change-source adapter over `notify`, bridged into the runtime and drained on shutdown; the
  media-probe and file-storage adapters are otherwise unchanged; the composition root spawns the scan worker and the
  change observer; the two music repositories drop `audio_channel_id` from their joins.
- **Dependencies**: `notify` becomes the first new runtime dependency of this change, pinned exactly, cleared against
  `deny.toml`, recorded in `TechnicalDesign.md` § 6, and reported in `THIRD_PARTY_NOTICES.txt`, which the commit hook
  regenerates. Event coalescing is the ingest consumer's stability gate, not a separate debouncing crate.
- **Persistence**: `create_file` edited in place for the new columns and the optional hash; `create_audio` edited in
  place for the removed column and the relaxed checks; the `audio_channel` create and seed pairs deleted; `PERS-005` and
  the entity-relationship diagram updated in the same change (`PERS-011`).
- **Documentation**: `docs/development/TechnicalDesign.md` § 10 records two fixed paths instead of three and drops the
  storage-root sentence; `README.md` stops telling operators to move files into `data/` and documents the mount.
- **API**: unchanged. No route is added or altered, so `docs/openapi.json` does not change.
