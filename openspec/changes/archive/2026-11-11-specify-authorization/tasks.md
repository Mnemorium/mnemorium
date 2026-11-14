# Tasks

One section per area, in dependency order. Work test-first: write the failing test, see it fail for the expected reason,
then implement until it passes.

Test levels follow `docs/development/TechnicalDesign.md` § 5:

- No test module lives under `src/lib/domain/model/` (`TEST-019`). The resource, principal, grant, action and role
  models are covered by **application tests** through the use cases that construct and read them.
- The authorization engine is a **pure domain service** in `src/lib/domain/service/authorization.rs` (`ARCH-005`,
  `TEST-024`–`TEST-026`). It receives the caller, the resource, its authorization-parent chain, the matching grants and
  the visibility as parameters, imports only `crate::domain::model`, holds no I/O and no port, and keeps its own test
  module in the same file with no mock.
- The SQLx adapter is covered by **repository integration tests** (`TEST-027`–`TEST-030`).
- REST handlers are covered by the **system/E2E** suite and Schemathesis; no Rust handler test module is added.
- The Rust test tasks in sections 1–9 are fallbacks: use one only for a mechanism the container cannot observe or a case
  the harness cannot reach yet. When a 10.x scenario proves the same outcome, the system test is the primary proof, the
  Rust test is not written, and any existing duplicate is deleted in 10.10.

Every test carries the three Allure labels: `epic=Authorization`, `feature=REQ-AUTHORIZATION-NNN` and one `story` per
`SCN-AUTHORIZATION-NNN-X` it proves. **The system suite is the primary proof.** A behavior the container can observe is
proven in `test/system/authorization.py`, not in a Rust test. A Rust test is written only when the container cannot
observe the mechanism — the grant algebra, the traversal bound, the explanation interior, a repository constraint or the
transaction — or when the system harness cannot reach the case yet; in that second case, say why in the task, and delete
the Rust test once the system case exists.

**Schemathesis owns the wire contract; no task re-asserts it.** A declared status (`200`, `201`, `204`, `400`, `401`,
`403`, `404`, `409`, `422`), the content type, the body shape or schema, a response header and the rejection of an
undeclared field all stay with Schemathesis. A task that writes a test for one of those is removed: the system suite
proves only the behavior behind a response (which resource a caller may address, and that a refusal changes nothing),
and the application and domain tests prove only the decision beneath it. If a task would assert a status code, rewrite
it to assert the decision or the stored state instead.

## 0. Foundations

- [x] 0.1 Update `docs/development/Glossary.md`: add **Resource**, **Resource kind**, **Authorization parent**,
      **Catalogue**, **Resource relation**, **Principal**, **Grant**, **Resource role**, **Creation surface** and
      **Explanation**; redefine **Owner**, **Visibility** and **Action**; remove the scope-only terms (`Scope`,
      `Permission`, `Permission override`, `Role preset`, `Surface action`, `Aggregate`). Verify:
      `markdownlint-cli2 '**/*.md'` and `prettier --check '**/*.md'` pass and no new term has a second definition.
- [x] 0.2 Write the migration design in `docs/development/TechnicalDesign.md` § 4: the three tables, the unique
      constraints, the `auth_parent_id` cycle/depth check and the backfill order. Verify: the persistence rules name the
      new tables and the § 4 review has no open question.
- [x] 0.3 Add the system-suite section to `docs/development/TechnicalDesign.md` § 5: `authorization.py` is the second
      feature module and one container may serve scenarios that share a fixture. Verify: the section names the module
      and the Markdown gates pass.
- [x] 0.4 Record the test-level split in `docs/development/TechnicalDesign.md` § 5: the engine is a domain service, the
      models are covered by application tests, the adapter by repository integration tests, and `authorization.py` by
      the system suite. Verify: the section names each level and the Markdown gates pass.
- [x] 0.5 Write the **call-site map** before any use case changes: one row per use-case file under
      `src/lib/application/use_case/`, giving the resource kind it addresses, the action (or the composed pair), the
      `ChildKind` where it creates, the error variant it answers on a denial, and the task that owns the change. The map
      is the scope definition for sections 4, 5, 7 and 9. Verify: a search for
      `Authorization::decide|decide_asset|decide_library_surface|decide_library_manage_surface` under
      `src/lib/application` returns one row per file, and the map is checked into the change.

## 1. The resource graph — `REQ-AUTHORIZATION-001` to `-004`

