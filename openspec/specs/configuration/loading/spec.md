# configuration/loading Specification

## Purpose

Resolves a complete configuration from built-in defaults, an optional configuration file and the environment, without
reading the datastore, and writes a complete default file when none exists. The environment overrides the file; the
identity seeds are creation input rather than settings.

## Requirements

### Requirement: Configuration resolves without the datastore

The system SHALL resolve configuration from built-in defaults, then the configuration file, then the environment, in
that order of increasing precedence, before the datastore is opened. It SHALL NOT read or write the datastore to obtain
configuration.

#### Scenario: Defaults when nothing is supplied

- **WHEN** no configuration file exists
- **THEN** every setting takes its built-in default

#### Scenario: File overrides defaults

- **WHEN** the configuration file sets a value
- **THEN** that value is effective

#### Scenario: Environment overrides the file

- **WHEN** the configuration file and the environment both set a setting
- **THEN** the environment value is effective

#### Scenario: Resolution does not need the datastore

- **WHEN** the datastore is absent or unreachable at startup
- **THEN** configuration resolution still produces a complete configuration

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

### Requirement: The generated file contains defaults only

The generated configuration file SHALL contain the built-in defaults. It SHALL NOT contain the site pepper or the token
signing secret.

#### Scenario: No secrets in the generated file

- **WHEN** the configuration file is generated
- **THEN** it contains neither the pepper nor the signing secret

### Requirement: The environment overrides the file for the settings it names

The environment SHALL supply a configuration setting only through a variable carrying the server prefix followed by the
setting path. A prefixed variable that names no setting SHALL be discarded without failing startup, and a variable that
names a setting but does not hold a value that setting accepts SHALL fail startup naming the variable. The identity seed
names SHALL NOT be settings and SHALL apply only when identity is created.

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

#### Scenario: A seed changes no setting

- **WHEN** only the identity seed variables are set and no configuration file exists
- **THEN** every configuration setting takes its built-in default and startup proceeds

### Requirement: Secrets and the datastore path are not configuration

The site pepper and the token signing secret SHALL come only from the instance identity. The datastore path SHALL be
fixed at `mnemorium.db` relative to the working directory and SHALL NOT be configurable.

#### Scenario: Identity is authoritative for secrets

- **WHEN** the server starts
- **THEN** the effective pepper and signing secret are the persisted identity's

#### Scenario: The datastore path is fixed

- **WHEN** the server starts
- **THEN** the datastore is opened at `mnemorium.db` and no setting can change that path

### Requirement: Resolved configuration is immutable for the process

The effective configuration SHALL NOT change while the process runs. A change to the configuration file SHALL take
effect only after a restart.

#### Scenario: A runtime edit has no effect

- **WHEN** the configuration file changes while the server is running
- **THEN** the effective configuration does not change

#### Scenario: A restart applies the change

- **WHEN** the server restarts after a configuration change
- **THEN** the new value is effective
