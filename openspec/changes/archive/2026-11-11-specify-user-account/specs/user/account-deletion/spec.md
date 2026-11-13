# Spec Delta

## REMOVED Requirements

### Requirement: Deleting a user account requires the user.delete permission

**Reason**: The requirement describes who may delete an account — a bearer token and the `user.delete` permission — not
what deleting an account does. Authorization belongs to the authorization capability, which is being specified
separately.

**Migration**: None for the account contract. The authorization workstream owns the bearer-token requirement, the
`user.delete` grant and the `403` outcome; `design.md` records the handoff.

### Requirement: Only a higher authority may delete an Admin account

**Reason**: The target rule is a relation guard between caller and target, not a property of the deleted account.
Authorization belongs to the authorization capability.

**Migration**: None for the account contract. The relation guard and the Root Admin's own-account refusal are handed to
the authorization workstream and listed in `design.md`.

### Requirement: A successful deletion answers 204 and removes the identity atomically

**Reason**: The deletion outcome and its atomicity belong to the account contract.

**Migration**: Restated as `REQ-USER-015` in the `user` capability, with the same `204` and `404` outcomes.

### Requirement: Deletion removes the user's galleries, playlists, and upload sessions

**Reason**: The owned-data effect is part of deleting an account.

**Migration**: Restated as `REQ-USER-016` in the `user` capability, without change.

### Requirement: Media private to the deleted user is removed with its bytes

**Reason**: The private-media disposition is part of deleting an account.

**Migration**: Restated as `REQ-USER-017` in the `user` capability, without change.

### Requirement: Media shared with another user or the catalogue survives under the Root Admin

**Reason**: The shared-media disposition is part of deleting an account.

**Migration**: Restated as `REQ-USER-018` in the `user` capability, naming the account with identifier `0` as the
transfer target.

### Requirement: A transferred file whose digest the Root Admin already owns is stored without it

**Reason**: The digest collision rule is part of the shared-media disposition.

**Migration**: Restated as `REQ-USER-019` in the `user` capability, without change.

### Requirement: Deletion ends the account's authority

**Reason**: Removing the account and its credential is stated by the account contract; refusing a token that names the
deleted account is an identity outcome, not an account property.

**Migration**: Account and credential removal is restated as `REQ-USER-015`. The post-deletion `401` is not absorbed by
`identity/authorization` (see its `specify-authorization` proposal) and does not belong to the account contract, so it
remains implemented and is asserted by the UC-005 journey; `design.md` records the decision.