- [x] 1.1 Write the **application tests** for the resource model through the use cases that create and read a resource:
      a stored resource resolves its own facts; a client claim is ignored; a season follows its series, an episode its
      season, a recording its medium, a film its catalogue, a catalogue the library, an asset its owner; a relation
      lends read access only and never becomes an authorization parent. Do not add a test module under `domain/model/`
      (`TEST-019`). Verify: the tests fail against the old permission model.
- [x] 1.2 Implement `src/lib/domain/model/resource.rs` with the resource, kind, relation and visibility types and the
      tree helpers, with no `#[cfg(test)]` module. Verify: the 1.1 tests pass and
      `script/rule_enforcer.sh check TEST-019` passes.
- [ ] 1.3 Write the **application tests** for assigning an authorization parent (single parent, cycle, 33rd edge
      rejected) and the **repository integration tests** for the unique constraint and the depth check. Verify: they
      fail because no `auth_parent_id` exists yet.
- [ ] 1.4 Implement the assignment check in the resource use case and the datastore constraint. Verify: the 1.3 tests
      pass and a failed assignment stores nothing.

## 2. Principals, actions and grants — `REQ-AUTHORIZATION-005`, `-011` to `-013`, `-034`, `-035`

- [x] 2.1 Write the **application tests** for grant construction and validation through the permission use cases: the
      principal kind, the grantable-role set, the `Allow`/`Deny` effect, and the rejection of `Owner`,
      `ManagePermissions`, `ChildKind::Admin`, a root principal and a duplicate. Verify: they fail against the old
      `Permission` aggregate.
- [x] 2.2 Implement `src/lib/domain/model/grant.rs` and `src/lib/domain/model/principal.rs`, with no test module.
      Verify: the 2.1 tests pass and `TEST-019` is clean.
- [x] 2.3 Add the action catalog and the six role-bundle cases to the **engine's domain-service test module** in
      `authorization.rs`: every action round-trips through its stored name; `Viewer`, `Downloader`, `Contributor`,
      `Editor`, `Manager` and `Owner` contain the documented actions. Do not add a test module under `domain/model/`.
      Verify: the tests pass and need no mock.
- [x] 2.4 Implement `src/lib/domain/model/action.rs` and `src/lib/domain/model/role.rs` with the thirteen actions, their
      stored names and the six bundles. Verify: the 2.3 tests pass.

## 3. The decision engine — `REQ-AUTHORIZATION-014` to `-018`, `-025`, `-029`, `-036`

- [x] 3.1 Write the **domain-service tests** in `authorization.rs` for the resolution order: upload owner-only first,
      then root, then own-account, then any deny, then any allow from grant/owner/role/visibility/relation lend, then
      default deny. Verify: they fail against `Authorization::decide`.
- [x] 3.2 Rewrite `src/lib/domain/service/authorization.rs` as a pure domain service around
      `authorize(caller, resource, chain, grants, visibility, action, child_kind)`. It SHALL import only
      `crate::domain::model`, hold no I/O and no port, and keep its test module in the same file. Verify: the 3.1 tests
      pass, `ARCH-005` passes, and the file imports no `domain::port` or `infrastructure`.
- [x] 3.3 Write the **domain-service tests** for deny-wins at the same resource, on an ancestor and on a descendant.
      Verify: they fail before the deny pass exists.
- [x] 3.4 Implement the deny pass and the explanation type (`allowed`, `reason`, `matched`, `denied_by`, `auth_path`,
      `visibility`). Verify: the 3.3 tests pass and every decision returns an explanation.
- [ ] 3.5 Write the **domain-service tests** for the 32-edge walk, the relation read lend (playlist, album and group),
      the asset chain and the orphan-asset rule. Verify: they fail before the chain helper lands.
- [x] 3.6 Implement the chain helper, the relation read lend and the asset decision. Verify: the 3.5 tests pass.
- [x] 3.7 Write the **application tests** for the single read transaction and default deny on a storage error, and the
      **repository integration test** that reads the resource, its chain and the matching grants atomically. Verify:
      they fail against the old repository shape.
- [x] 3.8 Implement the resource-and-grant read port, the SQLx adapter and the application wiring. Verify: the 3.7 tests
      pass.
- [x] 3.9 Confirm the engine needs no mock: `authorization.rs` has no `mockall`/`automock`, no double and no I/O.
      Verify: `script/rule_enforcer.sh check TEST-026` passes.

## 4. Ownership, root, uploads and accounts — `REQ-AUTHORIZATION-012`, `-017`, `-018`, `-019` to `-021`

