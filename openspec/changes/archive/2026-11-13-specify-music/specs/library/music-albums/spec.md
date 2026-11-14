# Spec Delta

## REMOVED Requirements

### Requirement: Album reads require an authenticated account

**Reason**: The capability is retired in favour of `music`, which names the `View`, `Discover` and `CreateChild` action
each album operation decides against instead of the deleted `library.read` permission. The token requirement is
`authentication`'s, not a music rule.

**Migration**: The token requirement is owned by `REQ-AUTHENTICATION-009`; the read outcomes are replaced by
`REQ-MUSIC-001`, `REQ-MUSIC-002`, `REQ-MUSIC-004`, `REQ-MUSIC-005` and `REQ-MUSIC-006`.

### Requirement: List music albums

**Reason**: The listing is one of the operations the new `music` capability owns, stated with the **Partial
representation** `api` now requires.

**Migration**: Replaced by `REQ-MUSIC-001`.

### Requirement: Album listing is filterable and paginated

**Reason**: Filter and pagination are part of the album listing rather than a separate behaviour, and the page model
belongs to `api`.

**Migration**: Replaced by `REQ-MUSIC-001`, whose scenario carries the `name` filter case.

### Requirement: Read one music album

**Reason**: The capability is retired in favour of `music`.

**Migration**: Replaced by `REQ-MUSIC-002`.

### Requirement: Medium types are CD or DIGITAL

**Reason**: The medium type is part of the medium contract the new capability states once.

**Migration**: Replaced by `REQ-MUSIC-003`.

### Requirement: Read a music medium

**Reason**: The capability is retired in favour of `music`.

**Migration**: Replaced by `REQ-MUSIC-004`.

### Requirement: Read the tracks of an album

**Reason**: The capability is retired in favour of `music`.

**Migration**: Replaced by `REQ-MUSIC-005`.

### Requirement: Read one track of an album

**Reason**: The capability is retired in favour of `music`.

**Migration**: Replaced by `REQ-MUSIC-006`.

### Requirement: Tracks are ordered by position

**Reason**: Ordering is a property of each read it applies to, not a requirement of its own.

**Migration**: Replaced by the ordering clauses of `REQ-MUSIC-002`, `REQ-MUSIC-005` and `REQ-MUSIC-023`.

### Requirement: Album track identity

**Reason**: The capability is retired in favour of `music`, which also states the single medium placement the schema
enforces and drops the impossible two-track scenario.

**Migration**: Replaced by `REQ-MUSIC-007` and `REQ-MUSIC-010`.

### Requirement: Album creation is an upsert by name and group

**Reason**: The capability is retired in favour of `music`, and the write names the action `authorization` requires
rather than the deleted `library.manage` permission.

**Migration**: Replaced by `REQ-MUSIC-008`.

### Requirement: Album metadata is updated partially

**Reason**: The album metadata write is withheld from the `music` capability: this change states album creation and the
medium and track writes, but not a partial update of album metadata.

**Migration**: No requirement replaces it. Until another capability owns the album metadata write, it has no
specification.

### Requirement: A medium is created or retyped

**Reason**: The capability is retired in favour of `music`.

**Migration**: Replaced by `REQ-MUSIC-009`.

### Requirement: A recording is placed or moved by destination

**Reason**: The capability is retired in favour of `music`.

**Migration**: Replaced by `REQ-MUSIC-010`.

### Requirement: Recording metadata is edited through the track path

**Reason**: The capability is retired in favour of `music`.

**Migration**: Replaced by `REQ-MUSIC-011`.

### Requirement: External metadata is applied to an album

**Reason**: An album apply is the **Metadata enrichment** pipeline's output, owned by `metadata-enrichment`; the album
capability keeps only what it uniquely owns.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-024` and `REQ-METADATA-ENRICHMENT-028`, which
`specify-metadata-enrichment` adds. When that change lands first, this removal is a no-op.
