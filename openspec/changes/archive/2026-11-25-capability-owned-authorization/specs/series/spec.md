# Spec Delta

## ADDED Requirements

### Requirement: REQ-SERIES-014 — A show is an unowned catalogue resource

A show SHALL be a resource under the catalogue, whose authorization parent SHALL be the catalogue. A season SHALL be a
resource under its show, and an episode a resource under its season. A show SHALL have no owner. A read SHALL follow
`View` and `Stream`, and a write SHALL follow `EditMetadata`, `ReplaceMedia`, `CreateChild` and `Delete` granted on the
resource or an ancestor. A Standard User SHALL NOT edit a show by default.

#### Scenario: SCN-SERIES-014-A — A standard user reads a show

- **WHEN** a Standard User with the seeded role reads a show
- **THEN** the read allows

#### Scenario: SCN-SERIES-014-B — A standard user cannot write a show

- **WHEN** a Standard User with the seeded role renames a show
- **THEN** the system answers `403 Forbidden` and applies nothing

#### Scenario: SCN-SERIES-014-C — An admin curates a season

- **WHEN** an Admin with the seeded role edits a season
- **THEN** the write applies

### Requirement: REQ-SERIES-015 — A show is created by an Admin by default

A Standard User SHALL NOT create a show. An Admin SHALL create a show by default. An explicit `Deny` on the catalogue
SHALL refuse creation.

#### Scenario: SCN-SERIES-015-A — A standard user cannot create a show

- **WHEN** a Standard User creates a show under the catalogue
- **THEN** the system answers `403 Forbidden` and creates nothing

#### Scenario: SCN-SERIES-015-B — An admin creates a show

- **WHEN** an Admin creates a show under the catalogue
- **THEN** the show is created

#### Scenario: SCN-SERIES-015-C — A denied catalogue refuses

- **WHEN** a caller whose `CreateChild` on the catalogue is denied creates a show
- **THEN** the system answers `403 Forbidden` and creates nothing
