# Spec Delta

## ADDED Requirements

### Requirement: REQ-MUSIC-029 — An album's metadata is updated partially

The system SHALL update an album a **Caller** may `View`, requiring `EditMetadata` on it. An absent field SHALL keep its
stored value and a `null` SHALL clear an optional value. A name and group that duplicate another album SHALL answer
`409 Conflict` and apply nothing; a value that fails validation SHALL answer `422 Unprocessable Content` and apply
nothing.

#### Scenario: SCN-MUSIC-029-A — Change the release date and genres

- **WHEN** a caller submits a new release date and genre list
- **THEN** the system stores them and returns the album representation

#### Scenario: SCN-MUSIC-029-B — Clear an optional value

- **WHEN** the caller sends `release_date: null`
- **THEN** the album stores no release date and reports `null`

#### Scenario: SCN-MUSIC-029-C — The group change collides

- **WHEN** the requested group makes the name and group duplicate another album
- **THEN** the system answers `409 Conflict` and applies nothing

#### Scenario: SCN-MUSIC-029-D — An absent field keeps its value

- **WHEN** the request names no production type
- **THEN** the stored production type is unchanged

#### Scenario: SCN-MUSIC-029-E — An invalid value applies nothing

- **WHEN** the name is empty or too long, a genre is empty, or the group does not exist
- **THEN** the system answers `422 Unprocessable Content` and applies nothing
