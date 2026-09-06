# Use Cases

Catalog of the use cases of Mnemorium. Every use case implemented in
`src/lib/application/use_case/` has an entry here, and every entry maps to
exactly one use-case file.

IDs are sequential across the whole catalog: `UC-001`, `UC-002`, …

## Field rules

### Title

- `## UC-<seq> - <Name>` heading, e.g. `## UC-001 - Register User`.
- The name is the use-case file name in words, e.g. `Register User` →
  `register_user.rs`.

### Description

- One or two sentences stating the business intent of the use case.

### Primary actor

- The actor who initiates the use case.
- Use the actor terms from the Glossary (`Standard User`, `Admin`,
  `Root Admin`).

### Secondary actor (optional)

- Other actors or systems that participate without initiating.
- Remove the section when there is none.

### Pre condition(s)

- Bullet list, one condition per bullet.
- Every condition must hold before the use case can run.

### Trigger(s)

- Bullet list of what initiates the use case (REST endpoint, scheduler, …).

### Bounded context(s)

- Bullet list of the bounded contexts the use case touches.
- The available bounded contexts are listed in the
  [Bounded context section of the Overview](Overview.md#bounded-context).

### Business rules

- Bullet list of the business rules the input must satisfy.

### Happy path

- Numbered steps of the main success scenario.

### Alternative flow

- Numbered steps, each branch starting from a happy-path step (e.g. `3a.` when
  branching from step 3).
- Every flow listed here is a `Critical exception path` (see Glossary): a
  scenario worth testing, covered following [Test.md](Test.md).
- List a flow only when it is needed; business rules do not require a one-to-one
  branch.

### Post condition(s)

- Bullet list, one condition per bullet.
- Every condition is guaranteed after the use case succeeds.

## Catalog

## UC-001 - Create a User Account

### Description

Allow an Administrator to create a new user in the system and assign the
appropriate profile information.

### Primary actor

- Admin

### Pre condition(s)

- The caller is authenticated as an Admin.

### Trigger(s)

- User creation request

### Bounded context(s)

- Identity
- User

### Business rules

- Username must be at least 4 characters long.
- Email must be a valid email address or be null.
- Password must be at least 8 characters long and contain at least one symbol.

### Happy path

1. The Admin submits a user creation request carrying a username, an optional
   email, a password, and the role to grant.
2. The system validates the payload against the business rules.
3. The system verifies the caller may grant the requested role.
4. The system verifies that no user already exists with the username or email.
5. The system hashes the password and saves the new user with its credential.
6. The system returns the created account.

### Alternative flow

- 2a. The payload is invalid — empty username, empty password, or any other
  business-rule violation.
- 3a. The Admin is not the Root Admin and requests the Admin role for the new
  user; the system rejects the request.

### Post condition(s)

- A new user account and its credential exist in the system.
- Username and email remain unique across users.

## UC-002 - User Authentication

### Description

Allow a registered user to securely authenticate with the system using valid
credentials and gain access to authorized features and resources.

### Primary actor

- Standard User

### Pre condition(s)

- The user is registered: an account with its credential exists in the system.

### Trigger(s)

- Authentication request

### Bounded context(s)

- Identity
- User

### Happy path

1. The user submits an authentication request carrying a username and a
   password.
2. The system finds the user by username.
3. The system verifies the password against the stored hash.
4. The system issues a token identifying the user.
5. The user gains access to the authorized features and resources.

### Alternative flow

- 2a. Bad username: no account matches the username; the system rejects the
  request.
- 3a. Bad password: the password does not match the stored hash; the system
  rejects the request.

### Post condition(s)

- The user holds a valid token granting access to authorized features and
  resources.
