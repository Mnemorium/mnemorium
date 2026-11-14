# Design

## Context

The active `specify-authorization` change was written against the scope engine: one per-user override of an action's
scope (`ALL`, `PUBLIC_OR_OWNED`, `OWNED`, `LISTED`, `NONE`), resolved against the caller's role preset. That model
cannot express a per-resource share, a deny, a role bundle, an explanation, or a containment edge that is not also a
permission edge. This rewrite replaces the engine rather than completing the old contract.

The current state:

- `src/lib/domain/service/authorization.rs` decides one action against one resource by resolving a scope and calling
  `Scope::covers`; `src/lib/domain/model/permission.rs` declares the twenty actions, their presets, scope sets, root
  access, concealment and event vocabulary.
- `permission` stores `(user_id, action_name, scope)` and `permission_resource` stores the `LISTED` aggregates.
- Approximately 148 call sites invoke `Authorization::decide` or one of its surface helpers.
- `music_track` is a placement row with either a medium or a playlist parent, `music_album_group` is many-to-many, and
  `gallery_item` is a placement. The current model has no way to distinguish containment from authorization.
- `Movie`, `Series`, `MusicAlbum` and `MusicGroup` are unowned; `library.manage` deliberately has no `OWNED` scope.
- The base `identity/authorization` specification has thirteen requirements; the active delta republishes forty-five.

## Goals / Non-Goals

**Goals:**

- One **resource graph** for every authorizable entity, with a single canonical **authorization parent** and separate
  containment/reference edges.
- One **decision service** that resolves grants, ownership, role, visibility and root in a fixed order, with **deny
  wins** and an explanation.
- A stable **action catalog** and **resource role** bundles that replace the action-per-family and scope model.
- Resource-scoped permission administration under `/api/v1/identity`, with no permission route elsewhere.
- A clean migration that drops the old override data and backfills the resource graph.

**Non-Goals:**

- No anonymous access. `Public` applies to authenticated callers until an anonymous route exists.
- No time-limited grants. `expires_at` is deliberately absent and a later change may add it with a migration.
- No ownership transfer. `Owner` is implicit from `owner_id`; a later change may add `TransferOwnership`.
- No dynamic roles. `Standard` and `Admin` remain built in; `Root` is a principal kind, not a role.
- No feature-spec who-may cleanup in this change. The eight `library/*` deltas are removed and will be re-proposed once
  the action vocabulary is stable.
- No cache. One read transaction per request is the starting point; a cache is a measured follow-up.

## Decisions

### 1. Model one resource graph and keep containment out of the auth-parent chain

**Decision:** every authorizable entity is a row in `resource` with a kind, an optional owner, an optional
`auth_parent_id` and a visibility. `AlbumMember`, `PlaylistEntry` and `GroupMember` are rows in `resource_relation`.
Only `auth_parent_id` inherits a full role. A relation lends `Discover`, `ViewMetadata`, `View` and `Stream` from its
container to its target and the target's authorization-parent descendants, and nothing else.

**Why:** the current model cannot tell a playlist reference from a permission parent, so a track in ten playlists would
inherit ten permission parents. Separating the edges keeps a playlist an ordinary reference for writing while letting a
playlist viewer listen to the recordings it contains. A related recording therefore never grants `Download`, a write, a
share or an admin action, and a `Deny` on the recording still wins.

**Alternative considered:** make every relation inert. Rejected by the stakeholder: a user with access to a playlist's
content must be able to listen to the recordings in it.

**Alternative considered:** one relation table for all edges, with `AuthParent` as a kind. Rejected as the storage
shape: the decision needs exactly one parent per resource, and a nullable column plus a partial unique constraint makes
that cheaper and harder to violate than a filtered relation table.

### 2. Keep the authorization parent single, acyclic and bounded

**Decision:** `resource.auth_parent_id` is nullable and holds at most one parent. A write rejects a cycle and a chain
deeper than 32. The use case performs the check in the same transaction as the assignment.

**Why:** a single parent makes the chain deterministic; the bound keeps a malformed graph from turning one decision into
an unbounded walk; a cycle would otherwise loop or silently truncate.

**Alternative considered:** a DAG with several parents. Rejected by the stakeholder: one canonical authorization parent
is the rule, and ordinary content references cover the many-to-many cases.

### 3. Separate principals, roles and ownership

