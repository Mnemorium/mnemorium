# Design

## Context

See `proposal.md` for why. The constraints that shape the approach:

- `get_asset_content` authorizes inside a unit of work, then calls `FileStorage::read(path)`, which returns
  `Option<Vec<u8>>`; `asset_content_response` wraps the whole vector in `Body::from`. The port and the response both
  carry the comment that streaming is a later change.
- `asset_access::access_file` decides from the image and video `scope` columns. `audio` has no scope column, and a
  catalogue video's scope is never set (the scope triggers cover cover art and gallery items only), so both fall through
  to `Authorization::decide_orphan_asset`: owner or Root Admin only.
- The scan's `report_absent` returns `Verdict::Missing`; a test asserts the record is retained. `MediaIngestRunner`
  warns once and does not spawn the scan when the root is absent.
- The instance identity already persists a signing secret (`state.yaml`) used by the JWT provider.
- `axum`'s `get` also serves `HEAD`; `Body::from_stream` exists, and `tokio-util` is already in the dependency tree.
- The trace middleware logs the path and never the query string (`OBS-003`), with a test proving it.

## Goals / Non-Goals

**Goals:**

- One byte-serving path that supports ranges, validators and conditional reads, shared by the file, image, audio and
  video content routes.
- A playable content link that works for a client which cannot set an `Authorization` header.
- Audio and video reads authorized through the resource that owns them, without a new scope column.
- Removal reconciled by the scan, with a defined cascade, so "row exists" implies "bytes exist" up to a race.
- A bounded, configurable startup decision for an absent library root.

**Non-Goals:**

- Transcoding, HLS or DASH packaging, trickplay thumbnails, subtitle delivery. The `stream`/`subtitle_stream` tables are
  not exercised here.
- Upload changes; the ingest's content-writing behaviour; media re-identification.
- Multi-range responses, cache-control policy beyond `no-store` on representations, or a reverse-proxy cache layer.
- Any change to the Flutter client.

## Decisions

### HTTP ranges, not an application chunk API

The client is a media player. A closed range, an open-ended range, a suffix range, `If-Range` and `If-None-Match` are
things a player, a download manager and a browser already speak; an upload-style `chunk/{n}` endpoint is something none
of them speak. The "chunk" of a download is a byte range.

### One serving path behind four routes

`file`, `image`, `audio` and `video` content handlers all call one shared function with the authorized file. The typed
routes are views over the canonical file, and `/image/{id}/file`-style aliases stay `303 See Other`. The handler tree is
excluded from Rust coverage, so every branch that can live elsewhere does: the shared function only maps the use case's
resolved outcome, a credential and a file into HTTP headers and a stream.

### Range semantics live above the handler

The content use case parses one `Range` header value inline: `bytes=` only, closed (`0-99`), open-ended (`100-`) and
suffix (`-500`), resolved against the size of the file that was actually opened. More than one range, an unknown unit or
a syntactically invalid value means "serve the whole content"; that is legal and avoids a multipart response nobody
needs. A range that cannot be satisfied is a distinct outcome, not an error.

```
no Range / unparsable / multi-range -> 200 whole
closed, open-ended, suffix           -> 206 + Content-Range: bytes <start>-<end>/<size>
start >= size                        -> 416 + Content-Range: bytes */<size>
```

`Content-Length` and the range arithmetic use the size of the file that was actually opened, not `file.size_bytes`: the
row can lag behind a file that changed on disk, and the opened handle is authoritative for the bytes being served.

### Streaming port

`FileStorage::read` becomes a bounded open: the adapter opens the path, reads its metadata, seeks to the range start and
takes the requested length, returning an opaque bounded reader. The REST layer bridges it to
`Body::from_stream(tokio_util::io::ReaderStream::new(reader))`. No `Vec` of the content exists anywhere.

Alternatives considered: a hand-rolled `Stream` over `tokio::fs::File` (avoids the direct `tokio-util` declaration but
adds a manual poll implementation for no gain) and chunked application-level reads into a channel (more moving parts).
`tokio-util` is already resolved in `Cargo.lock`, so this adds no new transitive dependency.

`HEAD` follows the same path and writes headers only: the handler does not open a stream for it.

### Validators come from what the row already holds

`Last-Modified` is `file.modified_at`. `ETag` is the stored whole-file digest when present. An ingested file may have a
NULL digest, and recomputing it per request would read the whole file; such a file is served with `Last-Modified` only,
and `If-Range` falls back to it. An `If-None-Match` is only honoured when an `ETag` exists. Backfilling digests at
ingest is a separate concern.

### The content link is a capability, not a session token

`_links.content` is minted per representation read as the content route plus `exp` and `sig`. `sig` is an HMAC over a
domain-separated message (`media-link`, file id, expiry) with the instance signing secret already in `state.yaml`; no
new secret and no new configuration. The credential is accepted only on content routes, only for the file it names, and
only until it expires.

Because the credential is minted after an authorization pass and is scoped to one file, it _is_ the authorization for
that read: a content request carrying it does not re-resolve the caller. That is the trade the TTL buys back.

```
Authorization: Bearer ...          -> resolve caller, authorize, serve
?exp=...&sig=...                   -> verify HMAC + expiry + file id, serve
both present                       -> 400 Bad Request (ambiguous credential)
session token in ?access_token=    -> ignored; only exp/sig are accepted
```

