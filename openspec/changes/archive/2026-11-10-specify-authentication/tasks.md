# Tasks

One section per requirement. Task prefixes match the requirement number, so `8.2` belongs to `REQ-AUTHENTICATION-008`,
and there are no sections 11 or 16.

A spec scenario is proven black box first: its primary test lives in `test/system/authentication.py` (section 0). A Rust
test follows only where the container cannot observe the outcome — a failure injected into a port, a redaction
mechanism, a sampling distribution, or response timing — and the task says which. A declared status, content type, body
shape or response header is checked by Schemathesis on every operation it reaches, so no task asserts it again. A case
the system suite replaces in `test/e2e/` or `test/configuration/` is deleted in the same section, so no scenario is
proven twice. Two scenarios that state the same outcome share one test, which carries one `story` label per scenario.

`Label` means Allure labels. A system test carries `@allure.epic(...)`, `@allure.feature(...)` and `@allure.story(...)`;
a Rust test carries `#[allure_test]` above its `#[test]`/`#[tokio::test]` and calls `allure.epic(...)`,
`allure.feature(...)` and `allure.story(...)` in the body. A test proving two scenarios calls `allure.story` once per
scenario.

## 0. The `test/system/` suite

- [x] 0.1 Add the suite to `pytest.ini`: `test/system` in `testpaths`. Add `test/system/conftest.py` collecting every
      `*.py` in that folder, because the global `python_files` patterns do not match a single-word feature area such as
      `authentication.py`. Reuse the `harness` plugin (`Sut`, `sut`, `hex64`), which `addopts = -p harness` already
      loads. Verify: `pytest test/system --collect-only` lists the module.
- [x] 0.2 Create `test/system/authentication.py` with a module docstring naming the feature area and a helper that
      starts a container with `MNEMORIUM_JWT_SECRET` and `MNEMORIUM_PEPPER` fixed, so a test can mint its own tokens and
      restart deterministically. Verify: one smoke test starts a container, logs in as `root`, and passes.
- [x] 0.3 Add `script/system.sh`, mirroring `script/configuration.sh` (`MNEMORIUM_IMAGE`, its own container prefix, the
      cleanup trap), and call it from the `system` job in `.github/workflows/ci.yml` after the configuration suite.
      Verify: `NO_BUILD=1 script/system.sh` runs the suite against the image.
- [x] 0.4 Document the suite in `docs/development/TechnicalDesign.md` § 5: black box, one module per feature area, the
      black-box-first rule, and the shared Allure label convention — `epic` = capability, `feature` = requirement ID,
      `story` = scenario ID, pytest decorators for system tests, `#[allure_test]` plus facade calls for Rust tests, and
      `#[allure_test]` as the outermost attribute. Verify: the section states the rules and the Markdown gates pass.
- [x] 0.5 Update the test taxonomy for the removed suites: `TEST-053` no longer names `security_settings.py` and says
      the `security.*` effects are proven by the system suite; the § E2E test text and `UseCases.md` UC-002 record that
      a use case whose whole behavior is a spec scenario is proven by `test/system/`, not an E2E file. Verify: the rules
      describe the tree that remains and the Markdown gates pass.

## 1. `REQ-AUTHENTICATION-001` — The first start creates the Root Admin atomically

- [x] 1.1 Write the system test for `SCN-AUTHENTICATION-001-A`: start a container in an empty working directory, read
      the revealed password, and assert `GET /api/v1/user/me` reports identifier `0`, username `root`, role `ADMIN` and
      no email. Label `epic=Authentication`, `feature=REQ-AUTHENTICATION-001`, `story=SCN-AUTHENTICATION-001-A`. Verify:
      it passes against the current server and is the scenario's proof.
- [x] 1.2 Write the system test for `SCN-AUTHENTICATION-001-B`: restart the same working directory and assert the server
      is healthy, reveals no password, and the first password still authenticates. Label as above with
      `story=SCN-AUTHENTICATION-001-B`. Verify: it fails if the bootstrap creates a second credential or re-reveals.
- [x] 1.3 Keep the four mock cases in `initialize_root_admin.rs` for the two container-unreachable failures
      (`SCN-AUTHENTICATION-001-C`, `-D`), and add the outbound-adapter transaction test next to `unit_of_work.rs`: fail
      the second insert and assert neither the user row nor the credential row persists, and that a successful start
      leaves exactly one credential for user `0`. Label `epic=Authentication`, `feature=REQ-AUTHENTICATION-001`, `story`
      `-C` and `-D`. Verify: the test fails when the inserts move outside one unit of work.