**Decision:** a grant names a `User`, a `Role` or the `Root`. `Standard` and `Admin` remain the only roles. `Root` is a
principal kind, not a stored role. Role grants are seeded rows on the instance and library roots and are immutable over
HTTP. `Owner` is implicit from the nearest non-null `owner_id` up the chain and is never stored as a user grant; a
seeded role grant may carry it.

**Why:** a user grant, a role-wide default and a break-glass principal are three different things; collapsing them would
make “why does this user see this?” unanswerable. Keeping ownership implicit means one row to change on transfer and no
way to grant “owner” by accident.

**Alternative considered:** store `Owner` as a grant on every resource. Rejected: a transfer would rewrite a subtree and
an `Owner` grant could be forged through the permission API.

### 4. Replace the per-family actions with one generic catalog

**Decision:** the catalog is `Discover`, `ViewMetadata`, `View`, `Stream`, `Download`, `CreateChild`, `EditMetadata`,
`ReplaceMedia`, `Share`, `ManagePermissions`, `Delete`, `ChangeRole` and `ReplaceCredential`. A grant carries an
explicit `ChildKind`, which matters only when its role includes `CreateChild`; `ChildKind::Admin` is root-only and not
grantable.

**Why:** `gallery.read`, `playlist.moderate` and `movie.manage` encoded the resource family in the action name, which
made a generic resource-scoped grant impossible and duplicated the same verb across families. A generic catalog plus a
role bundle says the same thing once, and `ChildKind` recovers the one distinction creation needs.

**Alternative considered:** keep the per-family actions and attach grants to them. Rejected: a grant would then have to
name both an action family and a resource, reintroducing the scope model.

### 5. Deny wins and the order is fixed

**Decision:** the decision applies upload owner-only, then the root, then the own-account read, then any matching
`Deny`, then any matching `Allow` from a grant, owner, role, visibility or a relation's read lend, and otherwise denies.
A deny on an ancestor defeats a descendant allow; a descendant deny defeats an inherited allow.

**Why:** “any matching forbid produces Deny; otherwise a matching permit produces Allow; otherwise Deny” is the rule the
stakeholder chose. It makes an exception readable and a concealment reliable.

**Alternative considered:** nearest-wins. Rejected: it makes a parent deny easy to defeat and an inherited grant hard to
explain.

### 6. Treat visibility as a grant with three tiers

**Decision:** `Private` adds nothing; `Restricted` grants `Discover` and `ViewMetadata` to authenticated callers;
`Public` grants `Viewer` to authenticated callers. Visibility is not inherited, and no tier implies `Download`,
`EditMetadata`, `Share` or `Delete`.

**Why:** visibility is the coarse, resource-level grant; a grant is the precise one. Keeping visibility out of the
inheritance chain lets a `Private` child sit under a `Public` parent without a second policy system.

**Alternative considered:** keep `Authenticated` and `Public` as separate tiers. Rejected by the stakeholder: with no
anonymous route the two are observationally identical, so `Authenticated` is dropped and `Public` is kept for the future
anonymous surface.

### 7. Resolve the owner up the chain

**Decision:** the implicit `Owner` is the nearest non-null `owner_id` on the addressed resource or its ancestors. The
owner's `Owner` bundle reaches the resource and its descendants. Catalogue resources have no owner.

**Why:** a gallery owner should reach the gallery's items without a row per item, and an ownership transfer should be
one update. The catalogue is shared, so it has no owner to resolve.

**Alternative considered:** copy `owner_id` onto every child. Rejected: a transfer would rewrite a subtree and a
backfill would have to guess.

### 8. Keep uploads owner-only and the root un-deniable, in that order

**Decision:** an upload is decided owner-only before the root short-circuit, so even the root cannot read another
caller's upload; no grant may name an upload. The root is implicit `Owner` everywhere else and bypasses every deny.

**Why:** an upload is an unfinished file with no content policy to inherit; owner-only is the only safe rule. The root
is the break-glass account and must not be lockable, but the upload exception is a deliberate product rule.

**Alternative considered:** let the root bypass uploads. Rejected by the stakeholder: the current owner-only rule is
kept for every caller.

### 9. Keep account administration root/admin and bounded

**Decision:** only the root creates an Admin account, `ChangeRole` and `ReplaceCredential` are root-only, an Admin
creates and manages Standard accounts, a non-root caller cannot touch an Admin, and the root account is immutable. An
Admin may replace a Standard user's grants because it administers that account. `ManagePermissions` is not a grantable
role; the Admin preset carries it on `User` and `Library`.

