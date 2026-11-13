# authentication Specification

## Purpose

Establishes and verifies a caller's identity: bootstraps the Root Admin credential on first start, exchanges a username
and password for a bearer token, validates that token on every protected request, and rotates a credential. It also owns
the general password policy that account creation and rotation enforce.

## Requirements

### Requirement: REQ-AUTHENTICATION-001 — The first start creates the Root Admin atomically

When no Root Admin exists, the system SHALL create user identifier 0 with username `root`, role Admin, no email, and a
credential holding the hash of a generated password, in one datastore transaction. A failure at any step SHALL persist
neither the user nor the credential and SHALL stop startup before the server accepts requests.

#### Scenario: SCN-AUTHENTICATION-001-A — Fresh instance

- **WHEN** the server starts and no user with identifier 0 exists
- **THEN** user 0 exists as `root` with the Admin role and exactly one credential

#### Scenario: SCN-AUTHENTICATION-001-B — Already initialized

- **WHEN** the server starts and user 0 already exists
- **THEN** the system creates no account, replaces no credential, and reveals no password

#### Scenario: SCN-AUTHENTICATION-001-C — Credential creation fails

- **WHEN** the credential cannot be stored
- **THEN** no user 0 exists, no credential exists, and startup fails before the server accepts requests

#### Scenario: SCN-AUTHENTICATION-001-D — Account creation fails

- **WHEN** the user cannot be stored after its credential was created
- **THEN** neither the user nor the credential is committed and startup fails before the server accepts requests

### Requirement: REQ-AUTHENTICATION-002 — The generated password follows the declared shape

The generated default password SHALL be 20 characters long: 19 alphanumeric characters (`A`-`Z`, `a`-`z`, `0`-`9`) plus
one symbol from `!@#$%^&*()-_=+`, the symbol at any position.

#### Scenario: SCN-AUTHENTICATION-002-A — Generated value shape

- **WHEN** the system generates the Root Admin default password
- **THEN** it is 20 characters long, 19 of them alphanumeric and one a symbol from the declared set

### Requirement: REQ-AUTHENTICATION-003 — The one-time password is never withheld

The system SHALL reveal the generated default password exactly once, on standard output. It SHALL refuse to create the
Root Admin when configuration disables that reveal on a first start, rather than create an account nobody can
authenticate.

#### Scenario: SCN-AUTHENTICATION-003-A — Reveal is enabled

- **WHEN** the Root Admin is created and `security.log_root_admin_password` is true
- **THEN** the password is printed to standard output and appears in no log file or API response

#### Scenario: SCN-AUTHENTICATION-003-B — Reveal is disabled on a first start

- **WHEN** the Root Admin would be created and `security.log_root_admin_password` is false
- **THEN** startup fails, no account is created, and the failure names the setting

#### Scenario: SCN-AUTHENTICATION-003-C — Reveal is disabled after initialization

- **WHEN** the Root Admin already exists and `security.log_root_admin_password` is false
- **THEN** startup proceeds and no password is revealed

### Requirement: REQ-AUTHENTICATION-004 — Valid credentials issue a bearer token

The login operation SHALL be reachable without a bearer token and, for a matching username and password, SHALL answer
`200 OK` with `application/hal+json` carrying `access_token`, `expires_in`, `token_type` set to `bearer`, and
`_links.self` naming the login operation.

#### Scenario: SCN-AUTHENTICATION-004-A — Successful login

- **WHEN** an existing user submits its username and matching password
- **THEN** the response is `200 OK`, `application/hal+json`, with `access_token`, `expires_in`, `token_type` of `bearer`
  and `_links.self` naming `/api/v1/identity/login`

#### Scenario: SCN-AUTHENTICATION-004-B — The issued token authenticates

- **WHEN** the caller presents the returned token on a protected endpoint
- **THEN** the endpoint acts as the authenticated user

#### Scenario: SCN-AUTHENTICATION-004-C — The response carries no password

- **WHEN** a login succeeds
- **THEN** the response contains no password or password hash

### Requirement: REQ-AUTHENTICATION-005 — Bad credentials are rejected uniformly

An unknown username and a wrong password SHALL be indistinguishable: the same `401 Unauthorized` status and the same
error message. The system SHALL perform comparable password work on both branches so the response time does not reveal
which value was wrong.

#### Scenario: SCN-AUTHENTICATION-005-A — Bad credentials

- **WHEN** an existing username is submitted with a wrong password, or a username no account holds is submitted
- **THEN** the system answers `401 Unauthorized` with the same error message for both

