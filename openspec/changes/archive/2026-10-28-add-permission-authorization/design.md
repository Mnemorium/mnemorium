# Design

## Context

See `proposal.md` for motivation and `specs/` for the behavior contract. The current code splits authorization across
the authentication middleware, the use cases, `src/lib/domain/service/library_access.rs`, and the handler error
mappings. The use case is the application service that owns the unit of work (`TechnicalDesign.md` § 7); repositories
are transaction-scoped views exposed through context traits such as `IdentityUnitOfWork`
(`src/lib/domain/port/identity_unit_of_work.rs`). Domain services are pure and hold no I/O (`ARCH-005`), and the
application layer owns security events (`OBS-002`, `src/lib/application/security_event.rs`). The Identity bounded
context currently owns credentials; the User context owns user rows.

The permission store is new. It belongs to Identity because it holds authority data about a caller, while the action
catalog spans the Library, Asset and User contexts.

## Goals / Non-Goals

**Goals:**

- One pure decision service, one permission store, one action vocabulary, one denial envelope.
- Per-user overrides of an action's scope, with role presets as the default.
- The Root Admin as a universal allow, except for uploads.
- Uniform `401` / `403` / `404` outcomes and one `authz_failed` event per denial.
- Migrate every existing use case off `LibraryAccess` and inline role/ownership checks.
- A root-only permission administration API.

**Non-Goals:**

- No file-serving or streaming endpoints. The `file.read` action and the metadata-mirror rule are recorded for the later
  route; see Decision 11.
- No Admin permission editing and no escalation ceiling. Only the Root Admin edits permissions.
- No data-driven action catalog, custom roles, or permission caching.
- No change to `PERS-006` (root user row immutable) or `PERS-007` (default gallery undeletable).
- No new third-party dependency.

## Decisions

### 1. `Permission` is the aggregate root, one per `(user, action)`

`Permission` holds `user_id`, `action`, and `scope`; `Scope::Listed(Vec<ResourceRef>)` is the child value persisted in
`permission_resource`. The user's whole set is a collection of aggregates (`Vec<Permission>`), not a `PermissionSet`:
the only invariant spanning permissions is one scope per `(user, action)`, already enforced by the parent's unique key
on the pair.

Alternatives considered:

- A user-level `PermissionSet` root: adds a layer without a new invariant; rejected.
- A nullable `resource_id` on `permission`: turns `LISTED` into N rows, makes the PK NULL-heavy, and lets `ALL` and
  `LISTED` coexist for one action; rejected.
- A JSON list column: opaque to SQL and inconsistent with the schema conventions; rejected.

### 2. The store lives in Identity, keyed by `user_id`

`IdentityUnitOfWork` gains `permissions()`. The new `PermissionRepository` exposes `search`, `save`, `delete` and
`delete_all`. The `permission.user_id` foreign key crosses into the User context, so it declares no cascade (`ARCH-004`,
`PERS-020`); a future delete-user use case must delete the grants. The `permission_resource` child cascades from
`permission` because both are Identity-owned.

### 3. The decision is a pure domain service; the use case loads the inputs

`src/lib/domain/service/authorization.rs` exposes one method:

```rust
pub fn decide(
    caller: &User,
    permissions: &[Permission],
    action: Action,
    resource: &impl Protected,
) -> Decision;
```

`Decision` is `Allow` or `Deny(Denial { action, conceal })`. The use case loads the caller, the permissions and the
resource, calls `decide`, emits `security_event::authorization_failed` on denial, and maps `conceal` to its own
`NoSuchX` or `Forbidden` error. No `application/authorization.rs` exists and the service never receives a unit of work.

Alternatives considered:

- **An application service owning the permission repository** — rejected: repositories are transaction-scoped views
  (`SqlxPermissionRepository::new(&mut transaction)`), so a long-lived service cannot own one without a second
  transaction. Taking `&mut U` per call would make it a facade, not an owner, and would become a second use case.
- **A domain service with a repository** — rejected: it violates `ARCH-005` and makes the rules untestable without
  mocks. The permission repository stays a port read by the use case.
- **An application `AccessContext` loader/wrapper** — rejected by the requested layering. The repetition of the caller
  and permission loads is accepted, as it is today with `LibraryAccess` and `security_event`.

### 4. The action catalog is a closed enum; presets and supported scopes are code

`Action` names every authorizable operation. Each variant declares:

- its role presets (`role_scope(Role)`);
- the scopes it supports (`supported_scopes()`);
- whether it is configurable (`is_configurable()`);
- its root access (`RootAccess::All` or `OwnerOnly`);
- its event `verb()` and `target()`;
- its concealment rule.

