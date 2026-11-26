# Design

## Context

See `proposal.md` — Why. The current `authorization` spec mixes the engine with capability facts, and the recent
specification refactor interpreted the glossary's **Who-may rule** as assigning every who-may statement to
`authorization`. Capability specs already name the action each operation requires; the missing half is the resource fact
and the default grant, which still live in `authorization`.

Two constraints shape the change:

- **A requirement is the atomic unit.** An OpenSpec `MODIFIED` block must keep every existing scenario, so a scenario
  cannot be cherry-picked out of a mixed requirement. The specify-logging change retired and re-added
  `REQ-AUTHORIZATION-027`/`028` for exactly this reason. The change therefore moves whole requirements and rewords the
  mixed ones without dropping their scenarios.
- **The system suite is owned by capability.** `TEST-054` and the proof section of `TechnicalDesign.md` make one module
  per feature area the primary proof, and a case another suite takes over is deleted in the same change. Moving a
  scenario means moving or merging its case, not only re-annotating it.

## Goals / Non-Goals

**Goals:**

- Make each capability the owner of its resource facts and operation bindings.
- Keep `authorization` as the single owner of the resource model, decision order, grant vocabulary, permission API and
  denial semantics.
- Remove the duplicate and stale rules that let `REQ-AUTHORIZATION-026-A` disagree with `REQ-GALLERY-007-E`.
- Keep every observable behavior unchanged except the stale `403` correction.

**Non-Goals:**

- A new `catalogue`, `library` or asset capability.
- A new route, payload, status, header or schema.
- A change to the visibility model, role bundles, decision order or permission API.
- A change to the upload, user, catalogue or asset implementation.

## Decisions

### 1. Doctrine and application, not engine-only

**Decision:** `authorization` owns the meaning of a decision; a capability owns the binding between its resource or
operation and that decision. The capability states its resource's parent, owner, visibility default, creation surface,
default seed and the action each operation requires, except the shared asset chain and the closed model vocabularies the
engine keeps. It cites `authorization` for deny-wins, role bundles, visibility semantics and the decision order; it
never restates them.

**Why:** the capability already owns the operation and its tests, so it is the only place that can keep the binding
honest. The engine stays stable when a route or resource is added.

**Alternative considered:** reduce `authorization` to the engine and move every resource fact, including the tree and
the special cases, into capabilities, adding a `catalogue`/`library` capability for the shared containers. Rejected: it
reintroduces the layer the `finish-spec-refactor` change removed, and it makes the canonical tree unreadable as one
contract.

**Alternative considered:** keep the current split and repoint capability specs at `authorization`. Rejected: it is the
state that produced the missing gallery seed and visibility mapping and the stale gallery `403`.

### 2. Move whole requirements; reword the mixed ones

**Decision:** retire and re-add the wholly capability-specific requirements: `010`, `018`–`022`, `024` and `026`. Keep
the mixed requirements `002`, `012`, `015` and `023` in `authorization`; reword `002`, `012` and `023` so the general
rule stays and the capability declares its instance, and keep every scenario as an illustrative example. Leave `003`'s
relation vocabulary, `015`'s decision order and `025`'s asset chain intact, and correct only `029`'s reason list.

**Why:** the wholly specific requirements move cleanly as whole blocks. The mixed requirements carry the engine's
general rule, and their scenarios demonstrate it; retiring them would renumber many stable scenarios and detach the
examples from the rule. The scenarios name a gallery, album or upload as examples, not as additional obligations.

**Alternative considered:** retire and re-add every mixed requirement to remove its examples. Rejected for this change:
it doubles the scenario churn and makes the authorization spec abstract without adding a behavior.

### 3. The asset chain stays with the engine

**Decision:** `REQ-AUTHORIZATION-025` stays in `authorization`. Its parent chain spans images, videos, audio and files
and attaches to gallery items, films, episodes and recordings, so it is a property of the authorization model rather
than of one capability's operation. The typed-asset capabilities keep their existing read requirements.

The same rule gives the other capabilities their own top-level edge: `upload` and `user` follow the instance root,
`music` follows the library for a playlist and the catalogue for a catalogue resource, and `movies` and `series` follow
the catalogue. The relation kinds stay in `REQ-AUTHORIZATION-003` as a closed model vocabulary, like the action catalog;
they are not an operation binding.

**Why:** moving the chain would scatter one cross-cutting parent rule across four capabilities. It would also codify the
catalogue-cover scenario as an image requirement, while the current attachment code only reassigns the gallery-media
parent; this change must not turn an unverified claim into four capability contracts.

**Alternative considered:** move `025` to `image`, `video`, `audio` and `file`. Rejected: it adds four deltas,
duplicates the fallback, and asserts a catalogue-cover parent the code does not record.

