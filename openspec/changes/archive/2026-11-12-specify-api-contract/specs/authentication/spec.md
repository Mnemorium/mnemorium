# Spec Delta

## MODIFIED Requirements

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
