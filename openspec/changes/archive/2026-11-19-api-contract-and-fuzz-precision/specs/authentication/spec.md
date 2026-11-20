# Spec Delta

## MODIFIED Requirements

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