### Requirement: REQ-AUTHENTICATION-006 — Password material never leaves the credential

The plaintext password SHALL NOT be persisted, returned in a response, or written to a log sink. The stored credential
SHALL hold only a salted password hash, and a type carrying the submitted password or the issued token SHALL redact it
from its debug representation.

#### Scenario: SCN-AUTHENTICATION-006-A — Only a hash is stored

- **WHEN** a password is accepted at creation or rotation
- **THEN** the stored credential holds a hash and not the plaintext

#### Scenario: SCN-AUTHENTICATION-006-B — Login secrets are redacted

- **WHEN** a login request or response is formatted for a log sink
- **THEN** the password and the access token are redacted

### Requirement: REQ-AUTHENTICATION-007 — Login reports the configured token lifetime

The login response SHALL advertise the lifetime configured by `security.jwt.ttl`, and the issued token SHALL expire that
many seconds after issue.

#### Scenario: SCN-AUTHENTICATION-007-A — Lifetime mirrors configuration

- **WHEN** `security.jwt.ttl` is a given number of seconds
- **THEN** `expires_in` equals that number and the token's expiry is that many seconds after issue

### Requirement: REQ-AUTHENTICATION-008 — Login is rate limited per client address and account

Login SHALL be limited both per client address and per user account. The account key SHALL be the resolved user
identifier; a username that resolves to no account SHALL use a derived key that does not reveal it. An attempt SHALL be
rejected when either key is exhausted, answering `429 Too Many Requests` with `Retry-After`. Both keys SHALL use the
configured burst and period. The client address SHALL be the peer unless it is a configured trusted proxy, whose
declared header is then read.

#### Scenario: SCN-AUTHENTICATION-008-A — Client burst is throttled

- **WHEN** one client address exceeds the configured burst of login attempts
- **THEN** the next attempt answers `429 Too Many Requests` with `Retry-After`

#### Scenario: SCN-AUTHENTICATION-008-B — The quota refills

- **WHEN** the configured period elapses after a throttled attempt
- **THEN** the next attempt is admitted

#### Scenario: SCN-AUTHENTICATION-008-C — A spoofed header cannot mint a new quota

- **WHEN** an untrusted peer sends a client-address header
- **THEN** the limiter keys on the peer address and ignores the header

#### Scenario: SCN-AUTHENTICATION-008-D — One account is throttled across addresses

- **WHEN** attempts against one account arrive from many client addresses
- **THEN** they answer `429 Too Many Requests` once the account's quota is spent, even while each address is below its
  own quota

#### Scenario: SCN-AUTHENTICATION-008-E — An unknown username is throttled identically

- **WHEN** attempts against a username no account holds exceed its quota
- **THEN** they answer `429 Too Many Requests`, indistinguishable from a username that exists

### Requirement: REQ-AUTHENTICATION-009 — Protected endpoints require a bearer token

Every protected endpoint SHALL require `Authorization: Bearer <token>`.

#### Scenario: SCN-AUTHENTICATION-009-A — A missing or invalid token cannot access a protected endpoint

- **WHEN** a protected endpoint is called without a valid Bearer token
- **THEN** the system answers `401 Unauthorized` and applies nothing

### Requirement: REQ-AUTHENTICATION-010 — A token is accepted only while valid

A token SHALL be accepted only when its signature verifies against the instance signing secret, it carries a required
expiry and a numeric subject, and its expiry is in the future. The system SHALL NOT accept a token past its expiry.

#### Scenario: SCN-AUTHENTICATION-010-B — Tampered signature

- **WHEN** a token's signature does not verify
- **THEN** the system answers `401 Unauthorized` and applies nothing

#### Scenario: SCN-AUTHENTICATION-010-C — Expired token

- **WHEN** a token's expiry is in the past
- **THEN** the system answers `401 Unauthorized` with no grace period

#### Scenario: SCN-AUTHENTICATION-010-D — Missing or non-numeric subject

- **WHEN** a token omits its subject or carries a subject that is not a user identifier
- **THEN** the system answers `401 Unauthorized`

### Requirement: REQ-AUTHENTICATION-012 — Tokens are stateless

An issued token SHALL remain valid until its expiry. The system SHALL NOT provide logout or revocation, and changing a
credential SHALL NOT invalidate tokens already issued.

#### Scenario: SCN-AUTHENTICATION-012-A — Rotation leaves live tokens valid

