# logging Specification

## Purpose

Defines what the server records and what it must never record: the two sinks and their rotation, the verbosity that
cannot silence a security event, the layer that owns each record, the content and redaction rules, and the closed
catalog of security events with the trigger, severity and fields of each.

## Requirements

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

### Requirement: REQ-LOGGING-002 — The file sink rotates and prunes

The active file SHALL be replaced at the configured period — minutely, hourly or daily — or never. Rotated files SHALL
be pruned to the configured maximum, and a maximum of zero SHALL keep every file.

#### Scenario: SCN-LOGGING-002-A — Rotation replaces the active file

- **WHEN** the configured period elapses and a further event is emitted
- **THEN** a new active file holds the further event and the previous file is retained

#### Scenario: SCN-LOGGING-002-B — Pruning bounds the retained files

- **WHEN** more files have rotated than the configured maximum allows
- **THEN** only the configured number of files remains

#### Scenario: SCN-LOGGING-002-C — A zero maximum keeps every file

- **WHEN** the configured maximum is zero and files rotate
- **THEN** every rotated file is retained

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

- **WHEN** the verbosity excludes a diagnostics level and the server emits a diagnostics event
- **THEN** that event reaches no sink

### Requirement: REQ-LOGGING-004 — A catalogued event cannot be suppressed

A catalogued event SHALL be emitted whatever the configured verbosity says. The reserved target SHALL carry a floor
equal to the lowest severity the catalog declares, so no configured value can silence a catalogued event.

#### Scenario: SCN-LOGGING-004-A — A success survives a fault-only verbosity

- **WHEN** the verbosity admits only `error` and a login succeeds
- **THEN** the `info` `authn_succeeded` event reaches the sinks

#### Scenario: SCN-LOGGING-004-B — The lowest declared severity survives

- **WHEN** the verbosity admits only `error` and an event the catalog declares at `debug` is emitted
- **THEN** that event reaches the sinks

### Requirement: REQ-LOGGING-005 — Only the outer layers record

Events SHALL be emitted only by the application and infrastructure layers. The domain SHALL return errors as values and
SHALL emit none; the composition root is infrastructure and may record lifecycle events.

#### Scenario: SCN-LOGGING-005-A — A domain rejection is recorded by its caller

- **WHEN** a domain rule rejects a value
- **THEN** exactly one event records the rejection and the domain emits none

#### Scenario: SCN-LOGGING-005-B — No emission site sits in the domain

- **WHEN** the repository's rules are checked
- **THEN** no emission site exists under the domain layer

### Requirement: REQ-LOGGING-006 — An error or security event is recorded exactly once

Each error or security event SHALL be recorded exactly once, by the layer that owns the decision that produced it or
that still holds its detail. Every layer above SHALL map it to a coarser form and SHALL NOT record it again.

#### Scenario: SCN-LOGGING-006-A — A denial is recorded once

- **WHEN** an authorization decision denies an action
- **THEN** exactly one event records the denial

#### Scenario: SCN-LOGGING-006-B — A dependency fault is recorded once

- **WHEN** an outbound adapter cannot reach its dependency
- **THEN** exactly one event records the fault

#### Scenario: SCN-LOGGING-006-C — A failed login is recorded once

- **WHEN** a login is rejected
- **THEN** exactly one event records the rejection

### Requirement: REQ-LOGGING-007 — An event carries a classification, never raw input

An event SHALL carry a stable classification — an event code or a `kind()` — and structured identifiers. It SHALL NOT
carry the rendered message of a framework, extractor, deserializer, database or filesystem error, and SHALL NOT carry a
raw client-supplied query string, file name or request-body value.

#### Scenario: SCN-LOGGING-007-A — A query string is withheld

- **WHEN** a request whose query string carries a value is traced
- **THEN** the event carries the path and not the query string

#### Scenario: SCN-LOGGING-007-B — A file name is withheld