- [x] 4.1 Write the **domain-service tests** for nearest-owner resolution, the implicit `Owner` bundle, catalogue
      ownerlessness, the root bypass, the upload owner-only rule and the own-account read. Verify: they fail against the
      old role-preset rules.
- [x] 4.2 Implement the owner resolution, root, upload and self-read special cases in the authorization service. Verify:
      the 4.1 tests pass.
- [ ] 4.3 Write the **application tests** for the account rules: only the root creates an Admin, an Admin creates and
      manages Standard accounts, a non-root caller cannot touch an Admin, `ChangeRole` and `ReplaceCredential` are
      root-only, the root account is immutable, and an upload grant is refused. Verify: they fail against the old
      relation guard.
- [ ] 4.4 Implement the account rules in the user use cases and the upload-grant refusal in the permission use case.
      Verify: the 4.3 tests pass.

## 5. Content, creation and assets — `REQ-AUTHORIZATION-022` to `-026`

- [ ] 5.1 Write the **application tests** for catalogue reads and writes under the `Catalogue` container, `CreateChild`
      with `ChildKind`, the Standard creation set (gallery, playlist, upload), the Admin creation set (every content
      kind, a Standard account) and root-only Admin creation. Verify: they fail against the old surface helpers.
- [ ] 5.2 Implement `CreateChild`, `ChildKind` and the seeded creation grants. Verify: the 5.1 tests pass.
- [ ] 5.3 Write the **application tests** for the content operations: `View`/`Stream` reads,
      `EditMetadata`/`ReplaceMedia`/`Delete` writes, and child inheritance from a series, an album and a medium. Verify:
      they fail against `library.*`.
- [ ] 5.4 Repoint the catalogue use cases to the generic actions. Verify: the 5.3 tests pass.
- [ ] 5.5 Write the **application tests** for referencing content: a gallery item needs `CreateChild` on the gallery and
      `View` on the image; a playlist entry needs `CreateChild` on the playlist and `ViewMetadata` on the recording; one
      refusal applies nothing. Verify: they fail against `file.use` and `playlist.moderate`.
- [ ] 5.6 Implement the composed checks. Verify: the 5.5 tests pass.

## 6. Denials, filtering and the explanation — `REQ-AUTHORIZATION-027` to `-030`

- [ ] 6.1 Write the **application tests** for the denial mapping: a denial returns the use case's concealing error
      variant, applies nothing and emits exactly one `authz_failed` event; plus the silent listing filter and the all-of
      composition. Verify: they fail against the old denial mapping. Assert the variant and the stored state, never a
      status code: the `404`/`403` declaration is Schemathesis's and 10.7 decides which case yields which.
- [ ] 6.2 Implement the denial rendering, the one `authz_failed` event and the listing filter. Verify: the 6.1 tests
      pass.
- [ ] 6.3 Write the **domain-service tests** for the explanation shape and the **application test** that the summary an
      ordinary caller receives omits the grant internals. Verify: they fail before the explanation is threaded through.
      Assert the projected decision, not the JSON: the wire schema is Schemathesis's.
- [ ] 6.4 Thread the explanation through the application layer and the log event. Verify: the 6.3 tests pass.

## 7. The permission API — `REQ-AUTHORIZATION-031` to `-035`

- [x] 7.1 Write the **application tests** for the permission-summary use case: self, an Admin with `ManagePermissions`
      on a Standard `User`, a refused non-admin, and the explanation query. Verify: they fail because the use case does
      not exist. The wire is 10.8's and Schemathesis's, not this task's.
- [x] 7.2 Implement the handler, use case and OpenAPI declaration. Verify: 7.1 passes and the served spec carries the
      route.
- [x] 7.3 Write the **application tests** for the resource-policy use cases: a `Share` holder reads, an owner replaces,
      a `Manager` without `ManagePermissions` is refused, and a rejected replacement leaves the previous set.
- [x] 7.4 Implement the resource policy handler, use case and OpenAPI declaration. Verify: 7.3 passes.
- [x] 7.5 Write the **application tests** for the share use cases: a `Manager` shares a `Viewer`, a `Share` holder
      cannot escalate, and a share is revoked.
- [x] 7.6 Implement the share handler, use case and OpenAPI declaration. Verify: 7.5 passes.
- [x] 7.7 Write the **application tests** for grant validation: a role principal, `Owner`, `ManagePermissions`,
      `ChildKind::Admin`, a duplicate, an unknown principal and an unknown resource. Verify: they fail. The refusal's
      `422` is Schemathesis's; assert the validation error, not the status.
