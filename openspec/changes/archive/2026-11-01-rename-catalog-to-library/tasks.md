# Tasks

## 1. Rename the action in the domain model

- [x] 1.1 Rename `Action::CatalogRead` to `Action::LibraryRead` in the enum, the `ACTIONS` array and every match arm in
      `src/lib/domain/model/permission.rs`, and verify `rg "CatalogRead" src/lib/domain/model/permission.rs` returns
      nothing.
- [x] 1.2 Change the wire value to `library.read` in `as_wire`/`from_wire` and the denial target to `library` in
      `target()`, and verify a unit test round-trips `Action::LibraryRead` through `as_wire`/`from_wire`.
- [x] 1.3 Update the permission model tests to the new variant and wire value, and verify
      `cargo test --lib domain::model::permission` passes.

## 2. Update the application call sites

- [x] 2.1 Replace `Action::CatalogRead` with `Action::LibraryRead` in the album, movie, series, season and episode read
      use cases, and verify `rg "CatalogRead" src/lib/application` returns nothing.
- [x] 2.2 Update the affected use-case tests (`get_movie`, `list_movies`, `authorization` and the album/series read
      tests) to the new variant, and verify the focused tests for those modules pass.
- [x] 2.3 Update the permission administration response test to expect `library.read`, and verify
      `cargo test --lib get_permission` passes.

## 3. Retire stored overrides without a migration

- [x] 3.1 Confirm no migration carries an action name: `create_permission` declares `action_name` and stores whatever
      `Action::as_wire` returns, so the rename needs no in-place SQL edit and no new `.up.sql`/`.down.sql` pair
      (`PERS-014`); verify `rg "catalog" migrations` returns nothing.
- [x] 3.2 Verify a fresh database stores `library.read` when a `library.read` override is written, and record that a
      development database holding `catalog.read` is reset rather than migrated.

## 4. Update the specifications and the guide

- [x] 4.1 Update `docs/Permissions.md` so the action table, the concealment list, the recipes and the examples name
      `library.read`, and verify `rg "catalog\.read" docs` returns nothing.
- [x] 4.2 Regenerate `docs/openapi.json` with `cargo run --bin openapi_gen` and verify the generated file is unchanged
      or committed if the permission response declaration changed.
- [x] 4.3 Run the Markdown gates (`markdownlint-cli2 '**/*.md'` and Prettier) over the changed documents, and verify
      they pass.

## 5. Integration verification

- [x] 5.1 Verify the permission endpoint returns `library.read` in its default presets and accepts a `library.read`
      override on create, update and read.
- [x] 5.2 Verify a `catalog.read` override is rejected by `from_wire` after the rename and the migration leaves no
      stored row with the old name.
- [x] 5.3 Run the full gate (`cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`,
      `cargo fmt --all -- --check`, `script/rule_enforcer.sh check`) and verify it passes.
