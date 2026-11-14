# Spec Delta

## REMOVED Requirements

### Requirement: Series reads require an authenticated account

**Reason**: Authentication and authorization own who may read; `series` names the action only.

**Migration**: Read `REQ-SERIES-001`, `REQ-SERIES-002` and the scenarios under them; the bearer requirement stays with
`authentication`, and the decision with `authorization`.

### Requirement: List series

**Reason**: The show collection is owned by `series`.

**Migration**: Read `REQ-SERIES-001`; the filters, pagination and partial items are unchanged.

### Requirement: Series listing is filterable and paginated

**Reason**: The filter and page rules are part of the `series` listing contract, and the collection conventions are
`api`'s.

**Migration**: Read `REQ-SERIES-001` and `REQ-API-006`; the `name`, `limit` and `offset` behaviour is unchanged.

### Requirement: Read one series

**Reason**: The show representation is owned by `series`.

**Migration**: Read `REQ-SERIES-002`; the representation fields are unchanged.

### Requirement: Season and episode identity

**Reason**: Parent-scoped identity for a season and an episode is owned by `series`.

**Migration**: Read `REQ-SERIES-003`; addressing by index within the parent is unchanged.

### Requirement: Read one season

**Reason**: The season representation is owned by `series`.

**Migration**: Read `REQ-SERIES-004`; the representation fields are unchanged.

### Requirement: List the episodes of a season

**Reason**: The episode collection is owned by `series`.

**Migration**: Read `REQ-SERIES-005`; the collection representation is unchanged.

### Requirement: Read one episode

**Reason**: The episode representation is owned by `series`.

**Migration**: Read `REQ-SERIES-006`; the representation fields are unchanged.

### Requirement: Seasons and episodes are ordered by position

**Reason**: The ascending index order is part of the `series` hierarchy contract.

**Migration**: Read `REQ-SERIES-007`; the ordering is unchanged.

### Requirement: A show is created

**Reason**: The show write is owned by `series`; the obsolete `library.manage` wording is replaced by the action.

**Migration**: Read `REQ-SERIES-008`; the payload and the statuses are unchanged.

### Requirement: Show metadata is updated partially

**Reason**: The partial update is owned by `series`; the obsolete `library.manage` wording is replaced by the action.

**Migration**: Read `REQ-SERIES-009`; the absent and null semantics are unchanged.

### Requirement: A season is created or replaced

**Reason**: The season write is owned by `series`, which states that the check is `CreateChild` on the show.

**Migration**: Read `REQ-SERIES-010`; the `PUT` semantics are unchanged.

### Requirement: Season metadata is updated partially

**Reason**: The partial update is owned by `series`.

**Migration**: Read `REQ-SERIES-011`; the absent and null semantics are unchanged.

### Requirement: An episode is created or replaced

**Reason**: The episode write is owned by `series`, which states that the check is `CreateChild` on the show.

**Migration**: Read `REQ-SERIES-012`; the `PUT` semantics are unchanged.

### Requirement: Episode metadata is updated partially

**Reason**: The partial update is owned by `series`.

**Migration**: Read `REQ-SERIES-013`; the absent and null semantics are unchanged.
