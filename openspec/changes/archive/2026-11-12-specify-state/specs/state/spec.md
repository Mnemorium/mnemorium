# Spec Delta

## Purpose

Defines the instance's non-derivable state: the site pepper, the token signing secret and the instant it was
established. The state survives restarts independently of the datastore and is never silently replaced; its creation
seeds are environment input that applies only at creation.

## ADDED Requirements

### Requirement: REQ-STATE-001 — The state holds the site pepper

The persisted state SHALL hold the site pepper as a 64-character hexadecimal string.

#### Scenario: SCN-STATE-001-A — The state carries the pepper

- **WHEN** the state is created
- **THEN** it holds a 64-character hexadecimal pepper

#### Scenario: SCN-STATE-001-B — A restart returns the same pepper

- **WHEN** the server restarts over the same working directory
- **THEN** the persisted pepper is unchanged

### Requirement: REQ-STATE-002 — The state holds the token signing secret

The persisted state SHALL hold the token signing secret as a 64-character hexadecimal string.

#### Scenario: SCN-STATE-002-A — The state carries the signing secret

- **WHEN** the state is created
- **THEN** it holds a 64-character hexadecimal signing secret

#### Scenario: SCN-STATE-002-B — A restart returns the same signing secret

- **WHEN** the server restarts over the same working directory
- **THEN** the persisted signing secret is unchanged

### Requirement: REQ-STATE-003 — The state records the creation instant

The persisted state SHALL record the instant it was established as Unix seconds.

#### Scenario: SCN-STATE-003-A — The state records the instant

- **WHEN** the state is created
- **THEN** it records the creation instant as Unix seconds

#### Scenario: SCN-STATE-003-B — A restart returns the same instant

- **WHEN** the server restarts over the same working directory
- **THEN** the persisted creation instant is unchanged

### Requirement: REQ-STATE-004 — Creation seeds apply only at creation

When creating state, the system SHALL use the pepper supplied through `MNEMORIUM_PEPPER` and the signing secret supplied
through `MNEMORIUM_JWT_SECRET`, and SHALL generate 32 random bytes for each that is absent. After creation the persisted
value SHALL be authoritative; a supplied seed that differs SHALL fail startup. A seed that is present but not 64
hexadecimal characters SHALL fail startup naming the variable and never the value.

#### Scenario: SCN-STATE-004-A — A seed is used at creation

- **WHEN** the environment supplies a signing secret and no state exists
- **THEN** the persisted signing secret equals the supplied value

#### Scenario: SCN-STATE-004-B — Secrets are generated when not seeded

- **WHEN** the environment supplies neither seed and no state exists
- **THEN** the system generates a 32-byte value for each secret and persists it as 64 hexadecimal characters

#### Scenario: SCN-STATE-004-C — A differing seed fails startup

- **WHEN** the environment supplies a signing secret that differs from the persisted one
- **THEN** startup fails and the persisted state is left unchanged

#### Scenario: SCN-STATE-004-D — A matching seed is accepted

- **WHEN** the environment supplies a value equal to the persisted one
- **THEN** startup proceeds and the persisted state is unchanged

#### Scenario: SCN-STATE-004-E — A malformed seed fails startup

- **WHEN** the environment supplies a seed that is not 64 hexadecimal characters
- **THEN** startup fails naming the variable and never the value

### Requirement: REQ-STATE-005 — The state is established before the datastore

The system SHALL establish or load the state before opening the datastore. A failure to read the state SHALL stop
startup before any datastore file is opened.

#### Scenario: SCN-STATE-005-A — An unreadable state stops startup early

- **WHEN** the persisted state cannot be read
- **THEN** startup stops and no datastore file is opened

#### Scenario: SCN-STATE-005-B — The state is read on every start

- **WHEN** the server starts and a persisted state exists
- **THEN** that state is loaded before the datastore is opened

### Requirement: REQ-STATE-006 — A missing state for an existing datastore fails startup

When the datastore exists and no state is persisted, the system SHALL refuse to start and SHALL NOT create a replacement
state. An existing state with no datastore SHALL be reused and the datastore created, and when neither exists the state
SHALL be created first. The datastore's presence SHALL be decided without treating an unreadable path as absent; an
undecidable presence SHALL fail startup.

#### Scenario: SCN-STATE-006-A — Existing datastore without state

- **WHEN** the datastore exists and no state is persisted
- **THEN** startup fails without writing a state and without modifying the datastore

#### Scenario: SCN-STATE-006-B — Fresh instance

- **WHEN** neither the datastore nor a state exists
- **THEN** the state is created and persisted before the datastore is created

#### Scenario: SCN-STATE-006-C — State without a datastore

- **WHEN** a persisted state exists and the datastore does not
- **THEN** the state is reused and the datastore is created

#### Scenario: SCN-STATE-006-D — Undecidable datastore presence

- **WHEN** the presence of the datastore cannot be decided
- **THEN** startup fails without writing a state

### Requirement: REQ-STATE-007 — Persistence is fixed, atomic and versioned

The state SHALL be persisted at the fixed path `state.yaml` and written atomically. The document SHALL declare a format
version and SHALL contain no other field. A version this build does not support, an unknown field, a non-hexadecimal
secret, a truncated document or a document larger than 4 KiB SHALL fail startup and SHALL NOT be treated as absent.

#### Scenario: SCN-STATE-007-A — Corrupt state is not regenerated

- **WHEN** the state document is truncated or corrupt
- **THEN** startup fails and no replacement state is written

#### Scenario: SCN-STATE-007-B — An unsupported version is refused

- **WHEN** the state declares a format version this build does not support
- **THEN** startup fails naming the version

#### Scenario: SCN-STATE-007-C — An oversized document is refused

- **WHEN** the state document exceeds 4 KiB
- **THEN** startup fails before decoding it

#### Scenario: SCN-STATE-007-D — An unknown field is refused

- **WHEN** the state document carries a field beyond the two secrets, the creation instant and the version
- **THEN** startup fails and no replacement state is written

#### Scenario: SCN-STATE-007-E — The state lives at the fixed path

- **WHEN** the state is created
- **THEN** it is written at `state.yaml` in the working directory

#### Scenario: SCN-STATE-007-F — A failed write leaves no partial state

- **WHEN** writing the state fails before the atomic rename
- **THEN** any previous `state.yaml` is left unchanged and no partial `state.yaml` is written

### Requirement: REQ-STATE-008 — The state is private

The state file SHALL be a regular file readable and writable by no account other than the server account. When its mode
permits another account, the server SHALL set the mode to `0600` before reading it. A file that belongs to another
account, or that cannot be made private, SHALL fail startup and SHALL be left unchanged.

#### Scenario: SCN-STATE-008-A — The state is private on creation

- **WHEN** the state is created
- **THEN** its file mode is `0600`

#### Scenario: SCN-STATE-008-B — A restored state with a wider mode is repaired

- **WHEN** a persisted state is readable or writable by another account
- **THEN** the server sets its mode to `0600` and startup proceeds

#### Scenario: SCN-STATE-008-C — A private restored state is accepted

- **WHEN** a persisted state is private to the server account
- **THEN** startup proceeds and the state is reused unchanged

#### Scenario: SCN-STATE-008-D — A state that cannot be made private fails startup

- **WHEN** the state belongs to another account or cannot be set to `0600`
- **THEN** startup fails and the file is left unchanged
