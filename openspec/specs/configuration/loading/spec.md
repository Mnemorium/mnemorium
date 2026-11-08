# configuration/loading Specification

## Purpose

Resolves a complete configuration from built-in defaults, an optional configuration file and the environment, and writes
a complete default file when none exists. The environment overrides the file.

## Requirements

### Requirement: Configuration resolves in layers

The system SHALL resolve configuration from built-in defaults, then the configuration file, then the environment, in
that order of increasing precedence. A value the configuration file names SHALL replace the built-in default, and a
value the environment names SHALL replace both.

#### Scenario: Defaults when nothing is supplied

- **WHEN** no configuration file exists
- **THEN** every setting takes its built-in default

#### Scenario: File overrides defaults

- **WHEN** the configuration file sets a value
- **THEN** that value is effective

#### Scenario: Environment overrides the file

- **WHEN** the configuration file and the environment both set a setting
- **THEN** the environment value is effective

### Requirement: Every setting has a default

Every configuration setting SHALL have a default value, so that an absent, empty or partial configuration file still
yields a complete configuration.

#### Scenario: Absent file

- **WHEN** the configuration file is absent
- **THEN** the configuration is complete and every setting takes its default

#### Scenario: Empty file

- **WHEN** the configuration file exists but is empty
- **THEN** the configuration is complete and every setting takes its default

#### Scenario: Partial file

- **WHEN** the configuration file sets only some settings
- **THEN** the remaining settings take their defaults

### Requirement: A default configuration file is written when absent

When the configuration file is absent, the system SHALL write a complete default configuration file at startup. It SHALL
NOT rewrite a file that already exists, including an empty or partial one.

#### Scenario: Absent file is written

- **WHEN** the configuration file is absent at startup
- **THEN** a complete default configuration file is written

#### Scenario: Existing file is left untouched

- **WHEN** the configuration file exists, empty or partial
- **THEN** it is not rewritten

#### Scenario: Unwritable location fails startup

- **WHEN** the configuration file is absent and cannot be written
- **THEN** startup fails naming the path

### Requirement: The environment overrides the file for the settings it names

The environment SHALL supply a configuration setting only through a variable carrying the server prefix followed by the
setting path.

#### Scenario: A variable overrides the file

- **WHEN** a prefixed variable names a setting the configuration file also sets
- **THEN** the environment value is effective

#### Scenario: An unknown variable is discarded

- **WHEN** a prefixed environment variable names no setting
- **THEN** startup proceeds and the variable changes no setting

#### Scenario: A malformed variable fails startup

- **WHEN** a prefixed variable names a setting but does not hold a value that setting accepts
- **THEN** startup fails naming the variable and not its value

#### Scenario: Overridden settings are reported

- **WHEN** the environment overrides one or more settings
- **THEN** startup reports which settings it overrode

### Requirement: Resolved configuration is immutable for the process

The effective configuration SHALL NOT change while the process runs. A change to the configuration file SHALL take
effect only after a restart.

#### Scenario: A runtime edit has no effect

- **WHEN** the configuration file changes while the server is running
- **THEN** the effective configuration does not change

#### Scenario: A restart applies the change

- **WHEN** the server restarts after a configuration change
- **THEN** the new value is effective