**Why:** the Admin role administers the instance's content and Standard accounts, but peer-Admin editing and role
escalation are root-only. Keeping `ManagePermissions` out of the grantable set prevents a manager from manufacturing an
owner or an admin.

**Alternative considered:** let an Admin manage another Admin. Rejected by the stakeholder.

### 10. The permission API lives under `/identity`

**Decision:** the only permission routes are:

- `GET /api/v1/identity/permission?user_id={id}` — the effective summary, with optional `resource_kind`, `resource_id`
  and `action` for the explanation.
- `GET|PUT /api/v1/identity/permission/resource/{kind}/{id}` — read the resource policy; replace its visibility and
  complete grant set.
- `POST|DELETE /api/v1/identity/permission/resource/{kind}/{id}/share[/{grant_id}]` — add or revoke a `Viewer` or
  `Downloader` grant.

`GET` on a resource needs `Share` or `ManagePermissions`; `PUT` needs `ManagePermissions`; `POST`/`DELETE share` needs
`Share` or `ManagePermissions`. The user summary is self or `ManagePermissions` on that `User`.

**Why:** authorization is an identity concern; one namespace makes the boundary obvious and removes the old split
between `/user/{id}/permission` and the domain routers. A separate share route keeps a `Share`-only caller from
replacing a grant set it cannot see.

**Alternative considered:** keep the route under `/user`. Rejected by the stakeholder: no permission endpoint outside
`/identity`.

### 11. Return an explanation and read in one transaction

**Decision:** `authorize()` returns a decision with the allowed flag, the reason, the matching grants, the denying
grant, the authorization path and the visibility. A request loads the resource, its chain and the matching grants in one
read transaction; a storage error denies.

**Why:** “why can this user not see this?” is the first question a support report asks. A single read keeps the
explanation consistent with the decision and avoids a per-row query in a listing.

**Alternative considered:** log only the action and target. Rejected: it cannot name the denying grant or the path.

### 12. Drop the old override data and backfill the resource graph

**Decision:** the migration drops `permission` and `permission_resource`, creates `resource`, `resource_relation` and
`grant`, backfills one resource row per existing entity, seeds the role grants, and translates no override.

**Why:** `gallery.read: ALL` has no faithful grant equivalent; translating it would either widen access to every content
kind or invent a role that does not exist. The project is pre-release, so discarding the overrides is the honest
migration.

**Alternative considered:** translate each scope into grants. Rejected: the result would look authoritative while
changing meaning.

### 13. Defer the feature-spec who-may cleanup

**Decision:** this change modifies only `identity/authorization`. The eight `library/*` deltas in the old change are
removed; the feature specs keep their behavior and will be repointed at the generic actions in a follow-up change.

**Why:** the old deltas cite `movie.read`, `music.manage` and requirement IDs that no longer exist. Keeping them would
put stale action names into the archive. Separating the engine from the prose cleanup keeps this change reviewable and
lets the engine land first.

**Alternative considered:** update every library delta mechanically. Rejected for this change: it doubles the review
surface and mixes a behavior change with a vocabulary change.

### 14. Prove the engine black box, with the test level the rulebook assigns

**Decision:** `test/system/authorization.py` is the primary proof and proves the published scenarios through the
container. The `Authorization` domain service keeps its own pure test module in `authorization.rs` for the grant
algebra, the traversal bound, the relation lend, the owner/root/upload special cases and the explanation. Application
tests cover the models through the use cases and the denial mapping only where the system suite cannot observe the
outcome; repository integration tests cover the migration, the constraints and the single read transaction. No test
module lives under `src/lib/domain/model/` (`TEST-019`), and no Rust handler test module is added. A Rust test is kept
only for a mechanism the container cannot observe or a case the harness cannot reach yet; when a system scenario proves
the same outcome, the Rust test is omitted, and any existing duplicate is deleted once the system suite is green. The
wire contract is not tested here at all: a declared status, content type, body shape, response header or
undeclared-field rejection stays with Schemathesis, so no task asserts one. Every test carries `epic=Authorization`,
`feature=REQ-AUTHORIZATION-NNN` and the `SCN-AUTHORIZATION-NNN-X` stories it proves.

**Why:** `TechnicalDesign.md` § 5 gives each layer its own proof, and the system suite is the one that observes the
behavior the product promises. The domain service is the fallback for the algebra the API deliberately does not expose;
the application and repository tests are fallbacks for the model, the constraints and the transaction. The duplicate
sweep keeps one primary proof per scenario.

