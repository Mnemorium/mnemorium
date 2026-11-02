# Spec Delta

## ADDED Requirements

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

## MODIFIED Requirements

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
