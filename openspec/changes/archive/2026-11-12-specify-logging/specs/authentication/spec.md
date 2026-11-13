# Spec Delta

## REMOVED Requirements

### Requirement: REQ-AUTHENTICATION-017 — Authentication events are recorded

**Reason**: The logging capability now owns every catalogued event — its trigger, its severity and its fields — so this
capability states the authentication outcomes and no longer restates how they are recorded.

**Migration**: `REQ-LOGGING-012` declares `authn_succeeded`, `authn_failed`, `session_validation_failed`,
`rate_limit_exceeded`, `token_regenerated` and `credential_rotated` with the same triggers; `REQ-LOGGING-015` declares
`system_object` for the Root Admin creation. The behaviour observed is unchanged and its witnesses move to the logging
scenarios.
