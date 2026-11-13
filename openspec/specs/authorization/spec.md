# authorization Specification

## Purpose

Defines who may perform which action on which resource: every authorizable entity is a resource in one
authorization-parent tree, every share is a grant on a principal, and every decision resolves through one fixed order
with uniform, concealed denial semantics.

## Requirements

### Requirement: REQ-AUTHORIZATION-001 — Every authorizable entity is a resource

The system SHALL model every authorizable entity as a **resource**: a surrogate identifier, a **resource kind**, an
optional **owner**, an optional **authorization parent**, and a **visibility**. The system SHALL resolve the kind, owner
and parent from stored state, and SHALL never trust a client-supplied kind, owner or parent. A resource that does not
exist SHALL be indistinguishable from one the caller may not discover.

#### Scenario: SCN-AUTHORIZATION-001-A — A resource resolves its stored facts

- **WHEN** a caller addresses an existing gallery
- **THEN** the system decides against the stored kind, owner, authorization parent and visibility

#### Scenario: SCN-AUTHORIZATION-001-B — A client claim is ignored

- **WHEN** a request body or query names an owner or a kind that differs from the stored resource
- **THEN** the decision uses the stored value and the claim changes nothing

#### Scenario: SCN-AUTHORIZATION-001-C — An unknown resource is concealed

- **WHEN** the addressed kind and identifier resolve to no resource
- **THEN** the system answers `404 Not Found` through the read path

### Requirement: REQ-AUTHORIZATION-002 — The resource tree defines authorization parents

The **authorization parent** SHALL be the single canonical permission-inheritance edge. The resource tree SHALL be the
one drawn by the PlantUML diagram in `docs/Permissions.md` § Resource tree; that diagram is the canonical picture, and
this requirement states the rule rather than a second copy of the edges.

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

### Requirement: REQ-AUTHORIZATION-003 — Containment and reference edges lend read access only

A **resource relation** SHALL be `AlbumMember`, `PlaylistEntry` or `GroupMember`. It SHALL lend `Discover`,
`ViewMetadata`, `View` and `Stream` from its container to the target and the target's authorization-parent descendants.
It SHALL NOT lend `Download`, a write, a share or an admin action, SHALL NOT become an authorization parent, and SHALL
lose to a `Deny`.

#### Scenario: SCN-AUTHORIZATION-003-A — A playlist lends listening

- **WHEN** a caller may view a playlist whose entry references a recording it may not otherwise view
- **THEN** the recording and its audio are discoverable, their metadata is readable and their stream allows

#### Scenario: SCN-AUTHORIZATION-003-B — A playlist does not lend download or editing

- **WHEN** a caller may view a playlist entry's recording
- **THEN** a download or an edit of that recording refuses

#### Scenario: SCN-AUTHORIZATION-003-C — A deny still wins

- **WHEN** the referenced recording carries a `Deny` for the caller
- **THEN** the stream refuses despite the playlist lend

#### Scenario: SCN-AUTHORIZATION-003-D — Album membership lends reading

- **WHEN** a caller may view an album and a recording is an `AlbumMember` of it
- **THEN** the recording is viewable and streamable

#### Scenario: SCN-AUTHORIZATION-003-E — Group membership lends reading

- **WHEN** a caller may view a music group and an album is a `GroupMember` of it
- **THEN** the album is viewable

### Requirement: REQ-AUTHORIZATION-004 — Authorization parents are single, acyclic and bounded

A resource SHALL have at most one authorization parent. Assigning one SHALL reject a cycle, and a decision SHALL NOT
walk more than 32 authorization-parent edges. An assignment that would create a cycle or exceed the bound SHALL apply
nothing.

#### Scenario: SCN-AUTHORIZATION-004-A — The bound admits 32 edges

- **WHEN** a resource sits 32 authorization-parent edges below the instance root
- **THEN** the decision reaches the root

#### Scenario: SCN-AUTHORIZATION-004-B — The bound refuses 33 edges

- **WHEN** an assignment would place a resource 33 edges below the instance root
- **THEN** the system rejects the assignment and stores nothing

#### Scenario: SCN-AUTHORIZATION-004-C — A cycle is refused

- **WHEN** an assignment would make a resource its own ancestor
- **THEN** the system rejects the assignment and stores nothing

### Requirement: REQ-AUTHORIZATION-005 — A grant names one principal

A **principal** SHALL be a user, a role or the root. A grant SHALL name exactly one principal. A user grant SHALL apply
only to that user; a role grant SHALL apply to every user holding that role; the root SHALL hold no stored grant.

#### Scenario: SCN-AUTHORIZATION-005-A — A user grant applies to one user

