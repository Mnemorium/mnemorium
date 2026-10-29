# Design

## Context

See `proposal.md` — Why. The current state that shapes the approach:

- `use_case/upload_layout.rs` owns every path: `STORAGE_ROOT = "data"` with `data/uploads` for staging and `data/files`
  for finished content. It declares itself the module that composes the layout so the storage adapter stays a pure I/O
  boundary, and `TechnicalDesign.md` § 10 fixes those paths as constants rather than settings.
- `ensure_storage_directories.rs` creates both folders at startup. That is correct while the server owns them.
- `complete_upload.rs` resolves the media kind from the **client-declared** mime type (`:175-181`), hashes the whole
  staged file, probes it, then writes `file` + `image`/`video` in one unit of work before promoting
  `data/files/{upload_id}_{file_name}` (`:312`).
- `FileStorage` (`domain/port/file_storage.rs`) has no traversal, no `stat` and no read: it is a write path for
  server-owned content.
- `MediaProbe` exposes `probe_image` and `probe_video` only. `LocalMediaProbe` drives `ffprobe` for video and the
  `image` crate for images, and owns a permit pool bounding concurrent probes.
- `audio_channel` is a seeded reference table; its `nb_channel` values are **not unique** (`3` covers both `2.1` and
  `3.0`), so a channel count cannot select a row.
- `audio.bit_depth` and `audio.block_size` are `NOT NULL` with power-of-two checks. `ffprobe` reports no block size for
  MP3/AAC and no meaningful bit depth for lossy codecs, so the constraints reject ordinary audio.
- `delete_gallery.rs:246` and `delete_gallery_item.rs:253` delete the bytes of every removed file after the transaction
  commits.
- The composition root drains axum on `SIGINT`/`SIGTERM` and owns no background task.
- `infer = "0.22"` is declared in `Cargo.toml` and used nowhere.

## Goals / Non-Goals

**Goals:**

- One root for all media, one path semantics, and an ingest path that reuses the completion pipeline rather than
  duplicating it.
- A scan whose cost scales with the library, never blocks readiness, and never writes to the content it examines.
- A schema that admits a file the server did not create without weakening what an upload guarantees.

**Non-Goals:**

- Enrichment: matching a file to a film, season, episode, album or recording, and deriving any metadata from a name.
- A download or streaming route. Registered files remain unreachable through the API in this change.
- Deleting bytes, in either direction: the scan never deletes, and gallery removal stops deleting.
- Re-scanning while the server runs, a scan API, a file watcher, or a scheduled re-scan.
- Configuring the media root. It is a constant.

## Decisions

### 1. One root: a `media/` constant replaces `data/files/`

`upload_layout.rs` gains `MEDIA_ROOT = "media"` and loses `FILES_FOLDER`, `files_path()` and `stored_file_path()`.
`uploads_path()` keeps composing `data/uploads`.

Relative rather than absolute, matching `STORAGE_ROOT`: a checkout runs with `./media`, and the container mounts
`/app/media`. An absolute `"/media"` was rejected because `cargo run` would then scan the host's `/media`, which does
not exist on macOS and is not empty on some Linux hosts.

`stored_file_path()` disappears because path composition collapses: a `file.path` is always relative to `MEDIA_ROOT`,
whoever wrote it.

_Alternative:_ keep `data/files` for uploads and treat `/media` as a second, read-only root. Rejected — it forces a
discriminator into path resolution and makes an uploaded file and a found file two different kinds of address, for no
benefit once the schema is allowed to change.

### 2. The media root is warned about, never created

`EnsureStorageDirectories` stops creating the media root and keeps creating `data/uploads`. An absent media root at
startup emits a `warn` and the server continues.

Creating it was correct while the server owned the folder. Now absence almost always means the volume did not mount, and
creating it silently redirects every write into the container's ephemeral layer, where it is lost at the next start.
Warn-and-continue is chosen over fail-fast because a server without a mount can still serve identity, playlists and
whatever it registered before; uploads fail at write time with a storage error, which the storage adapter already logs.

### 3. Content-type sniffing is its own port and owns no policy

A new `domain/port/content_type_sniffer.rs` (adapter over `infer`) answers one question: what media type is this file?

```
  ContentTypeSniffer::sniff(path) -> Result<Option<MediaType>, ...>
```

It does not decide whether a type is interesting, and it does not raise "unsupported" as a failure — it reports what it
found and the calling use case decides. This keeps the upload path able to adopt it later without inheriting ingest's
accept-list, and it keeps the accept-list visible at the one place that acts on it.

The ingest's accept-list is the seeded `mime_type` table, the same reference data the `file.mime_type_id` foreign key
uses. A sniffed type the table does not know is skipped as unsupported; it is not registered and it does not abort the
scan. The table stays the single source of truth for which media types the server can represent.

