# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-API-015 — A public index is the entry point

`GET /api/v1` SHALL return a public HAL representation whose `_links.self` names `/api/v1` and whose named links reach
the authentication operations, including login, registration and setup, each context's entry point (its collection or
its create operation) and the liveness probe. The index SHALL be caller-independent and SHALL NOT list individual item
routes.

#### Scenario: SCN-API-015-A — The index is public

- **WHEN** an unauthenticated client requests `/api/v1`
- **THEN** the system answers `200 OK` with an `application/hal+json` index and `_links.self` naming `/api/v1`

#### Scenario: SCN-API-015-B — The index reaches the surface

- **WHEN** a client follows the index's links
- **THEN** it reaches the login, registration and setup operations, each context's entry point and the liveness probe

#### Scenario: SCN-API-015-D — The index is caller-independent

- **WHEN** two callers with different permissions read the index
- **THEN** both receive the same links

## ADDED Requirements

### Requirement: REQ-API-025 — The public surface is closed

The public surface SHALL be closed: the **API index**, login, setup and the liveness probe at `GET /health` are the only
operations reachable without a bearer token. Every other operation SHALL require one. No public operation SHALL return
password, key or access-token material, except login, which issues the caller's own token.

#### Scenario: SCN-API-025-A — The public operations are exactly four

- **WHEN** the operations the published contract declares with an empty security requirement are listed
- **THEN** they are the liveness probe, the index, login and setup, and no other operation

#### Scenario: SCN-API-025-B — A public response carries no secret

- **WHEN** an unauthenticated client reads the index or the probe, or completes a setup
- **THEN** the response carries no password, key or access-token material, and only login carries the caller's own token
