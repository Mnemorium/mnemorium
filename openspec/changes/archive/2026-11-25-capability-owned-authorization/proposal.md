# Proposal

## Why

The `authorization` capability has become the owner of rules that belong to the resources and operations of other
capabilities: upload access, account administration, catalogue ownership and cross-resource reference checks all live
there, while the capability that serves the operation states only the action it requires. That split leaves capability
specifications incomplete. Gallery, for example, never states who may create a gallery by default and never states what
its `is_public` flag means, although both facts live in `authorization`; and `REQ-GALLERY-008` is the only gallery
requirement that names a route, because it explains authorization rather than access.

The split also lets rules drift. `REQ-AUTHORIZATION-026-A` says a gallery item whose image the caller may not view
answers `403`, while `REQ-GALLERY-007-E` says `422`; the code answers `422`, which is the status `REQ-API-013` assigns
to a body-named resource. Removing the duplicate authorization requirement resolves the conflict in favour of the code
and the gallery contract.

The recent specification refactor deferred this cleanup and read the glossary's **Who-may rule** as assigning every
who-may statement to `authorization`. This change makes the other reading explicit: the capability that owns a resource
or operation owns the binding between that operation and its action, and `authorization` owns the engine that gives the
action its meaning.

## What Changes

- Move the wholly capability-specific requirements out of `authorization` into the capabilities that own the resource or
  operation:
  - `REQ-AUTHORIZATION-010` (`ChangeRole` is root-only) → `user`.
  - `REQ-AUTHORIZATION-018` (an upload is owner-only) → `upload`.
  - `REQ-AUTHORIZATION-019` to `-021` (account administration, an Admin's grants, the own-account read) → `user`.
  - `REQ-AUTHORIZATION-022` and `-024` (catalogue ownership and content operations) → `movies`, `series` and `music`.
  - `REQ-AUTHORIZATION-026` (referencing content needs the container and the target) → `gallery`, `music` and
    `cover-art`; the generic all-of rule stays as `REQ-AUTHORIZATION-040`.
- Keep the engine's general requirements in `authorization`. Reword `002`, `012` and `023` so the general rule stays and
  the capability declares its instance; keep every scenario as an illustrative example. Leave `003`'s relation
  vocabulary, `015`'s decision order and `025`'s asset chain intact, as the engine's closed model vocabulary and model
  rules; the typed-asset capabilities keep their read requirements.
- Correct `REQ-AUTHORIZATION-029`'s reason list to include the `upload` and `own_account` reasons the engine already
  returns.
- Add the missing capability-owned facts:
  - `gallery`: its parent is the library, its items are resources under it, `is_public` selects visibility, and a
    Standard User and an Admin create galleries by default.
  - `upload`: an upload is owner-only and is a resource under the instance root.
  - `user`: account administration, the root-only role change, an Admin's bounded grant administration, the own-account
    read, and the account's instance-root parent.
  - `movies`, `series`, `music`: their resources are unowned catalogue children and their default creation grants.
  - `cover-art`: naming a cover needs the write action and `View` on the image, concealed as a conflict when it refuses.
- Remove the route literal from `REQ-GALLERY-008`; the requirement states the access, not the URL. It is the only
  gallery requirement that names a route, while the rest of the gallery contract leaves navigation to the representation
  links.
- State the ownership rule in the glossary's **Who-may rule** and in `docs/Permissions.md` and the proof section of
  `docs/development/TechnicalDesign.md`.
- No runtime behavior changes. The only contract correction is the stale `403` in `REQ-AUTHORIZATION-026-A`, replaced by
  the `422` that `REQ-GALLERY-007-E`, the code and `REQ-API-013` already state.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `authorization`: eight capability-specific requirements are removed, three mixed requirements are restated as general
  rules, the explanation reason list is corrected, and the generic second-resource rule remains. The relation
  vocabulary, decision order and asset chain stay as the engine's model rules.
- `upload`: owns the upload owner-only rule and the upload's resource parent.
- `user`: owns account administration, the root-only role change, the bounded grant administration, the own-account
  read, and the account's instance-root parent.
- `movies`: owns the film's unowned catalogue resource and its default creation grant.
- `series`: owns the show's unowned catalogue resource and its default creation grant.
- `music`: owns the unowned catalogue music resources, the default playlist and catalogue creation grants, and the
  playlist-entry reference check.
- `cover-art`: owns the cover-and-image reference check.
- `gallery`: states its resource parent, its item parent, its visibility mapping, its default creation grant, and reads
  one item without naming a route.

## Impact

- **Specs**: eight capability deltas; no new capability.
- **API**: no route, payload, status, header or schema change. The gallery item read keeps its route, discovered through
  the representation's `self` link as `REQ-API-004` and `REQ-API-005` already require.
- **Code**: no production behavior change. Comments that cite moved requirement identifiers are updated.
- **Tests**: the scenarios that move are re-annotated in their owning suite; the replaced authorization cases are
  deleted, and overlapping cases in the target suites are merged rather than duplicated. The traceability report is
  regenerated.
- **Docs**: `docs/development/Glossary.md` redefines the **Who-may rule**; `docs/Permissions.md` and the proof section
  of `docs/development/TechnicalDesign.md` point at the capability-owned bindings.
- **Persistence, configuration and dependencies**: unchanged.