- **WHEN** a credential is rotated while a token issued against the old password is unexpired
- **THEN** that token still authenticates the user until it expires

### Requirement: REQ-AUTHENTICATION-013 — An authenticated caller regenerates its token

An authenticated caller SHALL be able to regenerate its access token without resubmitting credentials. The response
SHALL match login: `200 OK` with a new `access_token`, `expires_in`, `token_type` of `bearer`, and `_links.self` naming
the regeneration operation. The new token SHALL carry the same subject and a fresh expiry of `security.jwt.ttl`, and the
previous token SHALL remain valid until its own expiry.

#### Scenario: SCN-AUTHENTICATION-013-A — Regeneration issues a fresh token

- **WHEN** an authenticated caller requests a new token
- **THEN** the response is `200 OK`, `application/hal+json`, with a new `access_token`, `expires_in`, `token_type` of
  `bearer` and `_links.self` naming `/api/v1/identity/token`

#### Scenario: SCN-AUTHENTICATION-013-B — The new token keeps the caller

- **WHEN** the caller presents the regenerated token on a protected endpoint
- **THEN** the endpoint resolves the same user as the token it replaced

#### Scenario: SCN-AUTHENTICATION-013-C — The previous token is not revoked

- **WHEN** a caller regenerates its token
- **THEN** the previous token still authenticates until its own expiry

### Requirement: REQ-AUTHENTICATION-014 — The Root Admin rotates any credential

The Root Admin SHALL replace the password behind a credential it names by identifier, for its own or another user's
credential, answering `204 No Content` and never returning the password. An unknown credential SHALL answer
`404 Not Found`. A non-root caller SHALL answer `403 Forbidden`, and a permission override naming `credential.patch`
SHALL answer `422 Unprocessable Content` because the action is not configurable.

#### Scenario: SCN-AUTHENTICATION-014-A — Own credential

- **WHEN** the Root Admin rotates its own credential with a policy-compliant password
- **THEN** the system answers `204 No Content`, the old password stops authenticating and the new one succeeds

#### Scenario: SCN-AUTHENTICATION-014-B — Another user's credential

- **WHEN** the Root Admin rotates another user's credential
- **THEN** the system answers `204 No Content` and that user's new password authenticates

#### Scenario: SCN-AUTHENTICATION-014-C — Unknown credential

- **WHEN** the Root Admin names a credential that does not exist
- **THEN** the system answers `404 Not Found` and stores nothing

#### Scenario: SCN-AUTHENTICATION-014-D — Non-root caller is refused

- **WHEN** an Admin or Standard User requests a credential rotation
- **THEN** the system answers `403 Forbidden` and stores nothing

#### Scenario: SCN-AUTHENTICATION-014-E — The action cannot be granted

- **WHEN** the Root Admin submits a permission override naming `credential.patch`
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

### Requirement: REQ-AUTHENTICATION-015 — Rotation is policy-checked and atomic

A rotation SHALL accept only a password satisfying the credential password policy (`REQ-AUTHENTICATION-018`). A rejected
rotation SHALL answer `422 Unprocessable Content` and leave the stored hash unchanged, and a successful rotation SHALL
replace the whole credential's password in one transaction.

#### Scenario: SCN-AUTHENTICATION-015-A — Password fails the policy

- **WHEN** the submitted password does not satisfy the credential password policy
- **THEN** the system answers `422 Unprocessable Content` and the old password still authenticates

#### Scenario: SCN-AUTHENTICATION-015-B — A failed rotation applies nothing

- **WHEN** the replacement cannot be committed
- **THEN** the previous hash remains and the new password does not authenticate

### Requirement: REQ-AUTHENTICATION-018 — Passwords follow the general policy

A user-chosen password SHALL be at least 8 characters long and SHALL contain at least one non-alphanumeric character.
The policy SHALL apply wherever a password is set: account creation and credential rotation.

#### Scenario: SCN-AUTHENTICATION-018-A — A compliant password is accepted

- **WHEN** a password is at least 8 characters long and contains a non-alphanumeric character
- **THEN** the operation that sets it is accepted

#### Scenario: SCN-AUTHENTICATION-018-B — A short password is rejected

- **WHEN** a password is shorter than 8 characters
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

#### Scenario: SCN-AUTHENTICATION-018-C — A password without a symbol is rejected

- **WHEN** a password of at least 8 characters contains only alphanumeric characters
- **THEN** the system answers `422 Unprocessable Content` and stores nothing
