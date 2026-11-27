# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-AUTHORIZATION-017 — The root is implicit owner and never denied

The root SHALL hold the implicit `Owner` role on every resource, SHALL bypass every `Deny`, and SHALL have no stored
grant.

#### Scenario: SCN-AUTHORIZATION-017-A — The root reads any resource

- **WHEN** the root addresses another caller's private resource
- **THEN** the decision allows

#### Scenario: SCN-AUTHORIZATION-017-B — The root bypasses a deny

- **WHEN** a resource carries a `Deny` naming the root
- **THEN** the deny cannot be stored and the root's decision allows
