# Tasks

## 1. Domain model and authorization rules

- [x] 1.1 Add `src/lib/domain/model/permission.rs` with `Permission`, `Action`, `Scope`, `ResourceRef`, `ResourceKind`,
      `Protected`, `Global`, and the permission validation error. Do not add a dedicated model test module (`TEST-019`);
      verify the crate compiles and the model is exercised by the domain-service tests in 1.3 and the
      permission-administration use-case tests in 4.1.
- [x] 1.2 Give every `Action` its role presets, supported scopes, configurable flag, root access, event verb and target.
      Add a catalog-completeness test that iterates every variant and fails when a preset or scope is missing; verify
      with `cargo test`.
- [x] 1.3 Implement `Authorization::decide` with the root short-circuit, the per-user override, the role preset, the
      five scope arms and the concealment rule. These are the dedicated domain-service tests (`TEST-024`–`TEST-026`);
      verify the matrix for `ALL`, `PUBLIC_OR_OWNED`, `OWNED`, `LISTED`, `NONE`, root reads, root manage, root upload
      denial, the `UserUpdate` relation guard, and read-dependent manage concealment.
- [x] 1.4 Implement `Protected` for `User`, `Gallery`, `MusicPlaylist`, `File`, `Upload`, `Movie`, `Series` and
      `MusicAlbum`. Verify the scope-matrix tests in 1.3 exercise each implementation; no dedicated model test
      (`TEST-019`).
- [x] 1.5 Add the Glossary terms (`Action`, `Scope`, `Permission`, `Authorization service`) and update the Domain
      Service example in `docs/development/TechnicalDesign.md`. Verify the Markdown gates pass and the `LibraryAccess`
      example is gone.
- [x] 1.6 Write `docs/Permissions.md` with the two axes, the action/scope tables, resolution order, root/upload rules,
      denial semantics, store shape, extension checklist and test split; add `Permissions: Permissions.md` to the
      top-level `mkdocs.yml` navigation and link it from `AGENTS.md`. Verify the Markdown gates and that every example
      matches the catalog and specs.

## 2. Persistence

- [x] 2.1 Add the `permission` and `permission_resource` migration pair with the parent/child keys, the scope check, the
      cross-context user FK without cascade and the Root Admin guard trigger. Verify the up/down pair applies, the root
      trigger aborts, and a deleted parent cascades its resources.
- [x] 2.2 Add `src/lib/domain/port/permission_repository.rs` with `PermissionFilter`, `search`, `save`, `delete` and
      `delete_all`, and expose `permissions()` from `IdentityUnitOfWork`. Verify the crate compiles and the test unit of
      work can implement the view.
- [x] 2.3 Add `src/lib/infrastructure/outbound/sqlx/permission_repository.rs` and its row model. Verify `search` groups
      child rows into `Scope::Listed`, `save` replaces the child set, `delete` cascades, `delete_all` resets, and an
      unknown action, resource kind or scope maps to `DataIntegrityViolation` in repository tests.
- [x] 2.4 Update the entity-relationship diagram and add the persistence rules for the two tables, the root guard and
      the text action/resource encoding. Verify `PERS-011` and the Markdown gates.

## 3. Use-case migration

- [x] 3.1 Add `MockPermissionRepository` to `TestUnitOfWork` in `src/lib/lib.rs` and extend the shared `expect_caller`
      helpers to expect the permission load. Verify every existing application test compiles before migrating a use
      case.
- [x] 3.2 Migrate the identity and user use cases (`get_user`, `list_users`, `update_user`, `register_user`,
      `patch_credential`, `get_current_user`) to `UserRead`, `UserUpdate`, `UserCreate`, `UserCreateAdmin`,
      `UserRoleChange` and `CredentialPatch`. Add `NoSuchCaller` to `register_user`/`patch_credential`; verify the new
      `401`, `403` and permission-override tests.
- [x] 3.3 Migrate the gallery use cases (`get_gallery`, `get_gallery_item`, `list_galleries`, `create_gallery`,
      `add_gallery_item`, `delete_gallery_item`, `delete_gallery`) to `GalleryRead`, `GalleryModerate`, `GalleryManage`
      and `FileUse`. Verify private-gallery reads answer `404`, root may delete any gallery, and the root-only file-use
      override still works.
- [x] 3.4 Migrate the playlist use cases (`get_playlist`, `get_playlist_track`, `list_playlists`, `update_playlist`,
      `delete_playlist`, `remove_playlist_track`, `reorder_playlist_tracks`) to `PlaylistRead`, `PlaylistModerate` and
      `PlaylistManage`. Verify root may rename/delete another user's playlist, a readable-but-not-moderatable caller
      gets `403`, and an invisible playlist gets `404`.