- **WHEN** a file-storage operation fails for a user-supplied name
- **THEN** the event carries the failure's classification and not the name

#### Scenario: SCN-LOGGING-007-C — A framework error is classified

- **WHEN** a database, filesystem or deserializer error is recorded
- **THEN** the event carries its classification and no rendered error message

### Requirement: REQ-LOGGING-008 — No secret or personal value reaches a sink

No secret or personal value SHALL reach a sink: application source, session identifiers, access tokens, passwords or
password material, database connection strings, the pepper, the signing secret, and personal data. A type that holds one
SHALL NOT render it in its debug representation.

#### Scenario: SCN-LOGGING-008-A — Login secrets stay out

- **WHEN** a password is submitted and a token is issued
- **THEN** neither the password nor the token appears in any sink

#### Scenario: SCN-LOGGING-008-B — Instance secrets stay out

- **WHEN** the instance identity generates its pepper and signing secret
- **THEN** neither value appears in any sink

#### Scenario: SCN-LOGGING-008-C — A secret-bearing type redacts its debug output

- **WHEN** a type holding a secret or personal data is formatted for diagnostics
- **THEN** its debug representation prints a placeholder instead of the value

#### Scenario: SCN-LOGGING-008-D — Personal data stays out

- **WHEN** an event names a user
- **THEN** it carries the user identifier and no username or email address

### Requirement: REQ-LOGGING-009 — Five severities carry declared meanings

An event SHALL use one of five severities: `error` for an operator-actionable fault, `warn` for a reviewable security
event, `info` for a security success or lifecycle event, `debug` for low-volume diagnostics, and `trace` for driver or
framework detail. An expected business rejection SHALL NOT be recorded at `error`.

#### Scenario: SCN-LOGGING-009-A — An expected rejection is not an error

- **WHEN** a use case rejects a request for an expected business reason
- **THEN** the event records it at `warn` or `info`, never at `error`

#### Scenario: SCN-LOGGING-009-B — A dependency fault is an error

- **WHEN** a datastore, filesystem or external service is unavailable
- **THEN** the event records it at `error`

### Requirement: REQ-LOGGING-010 — The catalog is closed and complete

Every emitted event SHALL be one the security-event catalog declares, SHALL target the reserved `security` target, and
SHALL carry the declared severity and exactly the declared fields. The server SHALL emit no undeclared event and SHALL
emit every declared event.

#### Scenario: SCN-LOGGING-010-A — An undeclared event is rejected

- **WHEN** an emission site names an event the catalog does not declare
- **THEN** the repository's rules reject it

#### Scenario: SCN-LOGGING-010-B — A declared event carries its declared shape

- **WHEN** a catalogued event is emitted
- **THEN** it targets `security` at the declared severity with exactly the declared fields

#### Scenario: SCN-LOGGING-010-C — The catalog holds no dormant event

- **WHEN** the catalog is checked against the emission sites
- **THEN** every declared event has at least one emission site

The security-event catalog:

