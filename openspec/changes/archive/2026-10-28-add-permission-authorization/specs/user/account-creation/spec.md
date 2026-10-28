# Spec Delta

## MODIFIED Requirements

### Requirement: User account creation requires an authenticated Admin

Every request to create a user account SHALL require a valid bearer token and the `user.create` permission. The default
role presets grant it to Admin and Root, so a Standard User is refused without an override. A token that names an
account which no longer exists SHALL be rejected.

#### Scenario: Missing or invalid credentials

- **WHEN** a request to create a user account reaches the endpoint without a valid bearer token
- **THEN** the system returns `401 Unauthorized` and creates nothing

#### Scenario: Standard User cannot create an account

- **WHEN** an authenticated Standard User with no override submits a user-creation request
- **THEN** the system returns `403 Forbidden` and creates nothing

#### Scenario: A Standard User granted user creation creates a Standard user

- **WHEN** a Standard User whose `user.create` override is `ALL` submits a user-creation request for a Standard user
- **THEN** the system creates the account with the Standard role

### Requirement: The caller may grant only the roles it is permitted to grant

Creating an Admin SHALL require the `user.create_admin` permission; the default role presets grant it only to the Root
Admin. Creating a Standard user SHALL require the `user.create` permission; the default presets grant it to Admin and
Root. A request for a role the caller may not grant SHALL be rejected and SHALL create nothing.

#### Scenario: Root Admin grants the Admin role

- **WHEN** the Root Admin submits a user-creation request with the Admin role
- **THEN** the system creates the account with the Admin role

#### Scenario: Admin grants the Standard role

- **WHEN** an Admin who is not the Root Admin submits a user-creation request with the Standard role
- **THEN** the system creates the account with the Standard role

#### Scenario: Admin requests the Admin role

- **WHEN** an Admin who is not the Root Admin submits a user-creation request with the Admin role
- **THEN** the system returns `403 Forbidden` and creates neither the user nor a credential for it

#### Scenario: A caller granted Admin creation grants the Admin role

- **WHEN** a caller whose `user.create_admin` override is `ALL` submits a user-creation request with the Admin role
- **THEN** the system creates the account with the Admin role
