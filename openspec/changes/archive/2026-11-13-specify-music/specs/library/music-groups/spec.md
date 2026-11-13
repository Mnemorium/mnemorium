# Spec Delta

## REMOVED Requirements

### Requirement: Group reads require an authenticated account

**Reason**: The capability is retired in favour of `music`, which names the `View` and `Discover` action each group read
decides against instead of the deleted `library.read` permission. The token requirement is `authentication`'s, not a
music rule.

**Migration**: The token requirement is owned by `REQ-AUTHENTICATION-009`; the read outcomes are replaced by
`REQ-MUSIC-012` and `REQ-MUSIC-013`.

### Requirement: List and read music groups

**Reason**: Both operations are owned by the new `music` capability, with the **Partial representation** `api` requires.

**Migration**: Replaced by `REQ-MUSIC-012` and `REQ-MUSIC-013`.

### Requirement: Only an administrator creates or renames a group

**Reason**: The requirement restated a **Who-may rule** that `authorization` owns. The new capability names the
`CreateChild` and `EditMetadata` action; which principal holds it is stated once, in `authorization`.

**Migration**: Replaced by `REQ-MUSIC-014` and `REQ-MUSIC-015`, with the role outcome proven by
`test/system/authorization.py`.

### Requirement: An album names its group

**Reason**: The album's group is part of the album contract, which the new capability states once.

**Migration**: Replaced by `REQ-MUSIC-017`.

### Requirement: The default group is protected

**Reason**: The seeded album is protected on the same terms, so one requirement covers both default rows.

**Migration**: Replaced by `REQ-MUSIC-016`.
