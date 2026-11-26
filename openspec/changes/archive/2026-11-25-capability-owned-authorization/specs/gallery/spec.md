# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-GALLERY-008 — Read one item of a gallery

A caller who may `View` the gallery SHALL read one item of it. The representation SHALL carry the item identifier,
gallery identifier, added-at instant, medium identifier, medium type, display name, backing file identifier and
`_links.self`. The display name SHALL be the image name or, for a video, the file path. An unknown item, or one of
another gallery, SHALL answer `404`.

#### Scenario: SCN-GALLERY-008-A — An image item is read

- **WHEN** an authenticated caller reads an image item of a gallery it may view
- **THEN** the representation carries the item, its medium identifier and type, the image name and the file identifier

#### Scenario: SCN-GALLERY-008-B — A video item is read

- **WHEN** an authenticated caller reads a video item of a gallery it may view
- **THEN** the display name is the video file's path

#### Scenario: SCN-GALLERY-008-C — The item belongs to another gallery

- **WHEN** the item identifier exists in a gallery other than the addressed one, or does not exist
- **THEN** the system returns `404 Not Found`

## ADDED Requirements

### Requirement: REQ-GALLERY-012 — A gallery and its items are resources under the library

A gallery SHALL be a resource whose authorization parent is the library. Each gallery item SHALL be a resource whose
authorization parent is its gallery. The `GALLERY_COLLECTION` surface SHALL NOT become a parent.

#### Scenario: SCN-GALLERY-012-A — A gallery follows the library

- **WHEN** a gallery is created
- **THEN** its authorization parent is the library

#### Scenario: SCN-GALLERY-012-B — An item follows its gallery

- **WHEN** an item is added to a gallery
- **THEN** its authorization parent is that gallery

#### Scenario: SCN-GALLERY-012-C — The surface is not the parent

- **WHEN** a gallery is created through `GALLERY_COLLECTION`
- **THEN** the surface is not the gallery's authorization parent

### Requirement: REQ-GALLERY-013 — `is_public` selects visibility

`is_public` SHALL select `Public` visibility when true and `Private` when false. `Public` SHALL grant the `Viewer`
actions and SHALL NOT grant edit, delete, share or moderation (`REQ-AUTHORIZATION-016`).

#### Scenario: SCN-GALLERY-013-A — A public gallery is created

- **WHEN** the creation request carries `is_public: true`
- **THEN** the gallery's visibility is `Public`

#### Scenario: SCN-GALLERY-013-B — A private gallery is created

- **WHEN** the creation request carries no `is_public` or `false`
- **THEN** the gallery's visibility is `Private`

#### Scenario: SCN-GALLERY-013-C — Public does not grant moderation

- **WHEN** a caller holds only the visibility a `Public` gallery grants
- **THEN** its edit, delete and share refuse

### Requirement: REQ-GALLERY-014 — A gallery is created by a standard user or an admin by default

A Standard User and an Admin SHALL create a gallery by default. An explicit `Deny` on `GALLERY_COLLECTION` SHALL refuse
creation.

#### Scenario: SCN-GALLERY-014-A — A standard user creates a gallery

- **WHEN** a Standard User creates a gallery
- **THEN** the gallery is created and owned by that caller

#### Scenario: SCN-GALLERY-014-B — An admin creates a gallery

- **WHEN** an Admin creates a gallery
- **THEN** the gallery is created and owned by that caller

#### Scenario: SCN-GALLERY-014-C — An explicit deny refuses

- **WHEN** the caller's grant on `GALLERY_COLLECTION` denies `CreateChild`
- **THEN** the system returns `403 Forbidden` and creates nothing
