# Spec Delta

## MODIFIED Requirements

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

- **WHEN** the addressed kind and identifier resolve to no resource, or to one the caller may not discover
- **THEN** the read path answers `404 Not Found`, indistinguishable from a missing resource

### Requirement: REQ-AUTHORIZATION-022 — Catalogue resources are unowned and shared

A film, show, album, medium, recording or music group SHALL live under the catalogue container and SHALL have no owner.
Its reads SHALL follow `View` and `Stream`; its writes SHALL follow `EditMetadata`, `ReplaceMedia`, `CreateChild` and
`Delete` granted on the resource or an ancestor. A Standard User SHALL NOT edit or privatise an unowned catalogue
resource by default.

#### Scenario: SCN-AUTHORIZATION-022-A — A standard user reads the catalogue

- **WHEN** a Standard User with the seeded role reads a film, a show, a music group or an album
- **THEN** the read allows

#### Scenario: SCN-AUTHORIZATION-022-B — A standard user cannot write the catalogue

- **WHEN** a Standard User with the seeded role renames a film, a music group or an album
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

- **WHEN** a caller other than the root creates an Admin account
- **THEN** the system answers `403 Forbidden` and creates nothing
- **AND** the root's own creation of an Admin account succeeds

#### Scenario: SCN-AUTHORIZATION-023-E — A creation surface is not an authorization parent

- **WHEN** a caller creates a gallery through the gallery creation surface
- **THEN** the gallery's authorization parent is the library, not the surface

#### Scenario: SCN-AUTHORIZATION-023-F — A denied creation surface refuses

- **WHEN** a Standard User whose `CreateChild` on the playlist collection is denied creates a playlist
- **THEN** the system answers `403 Forbidden` and creates nothing

#### Scenario: SCN-AUTHORIZATION-023-G — A standard user cannot create music catalogue content

- **WHEN** a Standard User creates a music group or a music album under the catalogue
- **THEN** the system answers `403 Forbidden` and creates nothing

### Requirement: REQ-AUTHORIZATION-039 — A filtered listing is not a denial

A listing SHALL return only the resources the caller may discover.

#### Scenario: SCN-AUTHORIZATION-039-A — A listing is filtered

- **WHEN** a caller may discover only some films, albums or stored files
- **THEN** each listing returns only the items the caller may discover

## ADDED Requirements

### Requirement: REQ-AUTHORIZATION-040 — An operation checks the second resource it names

An operation that addresses a second resource SHALL check the caller's action on it beside the action its own resource
requires, and SHALL apply nothing when either check refuses.

#### Scenario: SCN-AUTHORIZATION-040-A — An unreadable recording refuses the append

- **WHEN** a caller may add to a playlist but may not read the metadata of the recording it names
- **THEN** the system answers `403 Forbidden` and appends nothing
