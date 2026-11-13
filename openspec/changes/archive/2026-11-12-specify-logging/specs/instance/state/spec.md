# Spec Delta

## REMOVED Requirements

### Requirement: Identity creation is reported and secrets never leak

**Reason**: Both subjects of this requirement are logging obligations — the events that report creation and the
redaction that keeps the secrets out of a sink — and the logging capability now owns them.

**Migration**: `REQ-LOGGING-015` declares `system_object` and `secret_initialized` for identity creation and states that
reusing a persisted identity records neither; `REQ-LOGGING-008` states that the pepper and the signing secret never
reach a sink and that a type holding one redacts its debug output. Identity persistence, privacy and atomic write stay
in this capability.
