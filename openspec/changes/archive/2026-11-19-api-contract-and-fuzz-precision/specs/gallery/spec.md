# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-GALLERY-003 — Create a gallery

Creating a gallery SHALL require the `CreateChild` action on `GALLERY_COLLECTION`. The name SHALL be non-empty, at most
100 characters, and SHALL NOT begin or end with whitespace; such a name answers `422` and stores nothing. Interior
whitespace is allowed and the submitted value is stored unchanged. `is_public` SHALL default to `false`. A duplicate
name for the same owner SHALL answer `409 Conflict`; success SHALL answer `201 Created` with a `Location`.

#### Scenario: SCN-GALLERY-003-A — A gallery is created

- **WHEN** an authenticated caller creates a gallery with a valid name
- **THEN** the system returns `201 Created` with the gallery representation and a `Location`
- **AND** the gallery is owned by the caller and private

#### Scenario: SCN-GALLERY-003-B — A public gallery is created

- **WHEN** the caller creates a gallery with `is_public` true
- **THEN** the gallery is `Public`

#### Scenario: SCN-GALLERY-003-C — The owner already has that name

- **WHEN** the caller already owns a gallery with the same name
- **THEN** the system returns `409 Conflict` and creates nothing

#### Scenario: SCN-GALLERY-003-D — The name is unusable

- **WHEN** the submitted name is empty, only whitespace, begins or ends with whitespace, or longer than 100 characters
- **THEN** the system returns `422 Unprocessable Content` and creates nothing

#### Scenario: SCN-GALLERY-003-E — The caller may not create a gallery

- **WHEN** the caller's grant on `GALLERY_COLLECTION` denies `CreateChild`
- **THEN** the system returns `403 Forbidden` and creates nothing
