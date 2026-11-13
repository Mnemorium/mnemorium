# Spec Delta

## REMOVED Requirements

### Requirement: Configuration resolves in layers

**Reason**: The capability it belonged to is retired in favour of `configuration`, which states the same resolution
order together with the totality rule it was paired with.

**Migration**: Replaced by `REQ-CONFIGURATION-001`.

### Requirement: Every setting has a default

**Reason**: Totality and precedence are one contract: a complete configuration is what the three layers resolve, not a
separate promise about defaults.

**Migration**: Replaced by `REQ-CONFIGURATION-001`.

### Requirement: A default configuration file is written when absent

**Reason**: The generation rule now sits with the file's fixed location and its sparse document, which the retired
capability never stated.

**Migration**: Replaced by `REQ-CONFIGURATION-003`; the sparse document and the fixed location are stated by
`REQ-CONFIGURATION-002`.

### Requirement: The environment overrides the file for the settings it names

**Reason**: The environment's naming rule, its discard rule and the values it cannot carry are one contract, and the
refusal surface it shares with the file belongs with them rather than in a loading requirement.

**Migration**: Replaced by `REQ-CONFIGURATION-005`; the refusal contract is `REQ-CONFIGURATION-006` and the applied
override report is `REQ-CONFIGURATION-007`.

### Requirement: Resolved configuration is immutable for the process

**Reason**: The rule survives unchanged in substance and moves with the rest of the mechanism into the single
configuration capability.

**Migration**: Replaced by `REQ-CONFIGURATION-008`.
