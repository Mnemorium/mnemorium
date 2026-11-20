# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-LOGGING-001 — Every event reaches a file sink and a standard-output sink

The server SHALL write every emitted event to a rotating file under `logs/`, named `mnemorium.log`, and to standard
output. The file SHALL hold plain text; only standard output SHALL carry colour, and only when the installed
configuration enables it.

#### Scenario: SCN-LOGGING-001-A — The file sink holds the event

- **WHEN** the server emits an event
- **THEN** the event appears in `logs/mnemorium.log`

#### Scenario: SCN-LOGGING-001-B — The standard-output sink holds the event

- **WHEN** the server emits an event
- **THEN** the event appears on standard output

#### Scenario: SCN-LOGGING-001-C — The file sink is never coloured

- **WHEN** colour is enabled for standard output and an event is emitted
- **THEN** its file line carries no ANSI escape sequence

#### Scenario: SCN-LOGGING-001-D — A disabled configuration colours nothing

- **WHEN** colour is not installed and an event is emitted
- **THEN** neither sink carries an ANSI escape sequence

### Requirement: REQ-LOGGING-003 — Verbosity is startup-only and bounded

The verbosity directive SHALL be read once, at startup, and SHALL refine operational verbosity only. A value that is
empty, that disables logging as a whole (`off`, `none`), or that carries a directive for the reserved target SHALL be
refused at startup, naming the setting and never the value.

#### Scenario: SCN-LOGGING-003-A — An empty verbosity is refused

- **WHEN** the installed configuration carries an empty verbosity value
- **THEN** startup fails naming the setting and the server does not accept requests

#### Scenario: SCN-LOGGING-003-B — Disabling logging is refused

- **WHEN** the installed configuration disables logging as a whole with `off` or `none`
- **THEN** startup fails naming the setting and the value is not echoed

#### Scenario: SCN-LOGGING-003-C — Targeting the reserved target is refused

- **WHEN** the installed configuration carries a directive that names the reserved target
- **THEN** startup fails naming the setting and the server does not accept requests

#### Scenario: SCN-LOGGING-003-D — The value tunes operational verbosity

- **WHEN** a verbosity level is installed and the server emits operational records at each severity
- **THEN** the records at that severity and above reach a sink
- **AND** the records below it reach none
