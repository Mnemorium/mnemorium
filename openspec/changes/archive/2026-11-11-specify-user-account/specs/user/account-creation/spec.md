# Spec Delta

## REMOVED Requirements

### Requirement: User account creation requires an authenticated Admin

**Reason**: The requirement describes who may create an account — a bearer token and the `user.create` permission — not
what creating an account does. Authorization belongs to the authorization capability, which is being specified
separately; the account behaviour moves to the `user` capability.

**Migration**: None for the account contract. The authorization workstream owns the bearer-token requirement and the
`user.create` grant; `design.md` records the handoff.

### Requirement: The caller may grant only the roles it is permitted to grant

**Reason**: The role-grant matrix is an authorization rule. The account contract only states that a role is `ADMIN` or
`STANDARD` (`REQ-USER-002`).

**Migration**: None for the account contract. The role-grant matrix is handed to the authorization workstream and listed
in `design.md`.

### Requirement: Usernames are lowercase, bounded, and unique

**Reason**: The rule governs every write to an account's username, creation and update alike, so it moves to the
capability that owns the account contract rather than staying in a capability about creating one.

**Migration**: Restated as `REQ-USER-003` in the `user` capability, with the same validation and `409`/`422` outcomes.

### Requirement: Email is optional, validated, and unique

**Reason**: The rule governs every write to an account's email, creation and update alike, so it moves to the capability
that owns the account contract.

**Migration**: Restated as `REQ-USER-004` in the `user` capability, with the same validation and `409`/`422` outcomes.

### Requirement: A rejected creation applies nothing

**Reason**: Atomic creation is part of the account contract, not a creation-only concern about who may call it.

**Migration**: Restated as `REQ-USER-007` in the `user` capability, extended to cover a failure while storing the
account or its credential.

### Requirement: Creation requests are validated before use

**Reason**: The request shape and its validation belong to the account contract.

**Migration**: Restated as `REQ-USER-006` in the `user` capability, with the same `400` and `422` outcomes.

### Requirement: A created account is returned with its location

**Reason**: The creation response is part of the account contract.

**Migration**: Restated as `REQ-USER-008` in the `user` capability, with the same `201` and `Location` outcomes.
