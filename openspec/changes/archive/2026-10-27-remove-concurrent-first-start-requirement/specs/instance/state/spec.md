# Spec Delta

## REMOVED Requirements

### Requirement: Concurrent first starts create at most one identity

**Reason**: Not a product requirement. A second server pointed at one working directory is an operator error, and the
store's exclusive create still refuses to replace a persisted identity, so the safety property survives without the
promise. The only test for the requirement is a two-container race whose failures cannot be attributed between the
product and the harness.

**Migration**: None. No API, data, configuration or operator-visible behaviour changes, and no public surface ever
promised this.