- **WHEN** a grant names one user as its principal
- **THEN** another user of the same role does not receive it

#### Scenario: SCN-AUTHORIZATION-005-B — A role grant applies to the role

- **WHEN** a grant names the `Admin` role as its principal
- **THEN** every Admin receives it and a Standard User does not

#### Scenario: SCN-AUTHORIZATION-005-C — The root holds no grant

- **WHEN** a write names the root as a principal
- **THEN** the system rejects it with `422 Unprocessable Content` and stores nothing

### Requirement: REQ-AUTHORIZATION-006 — The action catalog is fixed and stable

The catalog SHALL be `Discover`, `ViewMetadata`, `View`, `Stream`, `Download`, `CreateChild`, `EditMetadata`,
`ReplaceMedia`, `Share`, `ManagePermissions`, `Delete`, `ChangeRole` and `ReplaceCredential`, stored as `discover`,
`view_metadata`, `view`, `stream`, `download`, `create_child`, `edit_metadata`, `replace_media`, `share`,
`manage_permissions`, `delete`, `change_role` and `replace_credential`. Every action SHALL round-trip through its stored
name, and an unknown action SHALL be refused.

#### Scenario: SCN-AUTHORIZATION-006-A — Every action round-trips

- **WHEN** each catalog action is stored and read back
- **THEN** the same action is returned

#### Scenario: SCN-AUTHORIZATION-006-B — An unknown action is refused

- **WHEN** a representation names an action outside the catalog
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

### Requirement: REQ-AUTHORIZATION-007 — Read actions distinguish discovery, metadata, content, stream and download

`Discover` SHALL report that a resource exists and include it in a listing. `ViewMetadata` SHALL read its metadata
without its content. `View` SHALL read its rendered content. `Stream` SHALL play its audio or video bytes. `Download`
SHALL retrieve its original bytes. `View` SHALL NOT imply `Download`, and `Discover` SHALL NOT imply `ViewMetadata` or
`View`.

#### Scenario: SCN-AUTHORIZATION-007-A — Restricted visibility separates discovery from content

- **WHEN** a caller relies on a `Restricted` resource's visibility
- **THEN** `Discover` and `ViewMetadata` allow, while `View` and `Stream` refuse

#### Scenario: SCN-AUTHORIZATION-007-B — View does not imply download

- **WHEN** a caller holds a role that includes `View` but not `Download`
- **THEN** the content is readable and the download is refused

#### Scenario: SCN-AUTHORIZATION-007-C — Stream is its own action

- **WHEN** a caller holds a role that includes `Stream`
- **THEN** audio and video bytes are served and a file download remains a separate decision

### Requirement: REQ-AUTHORIZATION-008 — Write actions distinguish creation, metadata, media and deletion

`CreateChild` SHALL create a child and SHALL carry an explicit `ChildKind`. `EditMetadata` SHALL change metadata and
SHALL NOT change visibility, ownership or a child. `ReplaceMedia` SHALL replace binary content. `Delete` SHALL remove
the resource. None of the four SHALL imply another.

#### Scenario: SCN-AUTHORIZATION-008-A — Creation names the child kind

- **WHEN** a caller creates a child without naming its `ChildKind`
- **THEN** the system answers `422 Unprocessable Content`

#### Scenario: SCN-AUTHORIZATION-008-B — Metadata does not change visibility

- **WHEN** a caller holds `EditMetadata` but not `ManagePermissions`
- **THEN** a metadata write applies and a visibility change is refused

#### Scenario: SCN-AUTHORIZATION-008-C — Media replacement is separate

- **WHEN** a caller holds `ReplaceMedia` but not `EditMetadata`
- **THEN** the bytes are replaced and a metadata write is refused

#### Scenario: SCN-AUTHORIZATION-008-D — Deletion is separate

- **WHEN** a caller holds `Delete` but not `EditMetadata`
- **THEN** the resource is deleted and a metadata write before it is refused

### Requirement: REQ-AUTHORIZATION-009 — Sharing and permission administration are distinct

`Share` SHALL add or revoke `Viewer` and `Downloader` grants and SHALL NOT grant a higher role or a `Deny`.
`ManagePermissions` SHALL change visibility and replace the complete user grant set. `ManagePermissions` SHALL NOT be
grantable and SHALL NOT belong to the `Manager` bundle.

#### Scenario: SCN-AUTHORIZATION-009-A — Sharing is capped

- **WHEN** a caller holding only `Share` submits a `Manager` grant
- **THEN** the system answers `403 Forbidden` and stores nothing

#### Scenario: SCN-AUTHORIZATION-009-B — Administration grants a higher role

- **WHEN** a caller holding `ManagePermissions` submits a `Manager` grant
- **THEN** the system stores it