| Event                            | Severity | Fields                                                                             | Recorded when                                                                               |
| -------------------------------- | -------- | ---------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| `input_validation_failed`        | `warn`   | `field`, `reason`                                                                  | a client-supplied value fails validation in a handler or use case                           |
| `enum_validation_failed`         | `warn`   | `field`, `reason`                                                                  | a discrete, finite list (for example `Role`) rejects a value                                |
| `output_validation_failed`       | `warn`   | `reason`                                                                           | stored data fails to decode, or an integrity check fails                                    |
| `authn_succeeded`                | `info`   | `actor`                                                                            | a credential is accepted                                                                    |
| `authn_failed`                   | `warn`   | `claimed_identity`, `reason`                                                       | a credential is rejected; one reason, never distinguishing a bad identity from a bad secret |
| `authz_failed`                   | `warn`   | `actor`, `action`, `target`                                                        | an authorization decision denies an action                                                  |
| `session_validation_failed`      | `warn`   | `reason`                                                                           | a token is missing, invalid or expired                                                      |
| `rate_limit_exceeded`            | `warn`   | `route`, `reason`                                                                  | a request is rejected by a rate limiter                                                     |
| `limit_exceeded`                 | `warn`   | `source`, `reason`                                                                 | a request body or query string exceeds a configured size limit                              |
| `metadata_provider_fallback`     | `warn`   | `provider`, `classification`                                                       | a metadata provider is skipped because of a classified failure                              |
| `metadata_provider_unconfigured` | `warn`   | `provider`                                                                         | a lookup names a metadata provider that is not configured                                   |
| `metadata_genre_unmatched`       | `debug`  | `count`                                                                            | a metadata provider reports a genre outside the shared vocabulary                           |
| `metadata_artwork_failed`        | `warn`   | `provider`, `reason`                                                               | artwork a provider reports cannot be downloaded or accepted                                 |
| `deserialization_failed`         | `warn`   | `source`, `reason`                                                                 | a JSON body or the configuration fails to deserialize                                       |
| `extractor_fault`                | `error`  | `reason`                                                                           | an inbound extractor rejects a request for a server-side fault                              |
| `ingest`                         | `info`   | `registered`, `completed`, `untouched`, `changed`, `missing`, `removed`, `skipped` | a library ingest operation (scan or observed change) finishes; never a file name or path    |
| `ingest_fault`                   | `warn`   | `kind`, `operation`                                                                | the change observer degraded (a failed watch registration, a watcher error or overflow)     |
| `application_error`              | `error`  | `operation`                                                                        | a use case originates an internal `Unknown`                                                 |
| `port_fault`                     | `error`  | `kind`, `operation`                                                                | an outbound adapter maps a dependency failure                                               |
| `constraint_rejected`            | `warn`   | `kind`, `operation`                                                                | an outbound adapter maps an expected constraint to a business result                        |
| `token_provider_fault`           | `error`  | `kind`                                                                             | the inbound token boundary receives an operational fault from the token provider            |
| `lifecycle`                      | `info`   | `component`, `outcome`                                                             | startup, shutdown or logging initialization                                                 |
| `user_admin`                     | `info`   | `actor`, `action`, `target`                                                        | a user is created, updated or deleted, a role or credential changes                         |
| `secret_initialized`             | `info`   | `component`                                                                        | the pepper or the signing secret is generated when the instance identity is created         |
| `credential_rotated`             | `info`   | `actor`                                                                            | a password is replaced or rehashed                                                          |
| `token_regenerated`              | `info`   | `actor`                                                                            | a caller regenerates its access token                                                       |
| `upload`                         | `info`   | `actor`, `upload`, `outcome`                                                       | an upload is accepted, a chunk written, an upload completed or cancelled                    |
| `upload_reap`                    | `info`   | `expired`, `orphaned`, `unsealed`                                                  | an upload reaping run finishes; aggregate counts only, never a file name or path            |
| `suspicious_business_logic`      | `warn`   | `actor`, `action`, `reason`                                                        | an action is out of order, nonsensical in context, or exceeds a limit                       |
| `sensitive_data_access`          | `debug`  | `actor`, `object`                                                                  | a request reads personal data                                                               |
| `system_object`                  | `info`   | `object`, `action`                                                                 | a system-level object is created (the instance identity, the Root Admin)                    |
| `unexpected_http_method`         | `warn`   | `method`, `path`                                                                   | a request uses a method the route does not support                                          |

The `authz_failed` fields are a closed vocabulary shared with the permission catalog: `action` is one of `read`,
`moderate`, `manage`, `use`, `write`, `update`, `update_role`, `create`, `delete` or `patch_credential`; `target` is one
of `library`, `gallery`, `playlist`, `file`, `upload` or `user`.