Adding an action is a code change — enum variant, preset, scopes, resource kind, use-case call — not a schema change. A
missing preset or scope is a bug; the service fails closed with `Scope::None`, and a test iterates the catalog to assert
each action is fully described.

| Action             | Standard        | Admin           | Root  | Configurable | Target                    |
| ------------------ | --------------- | --------------- | ----- | ------------ | ------------------------- |
| `CatalogRead`      | ALL             | ALL             | All   | yes          | Movie, Series, MusicAlbum |
| `GalleryRead`      | PUBLIC_OR_OWNED | PUBLIC_OR_OWNED | All   | yes          | Gallery                   |
| `GalleryModerate`  | PUBLIC_OR_OWNED | PUBLIC_OR_OWNED | All   | yes          | Gallery                   |
| `GalleryManage`    | OWNED           | OWNED           | All   | yes          | Gallery                   |
| `PlaylistRead`     | PUBLIC_OR_OWNED | PUBLIC_OR_OWNED | All   | yes          | MusicPlaylist             |
| `PlaylistModerate` | PUBLIC_OR_OWNED | PUBLIC_OR_OWNED | All   | yes          | MusicPlaylist             |
| `PlaylistManage`   | OWNED           | OWNED           | All   | yes          | MusicPlaylist             |
| `FileRead`         | OWNED           | OWNED           | All   | yes          | File                      |
| `FileUse`          | OWNED           | OWNED           | All   | yes          | File                      |
| `UploadRead`       | OWNED           | OWNED           | OWNED | no           | Upload                    |
| `UploadWrite`      | OWNED           | OWNED           | OWNED | no           | Upload                    |
| `UserRead`         | NONE            | ALL             | All   | yes          | User / Global             |
| `UserUpdate`       | NONE            | ALL             | All   | yes          | User                      |
| `UserCreate`       | NONE            | ALL             | All   | yes          | Global                    |
| `UserCreateAdmin`  | NONE            | NONE            | All   | yes          | Global                    |
| `UserRoleChange`   | NONE            | NONE            | All   | yes          | User                      |
| `CredentialPatch`  | NONE            | NONE            | All   | yes          | Global                    |
| `PermissionManage` | NONE            | NONE            | All   | no           | Global                    |

The table is the default matrix. The Root column is the short-circuit; only the upload rows fall through to the preset.
`CatalogRead` uses one action for films, shows and albums; a `LISTED` scope names top-level resources, and a season,
episode, medium or track authorizes against its series or album.

### 5. Scopes are a five-arm algebra over `Protected`

```rust
pub trait Protected {
    fn resource_ref(&self) -> Option<ResourceRef>;
    fn owner_id(&self) -> Option<NumericID>;
    fn is_public(&self) -> bool;
}
```

`ALL` allows; `PUBLIC_OR_OWNED` allows when `is_public` or the owner matches; `OWNED` requires the owner; `LISTED`
requires the `(kind, id)` in the grant; `NONE` denies. `Global` is a unit `Protected` for collection and creation
actions. The domain models keep their existing `is_public`/owner accessors; `Gallery` and `MusicPlaylist` already expose
`is_owned_by`, and `File`/`Upload` expose `user_id`.

### 6. Root is a short-circuit, not a row

`decide` returns `Allow` before the scope lookup when the caller is the Root Admin and the action's root access is
`All`. The Root Admin has no `permission` rows: the permission-administration use case refuses the root target, the
database trigger aborts a row, and the policy ignores any row that somehow exists. `PERS-006` and `PERS-007` remain
state invariants: the root may attempt to modify its own user row or delete the default gallery, and the state refuses.

### 7. Uploads are fixed owner-only

`UploadRead` and `UploadWrite` have `RootAccess::OwnerOnly` and support only `OWNED`; they are not configurable. A
permission row naming them is rejected. The use cases still resolve the caller (a deleted caller is `401`), then pass an
empty permission slice and check ownership. The result is `404` for a non-owner, including the Root Admin.

### 8. Concealment is decided by the action

- Read denials (`CatalogRead`, `GalleryRead`, `PlaylistRead`, `FileRead`) and upload denials are concealed (`404`).
- `GalleryManage`/`PlaylistManage` conceal when the caller cannot read the resource, and reveal (`403`) when they can.
- `GalleryModerate`/`PlaylistModerate` follow the same read check.
- `UserRead`, `UserUpdate`, `UserCreate`, `UserCreateAdmin`, `UserRoleChange`, `CredentialPatch` and `PermissionManage`
  reveal (`403`). `FileUse` reveals because the file is a request-body value.
