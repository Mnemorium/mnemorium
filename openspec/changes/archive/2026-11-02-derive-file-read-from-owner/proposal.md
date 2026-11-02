# Proposal

## Why

`file.read` carries its own scope while an asset's real visibility belongs to the resource that owns it: an album cover
is as readable as the album, a gallery item as its gallery, a playlist cover as its playlist. Keeping a second
`file.read` rule duplicates visibility, drifts from the referrer, and let a known image id bypass a private gallery.

## What Changes

- Make `file.read` a derived, non-configurable action: no scope, no override and no `LISTED` resources.
- Resolve an `AssetContext` (`Library`, `Gallery`, `Playlist`) for every asset read and delegate the decision to
  `library.read`, `gallery.read` or `playlist.read` on the owning resource.
- Stop using `file.is_public` as an authorization input.
- Keep `file.use` as the asset write action: Standard `OWNED`, Admin `ALL`, Root Admin `ALL`.
- Root Admin sees every asset through the short-circuit; no separate Admin bypass.
- Out of scope: the Asset endpoints themselves (change 5) and any change to `file.use` semantics.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `identity/authorization`: adds the delegated asset read rule and removes `file.read` from the configurable matrix.

## Impact

- `src/lib/domain/model/permission.rs` — `FileRead` is derived, not configurable, and has no scope.
- `src/lib/domain/service/authorization.rs` and the asset repositories that resolve the owner context.
- `docs/Permissions.md` — a derived-actions section and the removal of the `file.read` row.
- No datastore change; `image.scope` and `video.scope` are supplied by change 3.
