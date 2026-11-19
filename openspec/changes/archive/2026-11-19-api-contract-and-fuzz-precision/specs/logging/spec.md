# Spec Delta

## MODIFIED Requirements

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
| `authn_failed`                   | `warn`   | `reason`                                                                           | a credential is rejected; one reason, never distinguishing a bad identity from a bad secret |
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

Events with no surface in this server are deliberately absent: import/export, network and TLS failures, payment,
geolocation and consent, fraud, excessive use, and key rotation.

### Requirement: REQ-LOGGING-012 — Every authentication outcome is recorded

Every authentication outcome SHALL record exactly one catalogued event: success, rejection, token rejection, rate
limiting, token regeneration, credential rotation and token-provider fault, each at its declared severity with its
declared fields.

#### Scenario: SCN-LOGGING-012-A — A success names the actor

- **WHEN** a credential is accepted
- **THEN** exactly one `authn_succeeded` event names the actor

#### Scenario: SCN-LOGGING-012-B — A rejection is uniform

- **WHEN** a wrong password or an identifier that resolves to no account is rejected
- **THEN** exactly one `authn_failed` event records the same reason for both

#### Scenario: SCN-LOGGING-012-C — A token rejection names its reason

- **WHEN** a request is rejected for a missing, invalid or expired token
- **THEN** exactly one `session_validation_failed` event records the reason

#### Scenario: SCN-LOGGING-012-D — A throttled request names its route

- **WHEN** a rate limiter rejects a request
- **THEN** exactly one `rate_limit_exceeded` event records the route and the reason

#### Scenario: SCN-LOGGING-012-E — Regeneration and rotation name the actor

- **WHEN** a caller regenerates its token or the Root Admin replaces a credential
- **THEN** exactly one `token_regenerated` or `credential_rotated` event names the actor

## REMOVED Requirements

### Requirement: REQ-LOGGING-011 — A claimed identity cannot forge a record

**Reason**: Its entire subject is the one catalogued field that carries a client-supplied identity. `authn_failed` no
longer carries `claimed_identity`, so there is no client-supplied identity in any event to escape or bound.

**Migration**: No client change. A rejected login still records `authn_failed` with `reason = invalid_credentials`; the
per-account correlation the field offered is recorded as a follow-up issue instead.
