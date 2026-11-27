# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-USER-021 — Account administration is root and admin

Only the root SHALL create an Admin account. An Admin SHALL create, update, delete and read Standard accounts. A
non-root caller SHALL NOT update, delete or change the role of an Admin account. The root SHALL be permitted to update
its own email; no caller SHALL change the root's username or role, and no caller SHALL delete the root account.

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

- **WHEN** a caller other than the root changes the root's email, or any caller changes its username or role or deletes
  the root account
- **THEN** the system answers `403 Forbidden` and applies nothing

#### Scenario: SCN-USER-021-F — A standard user cannot create an account

- **WHEN** a Standard User creates a Standard or an Admin account
- **THEN** the system answers `403 Forbidden` and creates nothing

#### Scenario: SCN-USER-021-G — An admin cannot delete itself

- **WHEN** an Admin deletes its own account
- **THEN** the system answers `403 Forbidden` and the account remains

#### Scenario: SCN-USER-021-H — An admin updates a standard account

- **WHEN** an Admin updates a Standard account
- **THEN** the update applies

#### Scenario: SCN-USER-021-I — The root maintains its own email

- **WHEN** the root updates account 0 with a new valid email and no other field
- **THEN** the system answers `200 OK` and the stored email changes
