# Design

## Context

The permission action catalogue is a closed Rust enum (`Action`) with a wire value per variant, persisted in
`permission.action_name` and exposed by the permission administration response. `CatalogRead` is one of eighteen
actions; it guards film, show and album reads and is named in four capability specifications and `docs/Permissions.md`.

The schema is unreleased (`PERS-014`), so migrations are mutable and the action name has no external clients to keep
compatible. See `proposal.md` for motivation.

## Goals / Non-Goals

**Goals:**

- One name for the permission family across the enum, the wire value, the denial event target, the specifications and
  the permissions guide.
- Preserve every scope, target, default and concealment rule exactly.
- Leave no stored value the renamed action cannot parse, without adding a datastore migration (`PERS-014`).

**Non-Goals:**

- Adding `library.manage`; that is the next change.
- Renaming the generic noun "catalog" where it means the action registry or the collection itself.
- Any change to the authorization algorithm.

## Decisions

### Rename the variant and wire value in place, with no alias

`CatalogRead` becomes `LibraryRead`, and `as_wire`/`from_wire` use `library.read` only. No compatibility branch accepts
`catalog.read`.

Alternatives considered:

- **Accept both names on input for a transition period.** Rejected: the schema is unreleased, and a dual-name period
  keeps the exact ambiguity this change removes.
- **Add a new action and deprecate the old one.** Rejected: the old action would still exist, and every caller would
  need to migrate twice.

### Rename the action family, not the English noun

The change covers the action identifier, the `target()` string used by denial events, and the spec prose that names the
action or its family ("read every catalog film, show and album" becomes "read every library film, show and album"). It
does not rewrite "the action catalog is fixed", "outside the catalog", or the music identification spec's "the
catalogue's discrete list", where "catalog" means the registry or the collection.

Alternative considered: a repository-wide word replacement. Rejected: it would rename the action registry and unrelated
domain prose, producing sentences like "an action outside the library".

A MODIFIED requirement must keep its existing scenario names, so labels such as "A caller with no catalog access lists
no albums" stay as they are; their bodies use `library.read`. Renaming a scenario label is a separate editorial change
that removes and re-adds the requirement, not part of this rename.

### No data migration: retire stored rows with the schema, not with an `UPDATE`

The schema is unreleased (`PERS-014`), so no database outside development holds a `catalog.read` row, and the rename
touches no table, column, constraint, trigger or seed. `create_permission` declares `action_name` and stores whatever
`Action::as_wire` returns, so a fresh schema persists `library.read` with no SQL change: the rename is a code change
only, with nothing to edit in place and no new `.up.sql`/`.down.sql` pair.

A development database that still holds `catalog.read` rows is reset, as `PERS-014` allows. Once reset, `from_wire`
rejects the old value, which is the loud failure this change wants rather than a compatibility shim.

Alternatives considered:

- **A data migration rewriting stored rows.** Rejected: it would add a permanent `UPDATE` pair to the migration history
  to carry a database the release does not support, while no released instance can hold the old value.
- **Editing `create_permission` in place.** Rejected: that pair holds no action name, so an in-place edit would only
  churn the migration checksum without changing what a fresh schema stores.

### Change the denial event target too

`Action::target()` returns `"library"` for the renamed action, so `authz_failed` events name the new family. Keeping
`"catalog"` only in logs would preserve a second vocabulary and make the event harder to correlate with the action.

### Keep the rename mechanical

`role_scope`, `resource_kinds`, `supported_scopes`, `root_access`, `conceals_denial`, `read_action` and `verb` keep
their current values. The tests change only the action name.

## Risks / Trade-offs

- **Breaking wire value** → the change is unreleased, and the specs and the guide move with the code. No compatibility
  shim is warranted, and a development database holding the old value is reset (`PERS-014`).
- **A stale `catalog.read` string survives somewhere** → `from_wire` returns `None` for it and the permission load fails
  loudly. Mitigate with a repository-wide search (`rg "catalog.read|CatalogRead"`) and a round-trip test over `ACTIONS`.
- **The generic noun stays in the guide**, so a reader may still see "catalog" near `library.read` → the action table is
  the authoritative place; the open question below tracks a possible follow-up.
- **`docs/openapi.json` does not currently contain the action name** → regeneration is not expected to change it, but
  the implementation should still run `cargo run --bin openapi_gen` and commit the result if the generated document
  changes.

## Migration Plan

1. Ship the code rename; no datastore migration runs and no migration pair changes.
2. A development database that stored `catalog.read` overrides is reset (`PERS-014`); without the reset the permission
   load fails on the unknown action, which is the intended loud failure.
3. Verify on a fresh database that the permission response returns `library.read` and accepts a `library.read` override.

## Open Questions

- Should the generic noun "catalog" (the action registry, "outside the catalog", "hide the catalog") be renamed to
  "library" in a separate documentation change? It does not affect behavior or the specs.
