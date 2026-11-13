# Design

## Context

See `proposal.md` — Why. The constraints that shape the approach:

- `library/assets` holds 27 requirements today: 16 intake requirements leave with `specify-upload`, leaving 11 serving
  requirements. This change removes 5 more, leaving 6 that describe the file resource, content serving, ranges,
  validators, the content credential and the outcome distinction.
- The typed rows are `image`, `audio` and `video`. Their characteristics live in `domain/model/{image,audio,video}.rs`,
  their representations in `handler/asset/get_{image,audio,video}.rs`, and their application tests in
  `use_case/get_{image,audio,video}.rs`.
- The routes stay at `/api/v1/asset/{image,audio,video}` and `.../file`; the served OpenAPI document is generated from
  the handlers and does not change.
- `library/assets`' Purpose is being rewritten directly by `specify-upload` because `openspec archive` preserves an
  existing Purpose. This change must be authored against that post-upload tree.
- The system suite starts one container per case, navigates by links and asserts only operator-observable behavior
  (`TechnicalDesign.md` § 5). `test/e2e/asset/uc031_read_asset_content.py` currently proves content retrieval end to
  end.

## Goals / Non-Goals

**Goals:**

- One owner per typed entity, with stable `REQ-*`/`SCN-*` identifiers.
- The entity contract states what the row is, what it measures, what may be absent, and how it is read.
- `library/assets` names only the serving contract it uniquely owns.
- One black-box proof per scenario in the system suite.

**Non-Goals:**

- Moving or renaming the `/asset/*` routes, handler modules, DTOs or OpenAPI tags.
- Specifying the asset scope, attachment rules or deletion cascades; `cover-art`, `gallery` and persistence own them.
- Re-authoring `library/assets`' remaining serving requirements or `library/media-ingest`.
- Changing the probe adapters or the measurements they report.

## Decisions

### 1. Three top-level capabilities, one per entity

A functional area is a cohesive externally observable behavior, not a folder mirror of the route tree. An image, an
audio and a video each have their own characteristics, absence rules and embedded-summary consumers, so each is one
capability. The routes stay under `/api/v1/asset`, the same way `upload` kept `/api/v1/asset/upload`.

**Alternative considered:** `library/assets/image`, `library/assets/audio`, `library/assets/video`. Rejected: it keeps
the serving area as the parent of three unrelated entity areas and contradicts the flat top-level organization the
redesign is following.

### 2. The entity owns its representation and file alias; `library/assets` keeps serving mechanics

Each entity spec owns its `GET /asset/{entity}/{id}` representation and its `/file` alias. `library/assets` keeps the
content routes, byte ranges, validators, the signed content credential and the outcome distinction, because all four
content routes share one serving path (`TechnicalDesign.md` § 3, Asset content retrieval) and duplicating that path in
three specs would let the copies drift.

The entity representation still carries `_links.content`; a **caller** reads the bytes under the serving contract, while
the behavior behind the link stays with `library/assets`.

**Alternative considered:** move each typed content route into its entity spec. Rejected: three copies of one range,
validator and credential contract, and a cross-entity rule that no single entity owns.

### 3. The asset scope is not an entity requirement

`image` and `video` store a nullable scope, but the scope is not in their representation; it becomes observable only
when a cover or a gallery item attaches the row. `cover-art` owns the cover side (`REQ-COVER-ART-003`) and `gallery`
owns the media side, so the entity specs state neither the field nor its immutability.

**Alternative considered:** state the scope as a characteristic of image and video. Rejected: a requirement with no
observable scenario would be a persistence note in a behavior spec, and it would duplicate the attachment rules.

### 4. The name is an entity characteristic; intake keeps choosing it

`REQ-IMAGE-002` and `REQ-AUDIO-002` state that the name is non-empty. `upload` keeps the rule that an uploaded row takes
the original file name, and `library/media-ingest` keeps the path-derived rule. The `video` row has no name, so the old
"an upload names its media row" requirement is removed rather than moved: it claimed a field the schema does not have.

This supersedes `specify-upload` Decision 2, which deliberately kept the naming rule in `library/assets` to avoid
inventing a requirement in an old-format spec. The typed capabilities are exactly the requirements that decision
deferred.

### 5. The video summary shape is owned by `video`

`REQ-VIDEO-005` states the subset a film or an episode embeds: identifier, backing file, codec, duration, width, height
and scan type, without the colour identifier or the frame count. `movies` and `series` reference this requirement
instead of restating the fields, so a changed summary has one owner.

**Alternative considered:** let each **catalogue** spec state the fields it embeds. Rejected: the two catalogue specs
and the video representation would drift independently.

## Risks / Trade-offs

- **The removals depend on `specify-upload`'s Purpose rewrite** → author and archive this change after `specify-upload`;
  if the two are integrated together, rebase the assets delta and the direct Purpose edit in one window.
- **The naming-rule removal reverses a decision another in-flight change recorded** → the proposal and design state the
  supersession; `specify-upload`'s impact text is amended in the integration window, or this change lands after it.
- **The naming-rule removal leaves the rule unstated, not moved** → the delta's migration names `upload` and
  `library/media-ingest` as the owners, but neither capability's spec states that a row takes the original uploaded name
  or the path-derived one, and `specify-upload` deliberately kept that rule in `library/assets` rather than inventing
  it. The migration therefore maps the two removed naming scenarios to a rule that does not yet exist; it lands with
  those capabilities' own changes (or an amended `specify-upload`), and the behavior stays implemented in
  `complete_upload` meanwhile.
- **`uc031` may lose cases the new system modules prove** → reduce the E2E module only after the system modules pass,
  and keep the content-retrieval cases that belong to `library/assets`.
- **A representation field could change without a spec scenario** → the system modules assert the full representation
  fields, so a removed or renamed field fails a labelled scenario.

## Migration Plan

1. Land `specify-upload` and its `library/assets` Purpose rewrite.
2. Apply this change; `openspec archive` retires the five requirements from `library/assets`.
3. Rebuild and regenerate the OpenAPI document if the build regenerates it; no document change is expected.
4. Rollback is the reverse archive; no data migration is involved.

## Open Questions

None. The audio-to-recording cardinality belongs to `music` and is not decided here.
