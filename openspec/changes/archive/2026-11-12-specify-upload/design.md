# Design

## Context

See `proposal.md` — Why. The constraints that shape the approach:

- `library/assets` currently holds 27 requirements: 17 describe intake or the media row it produces, 10 serve stored
  content. It has no `REQ-*`/`SCN-*` identifiers; the areas re-authored since the convention (`api`, `authentication`,
  `identity/authorization`) do.
- `openspec archive` preserves an existing capability's `## Purpose`; a delta's `## Purpose` is used only when the main
  spec is created. A rewrite of `library/assets`' Purpose is therefore a direct edit to the main spec, ordered after the
  delta is applied.
- `identity/authorization` already owns upload ownership (`REQ-AUTHORIZATION-018`), an upload's authorization parent
  (`SCN-AUTHORIZATION-002-E`) and the decision order (`SCN-AUTHORIZATION-015-B`).
- The upload routes are served from `handler/asset/` and implemented by
  `use_case/{begin_upload,write_upload_chunk, get_upload,complete_upload,cancel_upload,reap_uploads}.rs`;
  `domain/model/upload.rs` owns the chunk bitmap. `complete_upload` records the promoted file with `Origin::Uploaded`
  and `Visibility::Private`.
- The system suite starts its own container per case through `harness.Sut.start(config=...)`, navigates by links and
  asserts only what an operator observes (`TechnicalDesign.md` § 5). The configuration suite uses the same harness.
- `test/e2e/asset/uc006_upload_media_file.py` proves the lifecycle end to end; `test/configuration/asset_settings.py`
  holds four cases that each set one `asset.upload.*` value and assert a scenario.
- The staged authorization refactor on this branch adds resource rows to begin and completion; the contract describes
  the post-refactor behavior and the test work assumes that refactor is merged.

## Goals / Non-Goals

**Goals:**

- One normative owner for the intake contract, with exact outcomes and the completed file's attribution stated.
- `library/assets` reduced to the serving contract it uniquely owns.
- One black-box proof of every upload scenario in `test/system/upload.py`, with the replaced E2E and configuration cases
  deleted in the same change.
- The spec tree names intake as an area, not as half of `library/assets`.

**Non-Goals:**

- Moving the `/api/v1/asset/upload` routes, renaming the `asset` handler module, or changing any payload, status or
  schema.
- Re-authoring `library/media-ingest`, `library/assets`' remaining serving requirements, or the other old-format library
  specs.
- Moving the media-row naming rule out of `library/assets` (see Decision 2).
- Changing the authorization rule or adding an authorization delta.

## Decisions

### 1. The capability is top-level `upload`, and the URL prefix does not move

A functional area groups a capability's externally observable behavior; it is not a folder mirror of the route tree.
Upload's subject is a session lifecycle — begin, chunk, verify, promote, expire, reap — with its own persistence, timer,
settings and logs, and no catalogue dependency. That is the same shape as `authentication`, and `media-ingest` already
names itself "the path that populates the library without passing through the upload API", so the two intakes are
siblings. The routes stay at `/api/v1/asset/upload`; a client is unaffected.

**Alternative considered:** `library/upload`. Rejected: it leaves the area map saying intake belongs to the content
browsing area, and the redesign is explicitly giving each capability its own owner.

### 2. Seventeen requirements, one of them new, and the naming rule stays in `library/assets`

The sixteen moved requirements are re-authored with exact outcomes. `REQ-UPLOAD-017` is new contract content: the
completed file is the caller's, `Private`, origin `UPLOADED`, carries the content's size and instant, at a unique
root-relative path. UC-006's postcondition and `complete_upload` already promise this; the old spec never stated it, so
the area would otherwise bless a behavior its contract omits.

The media-row naming rule ("An upload names its media row after the uploaded file", including the found-file scenario)
stays in `library/assets`. It describes the `name` field of the row that `library/assets` serves, for either producer;
keeping it there avoids adding a requirement to `library/media-ingest`, which has no `REQ-*` sequence yet, and avoids
splitting one rule across two capabilities. A later re-authoring of `library/` can rename it to a producer-neutral rule.

**Alternative considered:** split it — uploaded half to `upload`, found-file half to `library/media-ingest`. Rejected:
it invents a requirement identity in an old-format spec and makes the scan area own a representation field.

### 3. Ownership is stated once, by `identity/authorization`

`upload` records who owns a session and completes as that owner; it never states who is denied. The removed "An upload
session belongs to one caller" was a duplicate of `REQ-AUTHORIZATION-018` and its scenarios, which already prove the
non-owner `404` and the root's non-exemption in `test/system/authorization.py`. No authorization delta is needed.

### 4. Exact outcomes are in the spec; the API fuzz test proves the declared ones

The spec states the status of each failure (`400`, `413`, `415`, `422`, `409`, `410`, `404`, `503`) because a client
relies on it. Their _proof_ stays with Schemathesis per § 5: the system suite asserts the effect an operator observes —
no session, no staged content, an existing file returned — while the declared status, media type and body shape belong
to the fuzz suite. The system module does not restate the declarative contract.

### 5. The system suite owns the scenarios; E2E and configuration upload cases are deleted

