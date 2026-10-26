# instance/state Specification

## Purpose

Preserves the instance's non-derivable identity — the generated site pepper and token signing secret — across restarts
independently of the datastore, and refuses to start rather than silently replace it.

## Requirements

### Requirement: Identity is resolved before configuration and the datastore

The system SHALL establish or load the instance identity before resolving configuration and before opening the
datastore. A failure to read the identity SHALL stop startup before any datastore file is opened.

#### Scenario: Unreadable identity stops startup early

- **WHEN** the persisted identity cannot be read
- **THEN** startup stops, no configuration file is written, and no datastore file is opened

#### Scenario: Identity is read on every start

- **WHEN** the server starts and a persisted identity exists
- **THEN** that identity is loaded before configuration is resolved

### Requirement: Identity holds only non-derivable secrets

The persisted identity SHALL hold the site pepper, the token signing secret and the instant it was established. It SHALL
NOT hold any value that configuration can supply, and both secrets SHALL be 64-character hexadecimal strings.

#### Scenario: Identity carries the two secrets

- **WHEN** an identity is created
- **THEN** it holds a 64-character hexadecimal pepper and a 64-character hexadecimal signing secret

#### Scenario: Configuration changes do not alter identity

- **WHEN** the server restarts with a different configuration file
- **THEN** the persisted identity and its secrets are unchanged

### Requirement: Environment seeds apply only at creation

When creating identity, the system SHALL use the pepper and signing secret supplied by the environment, and SHALL
generate 32 random bytes for each that is absent. After creation, the persisted value SHALL be authoritative; a supplied
seed that differs from it SHALL fail startup.

#### Scenario: Seed is used at creation

- **WHEN** the environment supplies a signing secret and no identity exists
- **THEN** the persisted signing secret equals the supplied value

#### Scenario: Secrets are generated when not seeded

- **WHEN** the environment supplies neither secret and no identity exists
- **THEN** the system generates a 32-byte value for each secret and persists it

#### Scenario: A differing seed fails startup

- **WHEN** the environment supplies a signing secret that differs from the persisted one
- **THEN** startup fails and the persisted identity is left unchanged

#### Scenario: A matching seed is accepted

- **WHEN** the environment supplies a value equal to the persisted one
- **THEN** startup proceeds and the persisted identity is unchanged

### Requirement: A missing identity for an existing datastore fails startup

When the datastore already exists and no identity is persisted, the system SHALL refuse to start and SHALL NOT create a
replacement identity. An existing identity with no datastore SHALL be reused and the datastore created.

#### Scenario: Existing datastore without identity

- **WHEN** the datastore exists and no identity is persisted
- **THEN** startup fails without writing an identity and without modifying the datastore

#### Scenario: Fresh instance

- **WHEN** neither the datastore nor an identity exists
- **THEN** the system creates and persists the identity before creating the datastore

#### Scenario: Identity without a datastore

- **WHEN** a persisted identity exists and the datastore does not
- **THEN** the system reuses that identity and then creates the datastore

### Requirement: Identity persistence is atomic, versioned and private

The identity SHALL be written atomically and SHALL be readable only by the account the server runs as. A file that
cannot be decoded, is truncated or corrupt, or declares a newer format version SHALL fail startup and SHALL NOT be
treated as absent.

#### Scenario: Corrupt identity is not regenerated

- **WHEN** the identity file is truncated or corrupt
- **THEN** startup fails and no replacement identity is written

#### Scenario: Newer format is refused

- **WHEN** the identity declares a format version this build does not support
- **THEN** startup fails naming the unsupported version

#### Scenario: Identity is private

- **WHEN** the identity is written
- **THEN** only the account the server runs as can read or write it

#### Scenario: No partially written identity

- **WHEN** the system writes the identity
- **THEN** a crash never leaves a partially written file in place of a complete one

### Requirement: Identity creation is reported and secrets never leak

The system SHALL emit one creation event when it creates identity, plus one secret-initialization event for each secret
it generated, and SHALL emit neither when it reuses an existing identity. The pepper and the signing secret SHALL NOT
appear in logs, error output or debug representations.

#### Scenario: Creation is reported

- **WHEN** identity is created
- **THEN** exactly one creation event is emitted, followed by one event per generated secret

#### Scenario: A seeded secret reports no generation

- **WHEN** identity is created with a secret supplied by the environment
- **THEN** no secret-initialization event is emitted for that secret

#### Scenario: Reuse emits no creation event

- **WHEN** a persisted identity is loaded
- **THEN** no creation event is emitted

#### Scenario: Secrets never reach a sink

- **WHEN** startup fails while reading or creating identity
- **THEN** neither secret appears in the error or in any log output

### Requirement: Concurrent first starts create at most one identity

When two processes start against the same new instance, at most one SHALL persist an identity. No process SHALL serve
requests using a secret it did not persist.

#### Scenario: Concurrent creation

- **WHEN** two processes start against a fresh instance at the same time
- **THEN** exactly one identity is persisted and neither process serves requests using an unpersisted secret
