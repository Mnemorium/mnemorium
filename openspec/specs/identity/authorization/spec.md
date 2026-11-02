# identity/authorization Specification

## Purpose

Defines who may perform which action on which resource, with per-user permission overrides, a fixed Root Admin
authority, and uniform denial semantics.

## Requirements

### Requirement: Authorization resolves the caller's effective permission

The system SHALL allow an action only when the caller's effective scope for that action covers the addressed resource.
The effective scope SHALL be the Root Admin rule, then the caller's per-user override, then the caller's role preset.

#### Scenario: Default caller reads a public gallery

- **WHEN** a Standard User with no override requests a public gallery
- **THEN** the system returns `200 OK`

#### Scenario: Default caller cannot read another user's private gallery

- **WHEN** a Standard User with no override requests another user's private gallery
- **THEN** the system returns `404 Not Found`

#### Scenario: Override widens access

- **WHEN** a Standard User whose `playlist.read` override names a private playlist requests that playlist
- **THEN** the system returns `200 OK`

#### Scenario: Override narrows access

- **WHEN** a Standard User whose `library.read` override is `NONE` requests a film
- **THEN** the system returns `404 Not Found`

### Requirement: Permission scopes cover resources uniformly

A permission scope SHALL be `ALL`, `PUBLIC_OR_OWNED`, `OWNED`, `LISTED`, or `NONE`. `ALL` covers every resource of the
action's family; `PUBLIC_OR_OWNED` covers public resources and the caller's own; `OWNED` covers only the caller's own;
`LISTED` covers exactly the named aggregates; `NONE` covers none.

#### Scenario: OWNED hides another user's public resource

- **WHEN** a caller whose `gallery.read` override is `OWNED` requests another user's public gallery
- **THEN** the system returns `404 Not Found`

#### Scenario: LISTED covers exactly the named aggregates

- **WHEN** a caller whose `gallery.read` override is `LISTED` naming one gallery requests that gallery and another
  gallery
- **THEN** the system returns `200 OK` for the named gallery and `404 Not Found` for the other

#### Scenario: ALL covers a private resource

- **WHEN** a caller whose `gallery.read` override is `ALL` requests another user's private gallery
- **THEN** the system returns `200 OK`

#### Scenario: NONE covers no resource

- **WHEN** a caller whose `playlist.read` override is `NONE` requests a public playlist
- **THEN** the system returns `404 Not Found`

### Requirement: Default permissions follow the role presets

When a caller has no override, the role preset SHALL decide. A Standard User SHALL read every library film, show and
album, read and moderate public or owned galleries and playlists, manage only owned content, and read or write only
owned uploads. An Admin SHALL additionally manage every library film, show and album, read, list, create and update user
accounts, except that only the Root Admin may create an Admin or change a role or credential.

#### Scenario: Standard User reads the catalog

- **WHEN** a Standard User with no override requests a film, a show or an album
- **THEN** the system returns `200 OK`

#### Scenario: Standard User reads a public playlist

- **WHEN** a Standard User with no override requests another user's public playlist
- **THEN** the system returns `200 OK`

#### Scenario: Admin administers a Standard user

- **WHEN** an Admin with no override lists users or creates a Standard user
- **THEN** the system returns the documented success response

#### Scenario: Admin cannot grant the Admin role

- **WHEN** an Admin with no override creates an Admin user
- **THEN** the system returns `403 Forbidden` and creates nothing

#### Scenario: Admin manages a library item

- **WHEN** an Admin with no override writes a film, a show or an album
- **THEN** the system applies the write

### Requirement: Library management is an administration action

The system SHALL authorize every film, show and album write through `library.manage`. A Standard User SHALL be granted
`NONE` by default, an Admin `ALL`, and the Root Admin SHALL be allowed. The action SHALL support `ALL`, `LISTED` and
`NONE`, and SHALL reject `OWNED` and `PUBLIC_OR_OWNED`.

#### Scenario: Standard User cannot manage a library item

- **WHEN** a Standard User with no override submits a write to a film, show or album it can read
- **THEN** the system returns `403 Forbidden` and applies nothing