- [x] 3.5 Migrate the catalog use cases (`get_movie`, `list_movies`, `get_series`, `list_series`, `get_series_season`,
      `get_series_episode`, `list_series_episodes`, `get_album`, `list_albums`, `get_album_medium`, `get_album_track`,
      `list_album_tracks`) to `CatalogRead`. Verify a `NONE` override returns `404` on detail and an empty collection,
      and a `LISTED` override returns only the named top-level resources.
- [x] 3.6 Migrate the upload use cases (`get_upload`, `write_upload_chunk`, `complete_upload`) to resolve the caller and
      apply `UploadRead`/`UploadWrite`. Verify a deleted caller gets `401`, a non-owner gets `404`, and the Root Admin
      cannot read or write another user's upload.
- [x] 3.7 Remove `src/lib/domain/service/library_access.rs`, its module entry and every `LibraryAccess` reference.
      Verify `rg LibraryAccess src docs` returns no result and the full test suite passes.
- [x] 3.8 Update `docs/development/Overview.md` with the new model, port, service and adapter files and without
      `library_access.rs`. Verify the source-layout table matches the tree and the Markdown gates pass.

## 4. Permission administration API

- [x] 4.1 Add the `get_permission` and `replace_permission` ports and use cases in the Identity factory. Verify
      `PermissionManage` allows only the Root Admin, the Root Admin target is refused, `PUT` replaces atomically, an
      empty replacement resets, and validation answers `422` for an unknown action, unsupported scope, empty list,
      duplicate action or non-configurable action. Cover `Permission::try_new` (unsupported scope, empty or duplicate
      `LISTED`, mismatched kind) through these use-case tests; the root target stays a `403` before validation.
- [x] 4.2 Add the `/api/v1/user/{user_id}/permission` handlers and routes with the effective-defaults and override
      representation. Verify handler tests for `200`, `401`, `403`, `404` and `422`, and that the OpenAPI paths and
      schemas are generated.
- [x] 4.3 Add the E2E permission round trip (read, replace, reset), a Standard user receiving `user.create`, a
      `catalog.read = NONE` user seeing an empty library, and a `LISTED` playlist share. Verify with
      `pytest -p no:cacheprovider test/e2e`.
- [x] 4.4 Add the permission-administration business use case to `docs/development/UseCases.md` under Identity, with its
      diagram node and the `DOC-005`, `DOC-007` and `DOC-008` conventions. Verify the Markdown gates and the use-case
      diagram rules.
- [x] 4.5 Add the `GET`/`PUT` examples and the permission recipes (all content, owned only, one shared playlist, no
      catalog, reset through an empty replacement) to `docs/Permissions.md`. Verify each example against the handler
      tests and the generated OpenAPI.

## 5. Client and specification alignment

- [x] 5.1 Update the OpenAPI descriptions for gallery concealment, Root Admin playlist/gallery management, catalog
      permissions and account-creation permissions, then regenerate `docs/openapi.json` with
      `cargo run --bin openapi_gen`. Verify the generated diff contains only the intended description and schema
      changes.
- [x] 5.2 Update the E2E suite for the root playlist/gallery flips, the gallery private-read `404`, the catalog
      restriction and the account-creation grant. Verify `pytest -p no:cacheprovider test/e2e`.
- [x] 5.3 Update the affected business use cases in `docs/development/UseCases.md` where the Root Admin and permission
      rules change, keeping the entries business-level and free of HTTP and implementation terms (`DOC-007`). Verify the
      Markdown gates.
- [x] 5.4 Add the `authz_failed` action vocabulary and the permission rules to the security-event documentation, and
      confirm the emitted verbs/targets match the catalog. Verify the documentation links and the Markdown gates.

## 6. Integration verification

- [x] 6.1 Run `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test`.
      Fix every failure.
- [x] 6.2 Run `cargo llvm-cov --lib --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80` and close
      any coverage gap.
- [x] 6.3 Run `devenv test` (all pre-commit hooks, Markdown lint and formatting) and fix every failure.
- [x] 6.4 Run the E2E suite against the Docker `mnemorium-e2e` server with `pytest -p no:cacheprovider test/e2e`.
- [x] 6.5 Verify `docs/openapi.json` is regenerated and committed, and that the change's specs, design and tasks have no
      unresolved contradiction.

## Workflow follow-up

- Archive the change after the project's review requirements are satisfied.
- Verify the archived result and that the new capability spec and the modified specs are in the main spec tree.