- The use case performs the read check first where the endpoint addresses a possibly invisible resource, then the
  operation check; the `Denial` carries the final `conceal` flag.

### 9. One event per denial, emitted by the use case

The domain service does not log. On `Decision::Deny`, the use case calls
`security_event::authorization_failed(caller.id(), denial.action().verb(), denial.action().target())` once and maps the
denial. A list filter calls `decide` and keeps `Allow` without emitting an event.

### 10. `LibraryAccess` is folded in

`LibraryAccess::can_read_gallery` and `can_read_playlist` become `GalleryRead` and `PlaylistRead` with
`PUBLIC_OR_OWNED`. The domain models keep `is_public`, `is_owned_by` and `is_root_admin`; the policy composes them. The
Glossary and `TechnicalDesign.md` § 2 stop naming `LibraryAccess` as the domain-service example.

### 11. File access mirrors the owning metadata

`FileRead` covers the raw asset route: owner or Root Admin. A file served through a library resource uses that
resource's action (`GalleryRead`, `PlaylistRead` or `CatalogRead`). The file route shape is out of scope; when it lands,
a contextual route uses the metadata action, and a generic route either stays owner-only or resolves the metadata. The
rule needs no new policy.

### 12. Permission administration is a root-only use-case pair set

Two use cases in `application/use_case/` (`get_permission`, `replace_permission`) and their ports back:

- `GET /api/v1/user/{user_id}/permission` — effective defaults plus overrides;
- `PUT /api/v1/user/{user_id}/permission` — replace the overrides atomically; an empty set resets the user to the role
  presets.

They live in the Identity factory because the repository is Identity, while the handler sits under the `/user` route and
resolves them through `state.identity_use_case_factory()`. `PermissionManage` is non-configurable: the Root Admin's
short-circuit allows it and every other role resolves to `NONE`. The target user is loaded through `UserUnitOfWork`; the
Root Admin target is refused; the request is validated by `Permission::try_new` and replaced in one unit of work. No
escalation ceiling is needed while only the root edits.

### 13. Persistence

```sql
CREATE TABLE permission (
    permission_id INTEGER,
    user_id       INTEGER NOT NULL,
    action_name   TEXT    NOT NULL,
    scope         TEXT    NOT NULL,
    CONSTRAINT pk_permission_permission_id PRIMARY KEY (permission_id),
    CONSTRAINT uq_permission_user_action UNIQUE (user_id, action_name),
    CONSTRAINT fk_permission_user FOREIGN KEY (user_id) REFERENCES user (user_id),
    CONSTRAINT chk_permission_scope CHECK (scope IN ('ALL','PUBLIC_OR_OWNED','OWNED','LISTED','NONE'))
);

CREATE TABLE permission_resource (
    permission_id INTEGER NOT NULL,
    resource_type TEXT    NOT NULL,
    resource_id   INTEGER NOT NULL,
    CONSTRAINT pk_permission_resource PRIMARY KEY (permission_id, resource_type, resource_id),
    CONSTRAINT fk_permission_resource_permission FOREIGN KEY (permission_id)
        REFERENCES permission (permission_id) ON DELETE CASCADE
);
```

`action_name` and `resource_type` are stored as text and parsed by the domain; an unknown value is
`RepositoryError::DataIntegrityViolation`, so adding an action or a resource kind stays a code change. `scope` has a
CHECK because its set is closed. A `BEFORE INSERT/UPDATE` trigger aborts a `permission` row with `user_id = 0`. The ER
diagram gains the two entities (`PERS-011`).

`Permission::try_new` enforces: supported scope for the action; non-empty, duplicate-free `LISTED`; and only supported
resource kinds per action. The root target is refused by the permission-administration use case and by the database
trigger, not by a model test.

### 14. Migration of use cases

Every authenticated use case loads the caller, then the permissions (except the fixed upload actions, which pass `&[]`),
then the resource, then calls `decide`. The `F::Uow` bound gains `IdentityUnitOfWork` where permissions are read. The
handler error maps keep the existing `NoSuchCaller` → `401`; `register_user` and `patch_credential` gain that variant
instead of returning `Forbidden` for a deleted caller. The gallery use cases replace `Forbidden` with `NoSuchGallery`
for a read denial. `update_playlist` composes `PlaylistManage` (metadata) and `PlaylistModerate` (tracks); a mixed
request is atomic and a non-owner's metadata field returns `403`. `add_gallery_item` composes `GalleryModerate` and
`FileUse`. `delete_gallery`/`delete_gallery_item` use `GalleryManage`/`GalleryModerate` and `FileUse` for bytes.