## 2. `REQ-AUTHENTICATION-002` — The generated password follows the declared shape

- [x] 2.1 Write the system test for `SCN-AUTHENTICATION-002-A`: start one container, read the revealed password and
      assert it is 20 characters, 19 alphanumeric and one symbol from `!@#$%^&*()-_=+`. Label `epic=Authentication`,
      `feature=REQ-AUTHENTICATION-002`, `story=SCN-AUTHENTICATION-002-A`. Verify: it passes against the current
      generator.
- [x] 2.2 Add the supporting unit test in `password_generator.rs`: repeated samples each match the shape, because one
      container start yields one sample and cannot show the distribution. Annotate it `#[allure_test]` above
      `#[tokio::test]` and label it `allure.epic("Authentication")`, `allure.feature("REQ-AUTHENTICATION-002")`,
      `allure.story("SCN-AUTHENTICATION-002-A")`. Keep the sampling loop in a helper because `STY-RUST-029` forbids
      loops in a test body. Verify: `cargo test password_generator` passes and the Allure result carries the three
      labels.

## 3. `REQ-AUTHENTICATION-003` — The one-time password is never withheld

- [x] 3.1 Write the failing system test for `SCN-AUTHENTICATION-003-B`: a fresh working directory with
      `security.log_root_admin_password: false` fails through `start_expecting_failure`, the output names the setting,
      and a restart with the reveal enabled prints a password — proving the failed start created no account. Label
      `epic=Authentication`, `feature=REQ-AUTHENTICATION-003`, `story=SCN-AUTHENTICATION-003-B`. Verify: it fails on the
      current guard-less startup.
- [x] 3.2 Write the system test for `SCN-AUTHENTICATION-003-A`: with the flag true, the password is printed once to
      standard output, authenticates, and appears in no file under `logs/`. Label as above with `story` `-A`. Verify: it
      fails if the password reaches a log sink.
- [x] 3.3 Write the system test for `SCN-AUTHENTICATION-003-C`: restart the initialized directory with the flag false
      and assert the server is healthy, prints the hidden notice, and reveals nothing. Label as above with `story` `-C`.
- [x] 3.4 Delete `test_the_default_password_is_printed_when_enabled` and
      `test_the_default_password_is_not_printed_when_disabled` from `test/configuration/security_settings.py`; 3.2 and
      3.3 replace them. Verify: neither case remains and the configuration suite still collects.
- [x] 3.5 Extend the `InitializeRootAdmin` input with the reveal decision and add the guard after the existence check
      and before the password is generated, returning the dedicated error and rolling back. Verify: 3.1 passes and the
      existing bootstrap tests stay green.
- [x] 3.6 Wire the reveal decision in `app.rs` and map the new error to a startup failure that names
      `security.log_root_admin_password`. Verify: 3.1 passes end to end.
- [x] 3.7 Register `security.log_root_admin_password` in the startup-only settings registry
      (`docs/development/TechnicalDesign.md` § 10.1) with "a new value applies after a restart". Verify: the setting
      appears in the table and the section's rule is satisfied.

## 4. `REQ-AUTHENTICATION-004` — Valid credentials issue a bearer token

- [x] 4.1 Write the system test for the part of `SCN-AUTHENTICATION-004-A` the schema cannot express: the login response
      carries exactly `access_token`, `expires_in` and `token_type` and no fourth field. The API suite already checks
      the declared `200`, `application/json` and the field types. Label `epic=Authentication`,
      `feature=REQ-AUTHENTICATION-004`, `story=SCN-AUTHENTICATION-004-A`. Verify: it passes and asserts the exact field
      set.
- [x] 4.2 Write the system test for `SCN-AUTHENTICATION-004-B`: present the returned token on `GET /api/v1/user/me` and
      assert it resolves that user — identifier, username, role and the self link. Label `epic=Authentication`,
      `feature=REQ-AUTHENTICATION-004`, `story=SCN-AUTHENTICATION-004-B`.
- [x] 4.3 Write the system test for `SCN-AUTHENTICATION-004-C`: the login response carries no password or password hash.
      Label as above with `story=SCN-AUTHENTICATION-004-C`.
- [x] 4.4 Delete `test_uc002_user_authentication_happy_path` from `test/e2e/identity/uc002_user_authentication.py`;
      4.1–4.3 replace its assertions. Verify: the module still collects with its remaining two cases.

