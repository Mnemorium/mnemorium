# authentication Specification

## Purpose

Establishes and verifies a caller's identity: bootstraps the Root Admin credential on first start, exchanges a username
and password for a bearer token, validates that token on every protected request, and rotates a credential. It also owns
the general password policy that account creation and rotation enforce.

## Requirements

### Requirement: REQ-AUTHENTICATION-001 — The first start creates the Root Admin atomically

When no Root Admin exists, the system SHALL create user identifier 0 with username `root`, role Admin, no email, a
credential holding the hash of a generated secret that is never revealed, and one unclaimed setup grant, in one
datastore transaction. A failure at any step SHALL persist none of them and SHALL stop startup before the server accepts
requests.

#### Scenario: SCN-AUTHENTICATION-001-A — Fresh instance

- **WHEN** the server starts and no user with identifier 0 exists
- **THEN** user 0 exists as `root` with the Admin role, exactly one credential and one unclaimed setup grant

#### Scenario: SCN-AUTHENTICATION-001-B — Already initialized

- **WHEN** the server starts and user 0 already exists
- **THEN** the system creates no account, replaces no credential, creates no setup grant, and reveals no token

#### Scenario: SCN-AUTHENTICATION-001-C — Credential creation fails

- **WHEN** the credential cannot be stored
- **THEN** no user 0, no credential and no setup grant exist, and startup fails before the server accepts requests

#### Scenario: SCN-AUTHENTICATION-001-D — Account creation fails

- **WHEN** the user cannot be stored after its credential was created
- **THEN** neither the user nor the credential is committed and startup fails before the server accepts requests

#### Scenario: SCN-AUTHENTICATION-001-E — Setup grant creation fails

- **WHEN** the setup grant cannot be stored after the user and its credential were created
- **THEN** none of the user, the credential or the grant is committed, and startup fails before the server accepts
  requests

### Requirement: REQ-AUTHENTICATION-002 — The setup token follows the declared shape

The setup token SHALL be 8 characters from Crockford base32 (`0`-`9` and `A`-`Z` excluding `I`, `L`, `O` and `U`),
displayed as `XXXX-XXXX`. The dash SHALL be a separator, and a submitted token SHALL be accepted case-insensitively,
without its dash, and with `I` or `L` read as `1` and `O` as `0`.

#### Scenario: SCN-AUTHENTICATION-002-A — Generated value shape

- **WHEN** the system issues a setup token
- **THEN** it is 8 characters from the declared alphabet, displayed as `XXXX-XXXX`

#### Scenario: SCN-AUTHENTICATION-002-B — Input is normalized

- **WHEN** a submitted token differs from the issued one only in case, dash placement, or an `I`, `L` or `O` read as
  `1`, `1` or `0`
- **THEN** it names the same setup grant

### Requirement: REQ-AUTHENTICATION-003 — The one-time setup token is never withheld

The system SHALL reveal the setup token exactly once per issue, on standard output, as text and as a machine-scannable
QR code. It SHALL refuse to start when configuration disables the reveal of a token it would issue, rather than create
an account nobody can set up. Once setup has completed, a start SHALL issue and reveal nothing.

#### Scenario: SCN-AUTHENTICATION-003-A — Reveal is enabled

- **WHEN** the Root Admin is created and `security.root_admin_setup.reveal` is true
- **THEN** the token and its QR code are printed to standard output once and appear in no log file or API response

#### Scenario: SCN-AUTHENTICATION-003-B — Reveal is disabled on a first start

- **WHEN** the Root Admin would be created and `security.root_admin_setup.reveal` is false
- **THEN** startup fails, no account is created, and the failure names the setting

#### Scenario: SCN-AUTHENTICATION-003-C — Reveal is disabled after initialization

- **WHEN** the Root Admin exists with completed setup and `security.root_admin_setup.reveal` is false
- **THEN** startup proceeds and no token is revealed

#### Scenario: SCN-AUTHENTICATION-003-D — Reveal is disabled while setup is pending

- **WHEN** the Root Admin exists with an unclaimed setup grant and `security.root_admin_setup.reveal` is false
- **THEN** startup fails, the grant is unchanged, and the failure names the setting

### Requirement: REQ-AUTHENTICATION-004 — Valid credentials issue a bearer token

The login operation SHALL be reachable without a bearer token and, for a matching identifier and password, SHALL answer
`200 OK` with `application/hal+json` carrying `access_token`, `expires_in`, `token_type` set to `bearer`, and
`_links.self` naming the login operation. The identifier SHALL be a username or an email address; an identifier that
matches a username SHALL resolve as that username, and only otherwise SHALL it be looked up as an email address.

#### Scenario: SCN-AUTHENTICATION-004-A — Successful login

- **WHEN** an existing user submits its username or email address and matching password
- **THEN** the response is `200 OK`, `application/hal+json`, with `access_token`, `expires_in`, `token_type` of `bearer`
  and `_links.self` naming `/api/v1/identity/login`

#### Scenario: SCN-AUTHENTICATION-004-B — The issued token authenticates

- **WHEN** the caller presents the returned token on a protected endpoint
- **THEN** the endpoint acts as the authenticated user

#### Scenario: SCN-AUTHENTICATION-004-C — The response carries no password

- **WHEN** a login succeeds
- **THEN** the response contains no password or password hash

