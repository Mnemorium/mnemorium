# Spec Delta

## REMOVED Requirements

### Requirement: A query names the kind of work and carries a non-empty title

**Reason**: The capability is retired in favour of `metadata-enrichment`, which states the music query beside the video
one as part of one enrichment contract.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-018`.

### Requirement: A candidate is provider-independent

**Reason**: The candidate shape is one rule for both media kinds, and stating it per kind duplicated it.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-002`, whose scenarios carry the music-specific fields.

### Requirement: Candidates stay in the provider's order

**Reason**: Order preservation is one rule for both media kinds, and stating it per kind duplicated it.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-002`.

### Requirement: Album metadata is provider-independent

**Reason**: The three music metadata documents are one contract, stated once rather than per kind of work.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-004`.

### Requirement: Release metadata describes one edition

**Reason**: The three music metadata documents are one contract, stated once rather than per kind of work.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-004`.

### Requirement: Recording metadata stands alone

**Reason**: The three music metadata documents are one contract, stated once rather than per kind of work.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-004`.

### Requirement: Artwork is content

**Reason**: Artwork is one rule for both media kinds, and the apply stage's treatment of it was unstated.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-005`, which adds the scenario that an apply stores no artwork.

### Requirement: Unreported metadata is absent, never fabricated

**Reason**: Absence is one rule for both media kinds, and stating it per kind duplicated it.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-006`.

### Requirement: Production type comes from one discrete list

**Reason**: The rule survives unchanged in substance and moves with the rest of the music contract.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-019`.

### Requirement: Genres come from the provider's curated vocabulary

**Reason**: The genre rule is one contract with a vocabulary per kind, and stating it per kind obscured which vocabulary
applies to which work.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-007`.

### Requirement: Compact-disc editions come first

**Reason**: The rule survives unchanged in substance and moves with the rest of the music contract.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-020`.

### Requirement: Metadata is resolved through the reference's provider

**Reason**: Routed resolution is one rule for both media kinds, and stating it per kind duplicated it.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-008`.

### Requirement: Unavailable is not "no match"

**Reason**: The distinction is one rule for both media kinds, and stating it per kind duplicated it.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-001`.

### Requirement: A query is escaped for the provider's search syntax

**Reason**: The rule survives unchanged in substance and moves with the rest of the music contract.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-021`.

### Requirement: A provider's request limit is respected

**Reason**: The rule survives unchanged in substance and moves with the rest of the music contract, where its asymmetry
against the video once-per-search rule is stated rather than left implicit.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-022`.

### Requirement: Identification events carry no content

**Reason**: What a catalogued event records and what it must not carry is the logging capability's subject, not this
one's. The in-flight `specify-logging` change already removes this requirement; the removal here keeps the retirement
self-sufficient when that change has not landed yet.

**Migration**: `REQ-LOGGING-016` declares `metadata_provider_fallback`, `metadata_provider_unconfigured`,
`metadata_genre_unmatched` and `metadata_artwork_failed` with the provider and classification fields, and states that an
identification event carries no query text, title, artist name or file name. Provider selection, pacing and retry stay
with `metadata-enrichment`.

### Requirement: Music identification is served over HTTP

**Reason**: The routes are one surface with the video ones, and the capability that owns the pipeline now owns them.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-023`. The routes, payloads and statuses do not change.
