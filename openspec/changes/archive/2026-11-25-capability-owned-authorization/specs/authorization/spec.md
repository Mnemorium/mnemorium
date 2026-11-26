# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-AUTHORIZATION-002 — The resource tree defines authorization parents

The **authorization parent** SHALL be the single canonical permission-inheritance edge. Each capability SHALL declare
the authorization parent of the resources it defines. The resource tree SHALL be the one drawn by the PlantUML diagram
in `docs/Permissions.md` § Resource tree; that diagram is the canonical picture, and this requirement states the rule
rather than a second copy of the edges.

#### Scenario: SCN-AUTHORIZATION-002-A — A season follows its series

- **WHEN** a season is created under a series
- **THEN** its authorization parent is that series

#### Scenario: SCN-AUTHORIZATION-002-B — An episode follows its season

- **WHEN** an episode is created under a season
- **THEN** its authorization parent is that season, and the season in turn follows its series

#### Scenario: SCN-AUTHORIZATION-002-C — A recording follows its medium

- **WHEN** a recording is placed on a medium
- **THEN** its authorization parent is that medium, and the medium follows its album

#### Scenario: SCN-AUTHORIZATION-002-D — A typed asset follows its owner

- **WHEN** an image, a video or an audio is attached to a gallery item, a film, an episode or a recording
- **THEN** its authorization parent is that resource, and a file follows its typed asset

#### Scenario: SCN-AUTHORIZATION-002-E — User and upload roots

- **WHEN** a user account or an upload session is created
- **THEN** its authorization parent is the instance root

#### Scenario: SCN-AUTHORIZATION-002-F — The catalogue is a container

- **WHEN** a film, a show, an album or a music group is created
- **THEN** its authorization parent is the catalogue, whose authorization parent is the library

### Requirement: REQ-AUTHORIZATION-012 — Ownership is implicit and resolved up the chain

The **owner** SHALL be the nearest non-null `owner_id` on the addressed resource or its authorization-parent chain. The
owning principal SHALL hold the implicit `Owner` role for that resource. `Owner` SHALL NOT be stored as a grant to a
user. A resource with no owner on its chain SHALL be shared, and the capability that defines it SHALL state that it has
no owner.

#### Scenario: SCN-AUTHORIZATION-012-B — Catalogue resources have no owner

- **WHEN** a caller addresses a film, show, album or music group
- **THEN** no user holds the implicit `Owner` role for it

#### Scenario: SCN-AUTHORIZATION-012-C — Ownership is not grantable to a user

- **WHEN** a user grant names `Owner` as its role
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

### Requirement: REQ-AUTHORIZATION-023 — Creation names the child kind and the parent

`CreateChild` SHALL be decided against the target's authorization parent or a **creation surface** that names it, and
SHALL name a `ChildKind`. A surface SHALL NOT become the child's authorization parent. The capability that decides
`CreateChild` for a surface SHALL state which roles its default seed admits.

#### Scenario: SCN-AUTHORIZATION-023-A — A standard user creates a gallery

- **WHEN** a Standard User creates a gallery under the library
- **THEN** the gallery is created and owned by that caller

#### Scenario: SCN-AUTHORIZATION-023-B — A standard user cannot create a film

- **WHEN** a Standard User creates a film under the catalogue
- **THEN** the system answers `403 Forbidden` and creates nothing

#### Scenario: SCN-AUTHORIZATION-023-C — An admin creates content

- **WHEN** an Admin creates a music album under the catalogue
- **THEN** the album is created

#### Scenario: SCN-AUTHORIZATION-023-F — A denied creation surface refuses

- **WHEN** a Standard User whose `CreateChild` on the playlist collection is denied creates a playlist
- **THEN** the system answers `403 Forbidden` and creates nothing

#### Scenario: SCN-AUTHORIZATION-023-G — A standard user cannot create music catalogue content

- **WHEN** a Standard User creates a music group or a music album under the catalogue
- **THEN** the system answers `403 Forbidden` and creates nothing

### Requirement: REQ-AUTHORIZATION-029 — Every decision returns an explanation

