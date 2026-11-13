# Spec Delta

## Purpose

Defines the user account: its attributes and profile, and the create, read, list, update and delete operations.
Authorization and authentication are specified by their own capabilities.

## ADDED Requirements

### Requirement: REQ-USER-001 — An account has a stable profile

An account SHALL consist of a unique immutable numeric identifier, a username, an optional email address and a role.

#### Scenario: SCN-USER-001-A — Account profile

- **WHEN** an account exists
- **THEN** it carries an identifier, a username, an email or no email, and a role

### Requirement: REQ-USER-002 — A role is one of the declared values

The role of an account SHALL be `ADMIN` or `STANDARD`. A create or update request SHALL accept either value, and the
account profile SHALL carry the stored role.

#### Scenario: SCN-USER-002-A — A declared role is stored

- **WHEN** a request carries the role `ADMIN` or `STANDARD`
- **THEN** the account stores and returns that role

#### Scenario: SCN-USER-002-B — An undeclared role is rejected

- **WHEN** a request carries a role outside the declared values
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

### Requirement: REQ-USER-003 — Usernames are bounded, lowercase, and unique

A username SHALL be at least 4 and at most 100 characters long, SHALL contain no uppercase letter, and SHALL not begin
or end with whitespace. The system SHALL store the submitted value unchanged and SHALL NOT trim, lowercase or otherwise
normalize it. It SHALL be unique across accounts, compared exactly.

#### Scenario: SCN-USER-003-A — Surrounding whitespace is rejected

- **WHEN** a request carries a username that begins or ends with whitespace
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

#### Scenario: SCN-USER-003-B — Uppercase letters are rejected

- **WHEN** a request carries a username containing an uppercase letter
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

#### Scenario: SCN-USER-003-C — Boundary lengths are accepted

- **WHEN** a request carries a username of exactly 4 or exactly 100 characters
- **THEN** the account is created with that username

#### Scenario: SCN-USER-003-D — Username is too short

- **WHEN** a request carries an empty, blank, or 3-character username
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

#### Scenario: SCN-USER-003-E — Username is too long

- **WHEN** a request carries a username of 101 characters or more
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

#### Scenario: SCN-USER-003-F — Username is already taken

- **WHEN** a request carries a username another account already holds
- **THEN** the system answers `409 Conflict` with a message stating that the username is taken, and stores nothing

### Requirement: REQ-USER-004 — Email is optional, validated, and unique

An email SHALL be optional; an absent or null value SHALL be stored as no email. When present, it SHALL be a
syntactically valid mailbox without a display name and SHALL be unique across accounts, compared case-insensitively. The
submitted value SHALL be stored unchanged.

#### Scenario: SCN-USER-004-A — Email is omitted

- **WHEN** a creation request carries no email value
- **THEN** the account is created with no email address

#### Scenario: SCN-USER-004-B — Email is null

- **WHEN** a creation request carries an explicit null email
- **THEN** the account is created with no email address

#### Scenario: SCN-USER-004-C — Surrounding whitespace is invalid

- **WHEN** a request carries a valid email address surrounded by whitespace
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

#### Scenario: SCN-USER-004-D — Invalid format

- **WHEN** a request carries a value that is not a valid email address
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

#### Scenario: SCN-USER-004-E — Display name is invalid

- **WHEN** a request carries a mailbox with a display name, such as `Alice <alice@example.com>`
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

#### Scenario: SCN-USER-004-F — Email is already taken

- **WHEN** a request carries an email address another account already holds
- **THEN** the system answers `409 Conflict` with a message stating that the email is taken, and stores nothing

#### Scenario: SCN-USER-004-G — Case does not distinguish two email addresses

- **WHEN** one account holds `alice@example.com` and another is created with `Alice@example.com`
- **THEN** the system answers `409 Conflict` with a message stating that the email is taken, and creates nothing

### Requirement: REQ-USER-005 — The account profile exposes the account's detail

The system SHALL expose an account's profile, carrying its identifier, username, email or no email, and role.

#### Scenario: SCN-USER-005-A — Profile detail

