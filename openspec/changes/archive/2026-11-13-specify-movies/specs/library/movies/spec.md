# Spec Delta

## REMOVED Requirements

### Requirement: Movie reads require an authenticated account

**Reason**: Authentication and authorization own who may read; the `movies` capability names the action only.

**Migration**: Read `REQ-MOVIES-001` and `REQ-MOVIES-003`; the bearer requirement stays with `authentication`, and the
decision with `authorization`.

### Requirement: List movies

**Reason**: The film collection is owned by the `movies` capability.

**Migration**: Read `REQ-MOVIES-001` and `REQ-MOVIES-002`; the route, filters, pagination, partial items and order are
unchanged.

### Requirement: Movie listing is filterable and paginated

**Reason**: The collection's filter and page rules are part of the `movies` listing contract.

**Migration**: Read `REQ-MOVIES-001`; the `name`, `limit` and `offset` behaviour is unchanged.

### Requirement: Read one movie

**Reason**: The film representation is owned by `movies`, which references the video and cover contracts.

**Migration**: Read `REQ-MOVIES-003`; the route and representation fields are unchanged.

### Requirement: Movies are ordered by identifier

**Reason**: The stable order is part of the `movies` listing contract.

**Migration**: Read `REQ-MOVIES-002`; the ascending identifier order is unchanged.

### Requirement: A film is created from a video

**Reason**: The film write is owned by `movies`; the obsolete `library.manage` wording is replaced by the action.

**Migration**: Read `REQ-MOVIES-004`; the request, upsert by video, genre storage and statuses are unchanged.

### Requirement: Film metadata is updated partially

**Reason**: The partial update is owned by `movies`; the obsolete `library.manage` wording is replaced by the action.

**Migration**: Read `REQ-MOVIES-005`; the absent/null semantics and video immutability are unchanged.