- [x] 7.8 Implement the validation. Verify: 7.7 passes and a refused grant stores nothing.
- [x] 7.9 Delete the old `GET|PUT /api/v1/user/{user_id}/permission` route, its handlers, use cases and OpenAPI
      declarations. Verify: a search for `permission` under `handler/user` returns nothing and the served spec no longer
      declares the path.

## 8. Migration and seeding — `REQ-AUTHORIZATION-037`

- [x] 8.1 Write the **repository integration test** for the migration: seed a database with a gallery, a playlist, a
      film, a show, an album, a recording, an image, a file, an upload and two users; run the migration; assert one
      resource row per entity, the tree parents, the seeded role grants and no row from the old override tables. Verify:
      the test fails before the migration exists.
- [x] 8.2 Write the migration `.up.sql`/`.down.sql` pair under `migrations/`, following the DDL in `design.md` § Data
      model: create `resource`, `resource_relation` and `grant` with their indexes and constraints, backfill the
      resource graph, seed the role grants and drop `permission` and `permission_resource`. Verify: 8.1 passes and the
      rollback restores the old tables.
- [x] 8.3 Update the entity-relationship diagram in `docs/development/TechnicalDesign.md` § Persistence with the three
      new entities and their markers, and remove `permission` and `permission_resource`. Verify: `PERS-011`–`PERS-013`
      pass and the diagram matches the migration.
- [x] 8.4 Rewrite the seed data and the test fixtures to the new tables. Verify: a seeded run starts and
      `GET /api/v1/identity/permission?user_id=0` reports the seeded role.
- [x] 8.5 Seed the role grants in the migration and in the user-creation use case: `Standard` gets `Viewer` on
      `Catalogue`, `Contributor` with `ChildKind::Gallery` on `GalleryCollection`, `ChildKind::MusicPlaylist` on
      `PlaylistCollection`, and `ChildKind::Upload` on `UploadCollection`; `Admin` gets `Owner` on `Catalogue`,
      `Contributor` with `ChildKind::Gallery` and `ChildKind::MusicPlaylist` on the two collection surfaces,
      `Contributor` with `ChildKind::User` on `UserCollection`, and `Owner` on each `User`. User creation adds the Admin
      `Owner` grant for the new `User`. Verify: a new Standard user has the Standard grants, a new Admin has the Admin
      grants, and an Admin reads a Standard account but not another Admin's.

## 9. Use-case call sites

- [x] 9.1 Write the **application test** for a denied call: it returns the concealing error variant the `0.5` map names,
      never a distinguishing forbidden variant, emits exactly one `authz_failed` event carrying the explanation, and
      applies nothing. Verify: the test fails against the old `Decision::Deny` rendering.
- [x] 9.2 Implement the one **application-side authorizer** every use case calls. It takes the caller, the resource
      address, the action and the optional `ChildKind`, loads the resource, its chain and the matching grants in the
      caller's transaction, calls the domain engine, logs the event and maps the denial. Verify: 9.1 passes and it is
      the only place under `src/lib/application` that logs `authz_failed`.
- [x] 9.3 Replace `IdentityUnitOfWork::permissions()` with the resource-and-grant accessor and update `TestUnitOfWork`,
      the mock repositories and the shared fixtures under `src/lib/test_helpers`. Verify: `cargo build` passes and no
      `PermissionRepository`, `PermissionFilter` or `Permission` remains under `src/lib`.
- [x] 9.4 Repoint the **reads** from the `0.5` map: every gallery, playlist, film, show, season, episode, album, medium,
      recording, group and asset read. Rewrite each use case's test module to the resource fixtures in the same step.
      Verify: each use case's tests fail before its change and pass after.
- [x] 9.5 Repoint the **writes**: every create, metadata update, media replacement and delete.
- [x] 9.6 Repoint **creation**: `CreateChild` with its `ChildKind` on the creation surface, and the created child's
      authorization parent set to the surface's parent, never the surface.
- [x] 9.7 Repoint the **composed checks** through the 9.2 authorizer: a gallery item needs `CreateChild` on the gallery
      and `View` on the image; a playlist entry needs `CreateChild` on the playlist and `ViewMetadata` on the recording.
- [x] 9.8 Repoint the **administration** use cases: `register_user`, `get_user`, `list_users`, `update_user`,
      `delete_user`, `patch_credential`, `get_permission` and `replace_permission`.