`infer` is already a declared dependency and unused, so no dependency is added. The adapter is pure Rust over a bounded
header read, which keeps it inside the `OBS-008` boundary — no external parser receives the bytes.

_Alternative:_ a `is_ingestable` boolean on the port. Rejected — it hard-codes one caller's policy into a shared port.

### 4. Registration reuses the completion shape, one unit of work per file

Ingest follows `complete_upload`'s pipeline minus the session and the digest:

```
  walk media/  (read-only)
      |
      v
  sniff content  ── not image/video/audio, or not supported ──> skip, count
      |
      v
  path already registered ?
      |               |
     no              yes ──> verify (size, mtime) ; backfill media row if absent
      |
      v
  probe   image: image crate   video: ffprobe   audio: ffprobe (new)
      |
      v
  ONE unit of work: file row  + image|video|audio row
```

One unit of work per file, not per batch: a single unprobeable or unrepresentable file then costs one skipped file
instead of the whole scan, and it matches the transaction boundary `complete_upload` already uses (`STY-RUST-040`
cross-context write in one unit of work). The scan probes through the same `LocalMediaProbe` and its permit pool, so a
scan cannot starve a concurrent upload's probe.

A new `domain/port/media_source.rs` provides discovery: `walk()` yielding `{ relative_path, size_bytes, modified_at }`.
It is separate from `FileStorage`, which stays the write path for server-owned content; mixing read-only traversal into
it would give one port two owners and two trust levels.

Audio requires a third probe, `probe_audio` →
`ProbedAudio { duration_ms, codec, sample_rate_hz, bit_depth, block_size }`, implemented over `ffprobe` in the same
adapter.

### 5. Identity is location plus size plus modification instant

The scan does not hash. A `file` it registers records `size_bytes` and `modified_at` from `stat`; drift detection
compares those two values. Hashing a whole library at every start is measured in hours of I/O, and the digest buys
nothing for a file the server did not receive.

`file.integrity_hash` therefore becomes **nullable**, and is not dropped:

- an upload still carries the digest that proves the content it received is the content it will store, and
  `caller_file_id`'s deduplication still keys on it;
- SQLite treats `NULL` as distinct in a unique index, so `uq_file_user_id_integrity_hash` stops applying to a row with
  no digest for free, and `uq_file_path` keeps applying to every row.

_Alternative:_ drop the column. Rejected — it removes upload integrity verification and per-caller deduplication, both
of which exist and are tested, to serve a scan that never needed them.

### 6. `origin` is a policy hook with a fail-safe default

```sql
origin TEXT NOT NULL DEFAULT 'SCANNED' CHECK (origin IN ('UPLOADED','SCANNED'))
```

The default is `SCANNED` so a row created by a path that forgets to declare its origin is treated as content the server
did not create. That is the safe direction: the value answers "did we put these bytes here?", which is what a future
deletion policy needs, and an unlabelled row must not answer yes.

Path resolution no longer needs it (§ 1 collapsed the roots). It is kept because the merge **spreads** the deletion
hazard from scanned files to every file: with one root, the server can no longer tell by location alone whether it may
remove content. `delete_gallery` and `delete_gallery_item` currently stop deleting (§ 9), so nothing consumes the column
yet; it is recorded now, when the table is editable, because adding it after a release needs a migration and an answer
to what every existing row is.

_Alternative:_ `source_id` foreign key to a media-root row. Rejected as premature while there is exactly one root and no
attribute of a root to store.

### 7. `audio_channel` is removed; the two audio measurements become optional

`audio_channel` cannot be derived: three `nb_channel` values are duplicated across layouts, so a channel count from
`ffprobe` maps to no unique row, and a `channel_layout`-string mapping would carry gaps (`downmix`, `7.1(wide)`) nobody
needs yet. Removing it deletes the create pair, the seed pair, `domain/model/audio_channel.rs`, the `channel` field on
`Audio`, and `audio_channel_id` from the two music repositories' joins, and drops it from `PERS-005`'s seed list and the
entity-relationship diagram (`PERS-011`).

The channel _information_ is genuinely lost; a plain `channels INTEGER` would recover it without a reference table. It
is not added here because nothing reads it, and it can be added when enrichment lands and can populate it meaningfully.

`bit_depth` and `block_size` become nullable, each checked only when present:

```sql
bit_depth  INTEGER,   CHECK (bit_depth  IS NULL OR bit_depth  > 0)
block_size INTEGER,   CHECK (block_size IS NULL OR block_size > 0)
```

