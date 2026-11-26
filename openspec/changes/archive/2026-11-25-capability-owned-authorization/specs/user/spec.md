# Spec Delta

## ADDED Requirements

### Requirement: REQ-USER-021 — Account administration is root and admin

Only the root SHALL create an Admin account. An Admin SHALL create, update, delete and read Standard accounts. A
non-root caller SHALL NOT update, delete or change the role of an Admin account, and no caller SHALL modify or delete
the root account.

#### Scenario: SCN-USER-021-A — An admin creates a standard account

- **WHEN** an Admin creates a Standard account
- **THEN** the account is created

#### Scenario: SCN-USER-021-B — An admin cannot create an admin

- **WHEN** an Admin creates an Admin account
- **THEN** the system answers `403 Forbidden` and creates nothing

#### Scenario: SCN-USER-021-C — An admin cannot touch an admin

- **WHEN** an Admin updates, deletes or changes the role of another Admin
- **THEN** the system answers `403 Forbidden` and applies nothing

#### Scenario: SCN-USER-021-D — The root manages admins

- **WHEN** the root updates, deletes or changes the role of an Admin account
- **THEN** the operation applies

#### Scenario: SCN-USER-021-E — The root account is untouchable

- **WHEN** any caller updates or deletes the root account
- **THEN** the system answers `403 Forbidden` and applies nothing

### Requirement: REQ-USER-022 — An Admin administers a non-root user's grants

An Admin SHALL read and replace the grants of a non-root user it administers.

#### Scenario: SCN-USER-022-A — An admin edits a standard user's grants

- **WHEN** an Admin replaces a Standard user's grants
- **THEN** the replacement applies

### Requirement: REQ-USER-023 — A caller reads its own account

A caller SHALL read its own `User` resource regardless of its grants, including when no grant covers it. Reading another
account SHALL follow the ordinary decision.

#### Scenario: SCN-USER-023-A — The own account is readable

- **WHEN** a Standard User with no grant reads its own account
- **THEN** the system returns `200 OK`

#### Scenario: SCN-USER-023-B — Another account follows the grants

- **WHEN** a Standard User with no grant reads another account
- **THEN** the system returns `403 Forbidden`

### Requirement: REQ-USER-024 — Only the root changes a role

Changing a user's role SHALL be available only to the root.

#### Scenario: SCN-USER-024-A — Only the root changes a role

- **WHEN** a non-root caller changes a user's role
- **THEN** the system answers `403 Forbidden` and applies nothing

### Requirement: REQ-USER-025 — A user account is a resource under the instance root

A user account SHALL be a resource whose authorization parent is the instance root.

#### Scenario: SCN-USER-025-A — A created account follows the root

- **WHEN** an account is created
- **THEN** its authorization parent is the instance root
