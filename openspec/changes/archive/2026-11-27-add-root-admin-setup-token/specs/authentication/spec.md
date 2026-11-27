# Spec Delta

## RENAMED Requirements

- FROM: `### Requirement: REQ-AUTHENTICATION-002 — The generated password follows the declared shape`
- TO: `### Requirement: REQ-AUTHENTICATION-002 — The setup token follows the declared shape`
- FROM: `### Requirement: REQ-AUTHENTICATION-003 — The one-time password is never withheld`
- TO: `### Requirement: REQ-AUTHENTICATION-003 — The one-time setup token is never withheld`

## MODIFIED Requirements

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

## ADDED Requirements

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
