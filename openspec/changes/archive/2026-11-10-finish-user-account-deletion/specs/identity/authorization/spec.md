# Spec Delta

## MODIFIED Requirements

### Requirement: The Admin preset administers user accounts

An Admin SHALL additionally read, list, create, update and delete Standard user accounts, except that only the Root
Admin may create an Admin, change a role or credential, or delete an Admin. The Root Admin's own account SHALL never be
deleted by any caller.

#### Scenario: Admin administers a Standard user

- **WHEN** an Admin with no permission override lists users or creates a Standard user
- **THEN** the system returns the documented success response

#### Scenario: Admin cannot grant the Admin role

- **WHEN** an Admin with no permission override creates an Admin user
- **THEN** the system returns `403 Forbidden` and creates nothing

#### Scenario: Admin deletes a Standard user

- **WHEN** an Admin with no permission override deletes a Standard user
- **THEN** the system deletes the account

#### Scenario: Admin cannot delete an Admin

- **WHEN** an Admin with no permission override deletes an Admin account
- **THEN** the system returns `403 Forbidden` and deletes nothing

#### Scenario: Root Admin deletes an Admin

- **WHEN** the Root Admin deletes an Admin account
- **THEN** the system deletes the account

#### Scenario: Root Admin's own account is refused

- **WHEN** any caller deletes the Root Admin's account
- **THEN** the system returns `403 Forbidden` and deletes nothing