The TTL is 12 hours (a constant, not a setting): long enough for a film plus seeking, short enough to bound a revoked
permission. A player that outlives it re-reads the representation and gets a fresh link. Asset representations that mint
a link answer `Cache-Control: no-store`, so a cached link cannot outlive its usefulness. The trace middleware already
logs no query string, so the credential never reaches a log sink.

Alternative considered: the session JWT in the query string. Rejected because it is long-lived, account-wide, appears in
proxy logs and browser history, and cannot be scoped to one file.

### Delegation, not a copied scope

`asset_access` gains the audio and video half of the rule the authorization spec already states:

- audio → recording (by `audio_id`) → its medium/album → `library.read` with `Global`; no recording falls to the orphan
  rule.
- video with `LIBRARY` scope → `library.read`; with `USER` scope → gallery item → `gallery.read`, else the orphan rule;
  with no scope → a movie or an episode that names it → `library.read`, else the orphan rule.
- `access_file` considers the audio row too, so `/file/{id}/content` for a track follows the same path as the typed
  audio view.

Alternative considered: add `scope` to `audio` and a scope trigger for catalogue video. Rejected: it duplicates the
delegation rule into the schema, needs a migration, and audio is never gallery media; the owning row is the truth.

### Reconciliation is ordered deletes in one unit of work

The scan's removal path deletes in an order that respects the foreign keys, mirroring `delete_gallery`:

```
image -> cover_image_id set to NULL on music_group, music_album, music_playlist,
         movie, series and season; gallery_item rows cascade-delete
video -> stream rows, movie or episode that requires it, gallery items
audio -> music_recording (then its group/genre links, audio_stream and music_track
         placements), then the audio row
then   -> the file row
```

One unit of work per removed file. A failure rolls the file back, keeps the record, reports the file as `missing` and
lets the scan continue. The report gains `removed`; `missing` now means "absent but the removal did not complete".

Alternative considered: `ON DELETE CASCADE` in the schema. Rejected for now: it hides a destructive graph behind the
schema and diverges from how deletion is expressed today. If the graph grows, a migration can replace the ordering and
the repository tests already cover the constraints.

### Startup waits once, then decides

Two settings under `library.startup`:

```
media_root_wait_seconds   30    startup-only; 0 decides immediately
fail_without_media_root   false  false starts and reconciles as empty
```

`EnsureStorageDirectories` already reports whether the root exists; it becomes a wait-and-report use case that polls
every second up to the window. The composition root keeps the decision: with `fail_without_media_root` true and the root
still absent, startup returns an error before readiness and nothing is reconciled; otherwise the server starts, the scan
runs without a root, and every registered file is reconciled as removed.

When the flag is false the wait runs without delaying readiness, preserving "ingest does not delay readiness"; when it
is true the wait is the deliberate exception. `seed::apply` runs before this step and, under `test-seed`, creates the
root, so a seeded run never waits.

### Test seams

- The configuration harness can start without creating `media/` (it creates it unconditionally today).
- `script/e2e.sh` exports the host media directory to pytest, so the E2E suite can delete a file the API registered and
  watch the reconciliation land.
- The range parser, the credential verifier, the delegation and the cascade classifier live above the handler and are
  unit-tested; the transition statuses (`503`, `500`) are classified in the use case and asserted there rather than
  raced in a container.

### OpenAPI declarations

`Range`, `If-Range` and `If-None-Match` are declared header parameters, and the content operations declare `206`, `416`
and `304` alongside `200`, with `Content-Range`, `ETag`, `Last-Modified` and `Accept-Ranges` headers. `HEAD` is not
declared: it is served by the `GET` route. `docs/openapi.json` is regenerated from the handlers.

## Risks / Trade-offs

- **A signed link outlives a revoked permission** → 12-hour TTL, file-scoped credential, `no-store` representations.
- **The credential travels in a URL** (browser history, proxy access logs) → no session token, short life, one file; the
  server's own trace never logs the query.
- **The link expires mid-playback** → a 12-hour window; the client re-reads the representation for a fresh link.
- **A video loss deletes curated metadata** (movie, episode, track placements) → accepted, with re-ingest plus manual
  re-identification as the documented recovery.
- **An empty-but-present mount is indistinguishable from an emptied library** → accepted; only an absent root is guarded
  by the wait and the flag.
- **A mount that appears after the window with the flag enabled** → startup refuses; the operator raises the window or
  fixes mount ordering.
- **Cascade order drifts when a new foreign key is added** → the unit of work fails and reports the file as missing
  rather than corrupting the library; repository tests cover the current graph.
- **Range and size disagree after an on-disk change** → the opened file's metadata governs the response; the next scan
  reconciles the row.
- **`notify` may not deliver a host-side delete through the E2E bind mount** → validate early; the configuration suite's
  restart flow is the deterministic fallback.
- **A 100 GiB stream has no per-connection cap** → out of scope; the reverse proxy and operator policy own bandwidth.

## Migration Plan

No schema change and no new secret. The two settings default to the current spirit (warn and continue) with a bounded
wait, so deployment remains compatible. The behavioural change to announce is that a removed file's records are deleted
rather than retained; operators should know before upgrading and rely on re-ingest to rebuild. Rollback is a release
rollback; no datastore migration has to be undone.

## Open Questions

- Backfilling the whole-file digest at ingest, so every file carries an `ETag`. Deferrable; today `Last-Modified` covers
  those files.
- Multi-range responses, if a client ever needs them.
- Delivering the `stream` and `subtitle_stream` rows to a player. Deferrable; unrelated to byte retrieval.