#### Scenario: SCN-AUTHORIZATION-009-C — A manager does not administer

- **WHEN** a caller holding `Manager` without `ManagePermissions` changes visibility
- **THEN** the system answers `403 Forbidden`

### Requirement: REQ-AUTHORIZATION-010 — Administration actions are root-only

`ChangeRole` SHALL change a user's role and `ReplaceCredential` SHALL replace a credential; both SHALL be available only
to the root. `CreateChild` with `ChildKind::Admin` SHALL be available only to the root and SHALL NOT be grantable.

#### Scenario: SCN-AUTHORIZATION-010-A — Only the root changes a role

- **WHEN** a non-root caller submits `ChangeRole`
- **THEN** the system answers `403 Forbidden` and applies nothing

#### Scenario: SCN-AUTHORIZATION-010-B — Only the root replaces a credential

- **WHEN** a non-root caller submits `ReplaceCredential`
- **THEN** the system answers `403 Forbidden` and applies nothing

#### Scenario: SCN-AUTHORIZATION-010-C — Admin creation is not grantable

- **WHEN** a grant names `CreateChild` with `ChildKind::Admin`
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

### Requirement: REQ-AUTHORIZATION-011 — Resource roles are bundles

A **resource role** SHALL bundle actions: `Viewer` = `Discover`, `ViewMetadata`, `View`, `Stream`; `Downloader` =
`Viewer` + `Download`; `Contributor` = `Viewer` + `CreateChild`; `Editor` = `Contributor` + `EditMetadata`,
`ReplaceMedia`; `Manager` = `Editor` + `Share`, `Delete`; `Owner` = `Manager` + `ManagePermissions`.

#### Scenario: SCN-AUTHORIZATION-011-A — A viewer reads

- **WHEN** a caller holds `Viewer` on a resource
- **THEN** `Discover`, `ViewMetadata`, `View` and `Stream` allow and `Download` refuses

#### Scenario: SCN-AUTHORIZATION-011-B — A manager shares and deletes

- **WHEN** a caller holds `Manager` on a resource
- **THEN** `Share` and `Delete` allow and `ManagePermissions` refuses

#### Scenario: SCN-AUTHORIZATION-011-C — The owner administers

- **WHEN** a caller is the resource's owner
- **THEN** the implicit `Owner` bundle allows `ManagePermissions`

### Requirement: REQ-AUTHORIZATION-012 — Ownership is implicit and resolved up the chain

The **owner** SHALL be the nearest non-null `owner_id` on the addressed resource or its authorization-parent chain. The
owning principal SHALL hold the implicit `Owner` role for that resource. `Owner` SHALL NOT be stored as a grant to a
user, and catalogue resources SHALL have no owner.

#### Scenario: SCN-AUTHORIZATION-012-A — An owner reaches a gallery's items

- **WHEN** a caller owns a gallery
- **THEN** the implicit `Owner` role reaches the gallery and its gallery items

#### Scenario: SCN-AUTHORIZATION-012-B — Catalogue resources have no owner

- **WHEN** a caller addresses a film, show, album or music group
- **THEN** no user holds the implicit `Owner` role for it

#### Scenario: SCN-AUTHORIZATION-012-C — Ownership is not grantable to a user

- **WHEN** a user grant names `Owner` as its role
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

### Requirement: REQ-AUTHORIZATION-013 — Grants have a principal, resource, role and effect

A **grant** SHALL carry a principal, a resource, a resource role, a **child kind** and an **effect** of `Allow` or
`Deny`. The child kind SHALL be `All` or a concrete kind and SHALL matter only when the role includes `CreateChild`.
Grants SHALL NOT be time-limited in this version; no expiry is stored or accepted.

#### Scenario: SCN-AUTHORIZATION-013-A — An allow grants its role

- **WHEN** a user holds an `Allow` grant of `Viewer`
- **THEN** the viewer actions allow on that resource

#### Scenario: SCN-AUTHORIZATION-013-B — A deny withholds its role

- **WHEN** a user holds a `Deny` grant of `Viewer`
- **THEN** the viewer actions refuse on that resource

#### Scenario: SCN-AUTHORIZATION-013-C — Expiry is not accepted

- **WHEN** a representation supplies an expiry for a grant
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

### Requirement: REQ-AUTHORIZATION-014 — Deny wins over every allow

A decision SHALL deny when any matching grant has the `Deny` effect, whatever other grants, ownership, role or
visibility would allow. A deny on an ancestor SHALL defeat an allow on a descendant, and a deny on a descendant SHALL
defeat an allow inherited from an ancestor.

#### Scenario: SCN-AUTHORIZATION-014-A — A deny defeats a peer allow

