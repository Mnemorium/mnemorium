# Spec Delta

## ADDED Requirements

### Requirement: REQ-MUSIC-031 — A catalogue music resource is unowned and shared

An album, medium, recording or music group SHALL be a resource under the catalogue and SHALL have no owner. A read SHALL
follow `View` and `Stream`, and a write SHALL follow `EditMetadata`, `ReplaceMedia`, `CreateChild` and `Delete` granted
on the resource or an ancestor. A Standard User SHALL NOT edit a catalogue music resource by default.

#### Scenario: SCN-MUSIC-031-A — A standard user reads the catalogue

- **WHEN** a Standard User with the seeded role reads an album, a recording or a music group
- **THEN** the read allows

#### Scenario: SCN-MUSIC-031-B — A standard user cannot write the catalogue

- **WHEN** a Standard User with the seeded role renames an album or a music group
- **THEN** the system answers `403 Forbidden` and applies nothing

#### Scenario: SCN-MUSIC-031-C — An admin curates the catalogue

- **WHEN** an Admin with the seeded role edits an album
- **THEN** the write applies

### Requirement: REQ-MUSIC-032 — Music has two default creation surfaces

A playlist SHALL be a resource whose authorization parent is the library. A Standard User and an Admin SHALL create a
playlist by default. A Standard User SHALL NOT create a catalogue music resource, and an Admin SHALL. An explicit `Deny`
on the addressed creation surface SHALL refuse.

#### Scenario: SCN-MUSIC-032-A — A standard user creates a playlist

- **WHEN** a Standard User creates a playlist under the library
- **THEN** the playlist is created and owned by that caller

#### Scenario: SCN-MUSIC-032-B — A standard user cannot create catalogue music

- **WHEN** a Standard User creates a music group or a music album under the catalogue
- **THEN** the system answers `403 Forbidden` and creates nothing

#### Scenario: SCN-MUSIC-032-C — An admin creates catalogue music

- **WHEN** an Admin creates a music album under the catalogue
- **THEN** the album is created

#### Scenario: SCN-MUSIC-032-D — A denied playlist surface refuses

- **WHEN** a Standard User whose `CreateChild` on the playlist collection is denied creates a playlist
- **THEN** the system answers `403 Forbidden` and creates nothing