An explicit, narrow exception to the field rule: `authn_failed.claimed_identity` is the submitted username, because it
is the only identity available when authentication fails. It is escaped and bounded so it cannot forge a record
(`REQ-LOGGING-011`).

Events with no surface in this server are deliberately absent: import/export, network and TLS failures, payment,
geolocation and consent, fraud, excessive use, and key rotation.

### Requirement: REQ-LOGGING-011 — A claimed identity cannot forge a record

The one catalogued field that carries a client-supplied identity SHALL be escaped so control characters cannot forge or
split a record, and SHALL be bounded in length. It SHALL carry the submitted username and no other value.

#### Scenario: SCN-LOGGING-011-A — A control character is escaped

- **WHEN** a rejected login claims an identity carrying a newline
- **THEN** the event carries the newline escaped and the record stays on one line

#### Scenario: SCN-LOGGING-011-B — The identity is bounded

- **WHEN** a rejected login claims an identity longer than the bound
- **THEN** the event carries at most the bound and no further value

### Requirement: REQ-LOGGING-012 — Every authentication outcome is recorded

Every authentication outcome SHALL record exactly one catalogued event: success, rejection, token rejection, rate
limiting, token regeneration, credential rotation and token-provider fault, each at its declared severity with its
declared fields.

#### Scenario: SCN-LOGGING-012-A — A success names the actor

- **WHEN** a credential is accepted
- **THEN** exactly one `authn_succeeded` event names the actor

#### Scenario: SCN-LOGGING-012-B — A rejection is uniform

- **WHEN** a wrong password or an unknown username is rejected
- **THEN** exactly one `authn_failed` event records the claimed identity and the same reason for both

#### Scenario: SCN-LOGGING-012-C — A token rejection names its reason

- **WHEN** a request is rejected for a missing, invalid or expired token
- **THEN** exactly one `session_validation_failed` event records the reason

#### Scenario: SCN-LOGGING-012-D — A throttled request names its route

- **WHEN** a rate limiter rejects a request
- **THEN** exactly one `rate_limit_exceeded` event records the route and the reason

#### Scenario: SCN-LOGGING-012-E — Regeneration and rotation name the actor

- **WHEN** a caller regenerates its token or the Root Admin replaces a credential
- **THEN** exactly one `token_regenerated` or `credential_rotated` event names the actor

### Requirement: REQ-LOGGING-013 — A denied decision is recorded once

A denied authorization decision SHALL record exactly one `authz_failed` event naming the actor, the action and the
target. A listing that drops an item the caller may not discover is not a denial and SHALL record none. An action that
is out of order, nonsensical or over an application limit SHALL record `suspicious_business_logic`.

#### Scenario: SCN-LOGGING-013-A — A denial names actor, action and target

- **WHEN** an authorization decision denies an action
- **THEN** exactly one `authz_failed` event names the actor, the action and the target

#### Scenario: SCN-LOGGING-013-B — Filtering records no denial

- **WHEN** a listing drops an item the caller may not discover
- **THEN** no `authz_failed` event is recorded

#### Scenario: SCN-LOGGING-013-C — An out-of-order action is suspicious

- **WHEN** an action is out of order or exceeds an application limit
- **THEN** exactly one `suspicious_business_logic` event records the actor, the action and the reason

### Requirement: REQ-LOGGING-014 — A fault is classified where it is owned

A port failure SHALL record `port_fault` at `error` with its `kind` and operation, or `constraint_rejected` at `warn`
when the failure is an expected constraint. A use case that originates an internal error SHALL record
`application_error`.

#### Scenario: SCN-LOGGING-014-A — A dependency fault is classified

- **WHEN** an outbound adapter cannot reach its dependency
- **THEN** `port_fault` records the failure's `kind` and operation

#### Scenario: SCN-LOGGING-014-B — An expected constraint is a warning

- **WHEN** a constraint rejects an operation as an expected business outcome
- **THEN** `constraint_rejected` records its `kind` and operation at `warn`

