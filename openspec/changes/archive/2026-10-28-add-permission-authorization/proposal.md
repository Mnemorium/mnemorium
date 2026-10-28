# Proposal

## Why

Authorization is spread across use cases, `LibraryAccess`, and handler error mappings, so the same rule is written in
several dialects. The result is inconsistent — a private gallery answers `403` while a private playlist answers `404`, a
deleted caller answers `403` on some endpoints and `401` on others, and uploads do not resolve the caller at all — and
it is not configurable per user. The product needs one permission model that can grant a user all library content, only
what they own, or a specific list of aggregates, and that can grow with new actions such as deleting media.

## What Changes

- Add a **`Permission`** aggregate in the Identity bounded context, persisted in `permission` and `permission_resource`,
  holding per-user overrides of an action's scope: `ALL`, `PUBLIC_OR_OWNED`, `OWNED`, `LISTED` (specific aggregates), or
  `NONE`. Role presets remain in code.
- Add a pure **`Authorization`** domain service with one `decide(caller, permissions, action, resource)` method. Use
  cases load the caller, the caller's permissions and the resource, then call it, emit `authz_failed`, and map the
  denial. The domain service never receives a unit of work or a repository.
- Remove `LibraryAccess` and every inline `Role::Admin`, `is_root_admin`, visibility and ownership check from the use
  cases; all of them become actions in one catalog.
- **Root Admin is allowed every action except upload read/write/complete**, which stay owner-only for every role and
  cannot be granted. Root is a short-circuit, not a preset row.
- **BREAKING**: the Root Admin may now rename or delete another user's playlist and delete another user's gallery.
- **BREAKING**: an unreadable gallery answers `404 Not Found` instead of `403 Forbidden`, matching playlists, uploads
  and `API-044`.
- **BREAKING**: catalog reads (films, shows, albums) follow the `catalog.read` permission. The default still shows
  everything to every authenticated caller, but an explicit per-user override can narrow the visible set.
- Resolve the caller on every authenticated use case, including uploads, so a token naming a deleted account answers
  `401 Unauthorized` everywhere.
- Add a permission administration API at `/api/v1/user/{user_id}/permission` — `GET` effective and overridden
  permissions and `PUT` replace the overrides (an empty set resets them to the role presets). It is root-only in this
  change and refuses to store permissions for the Root Admin.
- Define the `file.read` action and the rule that a file served through a library resource uses that resource's action.
  File-serving endpoints are out of scope for this change; the rule is recorded so the later route needs no new policy.
- Update the affected capability specs, `docs/openapi.json`, and the E2E suite.

## Capabilities

### New Capabilities

- `identity/authorization`: the permission aggregate and store, the action catalog, scope resolution, Root Admin and
  upload rules, denial concealment, and the permission administration API.

### Modified Capabilities

- `library/movies`: reads follow `catalog.read`; a per-user override may narrow the visible films and a denied film
  answers `404`.
- `library/series`: the same rule for series, seasons and episodes.
- `library/music-albums`: the same rule for albums, mediums and tracks.
- `library/music-playlists`: the Root Admin may manage any playlist, and reads and moderation follow the playlist
  permissions.
- `user/account-creation`: creation is governed by `user.create` and `user.create_admin`, with the current Admin/Root
  defaults preserved.

## Impact

- **Domain**: new `domain/model/permission.rs` (`Permission`, `Action`, `Scope`, `ResourceRef`, `Protected`), new
  `domain/service/authorization.rs`; remove `domain/service/library_access.rs`.
- **Ports**: new `domain/port/permission_repository.rs`; add `permissions()` to `IdentityUnitOfWork`.
- **Persistence**: new migrations for `permission` and `permission_resource`, their constraints and the Root Admin guard
  trigger; update the entity-relationship diagram (`PERS-011`).
- **Application**: every authenticated use case changes its context bounds and authority checks; new use cases for
  reading and replacing permissions.
- **REST**: new `/api/v1/user/{user_id}/permission` routes, handlers and OpenAPI schemas; updated error mappings and
  route descriptions. `docs/openapi.json` is regenerated.
- **Tests**: new domain-service, repository, use-case and E2E coverage; the root-denied playlist/gallery E2E cases flip,
  the gallery private-read cases move to `404`, and the catalog read restriction is exercised.
- **Documentation**: a dedicated `docs/Permissions.md` guide (concepts, action/scope table, resolution, root/upload
  rules, denial semantics, API recipes and extension checklist), its top-level `mkdocs.yml` navigation entry and
  `AGENTS.md` link, Glossary terms (Action, Scope, Permission, Authorization service), the new persistence rules in
  `docs/development/TechnicalDesign.md`, and a business use case for permission administration in
  `docs/development/UseCases.md`.
- **Dependencies**: none added.