- **WHEN** a user holds both an `Allow` and a `Deny` for the same role and resource
- **THEN** the decision denies

#### Scenario: SCN-AUTHORIZATION-014-B — An ancestor deny defeats a child allow

- **WHEN** a `Deny` on a gallery and an `Allow` on one of its items both match
- **THEN** the decision on the item denies

#### Scenario: SCN-AUTHORIZATION-014-C — A child deny defeats an inherited allow

- **WHEN** an `Allow` on a gallery and a `Deny` on one of its items both match
- **THEN** the decision on the item denies

### Requirement: REQ-AUTHORIZATION-015 — The decision resolves special cases, then deny, then allow

A decision SHALL apply, in order: upload owner-only; the root; the own-account read; any matching `Deny`; any matching
`Allow` from a grant, the implicit owner, the role, the visibility or a relation's read lend; and otherwise `Deny`.

#### Scenario: SCN-AUTHORIZATION-015-A — The root allows

- **WHEN** the root addresses a resource it does not own
- **THEN** the decision allows, except for uploads

#### Scenario: SCN-AUTHORIZATION-015-B — Upload ownership precedes the root

- **WHEN** the root addresses an upload it does not own
- **THEN** the decision denies before the root rule

#### Scenario: SCN-AUTHORIZATION-015-C — No matching allow denies

- **WHEN** no grant, ownership, role, visibility or relation lend matches the action
- **THEN** the decision denies

### Requirement: REQ-AUTHORIZATION-016 — Visibility is a grant, not a second permission system

**Visibility** SHALL be `Private`, `Restricted` or `Public`. `Private` SHALL add no implicit grant. `Restricted` SHALL
grant `Discover` and `ViewMetadata` to every authenticated caller. `Public` SHALL grant `Viewer` to every authenticated
caller. No visibility SHALL imply `Download`, `EditMetadata`, `Share`, `Delete` or `ManagePermissions`. Visibility SHALL
apply only to the resource that carries it and SHALL NOT be inherited.

#### Scenario: SCN-AUTHORIZATION-016-A — Private is owner and grants only

- **WHEN** a `Private` playlist has no grant for the caller
- **THEN** the playlist is concealed

#### Scenario: SCN-AUTHORIZATION-016-B — Restricted exposes metadata

- **WHEN** a `Restricted` gallery is addressed by any authenticated caller
- **THEN** its metadata is readable and its items are not

#### Scenario: SCN-AUTHORIZATION-016-C — Public exposes the viewer actions

- **WHEN** a `Public` playlist is addressed by any authenticated caller
- **THEN** its metadata, entries and stream allow

#### Scenario: SCN-AUTHORIZATION-016-D — Public does not imply download

- **WHEN** a `Public` resource is addressed by a caller with no `Download` grant
- **THEN** the download refuses

#### Scenario: SCN-AUTHORIZATION-016-E — Visibility is not inherited

- **WHEN** a `Public` album holds a `Private` recording and the caller has no grant
- **THEN** the recording is concealed

#### Scenario: SCN-AUTHORIZATION-016-F — A deny beats visibility

- **WHEN** a `Public` resource carries a `Deny` for the caller
- **THEN** the decision denies

### Requirement: REQ-AUTHORIZATION-017 — The root is implicit owner and never denied

The root SHALL hold the implicit `Owner` role on every resource, SHALL bypass every `Deny`, and SHALL have no stored
grant. The root's own account SHALL remain immutable as a state rule outside the decision.

#### Scenario: SCN-AUTHORIZATION-017-A — The root reads any resource

- **WHEN** the root addresses another caller's private resource
- **THEN** the decision allows

#### Scenario: SCN-AUTHORIZATION-017-B — The root bypasses a deny

- **WHEN** a resource carries a `Deny` naming the root
- **THEN** the deny cannot be stored and the root's decision allows

#### Scenario: SCN-AUTHORIZATION-017-C — The root account is immutable

- **WHEN** any caller updates or deletes the root account
- **THEN** the system answers `403 Forbidden` and applies nothing

### Requirement: REQ-AUTHORIZATION-018 — Upload access is owner-only

An upload resource SHALL be read, written, completed and cancelled only by its owner. A non-owner SHALL receive
`404 Not Found`, no grant SHALL widen upload access, and the root SHALL not be exempt.

#### Scenario: SCN-AUTHORIZATION-018-A — A non-owner is concealed

- **WHEN** an authenticated caller addresses another user's upload
- **THEN** the system answers `404 Not Found` and applies nothing

#### Scenario: SCN-AUTHORIZATION-018-B — The root is not exempt

- **WHEN** the root addresses an upload it does not own
- **THEN** the system answers `404 Not Found`

