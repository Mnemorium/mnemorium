# Spec Delta

## REMOVED Requirements

### Requirement: Identification outcomes are observable without exposing content

**Reason**: The requirement states which catalogued events an identification records and what they must not carry, which
is the logging capability's subject.

**Migration**: `REQ-LOGGING-016` declares `metadata_provider_fallback`, `metadata_genre_unmatched`,
`metadata_artwork_failed` and `metadata_provider_unconfigured` with their declared fields, and states that an
identification event carries no query text, title, file name or API key. The identification pipeline stays with this
capability.