`test/system/upload.py` becomes the single black-box proof, grouped by fixture: the session group (begin, chunk, status,
seal, dedupe, cancel, finished identity), the completion group (image, video and audio against `test/seed/media`), and
the configured group (maximum size, chunk size, expiry, reap interval) whose cases start their own container with
`config=`. The shared `harness.begin_upload`/`put_chunk`/`complete_upload` helpers stay; the system conftest already
imports them.

`test/e2e/asset/uc006_upload_media_file.py` is deleted once the module proves the same scenarios, and
`test/configuration/asset_settings.py` is deleted with it. The four configuration cases each assert a requirement
scenario rather than a configuration-layer property, and the system suite can set the same values on its own container,
so keeping both would report one failure twice (§ 5). `TEST-053`'s module list and the § 5 text drop `asset_settings.py`
and say the `asset.upload.*` effects are proven by the system suite.

Rust keeps only what the container cannot observe: the concurrent-completion race, the reaper's clock and unsealing, and
the storage seams. A declared status or payload is not re-asserted at any Python level.

**Alternative considered:** keep `asset_settings.py` for the settings and let the system module prove behavior only.
Rejected by the operator's goal: the setting effect is the behavior, and two suites proving it is the duplication this
change removes.

### 6. Rewrite the `library/assets` Purpose after the archive, not before

A delta for an existing capability cannot replace its Purpose, and editing it before the removals are applied would
leave the main spec describing a capability it still has. The task is ordered last: apply the deltas, then rewrite the
Purpose to the serving-only sentence. `openspec validate` cannot detect a stale Purpose, so the task is explicit.

### 7. The contract moves first; a real divergence moves the code in the same change

The gaps found are textual: begin failures named no status, attribution was unstated, the interrupted-begin reclaim had
no scenario, and missing versus malformed chunk headers were not distinguished. The code already implements the precise
behavior, so the contract records it without a route, payload or status change. If the audit finds a real divergence,
the task records it in this design and either the spec or the code moves in the same change.

### 8. The audit's two code divergences are fixed in place

The audit behind the scenarios found two places where the code and the newly stated contract disagreed. Both are fixed
here, with no route, payload or status change:

- `IntegrityHash::try_new` accepted a 64-character hexadecimal digest in either case and lowercased it, while
  `REQ-UPLOAD-001` and `SCN-UPLOAD-001-F` require a lowercase digest. The constructor now rejects any character outside
  `[0-9a-f]`, so an uppercase digest answers the begin's declared `400` and creates nothing.
- Deleting an upload row left its `UPLOAD` resource row behind. SQLite can reuse the identifier for the next session,
  whose resource insert then conflicted and answered `500`. Every upload-row deletion now removes the matching resource
  row in the same unit of work: cancellation, the reaper's expired-session phase, and the lazy expiry cleanup in the
  status, chunk and completion use cases. The system suite begins a replacement session after each deletion path to
  prove the identifier is free.

`REQ-UPLOAD-003`'s two halves are proved apart: `SCN-UPLOAD-003-A` is the application test
`begin_upload_keeps_the_chunk_size_it_was_built_with` (the use case snapshots the value and never reads configuration),
and `SCN-UPLOAD-003-B` is the system case `test_a_new_chunk_size_applies_to_sessions_created_after_a_restart`.

UC-006 in `docs/development/UseCases.md` was read against `specs/upload/spec.md` and contradicts none of it, so its
postconditions and alternative flows are unchanged. The entry stays under the `## Asset` bounded context because it
remains the journey that produces an asset; only the normative contract moved to its own capability.

## Risks / Trade-offs

- **Time-based scenarios in the system suite** (expiry, reaping) → configure `expiry_seconds` and
  `reap_interval_seconds` at their 1 s minimum inside the 3 s bound of `TEST-056`; if a case proves flaky, keep the
  request-visible consequence in the system suite and the clock mechanism in its Rust test.
- **Deleting `asset_settings.py` removes a per-section configuration module** → `TEST-053` and § 5 are edited in the
  same change, and the system suite proves the same effects with its own configured container, so no setting loses its
  proof.
- **Deleting the E2E case removes the journey proof** → `test/system/upload.py` must cover the full lifecycle, including
  image, video and audio completion; the system image ships `ffprobe` as the E2E image does.
- **A stale `assets` Purpose after archive** → the purpose rewrite is the final, ordered task and is checked by reading
  the archived spec, not by validation.
- **The in-flight authorization refactor changes the resource rows begin and completion write** → the test work lands
  after that branch; the spec's attribution requirement describes the refactored behavior.
- **The area name differs from its route prefix** → the area is named for the capability, `api` owns the route prefix,
  and the spec names no endpoint; the map is by capability, not by path.

## Migration Plan

1. Apply the spec deltas: create `upload` and remove the sixteen requirements from `library/assets`.
2. Rewrite `library/assets`' Purpose to the serving-only sentence.
3. Add `test/system/upload.py`, prove every scenario, and delete `test/e2e/asset/uc006_upload_media_file.py` and
   `test/configuration/asset_settings.py` in the same change; remove any helper or constant they leave unused, keeping
   the shared upload helpers the system suite imports.
4. Update `TEST-053`, the § 5 system-module list and `docs/development/Glossary.md`.
5. Run the gates: `cargo test` (unchanged), `pytest test/system`, every named rule check, markdown lint and
   `openspec validate specify-upload --strict`.

Rollback is a revert of the spec and test files; no data, route or deployment change is involved.

## Open Questions

None.