#### Scenario: SCN-AUTHENTICATION-004-D — A login by email address

- **WHEN** an existing user submits the email address stored on its account and its matching password
- **THEN** the login succeeds exactly as it does for the username

### Requirement: REQ-AUTHENTICATION-005 — Bad credentials are rejected uniformly

An identifier that resolves to no account and a wrong password SHALL be indistinguishable: the same `401 Unauthorized`
status and the same error message. The system SHALL perform comparable password work on both branches so the response
time does not reveal which value was wrong.

#### Scenario: SCN-AUTHENTICATION-005-A — Bad credentials

- **WHEN** an existing identifier is submitted with a wrong password, or an identifier that resolves to no account is
  submitted
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
identifier; an identifier that resolves to no account SHALL use a derived key that does not reveal it. An attempt SHALL
be rejected when either key is exhausted, answering `429 Too Many Requests` with `Retry-After`. Both keys SHALL use the
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

#### Scenario: SCN-AUTHENTICATION-008-F — An unknown email is throttled identically

- **WHEN** attempts against an email address no account holds exceed its quota
- **THEN** they answer `429 Too Many Requests`, indistinguishable from an email address that exists

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
`404 Not Found`. A non-root caller SHALL answer `403 Forbidden`.

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

### Requirement: REQ-AUTHENTICATION-019 — The setup token is derived and never stored

The setup grant SHALL hold no token material: a nonce, its expiry and its consumption instant only. The system SHALL
derive the token from the instance's **Site pepper** and that nonce, so a datastore copy without the persisted state
reveals neither the token nor a way to verify a guess.

#### Scenario: SCN-AUTHENTICATION-019-A — The stored grant carries no token

- **WHEN** a first start issues a setup token
- **THEN** the setup grant holds a nonce, an expiry and no consumption instant, and no stored value is the token

### Requirement: REQ-AUTHENTICATION-020 — The setup token is single-use, expiring and rate-limited

The system SHALL accept the setup token only at the setup operation, before its expiry and only once, under a rate limit
keyed by the client address and a process-wide key. A token that is unknown, expired or already used SHALL answer one
indistinguishable `401 Unauthorized`. The token SHALL NOT authenticate any protected endpoint. While setup is pending,
every start SHALL replace the token and reveal the new one.

#### Scenario: SCN-AUTHENTICATION-020-A — A successful setup consumes the token

- **WHEN** a setup succeeds and the same token is submitted again
- **THEN** the second submission answers `401 Unauthorized` and applies nothing

#### Scenario: SCN-AUTHENTICATION-020-B — An expired token is refused

- **WHEN** a token is submitted after `security.root_admin_setup.ttl_seconds` has elapsed since it was issued
- **THEN** the system answers `401 Unauthorized` and applies nothing

#### Scenario: SCN-AUTHENTICATION-020-C — An unknown token is refused identically

- **WHEN** a token that was never issued is submitted
- **THEN** the answer is `401 Unauthorized`, identical to an expired token

#### Scenario: SCN-AUTHENTICATION-020-D — A restart replaces the token

- **WHEN** the server starts while setup is pending
- **THEN** the previously issued token answers `401 Unauthorized` and the newly revealed token is accepted

#### Scenario: SCN-AUTHENTICATION-020-E — Setup is rate limited

- **WHEN** setup submissions exceed the configured budget for a client address or for the process
- **THEN** the answer is `429 Too Many Requests` with `Retry-After`

#### Scenario: SCN-AUTHENTICATION-020-F — The setup token is not a session

- **WHEN** the setup token is presented as a bearer token on a protected endpoint
- **THEN** the system answers `401 Unauthorized`

### Requirement: REQ-AUTHENTICATION-021 — Setup replaces the Root Admin credential and optional email atomically

The `POST /api/v1/identity/setup` operation SHALL accept the setup token, a password that satisfies the credential
password policy and an optional email, and in one transaction SHALL replace the Root Admin credential's password, store
the email when one is supplied, and consume the grant; the root's username and role SHALL remain unchanged. A password
that fails the policy or an email another account holds SHALL change nothing and leave the grant usable.

#### Scenario: SCN-AUTHENTICATION-021-A — A complete setup

- **WHEN** a live token is submitted with a policy-compliant password and an unused email
- **THEN** the answer is `204 No Content`, the new password authenticates the Root Admin, the email is stored, and the
  grant is consumed

#### Scenario: SCN-AUTHENTICATION-021-B — The email is optional

- **WHEN** a live token is submitted with a policy-compliant password and no email
- **THEN** the answer is `204 No Content` and the Root Admin has no email

#### Scenario: SCN-AUTHENTICATION-021-C — A weak password is refused

- **WHEN** a live token is submitted with a password that fails the credential policy
- **THEN** the answer is `422 Unprocessable Content`, the credential is unchanged, and the grant remains usable

#### Scenario: SCN-AUTHENTICATION-021-D — A taken email is refused

- **WHEN** a live token is submitted with an email another account already holds
- **THEN** the answer is `409 Conflict`, the credential and the user are unchanged, and the grant remains usable

#### Scenario: SCN-AUTHENTICATION-021-E — A failed write applies nothing

- **WHEN** the credential replacement cannot be committed
- **THEN** the previous credential and the root's email remain, and the grant remains usable