### 4. The catalogue facts are split across movies, series and music

**Decision:** each catalogue capability states that its resources are unowned catalogue children and which default
creation grant applies. The general ownership rule stays in `REQ-AUTHORIZATION-012`.

**Why:** the catalogue is a container, but the resource kinds and their default grants belong to the feature that serves
them. A new `catalogue` capability would own no operation and would duplicate the existing feature boundaries.

**Alternative considered:** one `catalogue` capability. Rejected: it adds a capability with no route or use case of its
own and recreates the retired `library/*` layer.

### 5. Test cases move with their scenarios

**Decision:** move each moved scenario's black-box case to the owning capability's suite and delete the replaced case
from `test/system/authorization.py`. Merge a case into an existing target case when the target already proves the same
behavior. Update the Rust `// proves:` identifier when the scenario moves; keep the Rust case where it proves the engine
mechanism.

**Why:** the proof section already assigns each capability's scenarios to its own suite, and `TEST-054` forbids one
failure being reported twice. The authorization suite remains the proof of the engine: grant algebra, decision order,
visibility, explanation, permission API and denial semantics.

**Alternative considered:** keep every case in `authorization.py` and only change its annotation. Rejected: it leaves
the authorization suite as the primary proof of capability behavior it no longer owns, and the next reader cannot tell
which suite is authoritative.

### 6. The stale gallery conflict resolves toward the capability

**Decision:** `REQ-AUTHORIZATION-026-A` is not carried forward. A gallery item naming a missing or unowned medium
answers `422`, as `REQ-GALLERY-007-E`, `REQ-API-013` and `post_gallery_item.rs` already state.

**Why:** the authorization requirement's `403` was the duplicate and the drift. Removing it changes no code and makes
the contract agree with the API rule.

**Alternative considered:** change the gallery requirement to `403`. Rejected: it would change behavior and contradict
the code and `REQ-API-013`.

### 7. The glossary states the ownership boundary

**Decision:** redefine the glossary's **Who-may rule** as the binding between an operation and the action it requires on
a resource, owned by the capability that owns the operation; `authorization` owns what holding the action means.

**Why:** the previous definition was read as assigning every rule to `authorization`. The change makes the intended
reading normative. `docs/Permissions.md` keeps the canonical tree picture, and the proof section of `TechnicalDesign.md`
names the capability suites as the proofs of their own bindings.

**Alternative considered:** leave the glossary untouched and rely on the design. Rejected: the glossary is the canonical
vocabulary, and every future spec author reads it before the archive.

## Risks / Trade-offs

- **The tree becomes distributed** → `docs/Permissions.md` § Resource tree remains the one canonical picture, and
  `REQ-AUTHORIZATION-002` points at it; each capability states only its own edge.
- **Capabilities could restate the engine** → the glossary's **Who-may rule** and the design's doctrine/application
  split make the boundary explicit; a capability that restates deny-wins or role bundles is a review defect.
- **Requirement identifiers change** → every moved scenario is re-annotated and the traceability report is regenerated;
  no identifier is reused for a different meaning.
- **The test move is larger than the spec edit** → the tasks map each moved scenario to its owning suite, and an
  overlapping target case is merged rather than duplicated.
- **A reviewer reads the change as drift** → the proposal and design record that it reverses the deferred
  `specify-authorization` decision and why.
- **The upload creation surface has an unused seed** → `begin_upload` does not decide `CreateChild` on
  `UPLOAD_COLLECTION` today. This change does not add that check; `upload` states only the owner-only rule and the
  instance-root parent, and `REQ-AUTHORIZATION-023` is narrowed to a capability that actually decides `CreateChild`. The
  unused seed is a pre-existing gap outside this contract move.

## Migration Plan

1. Apply the `authorization` delta: modify `002`, `012`, `023` and `029`; remove `010`, `018`–`022`, `024` and `026`.
2. Apply the seven capability deltas: upload, user, movies, series, music, cover-art and gallery.
3. Update the glossary's **Who-may rule**, the `user` and `upload` purposes, `docs/Permissions.md` and the proof section
   of `docs/development/TechnicalDesign.md`.
4. Move or merge the black-box cases and update the Rust `// proves:` comments.
5. Regenerate `docs/development/artifacts/RequirementsTraceabilityCoverage.html` and run `openspec validate --specs`,
   `script/investigation.sh --check`, `markdownlint-cli2 '**/*.md'` and `prettier --check '**/*.md'`.
6. Archive the change. No runtime migration, deployment step or rollback is required; the server behavior is unchanged.

## Open Questions

None. The scope, the ownership boundary and the conflict resolution are decided above.
