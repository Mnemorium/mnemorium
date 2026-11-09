# Proposal

## Why

The Asset context serves bytes, but it was designed for artwork: every content request loads the whole file into memory,
there is no partial content, and the typed audio and video the file representation already links to do not exist. At the
configured 100 GiB file limit, that cannot serve a film or a track. Until retrieval actually works the gap is invisible;
the moment playback lands, two older lies become client-visible: `_links.audio` addresses a route nobody registered, and
a file row whose bytes vanish is kept forever and merely reported as `missing`. A media server has to be able to say,
truthfully, "here are your bytes", "that asset is gone" and "the storage is unavailable".

## What Changes

- Serve byte ranges on `GET /api/v1/asset/file/{file_id}/content` and on every typed content route:
  `Accept-Ranges: bytes`, `206 Partial Content` with `Content-Range`, `416 Range Not Satisfiable` with
  `Content-Range: bytes */<n>`, `ETag` from the stored whole-file digest, `Last-Modified`, and `If-Range` /
  `If-None-Match` handling. `HEAD` answers without reading the file. Content streams from disk in bounded chunks; no
  response body is assembled in memory.
- Add `GET /api/v1/asset/audio/{audio_id}` and `GET /api/v1/asset/video/{video_id}` with their content routes and
  `303 See Other` file aliases, mirroring the typed image view, so `_links.audio` stops addressing an unregistered route
  and `_links.video` is emitted.
- Add `_links.content` to the file, image, audio and video representations. On it, mint a short-lived, per-file signed
  playback link, so a client that cannot set an `Authorization` header (a browser media element) can play the asset
  without putting the session credential in a URL.
- Authorize audio and video reads through the resource that owns them: a movie or an episode follows `library.read`, a
  gallery video follows `gallery.read`, a track's audio follows the album's catalogue read. No scope is copied into a
  new column and no override is added. This fixes the current behaviour where a catalogue video or a track is readable
  only by the file's owner.
- **BREAKING** A registered file whose bytes are absent is no longer reported-and-retained. The scan removes the file
  record and its dependents in one unit of work: an image clears the cover references that point at it, a lost video
  removes the movie, episode or gallery item that requires it, a lost audio removes its recording and the track
  placements that use it.
- **BREAKING** Content statuses become truthful: `404` for an unknown or unreadable asset, `503` while the library root
  is absent, and `500` for a row whose bytes are missing (a race the next scan reconciles).
- Add two startup settings under `library.startup`: `media_root_wait_seconds` (default 30) bounds how long startup waits
  for the library root before deciding, and `fail_without_media_root` (default false) chooses between refusing startup
  and starting with the library reconciled as empty.
- Document re-ingest as the recovery path: after the root returns, the scan rebuilds the file and media rows, while
  identification, covers, galleries and playlists are redone by the operator.

## Capabilities

### New Capabilities

None. Every requirement this change needs belongs to an existing capability.

### Modified Capabilities

- `library/assets`: content responses gain partial content and validators; the typed audio and video views and their
  content routes are added; every asset representation carries `_links.content`; and the content outcomes become `404`,
  `503` and `500` instead of one undifferentiated `404`.
- `library/media-ingest`: an absent library root at startup is bounded by a configured wait and may refuse startup; a
  removal is reconciled instead of merely reported, which changes the fate of a missing file's record and its
  dependents; and the aggregate report distinguishes removed files.

## Impact

- REST handlers under `src/lib/infrastructure/inbound/rest/handler/asset/`, the asset router, and the credential carried
  by a content request; `docs/openapi.json` is regenerated.
- Application: `get_asset_content` (ranged read), `asset_access` (audio and video delegation), the ingest use case
  (removal cascade), a startup wait-and-decide use case, and `load_configuration` for the new settings.
- Domain ports: `FileStorage` gains a ranged/streamed read; the token provider or a sibling port gains the playback
  link; the asset unit of work gains the cascade delete path. Repositories gain the lookups that resolve an audio or a
  video to its owning catalogue row.
- Configuration: `library.startup.media_root_wait_seconds` and `library.startup.fail_without_media_root`, in
  `config.yaml`, the configuration table of `TechnicalDesign.md` § 10, and the loaders.
- Datastore: no schema change is required for retrieval; the cascade is expressed either as ordered use-case deletes or
  as schema cascades. Either way the ingest's removal path changes.
- Tests: Rust unit and integration tests for the range, delegation, cascade, wait and signing; a new catalogued use case
  and its E2E file; the configuration suite's `library_settings.py`; Schemathesis operations for the new endpoints and
  the `Range` header; two harness seams (a configuration start without `media/`, and the E2E media root exposed to
  pytest).
