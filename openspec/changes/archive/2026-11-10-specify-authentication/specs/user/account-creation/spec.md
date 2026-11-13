# Spec Delta

## REMOVED Requirements

### Requirement: Passwords follow the password policy

**Reason**: The general password policy is owned by the `authentication` capability (`REQ-AUTHENTICATION-018`), which
applies it wherever a password is set, creation included. Keeping a second copy here duplicated the numeric rule and
named the credentials capability as an authority it does not own.

**Migration**: None. Creation still rejects a password that fails the policy with `422 Unprocessable Content`, and
`REQ-AUTHENTICATION-006` still guarantees the password is stored only as a hash and never returned. No client-visible
behavior changes.