#### Scenario: Admin manages another user's album

- **WHEN** an Admin with no override submits a write to an album
- **THEN** the system applies the write

#### Scenario: LISTED grants one aggregate

- **WHEN** an Admin whose `library.manage` override is `LISTED` naming one album writes that album and another album
- **THEN** the system applies the write to the named album and returns `403 Forbidden` for the other

#### Scenario: An owner scope is rejected

- **WHEN** a permission override names `library.manage` with `OWNED` or `PUBLIC_OR_OWNED`
- **THEN** the system rejects the permission with `422 Unprocessable Content`

#### Scenario: An unreadable item is concealed

- **WHEN** a caller who cannot read an album attempts to manage it
- **THEN** the system returns `404 Not Found`, indistinguishable from an unknown album

### Requirement: The Root Admin may perform every action except uploads

The Root Admin SHALL be allowed every action on every resource. The only exception SHALL be upload read, write and
completion, which SHALL remain owner-only.

#### Scenario: Root Admin reads a private gallery

- **WHEN** the Root Admin requests another user's private gallery
- **THEN** the system returns `200 OK`

#### Scenario: Root Admin manages another user's playlist

- **WHEN** the Root Admin renames or deletes another user's playlist
- **THEN** the system applies the change

#### Scenario: Root Admin deletes another user's gallery

- **WHEN** the Root Admin deletes another user's gallery
- **THEN** the system deletes it

#### Scenario: Root Admin cannot read another user's upload

- **WHEN** the Root Admin requests another user's upload session
- **THEN** the system returns `404 Not Found`

### Requirement: Upload access is owner-only for every caller

An upload session SHALL be readable, writable and completable only by its owner. A non-owner SHALL receive
`404 Not Found`, and no permission SHALL grant access to another caller's upload.

#### Scenario: Non-owner reads an upload

- **WHEN** an authenticated caller requests another user's upload session
- **THEN** the system returns `404 Not Found`

#### Scenario: Non-owner writes a chunk

- **WHEN** an authenticated caller writes a chunk to another user's upload session
- **THEN** the system returns `404 Not Found` and writes nothing

#### Scenario: Upload permissions cannot be granted

- **WHEN** the Root Admin tries to grant an upload action in a permission override
- **THEN** the system rejects the request with `422 Unprocessable Content`

### Requirement: Denials are uniform and concealed

A caller who cannot read an addressed resource SHALL receive `404 Not Found`, indistinguishable from a missing resource.
A caller who can read a resource but may not modify it SHALL receive `403 Forbidden`. Every denied decision SHALL emit
one `authz_failed` security event.

#### Scenario: Private gallery read is concealed

- **WHEN** a caller cannot read a private gallery
- **THEN** the system returns `404 Not Found`, not `403 Forbidden`

#### Scenario: Visible playlist modification is revealed

- **WHEN** a caller can read a public playlist but may not rename it
- **THEN** the system returns `403 Forbidden`

#### Scenario: A denial is recorded once

- **WHEN** a caller is denied an action
- **THEN** the system emits one `authz_failed` event naming the actor, action and target

### Requirement: A deleted caller is unauthorized

Every authenticated endpoint SHALL resolve the caller's account before deciding an action. A token that names a deleted
account SHALL answer `401 Unauthorized`.

#### Scenario: Deleted caller creates a user

- **WHEN** a request carrying a token for a deleted account reaches user creation
- **THEN** the system returns `401 Unauthorized` and creates nothing

#### Scenario: Deleted caller reads an upload

- **WHEN** a request carrying a token for a deleted account reaches an upload endpoint
- **THEN** the system returns `401 Unauthorized`

#### Scenario: Deleted caller loads a gallery

- **WHEN** a request carrying a token for a deleted account reaches a gallery endpoint
- **THEN** the system returns `401 Unauthorized`

### Requirement: The Root Admin administers per-user permissions

