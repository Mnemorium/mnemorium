# Spec Delta

## Purpose

Lets an authorized administrator remove a user account together with the data it owns, while preserving media that
another user's resources or the shared catalogue still reference.

## ADDED Requirements

### Requirement: Deleting a user account requires the user.delete permission

Every request to delete a user account SHALL require a valid bearer token and an effective `user.delete` scope that
covers the target. The Standard preset SHALL be `NONE`, the Admin preset `ALL`, and the Root Admin SHALL be allowed on
every target. A request without a valid token SHALL answer `401 Unauthorized`; a denied caller SHALL answer
`403 Forbidden` and delete nothing.

#### Scenario: Missing or invalid credentials

- **WHEN** a deletion request reaches the endpoint without a valid bearer token
- **THEN** the system returns `401 Unauthorized` and deletes nothing

#### Scenario: Standard User cannot delete an account

- **WHEN** an authenticated Standard User with no permission override submits a deletion request
- **THEN** the system returns `403 Forbidden` and deletes nothing

#### Scenario: A Standard User granted user.delete deletes an account

- **WHEN** a Standard User whose `user.delete` override is `ALL` submits a deletion request for a Standard user
- **THEN** the system deletes the account

### Requirement: Only a higher authority may delete an Admin account

A caller that is not the Root Admin SHALL NOT delete an Admin account. The Root Admin SHALL be able to delete an Admin
account. The Root Admin's own account SHALL never be deleted by any caller. A refused target SHALL answer
`403 Forbidden` and delete nothing.

#### Scenario: Admin cannot delete another Admin

- **WHEN** an Admin who is not the Root Admin submits a deletion request for an Admin account
- **THEN** the system returns `403 Forbidden` and deletes nothing

#### Scenario: Admin cannot delete itself

- **WHEN** an Admin who is not the Root Admin submits a deletion request for its own account
- **THEN** the system returns `403 Forbidden` and deletes nothing

#### Scenario: Root Admin deletes an Admin

- **WHEN** the Root Admin submits a deletion request for an Admin account
- **THEN** the system deletes the account

#### Scenario: Root Admin's own account is refused

- **WHEN** any caller submits a deletion request for the Root Admin's account
- **THEN** the system returns `403 Forbidden` and deletes nothing

### Requirement: A successful deletion answers 204 and removes the identity atomically

A successful deletion SHALL answer `204 No Content` and, in one atomic unit of work, remove the target's permission
overrides, its credential and the user row. A rejected or failed deletion SHALL remove nothing. An unknown target SHALL
answer `404 Not Found`.

#### Scenario: Deleting an existing account

- **WHEN** an authorized caller deletes an existing user account
- **THEN** the system returns `204 No Content`, the account is no longer readable, and its stored permission overrides
  and credential no longer exist

#### Scenario: Unknown account

- **WHEN** an authorized caller deletes an identifier that names no user
- **THEN** the system returns `404 Not Found` and deletes nothing

#### Scenario: A failure removes nothing

- **WHEN** any part of the deletion fails
- **THEN** the account, its credential, its overrides and its owned data remain unchanged

### Requirement: Deletion removes the user's galleries, playlists, and upload sessions

Deleting a user SHALL remove every gallery it owns together with the gallery's items, every playlist it owns together
with its track placements, and every upload session it owns together with its chunks and staged content.

#### Scenario: Owned galleries are removed

- **WHEN** a user that owns a gallery with items is deleted
- **THEN** the gallery and its items no longer exist

#### Scenario: Owned playlists are removed

- **WHEN** a user that owns a playlist with tracks is deleted
- **THEN** the playlist and its track placements no longer exist

#### Scenario: Owned upload sessions are removed

- **WHEN** a user with an unfinished upload session is deleted
- **THEN** the session, its chunks and its staged content no longer exist

### Requirement: Media private to the deleted user is removed with its bytes

When no resource outside the deleted user's own galleries and playlists references a media row backed by one of the
user's files, the system SHALL delete the media row, the file row and the stored bytes. Byte removal SHALL happen after
the datastore commit and SHALL NOT fail the deletion.

#### Scenario: An image used only in the user's own gallery

- **WHEN** a user whose image is referenced only by the user's own gallery is deleted
- **THEN** the image and file records are gone, the stored bytes are removed, and the asset answers `404 Not Found`

#### Scenario: An uploaded file with no media row

- **WHEN** a user whose upload produced a file with no media row is deleted
- **THEN** the file record and its stored bytes are removed

#### Scenario: The user's own public gallery is not a shared reference

- **WHEN** a user's image is referenced only by a public gallery the user owns
- **THEN** the image and file are deleted with the account

### Requirement: Media shared with another user or the catalogue survives under the Root Admin

When another user's gallery or playlist, or a catalogue resource, references a media row backed by the deleted user's
file, the system SHALL keep the media row and its bytes and SHALL transfer the file to the Root Admin. The referencing
resource SHALL remain readable after the deletion.

#### Scenario: An image in another user's gallery

- **WHEN** a user whose image is referenced by another user's gallery is deleted
- **THEN** the gallery item remains readable by a caller who can read that gallery, and the Root Admin can read the file

#### Scenario: A video backing a film

- **WHEN** a user whose video backs a film is deleted
- **THEN** the film remains readable and the video's bytes are kept

#### Scenario: A catalogue cover image

- **WHEN** a user whose image is referenced as catalogue artwork is deleted
- **THEN** the artwork remains readable through the catalogue resource

### Requirement: A transferred file whose digest the Root Admin already owns is stored without it

When a file is transferred to the Root Admin and the Root Admin already owns a file with the same integrity hash, the
system SHALL clear the transferred row's digest, because the per-owner unique key ignores a null digest. The file SHALL
remain readable.

#### Scenario: Two users uploaded identical content

- **WHEN** two users who uploaded byte-identical content are both deleted
- **THEN** both remaining files are readable, and the second transferred file carries no digest

### Requirement: Deletion ends the account's authority

After a user account is deleted, a token that names it SHALL answer `401 Unauthorized` on every authenticated endpoint,
and its former credentials SHALL no longer authenticate.

#### Scenario: The deleted user's token is rejected

- **WHEN** a request carrying the deleted user's former token reaches an authenticated endpoint
- **THEN** the system returns `401 Unauthorized`

#### Scenario: The deleted user's credentials are rejected

- **WHEN** the deleted user's former username and password are submitted to the login endpoint
- **THEN** the system returns `401 Unauthorized` and issues no token
