# Permissions

This guide describes how Mnemorium decides whether a caller may perform an action. It covers the two axes of authority,
the action catalog, the five scopes, how a decision is resolved, the Root Admin and upload rules, what a denial looks
like, and how the Root Admin administers another user's permissions over the API.

## The two axes

Authority has two independent axes, and a change usually touches only one of them.

| Axis           | Decided by                                      | Example                                                       |
| -------------- | ----------------------------------------------- | ------------------------------------------------------------- |
| Administration | The role preset, optionally overridden per user | Creating a user, changing a role, reading user accounts       |
| Content        | Ownership and visibility, optionally overridden | Reading a gallery, moderating a playlist, reading the catalog |

The role (`Standard` or `Admin`) determines the administration axis and the default content scope. Visibility (`public`
or `private`) and ownership refine the content axis. A permission only overrides the _scope_ of an action; it never
changes the role.

## Actions

Every authorizable operation has exactly one action name, and the catalog is fixed.

| Action              | Target                    | Standard          | Admin             | Root Admin | Configurable |
| ------------------- | ------------------------- | ----------------- | ----------------- | ---------- | ------------ |
| `catalog.read`      | Movie, Series, MusicAlbum | `ALL`             | `ALL`             | All        | yes          |
| `gallery.read`      | Gallery                   | `PUBLIC_OR_OWNED` | `PUBLIC_OR_OWNED` | All        | yes          |
| `gallery.moderate`  | Gallery                   | `PUBLIC_OR_OWNED` | `PUBLIC_OR_OWNED` | All        | yes          |
| `gallery.manage`    | Gallery                   | `OWNED`           | `OWNED`           | All        | yes          |
| `playlist.read`     | MusicPlaylist             | `PUBLIC_OR_OWNED` | `PUBLIC_OR_OWNED` | All        | yes          |
| `playlist.moderate` | MusicPlaylist             | `PUBLIC_OR_OWNED` | `PUBLIC_OR_OWNED` | All        | yes          |
| `playlist.manage`   | MusicPlaylist             | `OWNED`           | `OWNED`           | All        | yes          |
| `file.read`         | File                      | `OWNED`           | `OWNED`           | All        | yes          |
| `file.use`          | File                      | `OWNED`           | `OWNED`           | All        | yes          |
| `upload.read`       | Upload                    | `OWNED`           | `OWNED`           | `OWNED`    | no           |
| `upload.write`      | Upload                    | `OWNED`           | `OWNED`           | `OWNED`    | no           |
| `user.read`         | User, Global              | `NONE`            | `ALL`             | All        | yes          |
| `user.update`       | User                      | `NONE`            | `ALL`             | All        | yes          |
| `user.create`       | Global                    | `NONE`            | `ALL`             | All        | yes          |
| `user.create_admin` | Global                    | `NONE`            | `NONE`            | All        | yes          |
| `user.role_change`  | User                      | `NONE`            | `NONE`            | All        | yes          |
| `credential.patch`  | Global                    | `NONE`            | `NONE`            | All        | yes          |
| `permission.manage` | Global                    | `NONE`            | `NONE`            | All        | no           |

The table is the default matrix. `Root Admin` is not a preset: it is a short-circuit, described below.

A `LISTED` scope names top-level resources only. A season, episode, medium or track is authorized through its series or
album, so `catalog.read` never names a season or a track.

## Scopes

A scope is the reach of an action over a resource family.

| Scope             | Covers                                     | Example                                                 |
| ----------------- | ------------------------------------------ | ------------------------------------------------------- |
| `ALL`             | Every resource the action targets          | Read every gallery, including other users' private ones |
| `PUBLIC_OR_OWNED` | Public resources and the caller's own      | The default for reading and moderating galleries        |
| `OWNED`           | Only the caller's own resources            | The default for renaming a playlist                     |
| `LISTED`          | Exactly the resources the permission names | Share one private playlist with one user                |
| `NONE`            | No resource                                | Hide the catalog from one user                          |

`LISTED` names resources by kind and identifier and must name at least one, without duplicates, of a kind the action
targets. `ALL`, `PUBLIC_OR_OWNED`, `OWNED` and `NONE` are supported by the content actions; the administration actions
support only `ALL` and `NONE`; the upload actions support only `OWNED`.

## Resolution order

For each action, the effective scope is resolved in this order:

1. **Root Admin short-circuit.** When the caller is the Root Admin and the action allows it, the decision is `Allow`
   immediately. The Root Admin ignores any stored override and has none of its own.
2. **Per-user override.** When the caller has an override for the action, its scope applies, whether it widens or
   narrows the preset.
3. **Role preset.** Otherwise the action's preset for the caller's role applies.

Two rules run alongside the scope:

- **Relation guard.** `user.update` refuses another Admin account unless the caller is the Root Admin, whatever the
  scope says. This keeps a non-root Admin from editing a peer.
- **Fixed uploads.** `upload.read` and `upload.write` always resolve to `OWNED` for every caller, including the Root
  Admin.

## Root Admin and uploads

The Root Admin is a universal allow: it reads every gallery and playlist, manages another user's playlist or gallery,
reads and creates user accounts, and administers permissions. The one exception is uploads: an upload session is
readable, writable and completable only by its owner.

