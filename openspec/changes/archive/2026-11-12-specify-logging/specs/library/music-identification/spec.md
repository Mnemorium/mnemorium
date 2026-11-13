# Spec Delta

## REMOVED Requirements

### Requirement: Identification events carry no content

**Reason**: The requirement states what a catalogued event records and what it must not, which is the logging
capability's subject, not this capability's.

**Migration**: `REQ-LOGGING-016` declares `metadata_provider_fallback`, `metadata_provider_unconfigured`,
`metadata_genre_unmatched` and `metadata_artwork_failed` with the provider and classification fields, and states that an
identification event carries no query text, title, artist name or file name. Provider selection and retry stay with this
capability.