## 5. `REQ-AUTHENTICATION-005` — Bad credentials are rejected uniformly

- [x] 5.1 Write the system test for `SCN-AUTHENTICATION-005-A`: a wrong password and an unknown username answer the same
      status, the same error body, and the same `WWW-Authenticate: Bearer` header — the one `401` header assertion,
      which also covers the login side of 009-A. Label `epic=Authentication`, `feature=REQ-AUTHENTICATION-005`,
      `story=SCN-AUTHENTICATION-005-A`. Verify: it fails if the two branches diverge.
- [x] 5.2 Confirm the unknown-username branch still runs the decoy hash, which response timing cannot prove black box
      (`SCN-AUTHENTICATION-005-A`). Keep the application test and strengthen it if it no longer asserts the hasher call.
      Verify: removing the decoy hash fails it.
- [x] 5.3 Delete `test_uc002_user_authentication_wrong_password` and
      `test_uc002_user_authentication_bad_credentials_are_indistinguishable`, then delete the now-empty
      `test/e2e/identity/uc002_user_authentication.py`; 5.1 replaces both. Verify: `test/e2e/identity/` still collects
      `uc024_manage_user_permissions.py`.

## 6. `REQ-AUTHENTICATION-006` — Password material never leaves the credential

- [x] 6.1 Write the system test for `SCN-AUTHENTICATION-006-B`: complete a login and a rotation, then assert the
      submitted password and the issued token appear in no file under `logs/` and not in the printed output. Label
      `epic=Authentication`, `feature=REQ-AUTHENTICATION-006`, `story=SCN-AUTHENTICATION-006-B`. Verify: it fails if
      either value reaches a sink.
- [x] 6.2 Keep the unit tests for the redaction mechanism: `LoginRequest`, `LoginResponse`, `IssuedToken` and the
      credential render the secret redacted while the rest stays readable. Label as above with `story` `-B`. Verify: a
      test fails if a redaction is removed.
- [x] 6.3 Keep the integration test that only a hash is stored — not observable over HTTP — and extend it to a creation
      and a rotation path. Label as above with `story=SCN-AUTHENTICATION-006-A`. Verify: it fails if the plaintext
      reaches the datastore.

## 7. `REQ-AUTHENTICATION-007` — Login reports the configured token lifetime

- [x] 7.1 Write the system test for `SCN-AUTHENTICATION-007-A`: start with `security.jwt.ttl` set, log in, and assert
      `expires_in` equals it and the token's `exp` is that many seconds after issue. Label `epic=Authentication`,
      `feature=REQ-AUTHENTICATION-007`, `story=SCN-AUTHENTICATION-007-A`. Verify: the case passes.
- [x] 7.2 Delete `test_ttl_sets_the_advertised_lifetime` from `test/configuration/security_settings.py`; 7.1 replaces it
      and the module docstring loses its `jwt.ttl` sentence. Verify: the case no longer exists and the suite still
      collects.

## 8. `REQ-AUTHENTICATION-008` — Login is rate limited per client address and account

- [x] 8.1 Write the failing system test for `SCN-AUTHENTICATION-008-D`: spend one account's quota from distinct
      trusted-proxy addresses and assert `429` with `Retry-After` while each address stays under its own quota. Label
      `epic=Authentication`, `feature=REQ-AUTHENTICATION-008`, `story=SCN-AUTHENTICATION-008-D`. Verify:
      `pytest test/system -k rate` fails on the missing account dimension.
- [x] 8.2 Write the system test for `SCN-AUTHENTICATION-008-E`: an unknown username is throttled identically, and the
      response is indistinguishable from an existing one. Label as above with `story` `-E`.
- [x] 8.3 Write the system tests for `SCN-AUTHENTICATION-008-A`, `-B` and `-C`: a burst is throttled, the quota refills
      after the period, a trusted proxy's declared header keys the bucket, and an untrusted peer's header is ignored.
      Label `epic=Authentication`, `feature=REQ-AUTHENTICATION-008`, and the matching `story` `-A`, `-B`, `-C`. Verify:
      the cases pass.
- [x] 8.4 Delete `test_burst_size_throttles_a_login_burst`, `test_period_controls_the_refill`,
      `test_a_trusted_proxy_keys_on_the_header` and `test_an_untrusted_peer_ignores_the_header` from
      `test/configuration/security_settings.py`; 8.3 replaces them. Verify: none remains.