The Root Admin has no overrides, and none can be given to it: the API refuses the Root Admin as a target, and any
override recorded for it is ignored.

Two rules sit outside permissions and refuse on their own: the Root Admin's own account cannot be modified or deleted,
and the default gallery cannot be deleted. The Root Admin may _attempt_ either; the request is refused.

## Denials

A denial is either concealed or revealed.

| Situation                                              | Response           |
| ------------------------------------------------------ | ------------------ |
| The caller cannot read the addressed resource          | `404 Not Found`    |
| The caller can read the resource but may not modify it | `403 Forbidden`    |
| The token names a deleted account                      | `401 Unauthorized` |
| The caller lacks an administration action              | `403 Forbidden`    |

Read actions (`catalog.read`, `gallery.read`, `playlist.read`, `file.read`) and the upload actions always conceal.
`gallery.moderate`, `gallery.manage`, `playlist.moderate` and `playlist.manage` conceal exactly when the caller cannot
read the resource, so an invisible playlist answers `404` and a visible one it may not rename answers `403`.
Administration actions and `file.use` reveal.

Every denial is logged once at warning level, naming the caller, the action and its target. Filtering a list is not a
denial, so an item quietly dropped from a listing logs nothing.

## Stored overrides

Only overrides are stored; the role presets are built in. An override replaces one action's scope for one user, and a
`LISTED` override also names the resources it covers. A user with no override for an action gets the role preset.

`PUT` replaces every override for a user atomically, and a replacement carrying no override removes them all. A role
change leaves them in place — a demoted user keeps an explicit grant until the Root Admin resets it. A `LISTED` override
whose resource is deleted later simply stops matching.

## Administration API

The Root Admin reads and replaces a non-root user's overrides under `/api/v1/user/{user_id}/permission`.

| Method | Meaning                                                                  |
| ------ | ------------------------------------------------------------------------ |
| `GET`  | The role defaults and the stored overrides, separately                   |
| `PUT`  | Replace every override for the user atomically; an empty set resets them |

A non-root caller is refused with `403`, the Root Admin as a target is refused with `403`, and an unknown user is `404`.
An action outside the catalog, an unsupported scope, an empty or mismatched `LISTED`, a repeated action, a
non-configurable action, or an invalid representation is `422` and stores nothing.

### Read a user's permissions

`GET /api/v1/user/{user_id}/permission` returns the role defaults and the stored overrides separately. `defaults` holds
one entry per configurable action; `overrides` holds only the actions the user has customized. An action absent from
`overrides` uses its default.

```http
GET /api/v1/user/5/permission
Authorization: Bearer <root-admin-token>
```

```json
{
  "user_id": 5,
  "role": "STANDARD",
  "defaults": [
    { "action": "catalog.read", "scope": "ALL", "resources": [] },
    { "action": "gallery.read", "scope": "PUBLIC_OR_OWNED", "resources": [] }
  ],
  "overrides": [{ "action": "gallery.read", "scope": "OWNED", "resources": [] }],
  "_links": { "self": { "href": "/api/v1/user/5/permission" } }
}
```

### Replace a user's permissions

`PUT /api/v1/user/{user_id}/permission` sends the complete override set the user must end with. An action omitted from
the body falls back to its role preset; an empty list resets every action. The replacement is atomic: a request that is
rejected stores nothing.

```http
PUT /api/v1/user/5/permission
Authorization: Bearer <root-admin-token>
Content-Type: application/json

{
  "permissions": [
    {
      "action": "playlist.read",
      "scope": "LISTED",
      "resources": [{ "kind": "MUSIC_PLAYLIST", "id": 12 }]
    }
  ]
}
```

The response carries the resulting defaults and overrides, in the same shape as the read. Replacing with an empty set
deletes every override, so the role presets apply again on the next request.

```json
{ "permissions": [] }
```

### Recipes

Set the content actions to the named scope; an action a recipe does not name keeps its role preset.

**All content.** Read any gallery, playlist and catalog item, public or private:

```json
{
  "permissions": [
    { "action": "catalog.read", "scope": "ALL", "resources": [] },
    { "action": "gallery.read", "scope": "ALL", "resources": [] },
    { "action": "playlist.read", "scope": "ALL", "resources": [] }
  ]
}
```

**Owned only.** Read only what the user owns, even for public content:

```json
{
  "permissions": [
    { "action": "catalog.read", "scope": "OWNED", "resources": [] },
    { "action": "gallery.read", "scope": "OWNED", "resources": [] },
    { "action": "playlist.read", "scope": "OWNED", "resources": [] }
  ]
}
```

`catalog.read` has no owner, so `OWNED` hides the whole catalog; use `NONE` to state the same intent.

**One shared playlist.** Share exactly one private playlist with one user:

```json
{
  "permissions": [
    {
      "action": "playlist.read",
      "scope": "LISTED",
      "resources": [{ "kind": "MUSIC_PLAYLIST", "id": 12 }]
    }
  ]
}
```

A `LISTED` scope names at least one resource and no duplicates; a season, episode, medium or track authorizes through
its series or album, so it is never named.

**No catalog.** Hide films, shows and albums from one user:

```json
{
  "permissions": [{ "action": "catalog.read", "scope": "NONE", "resources": [] }]
}
```

**Reset.** Replace the overrides with an empty set so the role presets apply again:

```json
{ "permissions": [] }
```
