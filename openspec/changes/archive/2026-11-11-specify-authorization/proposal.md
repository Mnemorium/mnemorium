# Proposal

## Why

The active `specify-authorization` change assumes the scope engine: one per-user override of an action's scope, resolved
against a role preset. That model is a dead end for the product:

- It cannot share one **resource** with one user without inventing a `LISTED` scope per action family.
- It cannot express a **deny**, so “share this album but hide one track” has no honest representation.
- It cannot separate **containment** from **authorization**, so a recording in several playlists has no canonical
  permission parent.
- It cannot return an **explanation**, so “why can this user not see this?” has no answer beyond the effective scope.
- Its action names encode the resource family (`gallery.read`, `movie.manage`), so a generic permission endpoint and a
  role bundle are impossible without duplicating every verb.
- It has no **role** between a user and an action; a user either has a per-action override or a role preset.

The change is unimplemented (no task is checked), the project is pre-release, and migrations are mutable. Rewriting it
in place is cheaper and more honest than landing the scope contract and replacing it next.

## What Changes

- **One resource graph.** Every authorizable entity — instance, library, catalogue, user, upload, gallery, gallery item,
  film, show, season, episode, music group, album, medium, recording, image, video, audio and file — becomes a
  **resource** with a kind, an optional owner, one **authorization parent** and a visibility. `AlbumMember`,
  `PlaylistEntry` and `GroupMember` become **resource relations** that order and contain and lend read access, but never
  write, share or admin authority. Galleries and playlists stay user content under the library; the catalogue is the
  shared, unowned container for films, shows, albums and music groups.
- **One grant model.** A **grant** names a principal (user, role or root), a resource, a **resource role**, a **child
  kind** and an effect (`Allow` or `Deny`). **Deny wins** over every allow, whatever its source. The parent chain is
  single, acyclic and capped at 32.
- **One role vocabulary.** `Viewer`, `Downloader`, `Contributor`, `Editor`, `Manager` and `Owner` are bundles of a fixed
  action catalog: `Discover`, `ViewMetadata`, `View`, `Stream`, `Download`, `CreateChild`, `EditMetadata`,
  `ReplaceMedia`, `Share`, `ManagePermissions`, `Delete`, `ChangeRole` and `ReplaceCredential`. A `CreateChild` grant
  carries an explicit `ChildKind`. `Owner` is implicit from ownership and is never a user grant; `ManagePermissions` is
  never grantable.
- **Visibility as a grant.** `Private`, `Restricted` and `Public` add zero, metadata-only and viewer grants to
  authenticated callers. No tier implies `Download`, `EditMetadata`, `Share` or `Delete`, and visibility is not
  inherited.
- **Root, uploads and accounts.** The root is an implicit owner and cannot be denied; an upload is owner-only for every
  caller, including the root. Only the root creates an Admin, changes a role or replaces a credential; an Admin creates
  and manages Standard accounts; a non-root caller cannot touch an Admin; the root account is immutable.
- **Permission administration under `/identity`.** The only permission routes are
  `GET /api/v1/identity/permission?user_id=…` (effective summary and explanation),
  `GET|PUT /api/v1/identity/permission/resource/{kind}/{id}` (read and replace a resource policy) and
  `POST|DELETE /api/v1/identity/permission/resource/{kind}/{id}/share[/{grant_id}]` (add or revoke read grants). No
  permission route lives under `/user`, `/library` or `/asset`.
- **A clean migration.** Drop `permission` and `permission_resource`, create `resource`, `resource_relation` and
  `grant`, backfill one resource row per entity, seed the role grants, and translate no override.
- **A republished capability.** `identity/authorization` is removed and re-added with stable `REQ-*`/`SCN-*`
  identifiers, one requirement per rule and one scenario per observable outcome.
- **Deferred feature-spec cleanup.** The eight `library/*` deltas from the old change are removed. They cite
  `movie.read`, `music.manage` and requirement identifiers that no longer exist; the feature-spec repointing follows
  once the engine vocabulary is stable.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `identity/authorization`: republished in full against the resource-and-grant engine. The thirteen existing
  requirements are removed; the new contract states the resource graph, the relation split, principals, the action
  catalog, role bundles, grants, deny-wins, resolution order, visibility, ownership, root and uploads, account
  administration, content and asset decisions, denials, filtering, explanation, the three permission endpoints, grant
  validation, transaction shape and the migration.

## Impact

- **Specs**: one delta, `identity/authorization`. The eight `library/*` deltas and their glossary references are removed
  from this change and will be re-proposed.
- **Code**: `src/lib/domain/model/permission.rs` is replaced by the resource, relation, principal, action, role and
  grant models; `src/lib/domain/service/authorization.rs` is rewritten around the chain and the grant algebra;
  `src/lib/domain/port/permission_repository.rs` and its SQLx adapter become resource and grant repositories; the old
  permission use cases and REST handlers are replaced by the three identity routes; approximately 148 call sites in
  `src/lib/application/use_case/` move from `Authorization::decide(action, scope)` to `authorize(resource, action)`.
- **Persistence**: one migration drops the two permission tables, creates three tables, backfills the resource graph and
  seeds the role grants. No override is translated.
- **Tests**: `test/system/authorization.py` is the primary proof and covers the published scenarios black box. The
  `Authorization` domain service's pure test module is the fallback for the grant algebra, the traversal bound, the
  relation lend, the root/owner/upload special cases and the explanation; application tests cover the models and the
  denial mapping only where the container cannot; repository integration tests cover the migration, the constraints and
  the single read transaction. No test module is added under `domain/model/` (`TEST-019`). The replaced E2E cases and
  the Rust tests that duplicate a system scenario are cleared once the system suite is green.
- **Docs**: `docs/Permissions.md` is rewritten around resources, grants and the identity endpoints, and carries the
  canonical PlantUML tree diagram; `docs/development/Glossary.md` gains the resource, grant and role terms and drops the
  scope-only ones; `docs/development/TechnicalDesign.md` § 5 names `authorization.py` and records the one-transaction
  rule.
- **Risk**: the engine touches every authorizable use case and the API surface at once. The migration test and the
  system suite are the guardrails; the feature-spec cleanup is deliberately out of scope so the review reads one
  contract, not nine.