The Root Admin SHALL read and replace a non-root user's permission overrides through
`/api/v1/user/{user_id}/permission`. `GET` SHALL return the role defaults and the overrides; `PUT` SHALL replace the
overrides, and a replacement carrying no override SHALL reset the user to the role presets. A non-root caller SHALL
receive `403 Forbidden`, and an unknown user SHALL receive `404 Not Found`.

#### Scenario: Read a user's permissions

- **WHEN** the Root Admin requests a user's permissions
- **THEN** the system returns the effective role defaults and the stored overrides

#### Scenario: Replace a user's permissions

- **WHEN** the Root Admin replaces a user's overrides with a valid set
- **THEN** the system stores exactly that set and subsequent requests use it

#### Scenario: Empty replacement resets a user's permissions

- **WHEN** the Root Admin replaces a user's overrides with an empty set
- **THEN** the system deletes the overrides and the role presets apply again

#### Scenario: Non-root caller is refused

- **WHEN** an Admin or Standard User requests or changes any user's permissions
- **THEN** the system returns `403 Forbidden`

#### Scenario: Root Admin target is refused

- **WHEN** the Root Admin requests or changes its own permission overrides
- **THEN** the system returns `403 Forbidden`

### Requirement: Permission overrides are validated

A permission SHALL name a configurable action and a scope that action supports. A `LISTED` scope SHALL name at least one
aggregate of a supported kind, without duplicates. An unknown action, unsupported scope, empty or mismatched list,
duplicate action, non-configurable action, or the Root Admin target SHALL be rejected.

#### Scenario: Unknown action

- **WHEN** a permission names an action outside the catalog
- **THEN** the system returns `422 Unprocessable Content` and stores nothing

#### Scenario: Unsupported scope

- **WHEN** a permission gives `OWNED` to an action that does not support it
- **THEN** the system returns `422 Unprocessable Content`

#### Scenario: Empty listed scope

- **WHEN** a permission gives `LISTED` with no aggregates
- **THEN** the system returns `422 Unprocessable Content`

#### Scenario: Duplicate action

- **WHEN** a permission set names the same action twice
- **THEN** the system returns `422 Unprocessable Content`

#### Scenario: Non-configurable action

- **WHEN** a permission names upload access or permission administration
- **THEN** the system returns `422 Unprocessable Content`

### Requirement: Permission replacement is atomic

A `PUT` that is rejected for any reason SHALL apply nothing. A successful `PUT` SHALL replace every override for the
target user and SHALL be visible to the next request.

#### Scenario: Rejected replacement leaves the previous set

- **WHEN** the Root Admin sends a set containing one invalid permission
- **THEN** the system returns the validation error and leaves the previous permissions unchanged

#### Scenario: Successful replacement is complete

- **WHEN** the Root Admin replaces a user's overrides
- **THEN** an override omitted from the request no longer applies

### Requirement: Asset reads delegate to the owning resource

The system SHALL authorize a file, image, audio or video read through the read action of the resource that owns it:
`library.read` for catalogue art, `gallery.read` for a gallery item and `playlist.read` for a playlist cover.
`file.read` SHALL be a derived action with no scope and no override. An asset with no owner SHALL be readable only by
its file owner and the Root Admin.

#### Scenario: Catalogue art follows the library

- **WHEN** a caller whose `library.read` covers an album requests that album's artwork
- **THEN** the system returns the image representation

#### Scenario: A private gallery image is concealed by id

- **WHEN** a caller who cannot read a private gallery requests one of its images by image id
- **THEN** the system returns `404 Not Found`, indistinguishable from an unknown image

#### Scenario: The gallery owner reads the image

- **WHEN** the owner of a private gallery requests one of its images by image id
- **THEN** the system returns the image representation

#### Scenario: A playlist cover follows the playlist

- **WHEN** a caller who can read a private playlist requests its cover by image id
- **THEN** the system returns the image representation

#### Scenario: A file read cannot be granted

- **WHEN** a permission override names `file.read`
- **THEN** the system rejects the permission with `422 Unprocessable Content`

#### Scenario: Root Admin reads any asset

- **WHEN** the Root Admin requests an image owned by another caller
- **THEN** the system returns the image representation