#### Scenario: SCN-AUTHORIZATION-018-C — An upload grant is refused

- **WHEN** a grant names an upload resource
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

### Requirement: REQ-AUTHORIZATION-019 — Account administration is root and admin

Only the root SHALL create an Admin account. An Admin SHALL create, update, delete and read Standard accounts. A
non-root caller SHALL NOT update, delete or change the role of an Admin account, and no caller SHALL modify or delete
the root account.

#### Scenario: SCN-AUTHORIZATION-019-A — An admin creates a standard account

- **WHEN** an Admin creates a Standard account
- **THEN** the account is created

#### Scenario: SCN-AUTHORIZATION-019-B — An admin cannot create an admin

- **WHEN** an Admin creates an Admin account
- **THEN** the system answers `403 Forbidden` and creates nothing

#### Scenario: SCN-AUTHORIZATION-019-C — An admin cannot touch an admin

- **WHEN** an Admin updates, deletes or changes the role of another Admin
- **THEN** the system answers `403 Forbidden` and applies nothing

#### Scenario: SCN-AUTHORIZATION-019-D — The root manages admins

- **WHEN** the root updates, deletes or changes the role of an Admin account
- **THEN** the operation applies

#### Scenario: SCN-AUTHORIZATION-019-E — The root account is untouchable

- **WHEN** any caller updates or deletes the root account
- **THEN** the system answers `403 Forbidden` and applies nothing

### Requirement: REQ-AUTHORIZATION-020 — Admin permissions and credentials are bounded

An Admin SHALL read and replace the grants of a non-root user it administers. `ChangeRole` and `ReplaceCredential` SHALL
remain root-only.

#### Scenario: SCN-AUTHORIZATION-020-A — An admin edits a standard user's grants

- **WHEN** an Admin replaces a Standard user's grants
- **THEN** the replacement applies

#### Scenario: SCN-AUTHORIZATION-020-B — An admin cannot change a role

- **WHEN** an Admin submits `ChangeRole`
- **THEN** the system answers `403 Forbidden` and applies nothing

#### Scenario: SCN-AUTHORIZATION-020-C — An admin cannot replace a credential

- **WHEN** an Admin submits `ReplaceCredential`
- **THEN** the system answers `403 Forbidden` and applies nothing

### Requirement: REQ-AUTHORIZATION-021 — A caller reads its own account

A caller SHALL read its own `User` resource regardless of its grants, including when no grant covers it. Reading another
account SHALL follow the ordinary decision.

#### Scenario: SCN-AUTHORIZATION-021-A — The own account is readable

- **WHEN** a Standard User with no grant reads its own account
- **THEN** the system returns `200 OK`

#### Scenario: SCN-AUTHORIZATION-021-B — Another account follows the grants

- **WHEN** a Standard User with no grant reads another account
- **THEN** the system returns `403 Forbidden`

### Requirement: REQ-AUTHORIZATION-022 — Catalogue resources are unowned and shared

A film, show, album, medium, recording or music group SHALL live under the catalogue container and SHALL have no owner.
Its reads SHALL follow `View` and `Stream`; its writes SHALL follow `EditMetadata`, `ReplaceMedia`, `CreateChild` and
`Delete` granted on the resource or an ancestor. A Standard User SHALL NOT edit or privatise an unowned catalogue
resource by default.

#### Scenario: SCN-AUTHORIZATION-022-A — A standard user reads the catalogue

- **WHEN** a Standard User with the seeded role reads a film, a show or an album
- **THEN** the read allows

#### Scenario: SCN-AUTHORIZATION-022-B — A standard user cannot write the catalogue

- **WHEN** a Standard User with the seeded role edits a film
- **THEN** the system answers `403 Forbidden` and applies nothing

#### Scenario: SCN-AUTHORIZATION-022-C — An admin curates the catalogue

- **WHEN** an Admin with the seeded role edits an album
- **THEN** the write applies

#### Scenario: SCN-AUTHORIZATION-022-D — An unowned resource has no owner to privatise it

- **WHEN** a Standard User changes a film's visibility
- **THEN** the system answers `403 Forbidden`

### Requirement: REQ-AUTHORIZATION-023 — Creation names the child kind and the parent

`CreateChild` SHALL be decided against the target's authorization parent or a **creation surface** that names it, and
SHALL name a `ChildKind`. A surface SHALL NOT become the child's authorization parent. The seeded grants SHALL let a
Standard User create a gallery, a playlist and an upload, an Admin every catalogue kind, a gallery, a playlist and a
Standard account, and only the root an Admin account.

#### Scenario: SCN-AUTHORIZATION-023-A — A standard user creates a gallery

- **WHEN** a Standard User creates a gallery under the library
- **THEN** the gallery is created and owned by that caller

