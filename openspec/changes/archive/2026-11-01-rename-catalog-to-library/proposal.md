# Proposal

## Why

The permission action `catalog.read` names the same collection the rest of the system calls the library. The mismatch is
leftover naming debt: it leaks into the permission API, stored overrides, denial events, the specifications and
`docs/Permissions.md`, and it will collide with the `library.manage` action that catalogue writes need next. Rename it
once, before more actions depend on the wrong word.

## What Changes

- Rename the permission action `catalog.read` to `library.read` in the action catalogue, its stored wire value, its
  `from_wire`/`as_wire` round trip, the `Action` variant (`CatalogRead` to `LibraryRead`), its role presets, its
  supported scopes, its resource kinds and its tests.
- Rename the action's denial event target from `catalog` to `library`, so `authz_failed` events report the new family.
- Rewrite no stored data: the schema is unreleased (`PERS-014`), so a development database holding `catalog.read`
  overrides is reset instead of migrated.
- Update the four affected capability specifications and `docs/Permissions.md` so every reference to the action and to
  the permission family uses `library`.
- Regenerate `docs/openapi.json` if the permission response declaration changes.
- **BREAKING**: a client that sends or reads `catalog.read` in the permissions API must switch to `library.read`; no
  stored value is migrated, so a development database holding the old value is reset.

Out of scope:

- The `library.manage` action (a separate change).
- The generic noun "catalogue" where it names the collection rather than the action, such as a provider's curated genre
  vocabulary in `library/music-identification`; that prose is not the permission family.
- Any change to scopes, targets, defaults, concealment or the Root Admin rule. This is a rename, not a behavior change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `identity/authorization`: the action catalogue, the override example and the default-preset wording name
  `library.read` and the library permission family.
- `library/music-albums`: the album read requirements and their scenarios name `library.read`.
- `library/movies`: the movie read requirements and their scenarios name `library.read`.
- `library/series`: the series read requirements and their scenarios name `library.read`.

## Impact

- `src/lib/domain/model/permission.rs` — `Action::CatalogRead` becomes `Action::LibraryRead`, the `ACTIONS` list, the
  wire value, the `target` string and the scope/resource-kind tables.
- `src/lib/domain/service/authorization.rs` and every use case that calls `Action::CatalogRead` (album, movie, series,
  season and episode reads).
- `src/lib/infrastructure/inbound/rest/handler/identity/get_permission.rs` and its response examples.
- No migration: the schema does not change, and no stored value is rewritten (`PERS-014`).
- `docs/Permissions.md`, the authorization specification and the three library read specifications.
- `docs/development/TechnicalDesign.md` § 9, whose `authz_failed` target vocabulary lists the renamed value.
- `docs/openapi.json` if the generated permission response changes.
