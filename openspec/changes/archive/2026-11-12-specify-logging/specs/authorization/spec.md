# Spec Delta

## REMOVED Requirements

### Requirement: REQ-AUTHORIZATION-027 — Denials are uniform and concealed

**Reason**: The requirement mixed an authorization outcome with the record it writes. The logging capability now owns
every catalogued event and its trigger (`REQ-LOGGING-013`), and the schema does not let a MODIFIED block drop a
scenario, so the requirement is retired and its outcome re-stated without the event.

**Migration**: The concealed `404`/`403` outcomes are carried by `REQ-AUTHORIZATION-038`; the denial event and the
absence of one for a filtered listing are declared by `REQ-LOGGING-013`.

### Requirement: REQ-AUTHORIZATION-028 — Filtering a listing is not a denial

**Reason**: Same as `REQ-AUTHORIZATION-027`: the listing outcome stays, and the event it does not write moves to the
logging capability.

**Migration**: The filtered listing is carried by `REQ-AUTHORIZATION-039`; `REQ-LOGGING-013` declares that a listing
filter records no denial.

## ADDED Requirements

### Requirement: REQ-AUTHORIZATION-038 — Denials are uniform and concealed

A caller that cannot `Discover` or `ViewMetadata` the addressed resource SHALL receive `404 Not Found`. A caller that
can read it but may not modify it SHALL receive `403 Forbidden`. An administration denial SHALL receive `403 Forbidden`.

#### Scenario: SCN-AUTHORIZATION-038-A — A hidden resource is concealed

- **WHEN** a caller cannot discover a private gallery
- **THEN** the system answers `404 Not Found`

#### Scenario: SCN-AUTHORIZATION-038-B — A visible write is revealed

- **WHEN** a caller can view a public playlist but may not edit it
- **THEN** the system answers `403 Forbidden`

### Requirement: REQ-AUTHORIZATION-039 — A filtered listing is not a denial

A listing SHALL return only the resources the caller may discover.

#### Scenario: SCN-AUTHORIZATION-039-A — A listing is filtered

- **WHEN** a caller may discover only some albums
- **THEN** the album listing returns only those albums
