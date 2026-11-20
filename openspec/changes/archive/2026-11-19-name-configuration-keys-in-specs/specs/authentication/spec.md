# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-AUTHENTICATION-008 — Login is rate limited per client address and account

Login SHALL be limited both per client address and per user account. The account key SHALL be the resolved user
identifier; a username that resolves to no account SHALL use a derived key that does not reveal it. An attempt SHALL be
rejected when either key is exhausted, answering `429 Too Many Requests` with `Retry-After`. Both keys SHALL use
`security.rate_limit.burst_size` and `security.rate_limit.period_seconds`. The client address SHALL be the peer unless
it is listed in `security.rate_limit.trusted_proxies`, whose `security.rate_limit.client_ip_header` is then read.

#### Scenario: SCN-AUTHENTICATION-008-A — Client burst is throttled

- **WHEN** one client address exceeds `security.rate_limit.burst_size` login attempts
- **THEN** the next attempt answers `429 Too Many Requests` with `Retry-After`

#### Scenario: SCN-AUTHENTICATION-008-B — The quota refills

- **WHEN** `security.rate_limit.period_seconds` elapses after a throttled attempt
- **THEN** the next attempt is admitted

#### Scenario: SCN-AUTHENTICATION-008-C — A spoofed header cannot mint a new quota

- **WHEN** a peer absent from `security.rate_limit.trusted_proxies` sends `security.rate_limit.client_ip_header`
- **THEN** the limiter keys on the peer address and ignores the header

#### Scenario: SCN-AUTHENTICATION-008-D — One account is throttled across addresses

- **WHEN** attempts against one account arrive from many client addresses
- **THEN** they answer `429 Too Many Requests` once the account's quota is spent, even while each address is below its
  own quota

#### Scenario: SCN-AUTHENTICATION-008-E — An unknown username is throttled identically

- **WHEN** attempts against a username no account holds exceed its quota
- **THEN** they answer `429 Too Many Requests`, indistinguishable from a username that exists