- **WHEN** an account's profile is exposed
- **THEN** it carries the account's identifier, username, email or no email, and role

### Requirement: REQ-USER-006 — A creation request declares the initial values

A creation request SHALL carry a username, a password and a role, and MAY carry an email. A body that is not valid JSON
SHALL answer `400 Bad Request`; a missing required value, a value of the wrong type, or an undeclared role SHALL answer
`422 Unprocessable Content`.

#### Scenario: SCN-USER-006-A — Malformed request body

- **WHEN** a creation request body cannot be parsed as JSON
- **THEN** the system answers `400 Bad Request`

#### Scenario: SCN-USER-006-B — Required value is missing

- **WHEN** a creation request omits the username, password or role
- **THEN** the system answers `422 Unprocessable Content`

#### Scenario: SCN-USER-006-C — Value has the wrong type

- **WHEN** a creation request carries a value whose type does not match the request schema
- **THEN** the system answers `422 Unprocessable Content`

#### Scenario: SCN-USER-006-D — Role is not in the discrete list

- **WHEN** a creation request carries a role other than `ADMIN` or `STANDARD`
- **THEN** the system answers `422 Unprocessable Content`

### Requirement: REQ-USER-007 — A valid request creates the account and its credential atomically

A creation request carrying valid values SHALL create the account and the credential that authenticates it in one unit
of work. A rejected or failed creation SHALL create neither.

#### Scenario: SCN-USER-007-A — Created account

- **WHEN** a creation request carries a valid username, password, role and email
- **THEN** the account exists with those values and exactly one credential

#### Scenario: SCN-USER-007-B — Rejected request leaves nothing

- **WHEN** a creation request is rejected for an invalid value or a duplicate username or email
- **THEN** no account exists for the requested username and no credential is left behind

#### Scenario: SCN-USER-007-C — Failed creation leaves nothing

- **WHEN** storing either the account or its credential fails
- **THEN** neither is committed

### Requirement: REQ-USER-008 — A created account is returned with its location

A successful creation SHALL answer `201 Created` with the account profile and SHALL carry the account's URI in the
`Location` header.

#### Scenario: SCN-USER-008-A — Created profile

- **WHEN** the system creates an account
- **THEN** the response carries `201 Created` and the account profile

#### Scenario: SCN-USER-008-B — Location addresses the account

- **WHEN** the system returns the created account
- **THEN** the `Location` header carries the URI of the created account

### Requirement: REQ-USER-009 — An account is readable by identifier

A request for an account by identifier SHALL answer `200 OK` with its profile. A path segment that is not a numeric
identifier SHALL answer `400 Bad Request`, and an identifier that names no account SHALL answer `404 Not Found`.

#### Scenario: SCN-USER-009-A — Known account

- **WHEN** a request addresses an existing account's identifier
- **THEN** the system answers `200 OK` with that account's profile

#### Scenario: SCN-USER-009-B — Unknown account

- **WHEN** a request addresses an identifier that names no account
- **THEN** the system answers `404 Not Found`

#### Scenario: SCN-USER-009-C — Malformed identifier

- **WHEN** a request addresses a path segment that is not a number
- **THEN** the system answers `400 Bad Request`

### Requirement: REQ-USER-010 — The user collection is accessible with or without filters

The user collection SHALL be accessible with or without filters. Without a filter it SHALL answer `200 OK` with every
account; with one or more of `username`, `email` and `role` it SHALL answer `200 OK` with the accounts matching every
supplied filter. An `email` filter SHALL match case-insensitively, and an empty result SHALL carry no items.

#### Scenario: SCN-USER-010-A — No filter returns every account

- **WHEN** the collection is requested without a filter
- **THEN** every account appears as an item

#### Scenario: SCN-USER-010-B — Filters narrow the result

- **WHEN** the collection is requested with a `username`, `email` or `role` filter
- **THEN** only the accounts matching every supplied filter appear

#### Scenario: SCN-USER-010-C — Empty result

- **WHEN** no account matches the filters
- **THEN** the system answers `200 OK` with no items

