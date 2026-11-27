# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-LOGGING-008 — No secret or personal value reaches a sink

No secret or personal value SHALL be recorded as an event or written to the file log: application source, session
identifiers, access tokens, passwords or password material, the setup token, database connection strings, the pepper,
the signing secret, and personal data. The one-time standard-output reveal of the setup token is the sole exception. A
type that holds one SHALL NOT render it in its debug representation.

#### Scenario: SCN-LOGGING-008-A — Login secrets stay out

- **WHEN** a password is submitted and a token is issued
- **THEN** neither the password nor the token appears in any sink

#### Scenario: SCN-LOGGING-008-B — Instance secrets stay out

- **WHEN** the instance identity generates its pepper and signing secret
- **THEN** neither value appears in any sink

#### Scenario: SCN-LOGGING-008-C — A secret-bearing type redacts its debug output

- **WHEN** a type holding a secret or personal data is formatted for diagnostics
- **THEN** its debug representation prints a placeholder instead of the value

#### Scenario: SCN-LOGGING-008-D — Personal data stays out

- **WHEN** an event names a user
- **THEN** it carries the user identifier and no username or email address

#### Scenario: SCN-LOGGING-008-E — The setup token stays out of the events

- **WHEN** the first start issues the setup token
- **THEN** it appears on standard output and in no event or file log