#### Scenario: SCN-LOGGING-014-C — An originated error is recorded by the use case

- **WHEN** a use case originates an internal error no port produced
- **THEN** `application_error` records the operation

### Requirement: REQ-LOGGING-015 — Lifecycle, instance and administration are recorded

Starting, stopping and initializing logging SHALL record `lifecycle`. Creating the instance identity SHALL record
`system_object` and one `secret_initialized` per generated secret, and neither when it reuses one. Creating, updating or
deleting a user, or changing a role or credential, SHALL record `user_admin`.

#### Scenario: SCN-LOGGING-015-A — Startup is a lifecycle event

- **WHEN** the server starts and installs logging
- **THEN** `lifecycle` records the component and the outcome

#### Scenario: SCN-LOGGING-015-B — Identity creation is reported

- **WHEN** the instance identity is created
- **THEN** exactly one `system_object` event is recorded, followed by one `secret_initialized` per generated secret

#### Scenario: SCN-LOGGING-015-C — Reuse reports no creation

- **WHEN** a persisted identity is loaded
- **THEN** neither `system_object` nor `secret_initialized` is recorded

#### Scenario: SCN-LOGGING-015-D — User administration names actor and target

- **WHEN** a user is created, updated or deleted, or a role or credential changes
- **THEN** one `user_admin` event records the actor, the action and the target

### Requirement: REQ-LOGGING-016 — Library, ingest and metadata outcomes are recorded

A finished ingest SHALL record `ingest` with the aggregate counts and no file name or path; a degraded change observer
SHALL record `ingest_fault`. A metadata provider that is skipped, unconfigured, unmatched or fails artwork SHALL record
its catalogued event with the provider and the classification, and no query, title, artist name or file name.

#### Scenario: SCN-LOGGING-016-A — An ingest reports counts only

- **WHEN** a scan finishes or an observed change is handled
- **THEN** one `ingest` event carries the registered, completed, untouched, changed, missing, removed and skipped counts
  and no file name or path

#### Scenario: SCN-LOGGING-016-B — A degraded observer is reported

- **WHEN** a watch registration fails or the watcher reports an error or overflow
- **THEN** `ingest_fault` records its kind and operation

#### Scenario: SCN-LOGGING-016-C — A provider fallback is classified

- **WHEN** a metadata provider is skipped because of a classified failure
- **THEN** `metadata_provider_fallback` records the provider and the classification only

#### Scenario: SCN-LOGGING-016-D — Identification events carry no content

- **WHEN** any identification event is emitted
- **THEN** it carries no query text, title, artist name or file name

### Requirement: REQ-LOGGING-017 — Upload outcomes are recorded

An accepted upload, a written chunk, a completed upload and a cancelled upload SHALL record `upload` with the actor, the
upload and the outcome. A reaping run SHALL record `upload_reap` with the expired, orphaned and unsealed counts and no
file name or path.

#### Scenario: SCN-LOGGING-017-A — An upload outcome is recorded

- **WHEN** an upload is accepted, a chunk is written, or an upload completes or is cancelled
- **THEN** one `upload` event records the actor, the upload and the outcome

#### Scenario: SCN-LOGGING-017-B — A reaping run reports counts only

- **WHEN** a reaping run finishes
- **THEN** one `upload_reap` event carries the three counts and no file name or path

### Requirement: REQ-LOGGING-018 — A personal-data read is recorded

A request that reads personal data SHALL record `sensitive_data_access` at `debug` with the actor and the object read,
and SHALL carry no personal value.

#### Scenario: SCN-LOGGING-018-A — The read is recorded

- **WHEN** a request reads personal data
- **THEN** one `sensitive_data_access` event records the actor and the object

#### Scenario: SCN-LOGGING-018-B — The value stays out

- **WHEN** a personal-data read is recorded
- **THEN** the event carries no username, email address or other personal value
