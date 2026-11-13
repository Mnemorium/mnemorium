# Spec Delta

## REMOVED Requirements

### Requirement: Ingest reports aggregate counts only

**Reason**: The rule's subject is the record an ingest writes, not the ingest's own outcome, and the logging capability
now owns every catalogued event and its fields.

**Migration**: `REQ-LOGGING-016` declares the `ingest` event with the registered, completed, untouched, changed,
missing, removed and skipped counts and states that it carries no file name or path. What each count means stays with
this capability.