The power-of-two rule was never a property of the content. `ffprobe`'s `frame_size` is 1152 for MP3 and 1024 for AAC, so
the rule rejects the most common audio there is; and neither value exists for PCM or for lossy codecs at all. "Unknown"
is the honest value, and fabricating a placeholder would be worse than the freedom to say nothing. `AudioError`'s two
validants move with the schema, since the domain is the authority: `Audio::try_new` validates only a value that is
present.

`bit_depth` can usually be derived from `sample_fmt` (`u8`→8, `s16`/`s16p`→16, `s32`/`s32p`/`flt`/`fltp`→32,
`dbl`/`dblp`→64), so it is populated whenever the codec reports a sample format. `block_size` is populated from
`frame_size` when the content reports one, and is left absent otherwise — the probe must not invent one.

### 8. The verification pass is also the repair mechanism

`walk()` yields the current truth; the registered rows are the recorded truth. Comparing them yields four verdicts, and
none of them rewrites content:

| Verdict     | Cause                                          | Action                           |
| ----------- | ---------------------------------------------- | -------------------------------- |
| `untouched` | size and mtime match                           | none                             |
| `changed`   | size or mtime differ                           | none this change — reported only |
| `missing`   | no entry under the root for a registered path  | none — the row is retained       |
| `backfill`  | a file row with no media row for its mime type | probe and insert the missing row |

`backfill` is what makes the upload/scan asymmetry a latency rather than a hole. `complete_upload` registers media rows
only for `image/` and `video/`, so an uploaded audio file has a `file` row and no `audio` row; the next scan completes
it. That is why `complete_upload` is left alone, with a `TODO(ingest)` recording the gap.

`changed` deliberately writes nothing. Acting on it needs a decision this change does not have — re-probe and update,
mark stale, or drop the row — and the proposal keeps that out of scope. Reporting it is what makes the decision possible
later.

### 9. Nothing deletes content under the media root

Both call sites that delete bytes — `delete_gallery.rs:246` and `delete_gallery_item.rs:253` — keep their plumbing and
comment the deletion out with a `TODO(ingest)` naming the reason: the path now addresses the operator's mount, and a
`SCANNED` file was never the server's to remove. Row deletion still commits; only the best-effort byte removal stops.

The scan itself never opens a file for writing and never removes one (§ spec). Together these are the whole of the
guarantee: the server may stop referencing content, but it does not destroy it.

### 10. The scan is a spawned worker, drained on shutdown

The composition root starts the scan on a background task after `EnsureStorageDirectories` and after the router is
built, so readiness never waits for it. The worker takes a cancellation signal and the graceful-shutdown path signals it
before draining axum, so a scan in progress does not outlive the process. The change observer (§ 12) is drained the same
way, so shutdown stops both drivers before the process exits.

The scan reports aggregate counts and never a file name or path: `OBS-003` forbids logging a raw file name, and a scan
that named files would be the one place in the server that did. Counts are what makes an operator's question — "did it
pick up my library?" — answerable.

_Alternative:_ run the scan synchronously before binding the listener. Rejected — with a large library the server is
unavailable for the duration, and it turns a slow mount into a startup timeout rather than a degraded mode.

### 11. Uploaded files keep their prefixed name

`complete_upload` keeps writing `{upload_id}_{file_name}` into the media root. The prefix guarantees a unique
`file.path`, so two callers uploading `film.mp4` do not collide on `uq_file_path`.

The cost is a name in a namespace the operator browses, which a pre-existing file will not have. Dropping the prefix was
rejected because the unique constraint would then need a name-disambiguation scheme, and the collision is a real one
rather than a theoretical one.

### 12. Change observation is an accelerator over the same per-path operation

The server observes the media root with `notify` across a tokio channel and dispatches each resulting path to the same
single-path operation the scan drives, after holding the path until its size and modification instant stop changing. The
scan and the observer are therefore two callers of one function, not two pipelines.

```
  startup walk ─┐
                ├─> process_one(path) ─> register | verify | backfill | skip
  notify event ─┘
```

What the observer handles, and what it deliberately does not:

| Event          | Action                                                          |
| -------------- | --------------------------------------------------------------- |
| create         | process after the content stops changing (§ stability below)    |
| rename         | update the recorded location in place, preserving the media row |
| remove         | report `missing`, retain the row                                |
| modify content | ignored — acting on it is the deferred `changed` decision (§ 8) |

The stability gate is not optional: `inotify` reports a creation as soon as the inode appears, which for a 40 GB copy is
long before the bytes are there, and probing a partial file yields a garbage media row or a probe failure. A path is
processed only once its size and modification instant have stopped moving across a short window.

Rename is the one case where events beat a rescan: `notify` correlates the old and new names of a move, so the recorded
location can be updated and the media row kept. A rescan cannot do that without a content digest, which § 5 deliberately
does not have — it would see a deletion and an unrelated arrival.