#### Scenario: SCN-AUTHORIZATION-023-B — A standard user cannot create a film

- **WHEN** a Standard User creates a film under the catalogue
- **THEN** the system answers `403 Forbidden` and creates nothing

#### Scenario: SCN-AUTHORIZATION-023-C — An admin creates content

- **WHEN** an Admin creates a music album under the catalogue
- **THEN** the album is created

#### Scenario: SCN-AUTHORIZATION-023-D — Only the root creates an admin

- **WHEN** the root creates an Admin account
- **THEN** the account is created

#### Scenario: SCN-AUTHORIZATION-023-E — A creation surface is not an authorization parent

- **WHEN** a caller creates a gallery through the gallery creation surface
- **THEN** the gallery's authorization parent is the library, not the surface

### Requirement: REQ-AUTHORIZATION-024 — Content operations follow the generic actions

A film, show, season, episode, album, medium, recording or music group SHALL be read through `View` and `Stream`, and
written through `EditMetadata`, `ReplaceMedia`, `CreateChild` and `Delete`. A child SHALL inherit the grants of its
authorization parent.

#### Scenario: SCN-AUTHORIZATION-024-A — A film is viewed

- **WHEN** a caller holds `View` on a film or its library ancestor
- **THEN** the film is readable

#### Scenario: SCN-AUTHORIZATION-024-B — An album's metadata is edited

- **WHEN** a caller holds `EditMetadata` on an album
- **THEN** the metadata write applies

#### Scenario: SCN-AUTHORIZATION-024-C — A season inherits its series

- **WHEN** a caller holds `EditMetadata` on a series
- **THEN** a season of that series is editable

#### Scenario: SCN-AUTHORIZATION-024-D — A recording inherits its medium

- **WHEN** a caller holds `Stream` on an album
- **THEN** a recording on one of its media is streamable

### Requirement: REQ-AUTHORIZATION-025 — Assets are resources with an authorization parent

A file, image, audio or video SHALL be a resource whose authorization parent is its typed asset, the gallery item, the
film, the episode, the recording or the library. `View`, `Stream` and `Download` SHALL follow that chain. An asset with
no authorization parent SHALL be readable only by its file owner and the root.

#### Scenario: SCN-AUTHORIZATION-025-A — Catalogue artwork follows its resource

- **WHEN** a caller holds `View` on an album
- **THEN** the album's artwork is readable

#### Scenario: SCN-AUTHORIZATION-025-B — A gallery image follows its gallery

- **WHEN** a caller holds `View` on a gallery
- **THEN** an image of one of its items is readable

#### Scenario: SCN-AUTHORIZATION-025-C — An orphan asset is owner-only

- **WHEN** a caller neither owns the file nor is the root and no authorization parent exists
- **THEN** the system answers `404 Not Found`

#### Scenario: SCN-AUTHORIZATION-025-D — A playlist cover follows its playlist

- **WHEN** a caller holds `View` on a playlist
- **THEN** its cover image is readable

### Requirement: REQ-AUTHORIZATION-026 — Referencing content needs the container and the target

Creating a gallery item SHALL need `CreateChild` on the gallery and `View` on the referenced image or video. Adding a
playlist entry SHALL need `CreateChild` on the playlist and `ViewMetadata` on the recording. Both checks SHALL be
all-of, and the request SHALL apply nothing when one refuses.

#### Scenario: SCN-AUTHORIZATION-026-A — A gallery item needs both

- **WHEN** a caller may create a gallery item but may not view the referenced image
- **THEN** the system answers `403 Forbidden` and adds nothing

#### Scenario: SCN-AUTHORIZATION-026-B — A playlist entry needs both

- **WHEN** a caller may add to a playlist but may not read the recording's metadata
- **THEN** the system answers `403 Forbidden` and adds nothing

#### Scenario: SCN-AUTHORIZATION-026-C — Both allow

- **WHEN** a caller holds both actions
- **THEN** the child is created

### Requirement: REQ-AUTHORIZATION-029 — Every decision returns an explanation

`authorize()` SHALL return a decision with whether it allowed, the reason (root, owner, visibility, grant, relation,
deny or default), the matching grants, the denying grant when one exists, the authorization path and the resource's
visibility. The explanation SHALL be available to the permission API and the logs, and SHALL NOT expose grant internals
to an ordinary caller.

#### Scenario: SCN-AUTHORIZATION-029-A — An allow names its reason

- **WHEN** a grant allows a viewer action
- **THEN** the explanation names the grant

#### Scenario: SCN-AUTHORIZATION-029-B — A deny names its cause

- **WHEN** a deny refuses an action
- **THEN** the explanation names the denying grant and the authorization path

