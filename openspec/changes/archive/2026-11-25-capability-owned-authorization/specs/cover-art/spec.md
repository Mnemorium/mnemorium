# Spec Delta

## ADDED Requirements

### Requirement: REQ-COVER-ART-008 — Naming a cover needs the write and the image

Naming a cover in a creation or a metadata update SHALL require the action that admits that write and `View` on the
image it names. The pair SHALL be all-of, and the request SHALL apply nothing when either refuses. A cover the caller
may not view SHALL answer `409 Conflict`, indistinguishable from a cover that names no image (`REQ-API-013`).

#### Scenario: SCN-COVER-ART-008-A — An unviewable cover is refused

- **WHEN** a caller may write a resource's metadata but may not view the image its cover names
- **THEN** the system answers `409 Conflict` and applies nothing

#### Scenario: SCN-COVER-ART-008-B — Both actions allow

- **WHEN** a caller holds the write action and may view the named image
- **THEN** the cover is stored