- [x] 9.9 Verify: the `0.5` map has no row left unassigned or open, a search under `src/lib/application` returns no
      `Authorization::decide`, `decide_asset`, `decide_library_surface` or `decide_library_manage_surface`, and every
      repointed use case's test module uses the resource fixtures rather than a `Permission` list.

## 10. The system suite — the primary proof — `REQ-AUTHORIZATION-001` to `-037`

- [x] 10.1 Create `test/system/authorization.py` with the shared fixtures (`root_token`, `provision_user`, `grant`,
      `create_gallery`, `create_playlist`, `begin_upload`) promoted from `test/e2e/conftest.py`, reusing the `harness`
      plugin. Verify: one smoke test reads `GET /api/v1/identity/permission?user_id=0` and passes.
- [x] 10.2 Prove the resource graph and the relation split: a playlist entry lends listen but not edit, an album member
      lends read, a group member lends read, and a deny on the target still refuses. Verify: the scenarios pass.
- [x] 10.3 Prove the grant algebra: a user grant, a role grant, deny-wins at the same resource, an ancestor deny and a
      descendant deny. Verify: the scenarios pass.
- [x] 10.4 Prove visibility: private, restricted metadata, public viewer, no download, no inheritance and a deny over
      visibility. Verify: the scenarios pass.
- [x] 10.5 Prove root, uploads and accounts: root everywhere except upload, an upload non-owner is refused as if the
      upload does not exist, Admin/senior account rules and the root account. Verify: the scenarios pass.
- [x] 10.6 Prove catalogue, creation, content and assets: creation kinds, child inheritance and the asset chain. Verify:
      the scenarios pass.
- [x] 10.7 Prove denials, filtering, explanation and composition: which case conceals as `404` and which answers `403`,
      the silent listing filter, the explanation and the composed pair. The statuses themselves stay with Schemathesis;
      these scenarios prove which case yields which and that a refusal changes nothing.
- [x] 10.8 Prove the three permission routes' authorization: who may read the summary, replace a policy and share, and
      that a refused caller changes nothing. The declared statuses, bodies and headers stay with Schemathesis; this
      scenario proves only the decision.
- [ ] 10.9 Delete the authorization cases in `test/e2e/identity/uc024_manage_user_permissions.py` and the `library/*`
      E2E modules that the system suite replaces. Verify: a search for the deleted route and the old action names under
      `test/e2e` returns nothing.
- [ ] 10.10 Clear the duplicate proofs after the system suite is green: compare every `test/system/authorization.py`
      scenario with the E2E, Rust and Schemathesis suites, delete each E2E case and each Rust test that re-asserts a
      scenario the system suite proves, keep only the Rust tests for the grant algebra, the traversal bound, the
      explanation interior, a repository constraint or the transaction and a case the harness cannot reach yet, and keep
      every declared status, content type, body shape and header with Schemathesis alone. Verify: every scenario has one
      primary proof, and a search finds no E2E case, Rust test or system case for a behavior Schemathesis or the system
      suite already asserts.

## 11. Documentation and cleanup

- [x] 11.1 Rewrite `docs/Permissions.md` around resources, the tree, principals, roles, grants, deny-wins, visibility,
      root, uploads, accounts and the identity routes, including the canonical PlantUML tree diagram. Verify: the guide
      names no `ALL`/`LISTED`/`OWNED`/`NONE` scope and cites the owning requirement for each rule.
- [ ] 11.2 Update `docs/development/UseCases.md` where a use case's actor or authorization obligation changes. Verify:
      the `DOC-005`/`DOC-007` gates pass.
- [x] 11.3 Remove the old scope model, its use cases, its handlers, its repository and its tests. Verify: a search for
      `Scope`, `PermissionOverride`, `LISTED`, `PUBLIC_OR_OWNED` and `library.read` under `src/` returns nothing except
      a deliberate migration comment, and no `#[cfg(test)]` module remains under `src/lib/domain/model/`.
- [ ] 11.4 Run the full gate: `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`,
      `cargo fmt --all -- --check`, `script/rule_enforcer.sh check`, `markdownlint-cli2 '**/*.md'`,
      `prettier --check '**/*.md'` and `script/e2e.sh`. Verify: all pass.

## 12. Deferred follow-up (not this change)

- [ ] 12.1 Re-propose the eight `library/*` feature-spec deltas against the generic actions, so each feature requirement
      keeps one pointer line to `identity/authorization` and restates no who-may rule.
