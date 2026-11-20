# Spec Delta

## REMOVED Requirements

### Requirement: REQ-AUDIO-004 — An audio addresses its file

**Reason**: The only scenario proves the `303 See Other` and its `Location`, which the operation declares; the API fuzz
suite validates that declared contract from the served document, so a black-box case restates it.

**Migration**: No client change. The alias route stays served with the same `303` and `Location`, and the API fuzz suite
covers it.