**Alternative considered:** put the model tests in a `domain/model/` test module. Rejected by `TEST-019`: the domain
models have no dedicated test, and the rule enforcer rejects one.

### 15. Seed the role grants, including a per-user Admin grant

**Decision:** the migration seeds the `Standard` role with `Viewer` on `Catalogue`, `Contributor` with
`ChildKind::Gallery` on `GalleryCollection`, `ChildKind::MusicPlaylist` on `PlaylistCollection`, and `ChildKind::Upload`
on `UploadCollection`. It seeds the `Admin` role with `Owner` on `Catalogue`, `Contributor` with `ChildKind::Gallery`
and `ChildKind::MusicPlaylist` on the two collection surfaces, `Contributor` with `ChildKind::User` on `UserCollection`,
and `Owner` on each `User` resource. User creation adds the Admin `Owner` grant for the new `User`. `Owner` is rejected
for a user principal but allowed as a seeded role grant; `ManagePermissions` is not a resource role and remains
owner/root only.

**Why:** the `Owner` bundle is the only bundle that carries `ManagePermissions`, so the Admin preset must use it to
administer the catalogue and each account. The collection surfaces let `Contributor` grant creation without its `Viewer`
bundle leaking onto the created content: the surface has no content children, and the created child's authorization
parent is the surface's parent. The `Catalogue` container is the other addition to the tree as first sketched: without
it, an Admin `Owner` grant on `Library` would reach every user's gallery and playlist, contradicting owner autonomy.
Galleries and playlists stay user content under `Library`; the catalogue is the shared, unowned half. A per-user grant
avoids giving the Admin role a blanket `Owner` on `Instance`; the account relation guard still refuses Admin and Root
targets. The seed is data, so the migration and the user-creation use case own it.

**Alternative considered:** hard-code the Admin capability in the resolver. Rejected: the engine would then have a role
special case that the specification cannot state as a grant.

## Data model

Three tables carry the engine. The migration creates them in this order, then backfills and seeds.

### `resource`

One row per authorizable entity. `entity_id` is the identifier of the row in its own table; `(kind, entity_id)` is the
external address the API uses.

```sql
CREATE TABLE resource (
    resource_id INTEGER,
    kind TEXT NOT NULL,
    entity_id INTEGER NOT NULL,
    owner_id INTEGER,
    auth_parent_id INTEGER,
    visibility TEXT NOT NULL DEFAULT 'PRIVATE',
    CONSTRAINT pk_resource_resource_id PRIMARY KEY (resource_id),
    CONSTRAINT uq_resource_kind_entity_id UNIQUE (kind, entity_id),
    CONSTRAINT fk_resource_owner FOREIGN KEY (owner_id)
        REFERENCES user (user_id) ON DELETE CASCADE,
    CONSTRAINT fk_resource_auth_parent FOREIGN KEY (auth_parent_id)
        REFERENCES resource (resource_id) ON DELETE CASCADE,
    CONSTRAINT chk_resource_kind CHECK (kind IN (
        'INSTANCE', 'LIBRARY', 'CATALOGUE', 'USER', 'UPLOAD',
        'GALLERY', 'GALLERY_ITEM', 'MOVIE', 'SERIES', 'SEASON', 'EPISODE',
        'MUSIC_GROUP', 'MUSIC_ALBUM', 'MUSIC_MEDIUM', 'MUSIC_RECORDING',
        'MUSIC_PLAYLIST', 'IMAGE', 'VIDEO', 'AUDIO', 'FILE',
        'GALLERY_COLLECTION', 'PLAYLIST_COLLECTION', 'UPLOAD_COLLECTION',
        'USER_COLLECTION'
    )),
    CONSTRAINT chk_resource_visibility CHECK (
        visibility IN ('PRIVATE', 'RESTRICTED', 'PUBLIC')
    ),
    CONSTRAINT chk_resource_no_self_parent CHECK (
        auth_parent_id IS NULL OR auth_parent_id <> resource_id
    ),
    CONSTRAINT chk_resource_user_has_no_owner CHECK (
        kind <> 'USER' OR owner_id IS NULL
    ),
    CONSTRAINT chk_resource_catalogue_has_no_owner CHECK (
        kind NOT IN (
            'CATALOGUE', 'MOVIE', 'SERIES', 'SEASON', 'EPISODE',
            'MUSIC_GROUP', 'MUSIC_ALBUM', 'MUSIC_MEDIUM', 'MUSIC_RECORDING'
        ) OR owner_id IS NULL
    )
);
```