#### Scenario: SCN-AUTHORIZATION-029-C — An ordinary caller sees no grants

- **WHEN** an ordinary caller receives a denial
- **THEN** the response carries no grant or principal detail

### Requirement: REQ-AUTHORIZATION-030 — A request is allowed only when every check allows

A request that checks several actions SHALL be allowed only when every check allows, and SHALL apply nothing when one
refuses. A mixed request SHALL not apply its permitted part.

#### Scenario: SCN-AUTHORIZATION-030-A — A mixed playlist update applies nothing

- **WHEN** a caller may read a playlist but may not add its track and sends both changes
- **THEN** the system answers `403 Forbidden` and applies neither

#### Scenario: SCN-AUTHORIZATION-030-B — A role change needs both actions

- **WHEN** a caller may update a user but may not change its role
- **THEN** the system answers `403 Forbidden` and applies nothing

### Requirement: REQ-AUTHORIZATION-031 — The user permission endpoint returns the effective summary

`GET /api/v1/identity/permission?user_id={user_id}` SHALL return a user's effective permissions. Supplying
`resource_kind`, `resource_id` and `action` SHALL also return the explanation for that decision. A caller SHALL read its
own summary; another user's summary SHALL need `ManagePermissions` on that `User` resource. The full explanation SHALL
be returned only to a caller holding `ManagePermissions`.

#### Scenario: SCN-AUTHORIZATION-031-A — A caller reads its own summary

- **WHEN** a caller requests its own permission summary
- **THEN** the system returns `200 OK` with the effective permissions

#### Scenario: SCN-AUTHORIZATION-031-B — An admin reads a standard user

- **WHEN** an Admin requests a Standard user's summary
- **THEN** the system returns `200 OK`

#### Scenario: SCN-AUTHORIZATION-031-C — Another user is refused

- **WHEN** a caller without `ManagePermissions` requests another user's summary
- **THEN** the system answers `403 Forbidden`

#### Scenario: SCN-AUTHORIZATION-031-D — The explanation is returned

- **WHEN** a caller holding `ManagePermissions` supplies a resource and an action
- **THEN** the response carries the decision and its explanation

### Requirement: REQ-AUTHORIZATION-032 — The resource permission endpoint reads and replaces grants

`GET /api/v1/identity/permission/resource/{kind}/{id}` SHALL return a resource's visibility, owner, grants and effective
roles; a caller SHALL need `Share` or `ManagePermissions` on it. `PUT` SHALL replace the visibility and the complete
user grant set atomically; a caller SHALL need `ManagePermissions`. A rejected replacement SHALL apply nothing.

#### Scenario: SCN-AUTHORIZATION-032-A — A sharer reads the grants

- **WHEN** a caller holding `Share` reads a resource's permission
- **THEN** the system returns `200 OK` with its visibility and grants

#### Scenario: SCN-AUTHORIZATION-032-B — An owner replaces the set

- **WHEN** the owner submits a new visibility and grant set
- **THEN** the system stores exactly that set and the next request uses it

#### Scenario: SCN-AUTHORIZATION-032-C — A manager cannot replace

- **WHEN** a caller holding `Manager` without `ManagePermissions` submits a replacement
- **THEN** the system answers `403 Forbidden` and stores nothing

#### Scenario: SCN-AUTHORIZATION-032-D — A rejected replacement leaves the previous set

- **WHEN** a replacement contains one invalid grant
- **THEN** the system returns the validation error and leaves the previous set unchanged

### Requirement: REQ-AUTHORIZATION-033 — The share endpoint adds and revokes read grants

`POST /api/v1/identity/permission/resource/{kind}/{id}/share` SHALL add or update one `Viewer` or `Downloader` `Allow`
grant, and `DELETE .../share/{grant_id}` SHALL revoke it. A caller SHALL need `Share` or `ManagePermissions`, and the
endpoint SHALL NOT grant a higher role or a `Deny`.

#### Scenario: SCN-AUTHORIZATION-033-A — A manager shares a viewer

- **WHEN** a caller holding `Manager` shares `Viewer`
- **THEN** the grant is stored

#### Scenario: SCN-AUTHORIZATION-033-B — Sharing cannot escalate

- **WHEN** a caller holding `Share` shares `Editor`
- **THEN** the system answers `403 Forbidden` and stores nothing

#### Scenario: SCN-AUTHORIZATION-033-C — A share is revoked

- **WHEN** a caller holding `Share` deletes the grant it may revoke
- **THEN** the grant is removed

### Requirement: REQ-AUTHORIZATION-034 — Grants are validated

