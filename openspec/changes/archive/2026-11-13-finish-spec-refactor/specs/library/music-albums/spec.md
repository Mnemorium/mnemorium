# Spec Delta

## REMOVED Requirements

### Requirement: Album reads require an authenticated account

**Reason**: Authentication and authorization own who may read; `music` names the action only.

**Migration**: Read `REQ-MUSIC-001` and `REQ-MUSIC-002`; the bearer requirement stays with `authentication`, and the
decision with `authorization`.

### Requirement: List music albums

**Reason**: The album collection is owned by `music`.

**Migration**: Read `REQ-MUSIC-001`; the filters, pagination and partial items are unchanged.

### Requirement: Album listing is filterable and paginated

**Reason**: The filter and page rules are part of the `music` listing contract, and the collection conventions are
`api`'s.

**Migration**: Read `REQ-MUSIC-001` and `REQ-API-006`; the `name`, `limit` and `offset` behaviour is unchanged.

### Requirement: Read one music album

**Reason**: The album representation is owned by `music`.

**Migration**: Read `REQ-MUSIC-002`; the representation fields are unchanged.

### Requirement: Medium types are CD or DIGITAL

**Reason**: The closed set of medium types is owned by `music`.

**Migration**: Read `REQ-MUSIC-003`; `CD` and `DIGITAL` are unchanged.

### Requirement: Read a music medium

**Reason**: The medium representation is owned by `music`.

**Migration**: Read `REQ-MUSIC-004`; the representation fields are unchanged.

### Requirement: Read the tracks of an album

**Reason**: The track collection of a medium is owned by `music`.

**Migration**: Read `REQ-MUSIC-005`; the collection representation is unchanged.

### Requirement: Read one track of an album

**Reason**: The track representation is owned by `music`.

**Migration**: Read `REQ-MUSIC-006`; the representation fields are unchanged.

### Requirement: Tracks are ordered by position

**Reason**: The ascending position order is part of the `music` track contract.

**Migration**: Read `REQ-MUSIC-007`; the ordering is unchanged.

### Requirement: Album track identity

**Reason**: Addressing a track by its position within the medium is owned by `music`.

**Migration**: Read `REQ-MUSIC-007`; the composite identity is unchanged.

### Requirement: Album creation is an upsert by name and group

**Reason**: The album write is owned by `music`; the obsolete `library.manage` wording is replaced by the action.

**Migration**: Read `REQ-MUSIC-008`; the `(name, group)` identity and the statuses are unchanged.

### Requirement: Album metadata is updated partially

**Reason**: The partial album update is owned by `music`, which states it once the omission is closed.

**Migration**: Read `REQ-MUSIC-029`; the fields, the absent and null semantics and the conflict outcome are unchanged.

### Requirement: A medium is created or retyped

**Reason**: The medium write is owned by `music`.

**Migration**: Read `REQ-MUSIC-009`; the idempotent create-or-retype semantics are unchanged.

### Requirement: A recording is placed or moved by destination

**Reason**: Placement of a recording at a destination is owned by `music`.

**Migration**: Read `REQ-MUSIC-010`; the destination addressing and the statuses are unchanged.

### Requirement: Recording metadata is edited through the track path

**Reason**: Editing a recording through its track position is owned by `music`.

**Migration**: Read `REQ-MUSIC-011`; the partial update semantics are unchanged.
