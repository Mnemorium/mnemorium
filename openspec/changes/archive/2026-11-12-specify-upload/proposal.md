# Proposal

## Why

Upload is implemented end to end — a resumable chunked session, a staging file, digest verification, promotion and a
reaper — but its contract lives inside `library/assets`, where 17 of its 27 requirements describe intake or the row it
produces and only 10 describe serving. The Purpose states both halves, no requirement carries the repository's
`REQ-*`/`SCN-*` convention, and a reader cannot tell which capability owns an **Upload session**, a chunk, completion or
reaping. The areas already re-authored as single-owner capabilities (`api`, `authentication`, `identity/authorization`)
set the pattern the spec redesign is following; `library/media-ingest` already describes itself as the path that
populates the library "without passing through the upload API", so the two are siblings in fact and in text.

Three costs are concrete. The contract never states exact outcomes for a failed begin, never says what the completed
file's **Owner**, **Visibility** or origin are, and has no scenario for reclaiming a staging file left by an interrupted
begin. The proof is scattered — E2E proves the lifecycle, the configuration suite proves the settings, and no system
module proves the scenarios — so one changed rule fails in three places or in none.

## What Changes

- Add a top-level `upload` capability at `openspec/specs/upload/spec.md` with 17 requirements, re-authored with
  `REQ-UPLOAD-*`/`SCN-UPLOAD-*` identifiers: session begin and its **Staged content**, fixed chunk size, chunk write and
  refusal, status read, whole-file **Integrity hash**, sealing, promotion and typed registration, declared **Content
  Type** authority, per-caller dedupe, expiry, cancellation, finished identity, reaping, the absent **Library root**
  refusal, and the completed file's attribution.
- Remove the 16 intake requirements from `library/assets`; it keeps its 10 serving requirements and its media-row naming
  rule, which describes the name `library/assets` serves for a row produced by either intake path. Rewrite its Purpose
  directly in the main spec, because `openspec archive` preserves an existing Purpose and a delta cannot replace it.
- Retire the restatement of upload ownership. `identity/authorization` already owns it (`REQ-AUTHORIZATION-018`,
  `SCN-AUTHORIZATION-002-E`, `SCN-AUTHORIZATION-015-B`); `upload` states who owns a session and never who is denied.
- State the tightened contract: exact failure outcomes for begin, chunk, completion and cancellation;
  `origin: UPLOADED`, caller ownership, `Private` visibility and the unique root-relative path of the promoted file; and
  the interrupted-begin reclaim (`PERS-016`).
- Move the proof into the system suite. Add `test/system/upload.py`, retire `test/e2e/asset/uc006_upload_media_file.py`,
  and reduce `test/configuration/asset_settings.py` to the setting effects `TEST-053` assigns, deleting every case that
  only re-proves a scenario. Declared statuses, media types and payload shapes stay with Schemathesis.
- Record the new vocabulary in `docs/development/Glossary.md` and name `upload.py` in the system-module list of
  `TechnicalDesign.md` § 5.

**BREAKING**: none. No route, request, response or status changes.

## Capabilities

### New Capabilities

- `upload`: the intake contract — beginning a resumable session, writing and refusing chunks, reading the session,
  verifying and sealing completion, promoting and registering the typed asset, per-caller dedupe, expiry, cancellation,
  reaping, the absent-root refusal and the completed file's attribution. Access rules belong to
  `identity/authorization`; serving belongs to `library/assets`.

### Modified Capabilities

- `library/assets`: the 16 intake requirements are removed and the Purpose is rewritten to serving only; the 10 serving
  requirements and the media-row naming rule are unchanged.

## Impact

- **Specs**: `openspec/specs/upload/spec.md` is created; `library/assets` falls from 27 to 11 requirements; no delta for
  `library/media-ingest`, `identity/authorization`, `api` or `configuration/loading`.
- **Tests**: a new `test/system/upload.py` proves the scenarios black box; `test/e2e/asset/uc006_upload_media_file.py`
  and the overlapping `test/configuration/asset_settings.py` cases are deleted in the same change; Schemathesis keeps
  the status, media-type and payload-shape proof; Rust tests keep only what the container cannot observe — concurrent
  sealing, the reaper clock and the storage seams.
- **Docs**: `docs/development/Glossary.md` gains **Upload session**, **Chunk**, **Staged content**, **Library root** and
  **Integrity hash**; `TechnicalDesign.md` § 5 names `upload.py` in the system-module list.
- **Code**: the two divergences the audit confirmed are fixed without changing the route surface, payloads, statuses or
  schema: an uppercase integrity hash is rejected, and deleting an upload row removes its `UPLOAD` resource row in the
  same unit of work. No persistence or dependency change.
