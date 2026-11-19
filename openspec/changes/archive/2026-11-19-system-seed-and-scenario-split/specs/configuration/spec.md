# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-CONFIGURATION-001 — A complete configuration resolves in three layers

The system SHALL resolve the complete configuration from the built-in defaults, then the settings the configuration file
names, then the settings the environment names, in that order of increasing precedence. Every setting SHALL have a
built-in default, so an absent, empty or partial file still yields a complete configuration.

#### Scenario: SCN-CONFIGURATION-001-A — Defaults when nothing is supplied

- **WHEN** no configuration file exists and no environment variable names a setting
- **THEN** every setting takes its built-in default

#### Scenario: SCN-CONFIGURATION-001-B — The file overrides a built-in default

- **WHEN** the configuration file names a setting
- **THEN** that value is effective in place of the built-in default

#### Scenario: SCN-CONFIGURATION-001-C — The environment overrides the file

- **WHEN** the configuration file and the environment both name a setting
- **THEN** the environment value is effective

#### Scenario: SCN-CONFIGURATION-001-D — A partial file keeps the remaining defaults

- **WHEN** the configuration file names only some settings
- **THEN** the settings it omits take their built-in default

#### Scenario: SCN-CONFIGURATION-001-E — The resolved value reaches the artefact its consumer emits

- **WHEN** a setting is resolved from a layer and the capability that consumes it derives an artefact from it
- **THEN** the artefact carries the resolved value
