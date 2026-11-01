# Spec Delta

## MODIFIED Requirements

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

### Requirement: Default permissions follow the role presets

When a caller has no override, the role preset SHALL decide. A Standard User SHALL read every library film, show and
album, read and moderate public or owned galleries and playlists, manage only owned content, and read or write only
owned uploads. An Admin SHALL additionally read, list, create and update user accounts, except that only the Root Admin
may create an Admin or change a role or credential.

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