- [x] 8.5 Write the limiter test in `rate_limit.rs`: one user identifier is throttled after its burst across different
      client addresses, a different identifier keeps its own quota, a username that resolves to no account is throttled
      under a derived key, the quota refills after the period, and pruning drops exhausted keys. Label as above. Verify:
      `cargo test rate_limit` fails for the missing account dimension.
- [x] 8.6 Implement the account-keyed limiter from the existing `security.rate_limit` burst and period with two key
      sources: the resolved user identifier, and a `SHA-256` digest of the submitted username when no account resolves.
      Expose a check that returns the wait time, prune it with the existing cleanup, and remove the dead-code
      expectation on `RateLimitCleanup::merge`. Verify: 8.1 and 8.5 pass.
- [x] 8.7 Add the `LoginAttemptLimiter` outbound port and check it inside `LoginUser` after the user lookup and before
      password verification; return `RateLimited` with the wait time. Map it in `post_login` to `429` with
      `Retry-After`. Inject the process-lifetime limiter through the identity use-case factory. Never log the username.
      Verify: a use-case test covers the rejection and a handler test covers the mapping.
- [x] 8.8 Update the startup-only registry wording in `TechnicalDesign.md` § 10.1 from "the login limiter" to the two
      login limiters it now builds. Verify: the registry describes both keys and the section's rule is satisfied.

## 9. `REQ-AUTHENTICATION-009` — Protected endpoints require a bearer token

`SCN-AUTHENTICATION-009-A` is owned by the API suite: it already calls every guarded operation without a token and
checks the declared `401`, so no system test is written for it. The `WWW-Authenticate` header on a real login `401` is
asserted by 5.1.

- [x] 9.1 Add the header in `ApiError::into_response` for a `401`: the response carries `WWW-Authenticate: Bearer` and
      still produces the HAL envelope. The system test 5.1 asserts it from outside, and 9.3 makes Schemathesis check it
      on every guarded operation. Verify: 5.1 and the existing error tests stay green.
- [x] 9.2 Extend the `hal_errors` header-preservation test to cover `WWW-Authenticate` alongside `Allow`. Verify: the
      rewritten response keeps both headers.
- [x] 9.3 Declare the `WWW-Authenticate` header on the `401` response of every operation guarded by `authenticate` and
      on login. Prefer one shared unauthorized-response declaration over 82 hand edits. Verify: the served OpenAPI
      document lists the header on a sampled protected operation and on `POST /api/v1/identity/login`.
- [x] 9.4 Delete the seventeen per-endpoint `requires_credentials` cases from the eight `test/e2e/library/*` modules
      (`uc012`, `uc022`, `uc023`, `uc025`, `uc026`, `uc027`, `uc029`, `uc030`); 9.3 plus the API suite replace them.
      Leave `test_uc001_create_user_account_unauthenticated`, which proves a `user/account-creation` scenario rather
      than 009-A. Verify: no `requires_credentials` case remains and every module still collects its other tests.

## 10. `REQ-AUTHENTICATION-010` — A token is accepted only while valid

- [x] 10.1 Write the failing system test for `SCN-AUTHENTICATION-010-B` to `-D`: with `MNEMORIUM_JWT_SECRET` fixed, mint
      three HS256 tokens with the standard library — tampered signature, expired, non-numeric subject — and assert
      `401`, `401`, `401`. Label `epic=Authentication`, `feature=REQ-AUTHENTICATION-010`, `story` `-B` to `-D`. Verify:
      the expired token is currently accepted.
- [x] 10.2 Set `validation.leeway = 0` in `JwtTokenProvider::validate` and keep the adapter test that pins it: a
      correctly signed token with `exp = now - 1` returns `TokenExpired`, `exp = now + 30` validates. Verify: 10.1
      passes and the sign-and-validate round-trip stays green.
- [x] 10.3 Delete `test_a_token_expires_after_the_configured_ttl` from `test/configuration/security_settings.py`; 10.1
      replaces it, and the leeway explanation in its docstring goes with it. Verify: the case no longer exists.
- [x] 10.4 Delete the now-empty `test/configuration/security_settings.py`; 3.4, 7.2, 8.4 and 10.3 removed its last case.
      Verify: the module is gone, `MSG_PASSWORD_HIDDEN` is used by the system module's `SCN-AUTHENTICATION-003-C` case,
      and the configuration suite still collects.

