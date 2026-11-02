# Proposal

## Why

The catalogue has a read action but no write action. Music album, group, medium and track metadata will be writable in
the next changes, and without a dedicated action any authenticated caller would either be unable to write or would have
to reuse `library.read`, which would let every reader rewrite shared catalogue metadata.

## What Changes

- Add the permission action `library.manage`, targeting films, shows and albums.
- Default it to `NONE` for a Standard User and `ALL` for an Admin; the Root Admin short-circuit applies.
- Support `ALL`, `LISTED` and `NONE` scopes. `OWNED` and `PUBLIC_OR_OWNED` are rejected because a film, show or album
  has no owner.
- Make `library.read` the read action of `library.manage`, so a denial conceals an unreadable item and reveals a
  readable one.
- Update the action catalogue, the permission administration response, `docs/Permissions.md` and
  `identity/authorization`.
- Out of scope: the music write endpoints themselves (change 9), and any change to `file.use`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `identity/authorization`: adds the library management action and its role preset.

## Impact

- `src/lib/domain/model/permission.rs` — a new `Action` variant, the `ACTIONS` array, the wire value, the target, the
  verb, the scope and resource-kind tables, and the read-action mapping.
- `src/lib/domain/service/authorization.rs` and `src/lib/infrastructure/inbound/rest/handler/identity/get_permission.rs`
  — tests and the documented presets.
- `docs/Permissions.md` and the authorization specification.
- No datastore change and no HTTP endpoint in this change.
