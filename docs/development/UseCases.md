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

- 2a. Bad credentials: no account matches the username, or the password does not
  match the stored hash; the system rejects the request.

### Post condition(s)

- The user holds a valid token granting access to authorized features and
  resources.

## UC-003 - Initialize Root Admin

### Description

On the first runtime the system creates the Root Admin account, issues and logs
a default password to standard output, and lets the Root Admin replace it with a
personal password. The default password stops being revealed once it has been
changed.

### Primary actor

- Root Admin

### Pre condition(s)

- First-Time Admin Authentication is enabled in configuration.
- The default password corresponding to the Root Admin account has not been
  replaced yet.

### Trigger(s)

- Start of system runtime while the configuration still enables First-Time Admin
  Authentication.

### Bounded context(s)

- Identity
- User
- Configuration

### Business rules

- The Root Admin account is created with a random default password when no Root
  Admin exists.
- The default password is written to standard output only while the Root Admin
  still uses it.
- The new password must be at least 8 characters long and contain at least one
  symbol.

### Happy path

1. The system starts with First-Time Admin Authentication enabled.
2. The system checks whether a Root Admin account exists; none does, so the
   system creates the account with a generated default password.
3. The system logs the default password to standard output.
4. The Root Admin submits an authentication request carrying the default
   password and a new password.
5. The system verifies the default password against the stored hash.
6. The system validates the new password against the business rules.
7. The system hashes the new password, replaces the credential, and the default
   password stops being logged.
8. The system returns the updated account.

### Alternative flow

- 2a. A Root Admin already exists: the system stops at step 1 and the
  functionality stays inactive — no new account and no logged password.
- 5a. Bad password: the default password does not match the stored hash; the
  system rejects the request.
- 6a. The new password violates the business rules; the system rejects the
  request.

### Post condition(s)

- A Root Admin account exists in the system.
- The Root Admin holds a personal credential.
- The default password is no longer valid and is no longer shown in the standard
  output while the configuration has First-Time Admin Authentication enabled.

## UC-004 - Load Configuration

### Description

On every runtime the system loads the application configuration by layering the
persisted configuration singleton with the optional configuration file and the
environment, generating the secrets on the first runtime.

### Primary actor

- System

### Pre condition(s)

- The datastore is reachable.

### Trigger(s)

- Start of system runtime.

### Bounded context(s)

- Configuration

### Business rules

- The configuration is merged following the persistence → file → environment
  precedence: the persisted singleton row is the base layer, the configuration
  file overrides it, and the environment overrides both.
- On the first runtime the singleton row does not exist; the system creates it
  with the default settings and freshly generated secrets before loading.
- The default settings enable logging the Root Admin default password
  (`log_root_admin_password` is `true`).
- Secrets left empty after the first runtime creation are never regenerated: the
  persisted row keeps them stable across restarts.

### Happy path

1. The system starts and checks whether the configuration singleton row exists;
   none does, so the system creates it with the default settings and freshly
   generated secrets.
2. The system loads the configuration: the singleton row is layered with the
   configuration file and the environment overrides.
3. The system returns the merged configuration to the runtime.

### Alternative flow

- 1a. The singleton row already exists: the system skips its creation and loads
  the configuration directly.

### Post condition(s)

- The configuration singleton row exists in the datastore.
- The runtime holds a complete configuration to wire its dependencies.

## UC-005 - Delete User Account

### Description

Allow an Administrator to remove a user account together with all the data it
owns, and prevent the affected user from accessing the system.

### Primary actor

- Admin

### Pre condition(s)

- The caller is authenticated as an Admin.

### Trigger(s)

- User deletion request

### Bounded context(s)

- Identity
- User

### Business rules

- The Root Admin account cannot be deleted.
- Deletion removes the user, its credential, and everything the user owns: its
  files, its playlists, and the media records that depend on those files.
- Tokens issued to the deleted user stop being accepted immediately.

### Happy path

1. The Admin submits a user deletion request identifying the target user.
2. The system verifies the target user exists.
3. The system verifies the target user is not the Root Admin.
4. The system deletes the data owned by the user and then the account with its
   credential.
5. The system stops accepting the tokens issued to the deleted user.
6. The system confirms the deletion.

### Alternative flow

- 1a. The caller is not an Admin; the system rejects the request.
- 2a. No user exists for the requested identifier; the system rejects the
  request.
- 3a. The target user is the Root Admin; the system rejects the request.

### Post condition(s)

- The user account and its credential no longer exist.
- Every file, playlist, and dependent media record owned by the user no longer
  exists.
- The deleted user cannot authenticate and any token it held is rejected.

## UC-006 - Upload Media File

### Description

Allow a user to upload a supported media file to the service for storage,
processing, cataloging, and distribution.

### Primary actor

- Standard User

### Pre condition(s)

- The caller is authenticated.

### Trigger(s)

- Upload request

### Bounded context(s)

- Asset

### Business rules

- The uploaded content must match one of the supported media MIME types (AUDIO,
  VIDEO, or IMAGE) recorded in the datastore.
- The media type is determined from the file content, not from the file name or
  declared content type alone.
- The stored file path is unique across files.
- An integrity hash of the uploaded content is computed and stored with the
  file; it is unique across files.
- The uploaded file is private by default (`is_public` is false).

### Happy path

1. The user submits an upload request carrying the media file.
2. The system determines the media type from the file content.
3. The system verifies the media type is supported.
4. The system computes the integrity hash of the content.
5. The system stores the file and saves its record bound to the caller.
6. The system returns the created file record.

### Alternative flow

- 3a. The media type is unsupported; the system rejects the upload and stores
  nothing.

### Post condition(s)

- The file exists on the storage with a unique path.
- A file record bound to the caller exists in the datastore with its media type,
  integrity hash, and private visibility.
