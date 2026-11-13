# Proposal

## Why

Authentication is implemented end to end — the Root Admin bootstrap, a password login that issues a bearer token, and
per-request token validation — but no capability spec describes it, and a caller cannot extend a session without logging
in again. The contract is inferred from code, the use-case catalog and the logging catalog, so reviewers cannot tell
intended behavior from accident and drift goes unnoticed. Capturing the contract surfaced four defects and one policy
gap — login is throttled only per client address, so a distributed attempt on one account is not slowed. All must be
corrected for the contract to be defensible.

## What Changes

- Add a new `authentication` capability spec covering the Root Admin bootstrap, login, bearer-token validation and
  credential rotation.
- Harden the behavior the spec would otherwise have to bless:
  - Make `credential.patch` non-configurable and Root-Admin-only. **BREAKING**: deployments that granted the action in a
    permission override will have that override rejected, because a grant currently lets the holder rotate any
    credential, including the Root Admin's.
  - Reject a token at its `exp` instead of accepting it for jsonwebtoken's default 60-second clock-skew leeway.
  - Refuse to create the Root Admin when `security.log_root_admin_password` is false, instead of creating an account
    whose generated password is never revealed. **BREAKING** for a first start configured that way.
  - Send `WWW-Authenticate: Bearer` on every `401` response.
  - Limit login per account, keyed by the resolved user identifier, as well as per client address, so attempts against
    one account are throttled across addresses.
- Add an authenticated token-regeneration endpoint (`POST /api/v1/identity/token`) that re-issues the caller's access
  token with a fresh expiry, leaves the previous token valid until its own expiry, and emits a `token_regenerated`
  security event.
- Move the general password policy into `authentication` and remove it from `user/account-creation`, which no longer
  states or references it.
- Establish a black-box `test/system/` suite, one module per feature area, and prove this capability's scenarios in
  `test/system/authentication.py` before any lower-level test of the same outcome.
- Record, without changing, the deliberate stateless-token contract: there is no logout or revocation, and changing a
  credential does not invalidate tokens already issued.
- Leave account deletion and the deleted-caller rule out of scope; that work is tracked separately and may already be
  under way.

## Capabilities

### New Capabilities

- `authentication`: bootstrapping the Root Admin credential, exchanging a username and password for a bearer token,
  validating that token on every protected request, regenerating a token to extend a session, and rotating a credential.

### Modified Capabilities

- `user/account-creation`: its password-policy requirement is removed; `authentication` owns the general rule and
  applies it to creation.

## Impact

- **Specs**: a new `authentication` capability is created, and `user/account-creation` gets a delta that removes its
  password-policy requirement.
- **Tests**: a new `test/system/` black-box suite, one module per feature area, starts with
  `test/system/authentication.py`; `pytest.ini` and a `script/system.sh` gain it, and the CI `system` job runs it after
  the configuration suite. Rust tests remain only where the container cannot observe the outcome.
- **Code**: `src/lib/domain/model/permission.rs` (catalog configurability),
  `src/lib/infrastructure/outbound/jwt/token_provider.rs` (expiry leeway),
  `src/lib/application/use_case/initialize_root_admin.rs` and `src/lib/app.rs` (bootstrap guard),
  `src/lib/infrastructure/inbound/rest/api_error.rs` (401 challenge),
  `src/lib/infrastructure/inbound/rest/middleware/rate_limit.rs` (account-keyed limiter),
  `src/lib/application/use_case/login_user.rs` and the new limiter port (account check),
  `src/lib/infrastructure/use_case_factory/identity.rs` and `app.rs` (build and inject),
  `src/lib/infrastructure/inbound/rest/handler/identity/post_login.rs` (429 mapping and shared token response),
  `src/lib/application/use_case/regenerate_token.rs` and
  `src/lib/infrastructure/inbound/rest/handler/identity/post_token.rs` (new endpoint).
- **API**: `POST /api/v1/identity/token` is added; `401` responses gain a `WWW-Authenticate` header; replacing
  permissions with an override that names `credential.patch` now answers `422 Unprocessable Content`.
- **Tests**: the new `test/system/authentication.py` proves the capability's scenarios black box; the E2E and
  configuration cases it replaces are deleted with it, declared response conformance stays with Schemathesis, and Rust
  tests cover only the injected failures and mechanisms the container cannot observe.
- **Docs**: `docs/Permissions.md` records `credential.patch` as non-configurable; `docs/development/TechnicalDesign.md`
  § 5 gains the system-suite section, § 9.6 the `token_regenerated` event and § 10.1 the two login limiters; the
  bootstrap guard is noted where the startup settings are described; UC-002 in `docs/development/UseCases.md` gains the
  session-extension step.
