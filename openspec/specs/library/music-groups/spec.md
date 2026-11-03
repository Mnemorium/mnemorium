# library/music-groups Specification

## Purpose

Lets a client browse music groups, read one, and let an administrator create or rename one and attach artwork, so an
album can name the group it belongs to.

## Requirements

### Requirement: Group reads require an authenticated account

Every music group endpoint SHALL require a valid bearer token, and a caller whose `library.read` permission is `NONE`
SHALL receive `404 Not Found`.

#### Scenario: Missing or invalid credentials

- **WHEN** a request reaches a music group endpoint without a valid bearer token
- **THEN** the system returns `401 Unauthorized`

#### Scenario: A caller without library access

- **WHEN** a caller whose `library.read` permission is `NONE` requests a group
- **THEN** the system returns `404 Not Found`

### Requirement: List and read music groups

The system SHALL return the groups through `GET /api/v1/library/music/group`, with an optional `name` filter and
`limit`/`offset` pagination, and one group through `GET /api/v1/library/music/group/{group_id}`. A group representation
SHALL carry its identifier, name, `cover_image_id` and `_links.self`, plus `_links.cover` when artwork is set.

#### Scenario: Authenticated caller lists groups

- **WHEN** an authenticated caller requests the group collection
- **THEN** the system returns a HAL collection of group representations

#### Scenario: Name filter and page

- **WHEN** the caller requests the collection with `name` and `offset`
- **THEN** the system returns only the groups whose name matches, starting at `offset`

#### Scenario: Read one group

- **WHEN** an authenticated caller requests an existing group identifier
- **THEN** the system returns the group representation

#### Scenario: Unknown group

- **WHEN** the requested group identifier is unknown
- **THEN** the system returns `404 Not Found`

### Requirement: Only an administrator creates or renames a group

The system SHALL allow `POST /api/v1/library/music/group` and `PATCH /api/v1/library/music/group/{group_id}` only to a
caller whose `library.manage` covers the group. The name SHALL be trimmed, non-empty and unique, and `cover_image_id`
SHALL reference a `LIBRARY` image.

#### Scenario: Administrator creates a group

- **WHEN** an administrator submits a new name and an optional cover image
- **THEN** the system creates the group and returns `201 Created` with its representation

#### Scenario: Standard User cannot write a group

- **WHEN** a Standard User submits a group creation or rename
- **THEN** the system returns `403 Forbidden` and applies nothing

#### Scenario: Duplicate name

- **WHEN** an administrator creates a group whose name already exists
- **THEN** the system returns `409 Conflict` and creates nothing

#### Scenario: Invalid name or cover

- **WHEN** the name is empty after trimming, or the cover references a gallery image
- **THEN** the system returns `422 Unprocessable Content` or `409 Conflict` and applies nothing

### Requirement: An album names its group

An album representation SHALL carry `_links.group` naming the group it belongs to, and an album creation request SHALL
accept the group identifier it belongs to.

#### Scenario: Album representation

- **WHEN** the system returns an album
- **THEN** `_links.group` names the album's group

#### Scenario: Album creation

- **WHEN** an administrator creates an album with a group identifier
- **THEN** the album is stored with that group and its unique `(name, group)` identity

### Requirement: The default group is protected

The seeded group 0 (`Unknown`) SHALL be readable like any other group and SHALL NOT be renamed or deleted.

#### Scenario: The default group is readable

- **WHEN** a caller reads group 0
- **THEN** the system returns its representation

#### Scenario: The default group cannot be changed

- **WHEN** an administrator renames group 0
- **THEN** the write is refused and nothing is stored
