# Spec Delta

## MODIFIED Requirements

### Requirement: Recording metadata stands alone

The system SHALL return recording metadata whose shape does not depend on the provider. A recording SHALL carry a title,
the artist credits and genres the provider reports and the ISRC codes the provider reports.

#### Scenario: Recording metadata

- **WHEN** a recording is resolved
- **THEN** it carries a title, its artist credits, its genres and its ISRC codes

#### Scenario: A recording on several releases

- **WHEN** a recording appears on more than one release
- **THEN** its metadata is returned once, independently of any release

## ADDED Requirements

### Requirement: Music identification is served over HTTP

The system SHALL expose candidate search at `GET /api/v1/library/music/identify` and metadata resolution at
`GET /api/v1/library/music/identify/{provider}/{kind}/{reference}`. Both SHALL require `library.read`, and both SHALL
report identification as unavailable when no provider is configured.

#### Scenario: Candidate search

- **WHEN** an authenticated caller searches for an album with a title, a group and a year
- **THEN** the system returns the provider's candidates in the provider's order

#### Scenario: Metadata resolution

- **WHEN** an authenticated caller resolves a candidate issued by the configured provider
- **THEN** the system returns the provider-independent metadata document

#### Scenario: Unknown provider

- **WHEN** the reference names a provider that is not configured
- **THEN** the system returns `404 Not Found`

#### Scenario: No provider configured

- **WHEN** no music provider is configured
- **THEN** the system reports identification as unavailable, distinct from an empty candidate list

#### Scenario: Unauthenticated search

- **WHEN** a request reaches either route without a valid bearer token
- **THEN** the system returns `401 Unauthorized`

#### Scenario: Caller without library read

- **WHEN** an authenticated caller's `library.read` scope is `NONE` or `OWNED`
- **THEN** the system returns `404 Not Found` for either route
