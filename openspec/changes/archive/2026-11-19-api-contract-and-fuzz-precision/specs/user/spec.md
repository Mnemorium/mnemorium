# Spec Delta

## REMOVED Requirements

### Requirement: REQ-USER-003 — Usernames are bounded, lowercase, and unique

**Reason**: The rule flips. Uppercase becomes allowed and whitespace becomes rejected anywhere in the value, so the
requirement's title and two scenario outcomes change. OpenSpec refuses a scenario rename inside a `MODIFIED`
requirement, so the rule is replaced by `REQ-USER-020` rather than modified in place.

**Migration**: No stored value changes, because every stored username is already lowercase and trimmed. A client that
rejected uppercase usernames must accept them; a client that sent surrounding whitespace must send the exact value.

## ADDED Requirements

### Requirement: REQ-USER-020 — Usernames are bounded, whitespace-free, and unique

A username SHALL be at least 4 and at most 100 characters long, SHALL contain no whitespace anywhere, and SHALL contain
no NUL (`U+0000`) character; MAY contain uppercase letters, and every other non-whitespace character is allowed. The
system SHALL store the submitted value unchanged and SHALL NOT trim, lowercase or otherwise normalize it. It SHALL be
unique across accounts, compared exactly.

#### Scenario: SCN-USER-020-A — Whitespace anywhere is rejected

- **WHEN** a request carries a username containing whitespace at any position
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

#### Scenario: SCN-USER-020-B — Uppercase letters are accepted

- **WHEN** a request carries a username containing uppercase letters
- **THEN** the account is created with that username, stored unchanged

#### Scenario: SCN-USER-020-C — Boundary lengths are accepted

- **WHEN** a request carries a username of exactly 4 or exactly 100 characters
- **THEN** the account is created with that username

#### Scenario: SCN-USER-020-D — Username is too short

- **WHEN** a request carries an empty, blank, or 3-character username
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

#### Scenario: SCN-USER-020-E — Username is too long

- **WHEN** a request carries a username of 101 characters or more
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

#### Scenario: SCN-USER-020-F — Username is already taken

- **WHEN** a request carries a username another account already holds
- **THEN** the system answers `409 Conflict` with a message stating that the username is taken, and stores nothing

#### Scenario: SCN-USER-020-G — A NUL character is rejected

- **WHEN** a request carries a username containing a NUL (`U+0000`) character
- **THEN** the system answers `422 Unprocessable Content` and stores nothing
