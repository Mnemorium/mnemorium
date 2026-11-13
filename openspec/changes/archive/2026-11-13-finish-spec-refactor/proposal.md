# Proposal

## Why

`openspec/specs/library/` is the last capability tree the `REQ-*`/`SCN-*` refactor never converted, and it now holds
nothing canonical. Six of its eight capabilities restate contracts that `movies`, `series`, `music` and `cover-art`
already own, for 63 requirements, and `openspec validate --specs` cannot see the duplication because it judges one
capability at a time — every `library/*` spec passes on its own.

The other two have no successor at all: `media-ingest`, with 16 requirements, and the stored-file read that `download`
left behind when it took the byte-serving contract. Deleting `library/` as it stands would delete 17 requirements that
nothing else states.

Five archived changes already record the intended retirement in their `REMOVED Requirements` blocks, but none declares
`retire_capabilities`, so the archive refuses to delete a spec whose last requirement goes and the deltas never ran.
Finishing the refactor therefore means running those retirements, and giving the two orphans an owner first, so that
removing `library/` loses no contract.

## What Changes

- Retire all eight `library/*` capabilities, deleting each spec and the directory `library/` then leaves behind: seven
  whose contract another capability already owns — `movies` (7 requirements), `series` (15), `music-albums` (15),
  `music-groups` (5), `music-playlists` (17), `cover-art` (4) and `assets` (6) — and `media-ingest` (16), whose
  requirements move to the new capability below.
- Declare `retire_capabilities: true` on this change, so the archive performs those deletions instead of refusing them.
- Add a `media-ingest` capability carrying the 16 requirements of `library/media-ingest`, re-authored with
  `REQ-MEDIA-INGEST-*`/`SCN-MEDIA-INGEST-*` identifiers. The behaviour is unchanged; the obsolete who-may clauses are
  replaced by the action `authorization` decides.
- Add a `file` capability for the stored-file resource, re-authoring the one surviving requirement of `library/assets`
  as three: what a stored file records, reading one by identifier, and the links it publishes.
- Add `REQ-MUSIC-029` to `music` for the partial album-metadata update. `library/music-albums` states it and `music`
  omitted it, while `PATCH /music/album/{album_id}` exists — so retiring the old spec without this would drop a contract
  the server still serves.
- Define `Stored file` in the glossary, and rewrite the `Typed asset` definition so it states the concept instead of
  naming a capability path.
- No route, payload, status, schema or migration change. **BREAKING**: none.

## Capabilities

### New Capabilities

- `media-ingest`: registers the media found under the library root, keeps what it registered reconciled with the
  directory as it changes, and owns what removing a record may and may not delete.
- `file`: the stored-file record — the content it names and its measurements, how a caller resolves it by identifier,
  and the typed views and content route its representation links.

### Modified Capabilities

- `music`: gains `REQ-MUSIC-029`, the partial album update that `library/music-albums` states and `music` omitted.
- `library/movies`: retired; `REQ-MOVIES-001`–`REQ-MOVIES-005` own its requirements.
- `library/series`: retired; `REQ-SERIES-001`–`REQ-SERIES-013` own its requirements.
- `library/music-albums`: retired; `REQ-MUSIC-001`–`REQ-MUSIC-011` and `REQ-MUSIC-029` own its requirements.
- `library/music-groups`: retired; `REQ-MUSIC-012`–`REQ-MUSIC-017` own its requirements.
- `library/music-playlists`: retired; `REQ-MUSIC-018`–`REQ-MUSIC-028` own its requirements.
- `library/cover-art`: retired; `REQ-COVER-ART-001`–`REQ-COVER-ART-006` own its requirements.
- `library/assets`: retired; `REQ-DOWNLOAD-001`–`REQ-DOWNLOAD-009` and `REQ-FILE-001`–`REQ-FILE-003` own its
  requirements.
- `library/media-ingest`: retired; `REQ-MEDIA-INGEST-001`–`REQ-MEDIA-INGEST-016` own its requirements.

## Impact

- `openspec/specs/`: eight capability directories removed, `media-ingest/` and `file/` added, and one requirement added
  to `music/`. After the change `openspec/specs/library/` does not exist.
- `docs/development/Glossary.md`: one term added and one definition rewritten.
- No source, API, dependency, configuration or migration change. The generated OpenAPI document is unchanged, because
  every route the retired capabilities described still exists and is still served.