#### Scenario: SCN-USER-010-D — Malformed filter

- **WHEN** a filter carries a username outside the length bounds, an invalid email address or an undeclared role
- **THEN** the system answers `422 Unprocessable Content`

#### Scenario: SCN-USER-010-E — The email filter ignores case

- **WHEN** the collection is requested with an `email` filter that differs from a stored address only by case
- **THEN** the matching account appears

### Requirement: REQ-USER-011 — A caller reads its own account

A request for the caller's own account SHALL answer `200 OK` with the profile of the account the request is
authenticated as.

#### Scenario: SCN-USER-011-A — Own account

- **WHEN** a request addresses the caller's own account
- **THEN** the system answers `200 OK` with that account's profile

### Requirement: REQ-USER-012 — An update changes only the submitted fields

An update request SHALL accept any of `username`, `email` and `role`, and an omitted field SHALL leave the stored value
unchanged. A successful update SHALL answer `200 OK` with the updated profile; an identifier that names no account SHALL
answer `404 Not Found`.

#### Scenario: SCN-USER-012-A — Username is changed

- **WHEN** an update request carries a new valid username
- **THEN** the stored username changes and the response carries it

#### Scenario: SCN-USER-012-B — Email is changed

- **WHEN** an update request carries a new valid email
- **THEN** the stored email changes and the response carries it

#### Scenario: SCN-USER-012-C — Role is changed

- **WHEN** an update request carries a declared role
- **THEN** the stored role changes and the response carries it

#### Scenario: SCN-USER-012-D — Omitted fields are unchanged

- **WHEN** an update request carries one field
- **THEN** every other stored value is unchanged

#### Scenario: SCN-USER-012-E — Empty body changes nothing

- **WHEN** an update request carries no field
- **THEN** the system answers `200 OK` and every stored value is unchanged

#### Scenario: SCN-USER-012-F — Unknown account

- **WHEN** an update addresses an identifier that names no account
- **THEN** the system answers `404 Not Found` and stores nothing

### Requirement: REQ-USER-013 — An update preserves the uniqueness of an attribute

A submitted username or email SHALL remain unique across accounts, compared as at creation. A value another account
already holds SHALL answer `409 Conflict` and change nothing.

#### Scenario: SCN-USER-013-A — Rename collides

- **WHEN** an update carries a username another account holds
- **THEN** the system answers `409 Conflict` and the stored values are unchanged

#### Scenario: SCN-USER-013-B — Email collides

- **WHEN** an update carries an email another account holds
- **THEN** the system answers `409 Conflict` and the stored values are unchanged

#### Scenario: SCN-USER-013-C — Keeping one's own value is not a collision

- **WHEN** an update carries the account's current username or email
- **THEN** the system answers `200 OK` and stores the value

#### Scenario: SCN-USER-013-D — A case-only email change collides

- **WHEN** an update carries an email that differs from another account's address only by case
- **THEN** the system answers `409 Conflict` and the stored values are unchanged

### Requirement: REQ-USER-014 — An update validates the submitted values

A submitted username or email SHALL satisfy the creation rules, and a role SHALL be one of the declared values. An
invalid value SHALL answer `422 Unprocessable Content` and change nothing, and a body that is not valid JSON SHALL
answer `400 Bad Request`.

#### Scenario: SCN-USER-014-A — Invalid username

- **WHEN** an update carries a username that breaks a creation rule
- **THEN** the system answers `422 Unprocessable Content` and the stored values are unchanged

#### Scenario: SCN-USER-014-B — Invalid email

- **WHEN** an update carries an invalid email address
- **THEN** the system answers `422 Unprocessable Content` and the stored values are unchanged

#### Scenario: SCN-USER-014-C — Undeclared role

- **WHEN** an update carries a role outside the declared values
- **THEN** the system answers `422 Unprocessable Content` and the stored values are unchanged

#### Scenario: SCN-USER-014-D — Malformed body

- **WHEN** an update body cannot be parsed as JSON
- **THEN** the system answers `400 Bad Request` and the stored values are unchanged