`GALLERY_COLLECTION`, `PLAYLIST_COLLECTION`, `UPLOAD_COLLECTION` and `USER_COLLECTION` are **creation surfaces**: a
permission anchor for `CreateChild` with no content children of their own. A grant of `Contributor` on a surface carries
the `Viewer` bundle on the surface (harmless, since the surface has no children) and `CreateChild` for one `ChildKind`.
The created child's authorization parent is the surface's authorization parent, **not** the surface, so the surface
never becomes a permission parent of user content.

### `resource_relation`

The containment and reference edges. They lend read access and never inherit a full role.

```sql
CREATE TABLE resource_relation (
    from_resource_id INTEGER NOT NULL,
    to_resource_id INTEGER NOT NULL,
    kind TEXT NOT NULL,
    position INTEGER,
    CONSTRAINT pk_resource_relation PRIMARY KEY (
        from_resource_id, to_resource_id, kind
    ),
    CONSTRAINT fk_resource_relation_from FOREIGN KEY (from_resource_id)
        REFERENCES resource (resource_id) ON DELETE CASCADE,
    CONSTRAINT fk_resource_relation_to FOREIGN KEY (to_resource_id)
        REFERENCES resource (resource_id) ON DELETE CASCADE,
    CONSTRAINT chk_resource_relation_kind CHECK (
        kind IN ('ALBUM_MEMBER', 'PLAYLIST_ENTRY', 'GROUP_MEMBER')
    ),
    CONSTRAINT chk_resource_relation_position CHECK (
        position IS NULL OR position >= 0
    ),
    CONSTRAINT chk_resource_relation_no_self CHECK (
        from_resource_id <> to_resource_id
    )
);

CREATE UNIQUE INDEX uq_resource_relation_position
ON resource_relation (from_resource_id, kind, position)
WHERE position IS NOT NULL;

CREATE INDEX idx_resource_relation_to
ON resource_relation (to_resource_id);
```

### `grant`

One row per grant. `principal_kind` is `USER` or `ROLE`; the root is implicit and is never stored. A `USER` grant names
a `user_id`; a `ROLE` grant names the role instead, using the same `ADMIN`/`STANDARD` vocabulary as `user.role`.

```sql
CREATE TABLE grant (
    grant_id INTEGER,
    principal_kind TEXT NOT NULL,
    principal_id INTEGER,
    principal_role VARCHAR(50),
    resource_id INTEGER NOT NULL,
    role TEXT NOT NULL,
    child_kind TEXT NOT NULL DEFAULT 'ALL',
    effect TEXT NOT NULL,
    CONSTRAINT pk_grant_grant_id PRIMARY KEY (grant_id),
    CONSTRAINT fk_grant_resource FOREIGN KEY (resource_id)
        REFERENCES resource (resource_id) ON DELETE CASCADE,
    CONSTRAINT chk_grant_principal_kind CHECK (
        principal_kind IN ('USER', 'ROLE')
    ),
    CONSTRAINT chk_grant_principal CHECK (
        (
            principal_kind = 'USER'
            AND principal_id > 0
            AND principal_role IS NULL
        )
        OR (
            principal_kind = 'ROLE'
            AND principal_id IS NULL
            AND principal_role IN ('ADMIN', 'STANDARD')
        )
    ),
    CONSTRAINT chk_grant_role CHECK (
        role IN ('VIEWER', 'DOWNLOADER', 'CONTRIBUTOR', 'EDITOR', 'MANAGER', 'OWNER')
    ),
    CONSTRAINT chk_grant_owner_is_a_role CHECK (
        role <> 'OWNER' OR principal_kind = 'ROLE'
    ),
    CONSTRAINT chk_grant_child_kind CHECK (child_kind IN (
        'ALL', 'GALLERY', 'MUSIC_PLAYLIST', 'UPLOAD', 'USER', 'ADMIN',
        'MOVIE', 'SERIES', 'SEASON', 'EPISODE', 'MUSIC_GROUP',
        'MUSIC_ALBUM', 'MUSIC_MEDIUM', 'MUSIC_RECORDING', 'GALLERY_ITEM',
        'IMAGE', 'VIDEO', 'AUDIO', 'FILE'
    )),
    CONSTRAINT chk_grant_admin_is_not_grantable CHECK (child_kind <> 'ADMIN'),
    CONSTRAINT chk_grant_effect CHECK (effect IN ('ALLOW', 'DENY'))
);

CREATE UNIQUE INDEX uq_grant_user
ON grant (principal_id, resource_id, role, child_kind, effect)
WHERE principal_kind = 'USER';

CREATE UNIQUE INDEX uq_grant_role
ON grant (principal_role, resource_id, role, child_kind, effect)
WHERE principal_kind = 'ROLE';

CREATE INDEX idx_grant_resource ON grant (resource_id);
```

