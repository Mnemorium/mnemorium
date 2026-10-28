# user/account-creation Specification

## Purpose

Lets an authenticated Admin create a user account with a valid lowercase username, an optional validated email, a
password that satisfies the password policy, and a role the caller is permitted to grant.

## Requirements

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

### Requirement: Usernames are lowercase, bounded, and unique

A username SHALL be at least 4 and at most 100 characters long, SHALL contain no uppercase letter, and SHALL not begin
or end with whitespace. The system SHALL store the submitted value unchanged and SHALL NOT trim, lowercase, or otherwise
normalize it. It SHALL be unique across users, compared exactly. An invalid username SHALL answer
`422 Unprocessable Content`; one already taken SHALL answer `409 Conflict` with a message stating that the username is
taken.

#### Scenario: Leading or trailing whitespace is rejected

- **WHEN** a creation request carries a username that begins or ends with whitespace
- **THEN** the system returns `422 Unprocessable Content` and creates nothing

#### Scenario: Uppercase letters are rejected

- **WHEN** a creation request carries a username containing an uppercase letter
- **THEN** the system returns `422 Unprocessable Content` and creates nothing

#### Scenario: Boundary lengths are accepted

- **WHEN** a creation request carries a username of exactly 4 or exactly 100 characters
- **THEN** the system creates the account

#### Scenario: Username is too short

- **WHEN** a creation request carries an empty, blank, or 3-character username
- **THEN** the system returns `422 Unprocessable Content` and creates nothing

#### Scenario: Username is too long

- **WHEN** a creation request carries a username of 101 characters or more
- **THEN** the system returns `422 Unprocessable Content` and creates nothing

#### Scenario: Username is already taken

- **WHEN** a creation request carries a username another user already holds
- **THEN** the system returns `409 Conflict` with a message stating that the username is taken, and creates nothing

### Requirement: Email is optional, validated, and unique

An email address SHALL be optional; an absent or null value SHALL be accepted and stored as no email. When present, it
SHALL be a syntactically valid email address and SHALL be unique across users, compared exactly and case-sensitively.
The system SHALL store the submitted value unchanged and SHALL NOT trim or otherwise normalize it. An invalid address
SHALL answer `422 Unprocessable Content`; a taken one SHALL answer `409 Conflict` with a message stating that the email
is taken.

#### Scenario: Email is omitted

- **WHEN** a creation request carries no email value
- **THEN** the system creates the account with no email address

#### Scenario: Email is null

- **WHEN** a creation request carries an explicit null email
- **THEN** the system creates the account with no email address

#### Scenario: Email with surrounding whitespace is invalid

- **WHEN** a creation request carries a valid email address surrounded by whitespace
- **THEN** the system returns `422 Unprocessable Content` and creates nothing

#### Scenario: Email has an invalid format

- **WHEN** a creation request carries a value that is not a valid email address
- **THEN** the system returns `422 Unprocessable Content` and creates nothing

#### Scenario: Email carries a display name

- **WHEN** a creation request carries a mailbox with a display name, such as `Alice <alice@example.com>`
- **THEN** the system returns `422 Unprocessable Content` and creates nothing

#### Scenario: Email is already taken

- **WHEN** a creation request carries an email address another user already holds
- **THEN** the system returns `409 Conflict` with a message stating that the email is taken, and creates nothing

#### Scenario: Case distinguishes two email addresses

- **WHEN** one user holds `alice@example.com` and another user is created with `Alice@example.com`
- **THEN** both accounts exist as distinct users

### Requirement: Passwords follow the password policy

A password SHALL be at least 8 characters long and SHALL contain at least one non-alphanumeric character. A password
that fails the policy SHALL answer `422 Unprocessable Content`. The password SHALL never appear in a response and SHALL
be stored only as a hash.

#### Scenario: Password satisfies the policy

- **WHEN** a creation request carries a password of at least 8 characters containing a non-alphanumeric character
- **THEN** the system creates the account

#### Scenario: Password is too short

- **WHEN** a creation request carries a password shorter than 8 characters
- **THEN** the system returns `422 Unprocessable Content` and creates nothing

#### Scenario: Password has no symbol

- **WHEN** a creation request carries a password of at least 8 characters that contains only letters and digits
- **THEN** the system returns `422 Unprocessable Content` and creates nothing

#### Scenario: Password is never returned

- **WHEN** the system returns any representation of a created account
- **THEN** the payload carries no password or password hash

### Requirement: A rejected creation applies nothing

When a creation request is rejected for any reason, the system SHALL create neither a user nor a credential.

#### Scenario: Rejected request leaves no account

- **WHEN** a creation request is rejected for an invalid value, a forbidden role, or a duplicate username or email
- **THEN** no account exists for the requested username and no credential is left behind

### Requirement: Creation requests are validated before use

A request body that is not valid JSON SHALL answer `400 Bad Request`. A missing required value, a value of the wrong
type, or a role outside the Admin or Standard list SHALL answer `422 Unprocessable Content`.

#### Scenario: Malformed request body

- **WHEN** a creation request body cannot be parsed as JSON
- **THEN** the system returns `400 Bad Request`

#### Scenario: Required value is missing

- **WHEN** a creation request omits the username, password, or role
- **THEN** the system returns `422 Unprocessable Content`

#### Scenario: Value has the wrong type

- **WHEN** a creation request carries a value whose type does not match the request schema
- **THEN** the system returns `422 Unprocessable Content`

#### Scenario: Role is not in the discrete list

- **WHEN** a creation request carries a role other than Admin or Standard
- **THEN** the system returns `422 Unprocessable Content`

### Requirement: A created account is returned with its location

A successful creation SHALL answer `201 Created` with the HAL representation of the account — its identifier, username,
email or null, role, and a self link — and SHALL carry the account's URI in the `Location` header.

#### Scenario: Created account representation

- **WHEN** the system creates a user account
- **THEN** the response carries `201 Created`, the account's identifier, username, email or null, role, and a self link

#### Scenario: Location matches the self link

- **WHEN** the system returns the created account
- **THEN** the `Location` header carries the same URI as the representation's self link
