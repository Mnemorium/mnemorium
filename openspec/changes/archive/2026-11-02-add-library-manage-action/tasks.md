# Tasks

## 1. Add the action

- [x] 1.1 Add `Action::LibraryManage`, the `library.manage` wire value, `target() = "library"`, `verb() = "manage"`,
      `ACTIONS` membership, `read_action() = LibraryRead` and `conceals_denial() = false`, and verify the permission
      model tests pass.
- [x] 1.2 Set `resource_kinds()` to `Movie`, `Series` and `MusicAlbum`, and verify a `LISTED` permission naming one
      album is accepted.
- [x] 1.3 Set `role_scope()` to `NONE` for Standard and `ALL` for Admin, and `root_access()` to `All`, and verify the
      preset tests pass.

## 2. Constrain the scopes

- [x] 2.1 Add `LIBRARY_MANAGE_SCOPES = ["ALL", "LISTED", "NONE"]` and return `UnsupportedScope` for `OWNED` and
      `PUBLIC_OR_OWNED`, and verify the handler maps it to `422 Unprocessable Content`.
- [x] 2.2 Verify the permission administration response lists `library.manage` with the documented default and that
      `catalog.read`-style stale names cannot be created.

## 3. Documentation

- [x] 3.1 Add `library.manage` to the action table and the administration recipes in `docs/Permissions.md`, and verify
      the document names the new action consistently.
- [x] 3.2 Run the Markdown gates over the changed documents and verify they pass.

## 4. Integration verification

- [x] 4.1 Verify a Standard User receives `403 Forbidden` when managing a readable album and `404 Not Found` when
      managing an unreadable one.
- [x] 4.2 Verify an Admin and the Root Admin can manage an album, and a `LISTED` override limits the reach to the named
      album.
- [x] 4.3 Run the full gate (`cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`,
      `cargo fmt --all -- --check`, `script/rule_enforcer.sh check`) and verify it passes.
