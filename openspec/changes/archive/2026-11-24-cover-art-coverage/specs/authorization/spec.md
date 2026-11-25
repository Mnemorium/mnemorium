# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-AUTHORIZATION-026 — Referencing content needs the container and the target

Creating a gallery item SHALL need `CreateChild` on the gallery and `View` on the referenced image or video. Adding a
playlist entry SHALL need `CreateChild` on the playlist and `ViewMetadata` on the recording. Naming a cover in a
creation or a metadata update SHALL need the action that admits that write and `View` on the image it names. Every pair
SHALL be all-of, and the request SHALL apply nothing when one refuses.

A cover the caller may not view SHALL be refused as a conflict (`409`), indistinguishable from a cover that names no
image, so the refusal does not disclose whether the image exists (`REQ-API-013`).

#### Scenario: SCN-AUTHORIZATION-026-A — A gallery item needs both

- **WHEN** a caller may create a gallery item but may not view the referenced image
- **THEN** the system answers `403 Forbidden` and adds nothing

#### Scenario: SCN-AUTHORIZATION-026-B — A playlist entry needs both

- **WHEN** a caller may add to a playlist but may not read the recording's metadata
- **THEN** the system answers `403 Forbidden` and adds nothing

#### Scenario: SCN-AUTHORIZATION-026-C — Both allow

- **WHEN** a caller holds both actions
- **THEN** the child is created

#### Scenario: SCN-AUTHORIZATION-026-D — A cover needs both

- **WHEN** a caller may write a resource's metadata but may not view the image its cover names
- **THEN** the system answers `409 Conflict` and applies nothing
