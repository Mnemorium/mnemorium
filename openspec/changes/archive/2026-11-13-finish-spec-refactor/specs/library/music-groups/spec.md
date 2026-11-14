# Spec Delta

## REMOVED Requirements

### Requirement: Group reads require an authenticated account

**Reason**: Authentication and authorization own who may read; `music` names the action only.

**Migration**: Read `REQ-MUSIC-012` and `REQ-MUSIC-013`; the bearer requirement stays with `authentication`, and the
decision with `authorization`.

### Requirement: List and read music groups

**Reason**: Both the collection and the representation are owned by `music`.

**Migration**: Read `REQ-MUSIC-012` and `REQ-MUSIC-013`; the collection and representation fields are unchanged.

### Requirement: Only an administrator creates or renames a group

**Reason**: `music` names the action each write requires, and `authorization` owns who holds it.

**Migration**: Read `REQ-MUSIC-014` and `REQ-MUSIC-015`; the actions and the statuses are unchanged.

### Requirement: An album names its group

**Reason**: The album's group link is owned by `music`.

**Migration**: Read `REQ-MUSIC-017` and `REQ-MUSIC-002`; the link is unchanged.

### Requirement: The default group is protected

**Reason**: Protection of the default album and group is owned by `music`.

**Migration**: Read `REQ-MUSIC-016`; the refusal is unchanged.
