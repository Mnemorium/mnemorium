# Spec Delta

## ADDED Requirements

### Requirement: REQ-FILE-004 — Stored files are listable

The system SHALL return the **Stored files** a **Caller** may `Discover` as a paginated collection, optionally
restricted to one exact root-relative path. Each item SHALL be the single-file representation. A stored file the caller
may not discover SHALL be absent from the collection rather than an error.

#### Scenario: SCN-FILE-004-A — A caller lists the files it may discover

- **WHEN** a caller requests the stored-file collection
- **THEN** the system answers `200 OK` with only the stored files the caller may discover
- **AND** each item is the single-file representation its own `self` link addresses

#### Scenario: SCN-FILE-004-B — An exact path restricts the collection

- **WHEN** the caller requests the collection with one exact root-relative path
- **THEN** only the stored file at that path appears

#### Scenario: SCN-FILE-004-C — A path no file carries lists nothing

- **WHEN** the caller requests the collection with a path no stored file carries
- **THEN** the system answers `200 OK` with an empty collection