_Why events at all, given the scan works:_ a rescan is cheap in this design (walk and `stat`, with probing only for new
files), but it still has to run on a timer to be useful, and stat-ing a large tree on every tick is worse than being
told what changed. Events make the common case — a file dropped into the library — immediate.

_Alternatives rejected:_ running the scan on a schedule instead (cheap to build, but a timer is either too slow to feel
responsive or too frequent to be kind to the disk); a route to trigger a scan (adds an API surface and a permission
question for no gain, since the server can observe the change itself).

_Assumption and boundary:_ the mechanism is a **local filesystem**. `inotify`, `FSEvents` and `ReadDirectoryChangesW`
report local changes only; a share mounted over SMB or NFS delivers no events for writes made by other clients, so a
watcher there is silently blind. A network filesystem is out of scope for this change, and the startup scan remains the
mechanism of record for any deployment the observer cannot cover — it is not a fallback that has to be added later, it
is already the authority (§ 10).

Operational limits the adapter must absorb: one kernel watch per directory, bounded by the host's
`fs.inotify.max_user_watches`, which a deep library can exceed; a failed watch registration is logged and leaves the
startup scan as the only mechanism, never failing startup; and a queue overflow reports itself, because the events lost
in it are exactly the files that would otherwise go unseen until the next start.

## Risks / Trade-offs

- **The mount must be writable.** A read-only export — the natural way to hand a media server a library — breaks uploads
  at promote time. → Document it in `README.md`; the failure is a logged `port_fault`, not silent corruption.
- **Content silently disappears into an ephemeral layer when the mount is absent and the operator ignores the warning.**
  → Never create the root (§ 2); make the warning name the root; an upload failing on write is the second signal.
- **`walk()` and the registered-row set are materialised in memory.** → Fine for a home library; a streaming walker and
  paged reconciliation are the follow-up if a library large enough to matter appears.
- **One transaction per file makes a first scan of a large library slow.** → It runs in the background and is bounded by
  the probe permit pool; the trade is isolation over throughput.
- **`mtime` is a weak identity.** A copy that preserves `mtime`, or an edit that does not change size or `mtime`, is
  reported `untouched`. → Acceptable: nothing destructive follows a wrong `untouched`, and re-hashing the library is not
  a cost worth paying to avoid it.
- **The media root is one flat namespace for uploads and pre-existing files.** An operator's folder literally named
  `files/` no longer collides with a server folder, but the server does write into a tree it also reads. → The scan
  tolerates its own output by construction: a path it registered is verified, not registered again (§ 8).
- **Removed `audio_channel` loses stereo-versus-surround.** → Recorded in § 7; recoverable later as a plain count.
- **The observer only works on a local filesystem.** A share mounted over SMB/NFS delivers no events, so a deployment
  there silently registers nothing after startup. → Out of scope and recorded in § 12; the startup scan still covers the
  library as of the last start, and the observer never becomes the mechanism of record.
- **A `create` event precedes the bytes.** Ingesting on the event alone would probe a partial file. → The stability gate
  in § 12 refuses to process a path until its size and modification instant stop moving.
- **Kernel watch limits and queue overflow are silent by default.** A deep library can exceed
  `fs.inotify.max_user_watches`, and a bulk copy can overflow the queue. → A failed registration and an overflow are
  both reported; neither fails startup, and the scan remains the repair path.
- **One new dependency that exists only for the observer.** → `notify` is the first new runtime crate in this change; it
  must clear `deny.toml`, § 6 and the notices hook, and the observer is the only consumer if it is ever removed. Event
  coalescing is the consumer's stability gate, not a separate debouncing crate.
- **`data/files/` removal breaks any deployment that populated it.** → Pre-release, so the migration is "edit the
  migrations and reset the database" (`PERS-014`).

## Migration Plan

No released schema, so the migrations change in place and a development database is reset, not migrated:

1. Add `notify`, pinned exactly, cleared against `deny.toml`, recorded in § 6, with `THIRD_PARTY_NOTICES.txt`
   regenerated by the commit hook.
2. Edit `create_file` for `size_bytes`, `modified_at`, `origin` and the nullable `integrity_hash` with its conditional
   check.
3. Edit `create_audio` to drop `audio_channel_id` and to make `bit_depth` and `block_size` nullable with the relaxed
   checks.
4. Delete the `create_audio_channel` and `seed_audio_channel` pairs.
5. Update `PERS-005`'s seed list and the entity-relationship diagram in the same change (`PERS-011`).
6. Delete `mnemorium.db` together with its `-wal` and `-shm` sidecars and restart; `sqlx::migrate!` fails with
   `VersionMismatch`/`VersionMissing` otherwise, which is the intended signal rather than an obstacle.

Rollback is a checkout of the previous revision plus the same database reset. There is no data to preserve.