Two partial unique indexes replace one composite unique constraint because SQLite treats `NULL` values as distinct: a
single `(principal_kind, principal_id, principal_role, …)` constraint would let a role grant repeat.

The `Owner` role is allowed only for a role principal, so a seeded Admin role grant can carry it while a user grant
cannot. `ChildKind::Admin` is rejected outright, so only the root's implicit policy creates an Admin.

### Backfill and seed

1. Create the `Instance`, `Library` and `Catalogue` singletons and the four collection surfaces.
2. Backfill one `resource` row per existing entity: galleries and playlists with `owner_id` and `visibility` from
   `is_public`; catalogue resources unowned and `PUBLIC`; users and uploads `PRIVATE`; assets parented to their typed
   owner or to the upload; files parented to the typed asset.
3. Backfill `resource_relation`: `ALBUM_MEMBER` from each album through its medium tracks, `PLAYLIST_ENTRY` from each
   playlist track, `GROUP_MEMBER` from `music_album_group`.
4. Seed `Standard`: `Viewer` on `Catalogue`; `Contributor` with `ChildKind::Gallery` on `GalleryCollection`,
   `ChildKind::MusicPlaylist` on `PlaylistCollection`, and `ChildKind::Upload` on `UploadCollection`.
5. Seed `Admin`: `Owner` on `Catalogue`; `Contributor` with `ChildKind::Gallery` and `ChildKind::MusicPlaylist` on the
   two collection surfaces, and `ChildKind::User` on `UserCollection`; `Owner` on every existing `User` resource.
6. Add the Admin `Owner` grant for a `User` when the account is created.
7. Drop `permission` and `permission_resource`.

## Risks / Trade-offs

- **The migration is large** → it drops two tables and backfills one resource row per entity. A seeded database must
  migrate with every entity resolvable and every user on the seeded role. The task list proves it with a migration test
  before the seed is rewritten.
- **The action rename is a wire change** → `permission.action` disappears and the new `grant.role` vocabulary replaces
  it. No compatibility shim: the project is pre-release and the old data is dropped.
- **The permission API is a hard change** → the old `/user/{id}/permission` route disappears. Clients are updated in the
  same change; there is no deprecation window.
- **A relation lends read access** → a private recording becomes listenable through any playlist that contains it. That
  is the intended sharing behavior; to protect one recording, put a `Deny` on it, because `Private` visibility alone
  cannot override the lend.
- **The generic actions lose per-family clarity** → `View` on a gallery and `View` on an album mean the same thing by
  design. The role bundle and the resource kind carry the difference; the spec states the action once instead of once
  per family.
- **Four creation surfaces add rows for no content** → they exist only so a `Contributor` grant has a stable anchor. The
  alternative is a `CreateChild` check with no resource to decide against; the surfaces keep every decision
  resource-scoped and never appear in the tree.
- **The feature specs still restate who-may rules** → this change deliberately leaves them; the follow-up change owns
  the cleanup, and the owning capability is complete before it starts.
- **No cache** → every decision reads the chain. Listings read the caller's grants once and filter the page; a cache is
  a measured follow-up, not a silent omission.
- **`Public` is currently authenticated-only** → the name anticipates anonymous access that does not exist. The spec
  says so explicitly; adding anonymous access is a separate change.

## Migration Plan

The DDL and the backfill/seed order are in [Data model](#data-model). The plan adds only the parts the DDL cannot show:

1. One `.up.sql`/`.down.sql` pair under `migrations/` (`PERS-002`), applied by `sqlx::migrate!` at startup.
2. The `.down.sql` restores `permission` and `permission_resource` empty, drops the three new tables, and drops the
   seeded singletons and surfaces. No old override row is restored: it was discarded by design.
3. The entity-relationship diagram in `docs/development/TechnicalDesign.md` § Persistence is updated in the same change
   (`PERS-011`), and the diagram uses only the declared markers (`PERS-012`, `PERS-013`).
4. Seed data in the `test-seed` feature and the test fixtures move to the new tables in the same change.

Archive order: this change is self-contained. The eight `library/*` deltas are removed from the change, so no other
capability is mid-republish.