## 12. `REQ-AUTHENTICATION-012` — Tokens are stateless

- [x] 12.1 Write the system test for `SCN-AUTHENTICATION-012-A`, shared with `SCN-AUTHENTICATION-013-C`: rotate a
      credential and assert the token issued before it still authenticates until expiry. Label `epic=Authentication`,
      `feature=REQ-AUTHENTICATION-012`, `story=SCN-AUTHENTICATION-012-A`. Verify: the case passes.
- [x] 12.2 Record the absent endpoints in the OpenAPI surface check: neither a logout nor a revocation operation is
      declared. Verify: the served document lists neither, and the pull request states that this is deliberate.

## 13. `REQ-AUTHENTICATION-013` — An authenticated caller regenerates its token

- [x] 13.1 Write the failing system test for `SCN-AUTHENTICATION-013-A` and `-B`: log in, regenerate, and assert the
      response matches login and the new token authenticates as the same user. `SCN-AUTHENTICATION-013-C` is the shared
      test from 12.1. Label `epic=Authentication`, `feature=REQ-AUTHENTICATION-013`, `story` `-A` and `-B`. Verify: it
      fails before the route exists.
- [x] 13.2 Implement the `RegenerateToken` use case and port, reusing `JwtTokenProvider::issue` and the shared token
      response extracted from login, and loading the caller so a deleted account cannot mint a successor. Verify: 13.1
      passes.
- [x] 13.3 Register `POST /api/v1/identity/token` on the protected identity router with `#[utoipa::path]`, the
      `bearer_auth` security declaration and its `200`/`401`/`500` responses. Verify: the served OpenAPI document lists
      the operation and it sits behind `authenticate`.
- [x] 13.4 Document the session-extension step in UC-002 in `docs/development/UseCases.md` and keep the Identity diagram
      accurate. Verify: the entry names regeneration and the capability is reachable from the catalog.

## 14. `REQ-AUTHENTICATION-014` — The Root Admin rotates any credential

- [x] 14.1 Write the failing system test for `SCN-AUTHENTICATION-014-A`, `-B` and `-C`: the Root Admin rotates its own
      credential (`204`, the old password answers `401`, the new one logs in), rotates another user's (`204`, the new
      one authenticates), and names an unknown credential (`404`). Label `epic=Authentication`,
      `feature=REQ-AUTHENTICATION-014`, `story` `-A`, `-B`, `-C`. Verify: it passes against the current rotation route.
- [x] 14.2 Write the failing system test for `SCN-AUTHENTICATION-014-D`: an Admin and a Standard User are refused with
      `403` and nothing is stored. Label as above with `story` `-D`. Verify: it fails on the current grantable behavior.
- [x] 14.3 Write the system test for `SCN-AUTHENTICATION-014-E`: an override naming `credential.patch` answers `422` and
      the defaults representation no longer lists the action. Label as above with `story` `-E`.
- [x] 14.4 Write the failing domain tests in `authorization.rs` and `permission.rs`: a non-root caller is denied
      `CredentialPatch` even with an `ALL` override, the Root Admin is allowed,
      `Action::CredentialPatch.is_configurable()` is false, and `Permission::try_new` returns `NotConfigurable`. Label
      as above with `story` `-D`. Verify: `cargo test` fails on the current unguarded behavior.
- [x] 14.5 Write the failing application test in `patch_credential.rs`: a non-root caller holding a stored
      `credential.patch: ALL` override is still `Forbidden`. Label as above with `story` `-D`.
- [x] 14.6 Implement both layers: exclude `CredentialPatch` from `is_configurable` and deny it to non-root callers in
      `Authorization::allows` before overrides are consulted. Verify: 14.2, 14.4 and 14.5 pass and the authorization and
      permission suites stay green.
- [x] 14.7 Update `docs/Permissions.md`: the `credential.patch` row becomes non-configurable, the Root Admin remains the
      only caller, and the override rules match 14.6. Verify: the guide no longer says the action can be granted.

## 15. `REQ-AUTHENTICATION-015` — Rotation is policy-checked and atomic

- [x] 15.1 Write the system test for `SCN-AUTHENTICATION-015-A`: a rotation with a short password and one with a
      symbol-free password each answer `422`, and the old password still authenticates afterwards. Label
      `epic=Authentication`, `feature=REQ-AUTHENTICATION-015`, `story=SCN-AUTHENTICATION-015-A`. Verify: both cases
      pass.
