# Spec Delta

## ADDED Requirements

### Requirement: REQ-UPLOAD-018 — An upload is owner-only

An upload SHALL be a resource whose authorization parent is the instance root. It SHALL be read, written, completed and
cancelled only by its owner. A non-owner SHALL receive `404 Not Found`, no grant SHALL widen upload access, and the root
SHALL not be exempt.

#### Scenario: SCN-UPLOAD-018-A — A non-owner is concealed

- **WHEN** an authenticated caller addresses another user's upload
- **THEN** the system answers `404 Not Found` and applies nothing

#### Scenario: SCN-UPLOAD-018-B — The root is not exempt

- **WHEN** the root addresses an upload it does not own
- **THEN** the system answers `404 Not Found`
