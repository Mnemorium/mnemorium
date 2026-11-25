# Spec Delta

## ADDED Requirements

### Requirement: REQ-API-024 — A declared constraint is enforced

A constraint attribute a parameter or a `#[schema(...)]` model property declares SHALL be enforced by the handler or use
case: a value that violates it SHALL answer `422 Unprocessable Content`, or the attribute SHALL NOT be declared.

#### Scenario: SCN-API-024-A — A parameter outside its bound is refused

- **WHEN** a request carries a path or query value outside the bound its declaration states
- **THEN** the system answers `422 Unprocessable Content` and applies nothing

#### Scenario: SCN-API-024-B — A property outside its bound is refused

- **WHEN** a body property violates the `minimum`, `maximum`, `min_length`, `max_length` or `pattern` its schema
  declares
- **THEN** the system answers `422 Unprocessable Content` and applies nothing