### 15. Testing

- Domain service: action-catalog completeness, preset matrix, each scope, root, uploads, concealment and relation guards
  (`UserUpdate` target rules). Pure tests, no mocks (`TEST-024`–`TEST-026`).
- `Permission` model: no dedicated test module (`TEST-019`); its validation is covered through the
  permission-administration use cases (`422`) and its `Protected` implementations through the domain-service matrix.
- Repository: in-memory SQLite for parent/child, cascade, unknown action/resource type, root trigger, replace.
- Use cases: add `MockPermissionRepository` to `TestUnitOfWork` in `src/lib/lib.rs`; extend the shared `expect_caller`
  helpers to expect the permission load; update existing tests; add gallery `404`, root manage, deleted caller `401`,
  upload non-owner `404`, catalog restrictions.
- Handlers: new permission routes; `401`/`403`/`404`/`422`.
- E2E: permission API round trip, root playlist/gallery flips, gallery concealment, catalog `NONE`/`LISTED`, upload root
  denial, grant to a Standard user (`user.create`).
- Coverage and the full gate.

### 16. Documentation

Add a dedicated `docs/Permissions.md` guide, linked from `mkdocs.yml` and `AGENTS.md`, covering:

- authentication versus authorization, and the two axes (administration by role preset, content by ownership and
  visibility);
- the action catalogue table (action, target, Standard/Admin presets, configurable, root exception);
- the five scopes with examples;
- the resolution order (root short-circuit, per-user override, role preset);
- the Root Admin rule and the upload exception, plus the `PERS-006`/`PERS-007` state invariants;
- denial semantics (`401`/`403`/`404`, concealment, one `authz_failed`);
- the permission administration API with `GET`/`PUT` examples and recipes (all content, owned only, one shared playlist,
  no catalog, reset through an empty replacement);
- the store shape (overrides only, role presets in code, role-change behaviour, no root rows);
- the extension checklist for a new action (enum, presets, scopes, root access, verb/target, resource kind, use-case
  call, tests) and the note that no migration is needed;
- the test split: domain-service tests, no model tests (`TEST-019`).

Glossary: **Action**, **Scope**, **Permission**, **Authorization service**; update the **Domain Service** example.
TechnicalDesign: the persistence rules for the two tables and the root trigger, the `authz_failed` vocabulary, and the
ER diagram. UseCases: a business use case for permission administration and the changed root/read rules. OpenAPI
regenerated. `Overview.md` gains the new files and drops `library_access.rs`.

## Risks / Trade-offs

- **Root ALL and gallery `404` are breaking** → update specs, OpenAPI, E2E and release notes in the same change.
- **Catalog reads can be hidden** → the modified catalogue specs state the default and the override; a `NONE` override
  intentionally empties the library for that user.
- **Two extra reads per request** (user + permissions) → acceptable at home scale; a cache is a later, measured
  optimisation with invalidation on every permission write.
- **Text action/resource kinds have no DB FK** → the adapter fails closed with `DataIntegrityViolation`; a catalog test
  keeps code and stored values aligned.
- **`LISTED` rows are polymorphic** → a deleted resource leaves an orphan grant that never matches; cleanup can follow
  the resource deletion later.
- **Cross-context user FK has no cascade** → a future delete-user use case must delete the grants; there is no
  delete-user use case today (`PERS-020`).
- **Overrides survive a role change** → a demoted user keeps an explicit grant until the Root Admin resets it; document
  it, and keep the empty replacement as the recovery.
- **Root-only administration** → no Admin delegation and no escalation ceiling yet; the `Action` catalog is ready for a
  later `permission.manage` grant with a ceiling.
- **File routes are not served** → the `file.read`/metadata rule is a design contract only; the route and its E2E arrive
  in a later change.

## Migration Plan

The change is additive: two new tables, their trigger and a new root-only API; existing users have no rows and keep the
role presets. No backfill is needed. The code and migration ship in the same release; the migration runs at startup
(`PERS-003`/`PERS-004`). Rollback is a revert of the code and a drop of the two tables/trigger; no existing table or row
changes. The permissions are empty after a rollback, so behavior returns to the role presets.

## Open Questions

- The exact permission-administration representation (field names, HAL links) can be settled during implementation
  without changing the specs or the store.
- The permission cache, Admin delegation and the `media.delete` action are explicitly deferred; none changes this design
  or the task breakdown.