A grant SHALL name a known principal, a known resource, a resource role from the grantable set (`Viewer`, `Downloader`,
`Contributor`, `Editor`, `Manager`) for a user principal, a child kind and an effect. A `CreateChild` grant SHALL name
its `ChildKind`, and `ChildKind::Admin` SHALL NOT be grantable. A duplicate grant, an unknown principal or resource, a
role outside the grantable set, or a malformed representation SHALL answer `422 Unprocessable Content` and store
nothing.

#### Scenario: SCN-AUTHORIZATION-034-A — The owner role is refused

- **WHEN** a grant names `Owner`
- **THEN** the system answers `422 Unprocessable Content`

#### Scenario: SCN-AUTHORIZATION-034-B — Permission administration is refused

- **WHEN** a grant names `ManagePermissions`
- **THEN** the system answers `422 Unprocessable Content`

#### Scenario: SCN-AUTHORIZATION-034-C — A duplicate is refused

- **WHEN** the set names the same principal, resource and role twice
- **THEN** the system answers `422 Unprocessable Content`

#### Scenario: SCN-AUTHORIZATION-034-D — An unknown principal is refused

- **WHEN** a grant names a user or role that does not exist
- **THEN** the system answers `422 Unprocessable Content`

#### Scenario: SCN-AUTHORIZATION-034-E — Admin creation is refused

- **WHEN** a grant names `CreateChild` with `ChildKind::Admin`
- **THEN** the system answers `422 Unprocessable Content`

### Requirement: REQ-AUTHORIZATION-035 — Role grants are seeded and immutable over HTTP

Role grants SHALL be seeded by the migration and SHALL NOT be editable through any HTTP endpoint. The resource
permission endpoint SHALL accept only user principals; a grant naming a role SHALL answer `422 Unprocessable Content`.

#### Scenario: SCN-AUTHORIZATION-035-A — A role grant cannot be written

- **WHEN** a replacement names a role principal
- **THEN** the system answers `422 Unprocessable Content`

#### Scenario: SCN-AUTHORIZATION-035-B — The seeded role applies

- **WHEN** a user with no per-resource grant reads a resource its role reaches
- **THEN** the seeded role grant decides

### Requirement: REQ-AUTHORIZATION-036 — Resolution runs in one transaction and defaults to deny on error

A decision SHALL read the resource, its authorization-parent chain and the matching grants in one transaction. A storage
error, a malformed row or a missing resource SHALL deny; no partial grant set SHALL be applied.

#### Scenario: SCN-AUTHORIZATION-036-A — A storage error denies

- **WHEN** the grant read fails
- **THEN** the decision denies and no action applies

#### Scenario: SCN-AUTHORIZATION-036-B — The chain is read once

- **WHEN** a request checks several actions on the same resource
- **THEN** the resource and its chain are read once for the request

### Requirement: REQ-AUTHORIZATION-037 — The migration drops the scope model and rebuilds the resource graph

The migration SHALL drop `permission` and `permission_resource`, create `resource`, `resource_relation` and `grant`,
backfill a resource row for every existing entity, seed the role grants, and translate no stored override. Existing
permission overrides SHALL be discarded.

#### Scenario: SCN-AUTHORIZATION-037-A — The seeded roles apply after migration

- **WHEN** the migration completes
- **THEN** each user's effective permissions are the seeded role grants

#### Scenario: SCN-AUTHORIZATION-037-B — No override survives

- **WHEN** the migration completes
- **THEN** no row from the old override tables remains

#### Scenario: SCN-AUTHORIZATION-037-C — Every entity has a resource row

- **WHEN** the migration completes
- **THEN** every gallery, playlist, film, show, season, episode, album, medium, recording, group, image, video, audio,
  file, upload and user resolves to a resource

### Requirement: REQ-AUTHORIZATION-038 — Denials are uniform and concealed

A caller that cannot `Discover` or `ViewMetadata` the addressed resource SHALL receive `404 Not Found`. A caller that
can read it but may not modify it SHALL receive `403 Forbidden`. An administration denial SHALL receive `403 Forbidden`.

#### Scenario: SCN-AUTHORIZATION-038-A — A hidden resource is concealed

- **WHEN** a caller cannot discover a private gallery
- **THEN** the system answers `404 Not Found`

#### Scenario: SCN-AUTHORIZATION-038-B — A visible write is revealed

- **WHEN** a caller can view a public playlist but may not edit it
- **THEN** the system answers `403 Forbidden`

### Requirement: REQ-AUTHORIZATION-039 — A filtered listing is not a denial

A listing SHALL return only the resources the caller may discover.

#### Scenario: SCN-AUTHORIZATION-039-A — A listing is filtered

- **WHEN** a caller may discover only some albums
- **THEN** the album listing returns only those albums