`authorize()` SHALL return a decision with whether it allowed, the reason (root, owner, visibility, grant, relation,
deny, upload, own_account or default), the matching grants, the denying grant when one exists, the authorization path
and the resource's visibility. The explanation SHALL be available to the permission API and the logs, and SHALL NOT
expose grant internals to an ordinary caller.

#### Scenario: SCN-AUTHORIZATION-029-A — An allow names its reason

- **WHEN** a grant allows a viewer action
- **THEN** the explanation names the grant

#### Scenario: SCN-AUTHORIZATION-029-B — A deny names its cause

- **WHEN** a deny refuses an action
- **THEN** the explanation names the denying grant and the authorization path

#### Scenario: SCN-AUTHORIZATION-029-C — An ordinary caller sees no grants

- **WHEN** an ordinary caller receives a denial
- **THEN** the response carries no grant or principal detail

## REMOVED Requirements

### Requirement: REQ-AUTHORIZATION-010 — Administration actions are root-only

**Reason**: `ChangeRole` applies only to a `User` resource, so its who-may rule belongs to the `user` capability. The
action catalog stays in `authorization` (`REQ-AUTHORIZATION-006`).

**Migration**: The rule is stated by `REQ-USER-024`.

### Requirement: REQ-AUTHORIZATION-018 — Upload access is owner-only

**Reason**: Upload access is a rule about an upload resource, not about the authorization engine. The `upload`
capability owns the session's lifecycle and now owns who may address it.

**Migration**: The rule is stated by `REQ-UPLOAD-018`; `REQ-AUTHORIZATION-015` keeps the upload special case in its
decision order.

### Requirement: REQ-AUTHORIZATION-019 — Account administration is root and admin

**Reason**: Account administration is a `User` resource rule. The `user` capability owns the account and now owns the
administration rule.

**Migration**: The rule is stated by `REQ-USER-021`.

### Requirement: REQ-AUTHORIZATION-020 — Admin permissions are bounded

**Reason**: The rule bounds which `User` resources an Admin may administer; it is part of account administration, not of
grant validation or the permission API.

**Migration**: The rule is stated by `REQ-USER-022`.

### Requirement: REQ-AUTHORIZATION-021 — A caller reads its own account

**Reason**: The own-account read is a special case on a `User` resource. The `user` capability owns the account read and
now owns the exception.

**Migration**: The rule is stated by `REQ-USER-023`; `REQ-AUTHORIZATION-015` keeps the own-account special case in its
decision order.

### Requirement: REQ-AUTHORIZATION-022 — Catalogue resources are unowned and shared

**Reason**: The rule describes the catalogue resources of `movies`, `series` and `music`, not a property of every
resource. Each catalogue capability now states the ownership and write rules of its own resources.

**Migration**: The film rule is `REQ-MOVIES-006`, the show rule is `REQ-SERIES-014`, and the music rule is
`REQ-MUSIC-031`. `REQ-AUTHORIZATION-012` keeps the general ownership rule.

### Requirement: REQ-AUTHORIZATION-024 — Content operations follow the generic actions

**Reason**: The requirement restates the action bindings of the catalogue resources. Each catalogue capability now
states the actions its own reads and writes require, and `REQ-AUTHORIZATION-002` keeps the parent chain.

**Migration**: The film facts are `REQ-MOVIES-006` and `REQ-MOVIES-007`, the show facts are `REQ-SERIES-014` and
`REQ-SERIES-015`, and the music facts are `REQ-MUSIC-031` and `REQ-MUSIC-032`.

### Requirement: REQ-AUTHORIZATION-026 — Referencing content needs the container and the target

**Reason**: The concrete pairs belong to the capabilities that serve the write: a gallery item to `gallery`, a playlist
entry to `music`, and a cover to `cover-art`. The generic all-of rule remains `REQ-AUTHORIZATION-040`. The gallery pair
also removes a conflict: `REQ-GALLERY-007-E` answers `422` for a missing or unowned medium, which matches the code and
`REQ-API-013`; this requirement's `403` was stale.

**Migration**: `REQ-GALLERY-007` owns the gallery pair, `REQ-MUSIC-025` owns the playlist pair, and `REQ-COVER-ART-008`
owns the cover pair. `REQ-AUTHORIZATION-040` keeps the generic second-resource rule.
