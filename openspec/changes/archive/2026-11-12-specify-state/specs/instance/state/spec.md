# Spec Delta

## REMOVED Requirements

### Requirement: Identity is resolved before configuration and the datastore

**Reason**: The capability is retired. The state-before-datastore guarantee moves to `REQ-STATE-005` in the new `state`
capability. The ordering against configuration is a startup-design statement in `TechnicalDesign.md` § 10.2, not a state
requirement.

**Migration**: `REQ-STATE-005` states that the state is established or loaded before the datastore is opened and that an
unreadable state stops startup before any datastore file is opened. The configuration ordering remains in § 10.2.

### Requirement: Identity holds only non-derivable secrets

**Reason**: The state model moves to the new `state` capability, where it is stated once with stable identifiers.

**Migration**: `REQ-STATE-001`, `REQ-STATE-002` and `REQ-STATE-003` state the site pepper, the signing secret and the
creation instant as separate requirements. `REQ-STATE-007` states the format version and the refusal of an unknown
field.

### Requirement: Environment seeds apply only at creation

**Reason**: Creation seeding moves to the new `state` capability. The same requirement is restated with the malformed
seed case the server already refuses.

**Migration**: `REQ-STATE-004` states the seed names, generation of 32 random bytes for an absent seed, the persisted
value's authority, the differing-seed refusal and the malformed-seed refusal. `SCN-STATE-004-E` adds the malformed seed
case.

### Requirement: A missing identity for an existing datastore fails startup

**Reason**: Datastore coupling moves to the new `state` capability. The same requirement is restated with the
undecidable-presence case the server already fails closed on.

**Migration**: `REQ-STATE-006` states the missing-state refusal, the fresh-instance order, the state-without-datastore
reuse and the undecidable-presence refusal. `SCN-STATE-006-D` adds the undecidable-presence case.

### Requirement: Identity persistence is atomic, versioned and private

**Reason**: Persistence moves to the new `state` capability and splits into the persistence rule and the privacy rule so
each states one behaviour.

**Migration**: `REQ-STATE-007` states the fixed `state.yaml` path, the atomic write, the format version, the unknown
field, the size cap and the refusal of a version this build does not support. `REQ-STATE-008` states that no account
other than the server account may read or write the file, including a restored file. The privacy scenarios add the
restored-file refusal.
