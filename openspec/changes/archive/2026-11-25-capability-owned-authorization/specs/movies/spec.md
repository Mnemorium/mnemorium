# Spec Delta

## ADDED Requirements

### Requirement: REQ-MOVIES-006 — A film is an unowned catalogue resource

A film SHALL be a resource under the catalogue, whose authorization parent SHALL be the catalogue. It SHALL have no
owner. A read SHALL follow `View` and `Stream`, and a write SHALL follow `EditMetadata`, `ReplaceMedia`, `CreateChild`
and `Delete` granted on the film or the catalogue. A Standard User SHALL NOT edit a film by default.

#### Scenario: SCN-MOVIES-006-A — A standard user reads a film

- **WHEN** a Standard User with the seeded role reads a film
- **THEN** the read allows

#### Scenario: SCN-MOVIES-006-B — A standard user cannot write a film

- **WHEN** a Standard User with the seeded role renames a film
- **THEN** the system answers `403 Forbidden` and applies nothing

#### Scenario: SCN-MOVIES-006-C — An admin curates a film

- **WHEN** an Admin with the seeded role edits a film
- **THEN** the write applies

### Requirement: REQ-MOVIES-007 — A film is created by an Admin by default

A Standard User SHALL NOT create a film. An Admin SHALL create a film by default. An explicit `Deny` on the catalogue
SHALL refuse creation.

#### Scenario: SCN-MOVIES-007-A — A standard user cannot create a film

- **WHEN** a Standard User creates a film under the catalogue
- **THEN** the system answers `403 Forbidden` and creates nothing

#### Scenario: SCN-MOVIES-007-B — An admin creates a film

- **WHEN** an Admin creates a film under the catalogue
- **THEN** the film is created

#### Scenario: SCN-MOVIES-007-C — A denied catalogue refuses

- **WHEN** a caller whose `CreateChild` on the catalogue is denied creates a film
- **THEN** the system answers `403 Forbidden` and creates nothing
