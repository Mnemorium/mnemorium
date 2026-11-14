# configuration Specification

## Purpose

Resolves the complete configuration from built-in defaults, an optional fixed-location file and the environment, refuses
every input it cannot accept, and reports what it applied. It leaves each setting's meaning to the capability that
consumes it, and keeps the instance identity's secrets and the server's fixed locations outside its scope.

## Requirements

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

### Requirement: REQ-CONFIGURATION-002 — The configuration file is a fixed location holding a sparse document

The configuration file SHALL be named `config.yaml` and live at a fixed location that is not itself a setting, and SHALL
be a sparse document: a key it omits keeps the value of the layer below, and a key it names is applied. The file MAY
omit any section or key at any depth.

#### Scenario: SCN-CONFIGURATION-002-A — An omitted key keeps the lower value

- **WHEN** the configuration file omits a setting
- **THEN** the setting keeps the value of the layer below

#### Scenario: SCN-CONFIGURATION-002-B — A nested key may be named alone

- **WHEN** the configuration file names one key inside a section and no other key of that section
- **THEN** only that key is applied and its siblings keep their lower value

#### Scenario: SCN-CONFIGURATION-002-C — The file's location is not a setting

- **WHEN** an environment variable appears to address the location of `config.yaml`
- **THEN** it names no setting, changes nothing, and does not move the file

### Requirement: REQ-CONFIGURATION-003 — A complete default file is written when the file is absent

When the configuration file is absent, the system SHALL write a complete default configuration file. It SHALL NOT
rewrite a file that already exists, including an empty or partial one, and it SHALL fail startup naming the path when
the file cannot be written.

#### Scenario: SCN-CONFIGURATION-003-A — An absent file is written complete

- **WHEN** the configuration file is absent at startup
- **THEN** a complete default configuration file is written

#### Scenario: SCN-CONFIGURATION-003-B — An empty file is left untouched

- **WHEN** the configuration file exists and is empty
- **THEN** it is not rewritten and every setting takes its built-in default

#### Scenario: SCN-CONFIGURATION-003-C — A partial file is left untouched

- **WHEN** the configuration file exists and names only some settings
- **THEN** it is not rewritten

#### Scenario: SCN-CONFIGURATION-003-D — An unwritable location fails startup naming the path

- **WHEN** the configuration file is absent and cannot be written
- **THEN** startup fails and reports the path it could not write

### Requirement: REQ-CONFIGURATION-004 — A configuration document the server cannot accept fails startup

The system SHALL refuse to start when the configuration file cannot be read, is not a valid document, names a key that
is not a setting, or gives a setting a value of the wrong type. It SHALL NOT fall back to the built-in defaults, ignore
the offending key, or serve with a partial configuration.

#### Scenario: SCN-CONFIGURATION-004-A — An unknown key refuses startup

- **WHEN** the configuration file names a key that is not a setting
- **THEN** startup fails naming the key

#### Scenario: SCN-CONFIGURATION-004-B — A malformed document refuses startup

- **WHEN** the configuration file is not a valid document
- **THEN** startup fails instead of reverting to the built-in defaults

#### Scenario: SCN-CONFIGURATION-004-C — A wrong type refuses startup

- **WHEN** the configuration file gives a setting a value of the wrong type
- **THEN** startup fails

#### Scenario: SCN-CONFIGURATION-004-D — An unreadable file refuses startup

- **WHEN** the configuration file exists but cannot be read
- **THEN** startup fails naming the path

### Requirement: REQ-CONFIGURATION-005 — The environment addresses a setting by its path

The environment SHALL supply a setting only through a variable carrying the server prefix followed by the setting path,
its segments joined by single underscores. A variable that names a setting SHALL override the file. A prefixed variable
that names no setting SHALL be discarded without failing startup. The environment SHALL NOT carry a setting whose value
is a list or a credential.

#### Scenario: SCN-CONFIGURATION-005-A — A variable naming a setting overrides the file

- **WHEN** a prefixed variable names a setting the configuration file also names
- **THEN** the environment value is effective

#### Scenario: SCN-CONFIGURATION-005-B — A variable naming no setting is discarded

- **WHEN** a prefixed variable names no setting
- **THEN** startup proceeds and the variable changes no setting

#### Scenario: SCN-CONFIGURATION-005-C — The environment carries no list and no credential

- **WHEN** the environment carries a variable shaped like a list or a credential setting
- **THEN** it names no setting and changes nothing

### Requirement: REQ-CONFIGURATION-006 — A rejected setting fails startup naming the setting, never its value

The system SHALL refuse to start when a setting a layer names fails validation. The refusal SHALL name the setting, or
the variable that supplied it, and SHALL NOT carry the value. The server SHALL NOT run with a partially applied
configuration.

#### Scenario: SCN-CONFIGURATION-006-A — A rejected value names the setting

- **WHEN** a layer gives a setting a value the setting does not accept
- **THEN** startup fails naming the setting

#### Scenario: SCN-CONFIGURATION-006-B — A malformed variable names the variable, not its value

- **WHEN** an environment variable names a setting but holds a value that setting does not accept
- **THEN** startup fails naming the variable and not its value

#### Scenario: SCN-CONFIGURATION-006-C — The refusal carries no value

- **WHEN** startup fails because of a rejected value
- **THEN** neither the reported message nor the logged output contains that value

### Requirement: REQ-CONFIGURATION-007 — Applied overrides are reported

The system SHALL report, once per start, every setting the environment applied, after the log sink is available. The
report SHALL name the settings and SHALL NOT carry their values.

#### Scenario: SCN-CONFIGURATION-007-A — Overridden settings are reported by path

- **WHEN** the environment overrides one or more settings
- **THEN** startup reports which settings it overrode

#### Scenario: SCN-CONFIGURATION-007-B — The report carries no value

- **WHEN** the environment overrides a setting
- **THEN** the report names the setting and not its value

#### Scenario: SCN-CONFIGURATION-007-C — Nothing is reported when nothing was overridden

- **WHEN** the environment names no setting
- **THEN** startup reports no applied override

### Requirement: REQ-CONFIGURATION-008 — The resolved configuration is immutable for the process

The effective configuration SHALL NOT change while the process runs. A change to the configuration file SHALL take
effect only after a restart, including for a setting the process reads while it starts.

#### Scenario: SCN-CONFIGURATION-008-A — A runtime edit has no effect

- **WHEN** the configuration file changes while the server is running
- **THEN** the effective configuration does not change

#### Scenario: SCN-CONFIGURATION-008-B — A restart applies the change

- **WHEN** the server restarts after a configuration change
- **THEN** the new value is effective