- [x] 15.2 Confirm `SCN-AUTHENTICATION-015-B` — a failed save leaves the previous hash — at the application level
      (`patch_credential_credential_save_failure_returns_unknown`) and add an adapter-level assertion if the rollback is
      untested. Label as above with `story` `-B`. Verify: the case fails if the update moves outside the transaction.

## 17. `REQ-AUTHENTICATION-017` — Authentication events are recorded

- [x] 17.1 Write the system test for `SCN-AUTHENTICATION-017-A`: one successful login emits exactly one
      `authn_succeeded` naming the caller. Label `epic=Authentication`, `feature=REQ-AUTHENTICATION-017`,
      `story=SCN-AUTHENTICATION-017-A`. Verify: it fails if the event is dropped or emitted twice.
- [x] 17.2 Write the system test for `SCN-AUTHENTICATION-017-B`: a wrong password and an unknown username each emit one
      `authn_failed` with the escaped claimed identity and the same reason. Label as above with `story` `-B`.
- [x] 17.3 Write the system test for `SCN-AUTHENTICATION-017-C`: a missing, an invalid and an expired token each emit
      one `session_validation_failed` naming the reason. Label as above with `story` `-C`.
- [x] 17.4 Write the system test for `SCN-AUTHENTICATION-017-D`: a bootstrap emits one `system_object` and a rotation
      one `credential_rotated`, each with the actor. Label as above with `story` `-D`.
- [x] 17.5 Write the system test for `SCN-AUTHENTICATION-017-E`: a regeneration emits exactly one `token_regenerated`
      with the actor. Label as above with `story` `-E`.
- [x] 17.6 Add `token_regenerated` to the security-event catalog in `docs/development/TechnicalDesign.md` § 9.6 (`info`,
      `actor`). Verify: the catalog lists the event and `script/rule_enforcer.sh check OBS-006` passes.

## 18. `REQ-AUTHENTICATION-018` — Passwords follow the general policy

- [x] 18.1 Write the system test for `SCN-AUTHENTICATION-018-A` to `-C`: a compliant password is accepted at creation
      and at rotation, while a short one and a symbol-free one each answer `422` and store nothing at both entry points.
      Label `epic=Authentication`, `feature=REQ-AUTHENTICATION-018`, `story` `-A`, `-B`, `-C`. Verify: the cases pass.
- [x] 18.2 Keep the domain table test in `src/lib/domain/service/password_policy.rs` as the mechanism proof with the
      same labels. Verify: `cargo test password_policy` passes.
- [x] 18.3 Delete the short-password and symbol-free rows from the invalid-payload table in
      `test/e2e/user/uc001_create_a_user_account.py` (the `carol` and `dave` entries and their comments); 18.1 replaces
      them, and the table keeps its username and email rows. Verify: the case still covers every remaining invalid
      value.
- [x] 18.4 Replace the `user/account-creation` delta at `specs/user/account-creation/spec.md` with a
      `REMOVED Requirements` block for "Passwords follow the password policy". The main spec's Purpose line no longer
      mentions the policy (edited directly, as `openspec archive` ignores a delta Purpose for an existing spec). Verify:
      `openspec validate specify-authentication --strict` passes and the delta path matches the main spec.

## 19. Integration gate

- [x] 19.1 Confirm the delta's identifiers are unique and the change validates. Re-run after every spec edit. Verify:
      `openspec validate specify-authentication --strict` exits 0.
- [x] 19.2 Run `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`,
      `cargo clippy --all-targets -- -D warnings`, and `cargo test`. Verify: all pass.
- [ ] 19.3 Run `script/rule_enforcer.sh check`. Verify: every registered rule passes. Deferred by request: the current
      failures are test-placement, factory-wiring and use-case-naming findings to resolve in a follow-up pass.
- [ ] 19.4 Run `script/configuration.sh`, `script/system.sh` and `script/e2e.sh` against the same image. Verify: all
      three suites pass. Configuration and system pass; E2E deferred by request.
- [x] 19.5 Run the Markdown gates (`markdownlint-cli2 '**/*.md'` and `prettier --check`). Verify: both pass.

## Workflow follow-up

- Archive the change after review; the delta syncs into `openspec/specs/authentication/spec.md` and removes the
  `user/account-creation` password-policy requirement.
- Re-run `openspec validate --strict` after archive and confirm the new main spec's purpose and requirements.
