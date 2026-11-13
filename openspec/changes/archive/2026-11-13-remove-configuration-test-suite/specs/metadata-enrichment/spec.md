# Spec Delta

## ADDED Requirements

### Requirement: REQ-METADATA-ENRICHMENT-029 — A provider list is accepted without a startup request

The system SHALL accept a valid provider list at startup without contacting a provider, and SHALL ask a provider only
when a read needs it.

#### Scenario: SCN-METADATA-ENRICHMENT-029-A — A configured list starts without a request

- **WHEN** the configuration lists valid video providers
- **THEN** startup proceeds
- **AND** no provider is contacted until a read needs one

## MODIFIED Requirements

### Requirement: REQ-METADATA-ENRICHMENT-016 — A provider credential never leaves the process

The system SHALL NOT write a configured API key to a log, an error message, a metric label or a request address it
emits. A provider that carries the key in a query string SHALL be sanitized before any failure is reported.

#### Scenario: SCN-METADATA-ENRICHMENT-016-A — A failed request

- **WHEN** a provider request fails
- **THEN** the reported failure contains no key

#### Scenario: SCN-METADATA-ENRICHMENT-016-B — A refused startup

- **WHEN** startup fails because of the provider configuration
- **THEN** no key value is written to any sink

#### Scenario: SCN-METADATA-ENRICHMENT-016-C — An artwork failure

- **WHEN** an artwork download fails
- **THEN** the reported failure contains no key and no address carrying one

#### Scenario: SCN-METADATA-ENRICHMENT-016-D — A clean start writes no key

- **WHEN** the server starts with a configured key and emits no failure
- **THEN** neither the standard output nor the log files contain the key
