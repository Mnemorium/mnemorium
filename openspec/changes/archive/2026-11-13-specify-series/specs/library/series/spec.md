# Spec Delta

## REMOVED Requirements

### Requirement: Series reads require an authenticated account

**Reason**: Authentication and authorization own who may read; the `series` capability names the action only.

**Migration**: Read `REQ-SERIES-001` and `REQ-SERIES-002`; the bearer requirement stays with `authentication`, and the
decision with `authorization`.

### Requirement: List series

**Reason**: The show collection is owned by the `series` capability.

**Migration**: Read `REQ-SERIES-001` and `REQ-SERIES-002`; the route, filters, pagination, partial items and order are
unchanged.

### Requirement: Series listing is filterable and paginated

**Reason**: The collection's filter and page rules are part of the `series` listing contract.

**Migration**: Read `REQ-SERIES-001`; the `name`, `limit` and `offset` behaviour is unchanged.

### Requirement: Read one series

**Reason**: The show representation is owned by `series`, which references the cover and video contracts.

**Migration**: Read `REQ-SERIES-002`; the route and representation fields are unchanged.

### Requirement: Season and episode identity

**Reason**: The parent-scoped identity belongs to the `series` hierarchy.

**Migration**: Read `REQ-SERIES-003`; the index-as-`id` rule and the 32-bit range are unchanged.

### Requirement: Read one season

**Reason**: The season representation is owned by the `series` hierarchy.

**Migration**: Read `REQ-SERIES-004`; the route, scope and representation fields are unchanged.

### Requirement: List the episodes of a season

**Reason**: The episode collection is owned by the `series` hierarchy.

**Migration**: Read `REQ-SERIES-005`; the route, fields and video summary reference are unchanged.

### Requirement: Read one episode

**Reason**: The episode representation is owned by the `series` hierarchy.

**Migration**: Read `REQ-SERIES-006`; the route, scope and representation fields are unchanged.

### Requirement: Seasons and episodes are ordered by position

**Reason**: The stable order is part of the `series` hierarchy contract.

**Migration**: Read `REQ-SERIES-007`; the ascending index order is unchanged.

### Requirement: A show is created

**Reason**: The show write is owned by `series`; the obsolete `library.manage` wording is replaced by the action.

**Migration**: Read `REQ-SERIES-008`; the request, insert-only creation, genre storage and statuses are unchanged.

### Requirement: Show metadata is updated partially

**Reason**: The partial update is owned by `series`; the obsolete `library.manage` wording is replaced by the action.

**Migration**: Read `REQ-SERIES-009`; the absent/null semantics and child immutability are unchanged.

### Requirement: A season is created or replaced

**Reason**: The season write is owned by `series`; the obsolete `library.manage` wording is replaced by the action.

**Migration**: Read `REQ-SERIES-010`; the `PUT` create-or-replace semantics and episode preservation are unchanged.

### Requirement: Season metadata is updated partially

**Reason**: The partial season update is owned by `series`; the obsolete `library.manage` wording is replaced by the
action.

**Migration**: Read `REQ-SERIES-011`; the absent/null semantics and episode immutability are unchanged.

### Requirement: An episode is created or replaced

**Reason**: The episode write is owned by `series`; the obsolete `library.manage` wording is replaced by the action.

**Migration**: Read `REQ-SERIES-012`; the `PUT` create-or-replace semantics, video replacement and statuses are
unchanged.

### Requirement: Episode metadata is updated partially

**Reason**: The partial episode update is owned by `series`; the obsolete `library.manage` wording is replaced by the
action.

**Migration**: Read `REQ-SERIES-013`; the absent/null semantics and video immutability are unchanged.
