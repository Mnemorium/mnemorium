# Spec Delta

## ADDED Requirements

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