### Requirement: REQ-USER-015 — Deletion removes the account atomically

A successful deletion SHALL answer `204 No Content` and, in one unit of work, remove the account, its credential and the
permission overrides recorded for it. A rejected or failed deletion SHALL remove nothing, and an identifier that names
no account SHALL answer `404 Not Found`.

#### Scenario: SCN-USER-015-A — Deleting an existing account

- **WHEN** an existing account is deleted
- **THEN** the system answers `204 No Content`, the account is no longer readable, and its credential and permission
  overrides no longer exist

#### Scenario: SCN-USER-015-B — Unknown account

- **WHEN** a deletion addresses an identifier that names no account
- **THEN** the system answers `404 Not Found` and deletes nothing

#### Scenario: SCN-USER-015-C — A failure removes nothing

- **WHEN** any part of the deletion fails
- **THEN** the account, its credential, its overrides and its owned data remain unchanged

### Requirement: REQ-USER-016 — Deletion removes the account's galleries, playlists, and upload sessions

Deleting an account SHALL remove every gallery it owns with the gallery's items, every playlist it owns with its track
placements, and every upload session it owns with its chunks and staged content.

#### Scenario: SCN-USER-016-A — Owned galleries are removed

- **WHEN** an account that owns a gallery with items is deleted
- **THEN** the gallery and its items no longer exist

#### Scenario: SCN-USER-016-B — Owned playlists are removed

- **WHEN** an account that owns a playlist with tracks is deleted
- **THEN** the playlist and its track placements no longer exist

#### Scenario: SCN-USER-016-C — Owned upload sessions are removed

- **WHEN** an account with an unfinished upload session is deleted
- **THEN** the session, its chunks and its staged content no longer exist

### Requirement: REQ-USER-017 — Media private to the deleted account is removed with its bytes

When no resource outside the deleted account's own galleries and playlists references a media row backed by one of its
files, the system SHALL delete the media row, the file row and the stored bytes. Byte removal SHALL happen after the
datastore commit and SHALL NOT fail the deletion.

#### Scenario: SCN-USER-017-A — An image used only in the account's own gallery

- **WHEN** an account whose image is referenced only by its own gallery is deleted
- **THEN** the image and file records are gone, the stored bytes are removed, and the asset answers `404 Not Found`

#### Scenario: SCN-USER-017-B — A file with no media row

- **WHEN** an account whose upload produced a file with no media row is deleted
- **THEN** the file record and its stored bytes are removed

#### Scenario: SCN-USER-017-C — The account's own public gallery is not a shared reference

- **WHEN** an account's image is referenced only by a public gallery the account owns
- **THEN** the image and file are deleted with the account

### Requirement: REQ-USER-018 — Media another resource still references survives under account 0

When another account's gallery or playlist, or a library resource, references a media row backed by the deleted
account's file, the system SHALL keep the media row and its bytes and transfer the file to the account with identifier
`0`. The referencing resource SHALL remain readable.

#### Scenario: SCN-USER-018-A — An image in another account's gallery

- **WHEN** an account whose image is referenced by another account's gallery is deleted
- **THEN** the gallery item remains readable, and the account with identifier `0` owns the file

#### Scenario: SCN-USER-018-B — A video backing a film

- **WHEN** an account whose video backs a film is deleted
- **THEN** the film remains readable and the video's bytes are kept

#### Scenario: SCN-USER-018-C — A catalogue cover image

- **WHEN** an account whose image is referenced as catalogue artwork is deleted
- **THEN** the artwork remains readable through the catalogue resource

### Requirement: REQ-USER-019 — A transferred file whose digest account 0 already owns is stored without it

When a transferred file's integrity hash is already held by a file the account with identifier `0` owns, the transferred
row's digest SHALL be cleared so the per-owner unique key does not collide. The file SHALL remain readable.

#### Scenario: SCN-USER-019-A — Two accounts uploaded identical content

- **WHEN** two accounts that uploaded byte-identical content are both deleted
- **THEN** both remaining files are readable, and the second transferred file carries no digest
