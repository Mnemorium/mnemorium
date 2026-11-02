# Tasks

## 1. Make the action derived

- [x] 1.1 Add `is_derived()` to `Action`, make `FileRead` return true and `is_configurable()` return false, and verify a
      `Permission::try_new` naming `file.read` fails with `NotConfigurable`.
- [x] 1.2 Remove `FileRead` from `resource_kinds` and any role-scope use, and verify `Permission::try_new` with a
      `LISTED` file scope fails.
- [x] 1.3 Verify the permission administration response no longer lists `file.read` among the configurable defaults.

## 2. Resolve the owner context

- [x] 2.1 Add `AssetContext` (`Library`, `Gallery(ResourceRef)`, `Playlist(ResourceRef)`) and resolve it from the
      `image`/`video` scope and its single referrer, and verify a `LIBRARY` asset resolves to `Library` and a `USER`
      asset to its referrer.
- [x] 2.2 Add the asset decision entry point that maps the context to `LibraryRead`, `GalleryRead` or `PlaylistRead`,
      and verify each calls `Authorization::decide` with the referrer's resource.
- [x] 2.3 Deny an asset with no context to everyone but its file owner and the Root Admin, and verify the concealed
      `404`.

## 3. Stop reading `is_public`

- [x] 3.1 Remove `file.is_public` from the asset authorization path and keep the column for unrelated semantics, and
      verify no asset decision reads it.
- [x] 3.2 Change `file.use` for Admin from `OWNED` to `ALL`, and verify an Admin can modify another caller's file while
      a Standard User cannot.

## 4. Documentation

- [x] 4.1 Add a derived-actions section to `docs/Permissions.md`, remove the `file.read` row from the configurable
      matrix and describe the delegation, and verify the document names no configurable `file.read` scope.
- [x] 4.2 Run the Markdown gates over the changed documents and verify they pass.

## 5. Integration verification

- [x] 5.1 Verify a private gallery image and a private playlist cover return `404` for a non-owner and `200` for the
      owner.
- [x] 5.2 Verify catalogue art follows `library.read` and the Root Admin reads every asset.
- [x] 5.3 Run the full gate (`cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`,
      `cargo fmt --all -- --check`, `script/rule_enforcer.sh check`) and verify it passes.
