# Spec Delta

## REMOVED Requirements

### Requirement: A media name yields ordered search queries

**Reason**: The capability is retired in favour of `metadata-enrichment`, which states the media-name stage beside the
music query as part of one enrichment contract.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-009`.

### Requirement: A query never fabricates an episode position

**Reason**: The rule survives unchanged in substance and moves with the rest of the video contract.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-010`.

### Requirement: Queries are ordered by supporting evidence

**Reason**: The rule survives unchanged in substance and moves with the rest of the video contract.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-011`.

### Requirement: A candidate is provider-independent

**Reason**: The candidate shape is one rule for both media kinds, and stating it per kind duplicated it.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-002`.

### Requirement: Metadata is provider-independent and artwork is content

**Reason**: The requirement bundled two rules that the merged capability keeps apart: the metadata document's shape and
artwork's treatment, the latter now including what an apply does with it.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-003` for the document shape and `REQ-METADATA-ENRICHMENT-005` for
artwork.

### Requirement: Unreported metadata is absent, never fabricated

**Reason**: Absence is one rule for both media kinds, and stating it per kind duplicated it.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-006`.

### Requirement: Genres come from one canonical vocabulary

**Reason**: The genre rule is one contract with a vocabulary per kind, and stating it per kind obscured which vocabulary
applies to which work.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-007`.

### Requirement: Country of origin is an ISO alpha-2 code

**Reason**: The rule survives unchanged in substance and moves with the rest of the video contract.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-014`.

### Requirement: Metadata is resolved through the reference's provider

**Reason**: Routed resolution is one rule for both media kinds, and stating it per kind duplicated it.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-008`.

### Requirement: Failover follows the configured index order

**Reason**: Failover, the once-per-search rule and the invalid-request rule are one policy for video, and stating them
as three requirements split one decision.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-013`.

### Requirement: A provider is asked at most once per search

**Reason**: The rule is part of the video failover policy, which the merged capability states as one requirement.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-013`.

### Requirement: Failover is not attempted for a request the system formed incorrectly

**Reason**: The rule is part of the video failover policy, which the merged capability states as one requirement.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-013`.

### Requirement: Identification stops at the first query that yields candidates

**Reason**: The rule survives unchanged in substance and moves with the rest of the video contract.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-012`.

### Requirement: Unavailable is not "no match"

**Reason**: The distinction is one rule for both media kinds, and stating it per kind duplicated it.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-001`.

### Requirement: Provider configuration is an ordered list

**Reason**: The provider list is one configuration rule for both kinds, with the key and the kind restriction stated
beside it rather than in two requirements.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-015`.

### Requirement: Invalid provider configuration fails startup

**Reason**: A refused list is the same configuration rule as the list's shape, and the merged capability states both
together.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-015`.

### Requirement: An API key never leaves the process

**Reason**: The rule survives unchanged in substance and moves with the provider configuration it belongs to.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-016`.

### Requirement: Identification outcomes are observable without exposing content

**Reason**: Which catalogued events an identification records, and what they must not carry, is the logging capability's
subject. The in-flight `specify-logging` change already removes this requirement; the removal here keeps the retirement
self-sufficient when that change has not landed yet.

**Migration**: `REQ-LOGGING-016` declares `metadata_provider_fallback`, `metadata_genre_unmatched`,
`metadata_artwork_failed` and `metadata_provider_unconfigured` with their fields, and states that an identification
event carries no query text, title, file name or API key. The identification pipeline stays with `metadata-enrichment`.

### Requirement: Identification reads require library read access

**Reason**: The rule restates a who-may rule that `authorization` owns. The merged capability names the action each
route carries and leaves denial semantics to the capability that decides them.

**Migration**: The action is named by `REQ-METADATA-ENRICHMENT-017` and `REQ-METADATA-ENRICHMENT-023`; the decision and
its concealment stay with `authorization`. The routes, and the statuses each denial renders, do not change.

### Requirement: A media name is identified through a kind-scoped read

**Reason**: The route's subject is the pipeline's first stage, which the merged capability now owns end to end.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-017`. The routes, parameters and statuses do not change.

### Requirement: A reference resolves to a metadata document

**Reason**: The route's subject is the pipeline's resolution stage, which the merged capability now owns end to end.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-017`. The routes, parameters and statuses do not change.

### Requirement: An episode is identified by its show reference and position

**Reason**: The position rule belongs to the resolution route and to the apply stage that repeats it, both of which the
merged capability states.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-017` for resolution and `REQ-METADATA-ENRICHMENT-027` for the apply.
