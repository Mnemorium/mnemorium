# Tasks

Every scenario is proved black box first, in `test/system/`. A Rust test follows only for an outcome the container
cannot observe — an injected transaction failure, a stored row, a sampling distribution, a response-timing mechanism —
and only that Rust test carries `// proves: SCN-…` when it is the scenario's sole proof. A Rust test that merely
supports a scenario the system suite already proves carries no `proves` line. The system suite declares its scenarios
with `@scenario("SCN-…", TestType.…)`. Each implementation task names the file it adds or changes.

## 1. Foundation: dependencies, configuration, the grant and the derivation

- [x] 1.1 Add `hmac = "0.13.0"` and `fast_qr = "0.14.0"` to `Cargo.toml`, regenerate `Cargo.lock`, and record both in
      `docs/development/DependencyManifest.md` § 1.1. Verify `cargo check`, `cargo tree -d` and `cargo deny check` are
      clean.
- [x] 1.2 Add the failing configuration case to `test/system/configuration.py`: a `config.yaml` naming
      `security.log_root_admin_password` fails startup naming the key, while `security.root_admin_setup.reveal` and
      `.ttl_seconds` resolve with defaults `true` and `1800`. Verify: the case fails on the current configuration model.
- [x] 1.3 Replace the setting in `src/lib/domain/model/configuration.rs`,
      `src/lib/domain/model/partial_configuration.rs`, `src/lib/infrastructure/outbound/envy/configuration.rs`,
      `src/lib/infrastructure/outbound/file_system/default_config.yaml`, `test/performance/server/config.yaml`, and the
      overrides in `test/system/authentication.py` and `test/system/configuration.py`; bound `ttl_seconds` to 1–86400.
      Verify 1.2 passes and the whole tree compiles.
- [x] 1.4 Add the failing domain-service unit tests in `src/lib/domain/service/setup_token.rs`: the token is 8 Crockford
      base32 characters displayed as `XXXX-XXXX`; normalization folds case, the dash and `I`/`L`/`O`; the same nonce and
      **Site pepper** derive the same token, a different nonce derives a different one, and a different pepper derives a
      different one. `// proves: SCN-AUTHENTICATION-002-A` (the distribution half). The **Setup grant** model's
      invariants (short nonce, past expiry) are exercised by the application tests in 2.4 and 7.3, not by a model test
      (`TEST-019`). Verify: each test fails before the service exists.
- [x] 1.5 Add `src/lib/domain/service/setup_token.rs` (`SetupTokenDeriver`, stateless and pure) and
      `src/lib/domain/model/setup_grant.rs`, declaring them in `src/lib/domain/service.rs` and
      `src/lib/domain/model.rs`, with the HMAC derivation behind a domain-separation prefix and a constant-time
      comparison. Verify 1.4 passes.
- [x] 1.6 Add the failing adapter test in `src/lib/infrastructure/outbound/sqlx/setup_grant_repository.rs`: create,
      replace, read, and a consume that affects zero rows for an absent, expired or already-consumed grant; assert the
      stored row holds a nonce, an expiry and a consumption instant and no token value.
      `// proves: SCN-AUTHENTICATION-019-A`. Verify: it fails before the migration and adapter exist.
- [x] 1.7 Add `migrations/20261126090000_create_root_admin_setup.up.sql` and its `.down.sql`,
      `src/lib/infrastructure/outbound/sqlx/model/setup_grant.rs`, and
      `src/lib/infrastructure/outbound/sqlx/setup_grant_repository.rs`; change
      `src/lib/infrastructure/outbound/sqlx.rs`, `src/lib/infrastructure/outbound/sqlx/model.rs`,
      `src/lib/infrastructure/outbound/sqlx/unit_of_work.rs` and `src/lib/lib.rs` (`TestUnitOfWork`); add
      `src/lib/domain/port/setup_grant_repository.rs` and change `src/lib/domain/port.rs` and
      `src/lib/domain/port/identity_unit_of_work.rs`. Verify 1.6 passes and `cargo test` is green.
- [x] 1.8 Change `docs/development/TechnicalDesign.md` § 10.1 to register `security.root_admin_setup.reveal` as
      startup-only and `security.root_admin_setup.ttl_seconds` as live. Verify: the table names both paths and the
      Markdown gate passes.

## 2. `REQ-AUTHENTICATION-001` — The first start creates the Root Admin atomically

- [x] 2.1 Change `test/util/harness.py` and `test/system/conftest.py`: add a `root_setup()` that reads the token with a
      new regex, posts it to `/api/v1/identity/setup` with `SECRET_PASSWORD`, and caches the chosen password; make
      `root_password()` return the cached value; replace `MSG_PASSWORD_LOGGED`/`MSG_PASSWORD_HIDDEN`. Verify: the suite
      collects, and the smoke case fails only because the endpoint does not exist yet.
- [x] 2.2 Add the failing system test in `test/system/authentication.py` for `SCN-AUTHENTICATION-001-A`, replacing
      `test_first_start_creates_the_root_admin`: a fresh start prints a token; before setup a login as `root` fails;
      after setup `GET /api/v1/user/me` reports identifier `0`, `root`, `ADMIN` and no email. Verify: it fails until the
      endpoint lands.
- [x] 2.3 Add the failing system test in `test/system/authentication.py` for `SCN-AUTHENTICATION-001-B`, replacing
      `test_restart_does_not_recreate_or_reveal_the_root_admin`: restart the initialized directory and assert the server
      is healthy, prints no token, creates no second account, and the chosen password still authenticates. Verify: it
      fails if the bootstrap re-reveals or re-creates.
- [x] 2.4 Add the container-unobservable Rust tests in `src/lib/application/use_case/initialize_root_admin.rs` and
      `src/lib/infrastructure/outbound/sqlx/unit_of_work.rs` for `SCN-AUTHENTICATION-001-C`, `-D` and `-E`: the three
      mock cases for a failed credential insert, a failed user insert and a failed grant insert, plus the transaction
      test; add the **Setup grant** invariant case here too (a mocked short nonce fails construction), because a domain
      model has no dedicated test (`TEST-019`). `// proves: SCN-AUTHENTICATION-001-C`, `-D`, `-E`. Verify: no user,
      credential or grant row persists on any failure.
- [x] 2.5 Change `src/lib/application/use_case/initialize_root_admin.rs`,
      `src/lib/application/port/initialize_root_admin.rs` and `src/lib/app.rs`: discard the generated secret, write the
      grant in the same transaction and return the three outcomes, replacing `default_password` with the setup token.
      Verify 2.2–2.4 pass once the endpoint lands.

## 3. `REQ-AUTHENTICATION-002` — The setup token follows the declared shape

- [x] 3.1 Add the failing system test in `test/system/authentication.py` for `SCN-AUTHENTICATION-002-A`, replacing
      `test_generated_password_follows_the_declared_shape`: read the printed token and assert it is 8 characters from
      `0`-`9`/`A`-`Z` without `I`, `L`, `O`, `U`, displayed `XXXX-XXXX`. Verify: it fails on the current 20-character
      password.
- [x] 3.2 Add the failing system test in `test/system/authentication.py` for `SCN-AUTHENTICATION-002-B`: submit the
      issued token lowercased, without its dash, and with `O`/`I` substitutions, and assert the setup succeeds. Verify:
      it fails until normalization lands.
- [x] 3.3 Add the sampling unit test in `src/lib/domain/service/setup_token.rs` because one start yields one sample:
      derive from many nonces and assert the shape on every one. `// proves: SCN-AUTHENTICATION-002-A` (the distribution
      half). Verify: `cargo test setup_token` passes and the sampling helper keeps the test body loop-free
      (`STY-RUST-029`).

## 4. `REQ-AUTHENTICATION-003` — The one-time setup token is never withheld

- [x] 4.1 Add the failing system test in `test/system/authentication.py` for `SCN-AUTHENTICATION-003-A` and
      `SCN-LOGGING-008-E`, replacing `test_the_password_is_printed_once_and_never_logged`: the token text and a
      non-empty QR block appear once on standard output, and no file under `logs/` holds it. Verify: it fails on the
      current password reveal.
- [x] 4.2 Add the failing system test in `test/system/authentication.py` for `SCN-AUTHENTICATION-003-B` and `-D`: a
      fresh start with `security.root_admin_setup.reveal: false` fails naming the setting; a pending instance started
      that way also fails and is unchanged; a restart with the reveal enabled issues a working token. Verify: it fails
      on the current guard.
- [x] 4.3 Add the failing system test in `test/system/authentication.py` for `SCN-AUTHENTICATION-003-C`, replacing
      `test_disabling_the_reveal_after_initialization_still_starts`: a completed instance started with the reveal
      disabled is healthy and reveals nothing. Verify: it reuses the restart harness and asserts no token line.
- [x] 4.4 Add `src/lib/infrastructure/outbound/qr.rs` and `src/lib/infrastructure/outbound/qr/renderer.rs`, declaring
      the module in `src/lib/infrastructure/outbound.rs`, with a unit test that the rendered block is non-empty for a
      token and stable for a fixed token. Verify: the renderer has one call site in `app.rs`.
- [x] 4.5 Change `src/lib/app.rs` to print the token and the QR from `fast_qr`, map the reveal refusal to a startup
      failure naming `security.root_admin_setup.reveal`, and keep `#[expect(clippy::print_stdout)]` with the
      one-time-secret reason. Verify 4.1–4.3 pass.

## 5. `REQ-AUTHENTICATION-019` — The setup token is derived and never stored

- [x] 5.1 Extend the adapter test in `src/lib/infrastructure/outbound/sqlx/setup_grant_repository.rs` to read the
      persisted row after a first start and assert it holds no token column or value, only the nonce, expiry and
      consumption instant. `// proves: SCN-AUTHENTICATION-019-A`. Verify: the test fails if a token is added to the row.

## 6. `REQ-AUTHENTICATION-020` — The setup token is single-use, expiring and rate-limited

- [x] 6.1 Add the failing system tests in `test/system/authentication.py` for `SCN-AUTHENTICATION-020-A`–`-D`: a second
      submission answers 401 with the first response's same body; `ttl_seconds: 1` plus a wait answers 401; an unknown
      token answers the same 401; a restart makes the old token 401 and the new one work. Verify: they fail until the
      endpoint lands.
- [x] 6.2 Add the failing system test in `test/system/authentication.py` for `SCN-AUTHENTICATION-020-E`: exceed the
      configured per-address burst and then the process-wide budget and assert `429 Too Many Requests` with
      `Retry-After`. Verify: it fails without the limiter.
- [x] 6.3 Add the failing system test in `test/system/authentication.py` for `SCN-AUTHENTICATION-020-F`: present the
      setup token as a bearer token on a protected endpoint and assert `401 Unauthorized`. Verify: it fails if the token
      is accepted anywhere else.
- [x] 6.4 Add the mechanism tests the container cannot observe: the constant-time comparison in
      `src/lib/domain/service/setup_token.rs` and the atomic consume race (two concurrent submissions, exactly one
      succeeds) in `src/lib/infrastructure/outbound/sqlx/setup_grant_repository.rs`. Verify: removing the conditional
      update makes the race test fail. These support 6.1 and carry no separate `proves` line.
- [x] 6.5 Add `src/lib/application/port/setup_root_admin.rs` and `src/lib/application/use_case/setup_root_admin.rs`,
      declaring them in `src/lib/application/port.rs` and `src/lib/application/use_case.rs`; add `SetupAttemptLimit` to
      `src/lib/infrastructure/rate_limit.rs`, mirroring `LoginAttemptLimit`, with the constant-time token comparison and
      the uniform `InvalidToken` error. Verify 6.1 and 6.4 pass.
- [x] 6.6 Change `src/lib/infrastructure/inbound/rest/handler/identity.rs` (after 7.5 registers the route) to apply the
      per-address limiter to the setup route and emit `rate_limit_exceeded` with route `identity_setup`; verify 6.2
      passes for both keys.

## 7. `REQ-AUTHENTICATION-021` — Setup replaces the Root Admin credential and optional email atomically

- [x] 7.1 Add the failing system tests in `test/system/authentication.py` for `SCN-AUTHENTICATION-021-A` and `-B`: the
      response is `204 No Content` with an empty body, the chosen password authenticates, and the email is stored or
      absent as submitted. Verify: they fail until the handler lands.
- [x] 7.2 Add the failing system tests in `test/system/authentication.py` for `SCN-AUTHENTICATION-021-C` and `-D`: a
      weak password answers `422` and a taken email answers `409`, and a valid submission with the same token then
      succeeds. Verify: they fail until the handler lands.
- [x] 7.3 Add the container-unobservable Rust test in `src/lib/application/use_case/setup_root_admin.rs` for
      `SCN-AUTHENTICATION-021-E`: inject a failed credential write and assert the email and the grant are unchanged.
      `// proves: SCN-AUTHENTICATION-021-E`. Verify: the test fails if the consume moves outside the transaction.
- [x] 7.4 Finish `src/lib/application/use_case/setup_root_admin.rs` and `src/lib/application/port/setup_root_admin.rs`:
      validate with `PasswordPolicy`, check the email case-insensitively through the user repository, hash with the
      existing `PasswordHasher`, rotate credential `0`, write the email, and consume the grant last inside one unit of
      work. Verify 7.1–7.3 pass once the handler lands.
- [x] 7.5 Add `src/lib/infrastructure/inbound/rest/handler/identity/post_setup.rs` with `#[utoipa::path]`
      (`security(())`, request body, `204`, `401`, `409`, `422`, `429`); change
      `src/lib/infrastructure/inbound/rest/handler/identity.rs`, `src/lib/infrastructure/use_case_factory/identity.rs`
      and `src/lib/app.rs` to register the route and wire the use case (with `SetupAttemptLimit`); redact the token and
      password in the command's `Debug` in `src/lib/application/port/setup_root_admin.rs`; map the errors to the
      declared statuses. Verify 2.2, 2.3, 3.1, 3.2, 4.1–4.3, 6.1–6.3 and 7.1–7.2 pass.
- [x] 7.6 Change `src/lib/application/use_case/setup_root_admin.rs` to emit `credential_rotated` (actor `0`) on success
      and `authn_failed` with one reason on a refused token; add the matching cases to `test/system/logging_events.py`
      and verify exactly one event per outcome with no token in either.

## 8. `REQ-API-015` and `REQ-API-025` — The index and the closed public surface

- [x] 8.1 Add the failing system test in `test/system/api.py` for `SCN-API-015-B` and `SCN-API-025-A`/`-B`: the index
      reaches setup, the document's operations with an empty `security` requirement are exactly `GET /health`,
      `GET /api/v1`, login and setup, and none of the public responses carries token, key or password material. Verify:
      it fails until the link and the operation are registered.
- [x] 8.2 Change `src/lib/infrastructure/inbound/rest/handler.rs` to add the `setup` link to `ApiIndexLinks` and
      `get_api_index`, with a short description; verify 8.1 passes.
- [x] 8.3 Change `test/api/scenarios.toml` and `test/api/checks.py` if the manifest enumerates public operations, so the
      new operation is exercised and its 204/401/409/422/429 responses are checked. Verify: `script/system.sh` reports
      no contract deviation for the operation.

## 9. `REQ-USER-021` and `REQ-AUTHORIZATION-017` — The root's own email

- [x] 9.1 Add the failing system tests in `test/system/user.py` for `SCN-USER-021-E` and `-I`: `PATCH /api/v1/user/0`
      with an email answers `200 OK` and `GET /me` reads it back; username, role, another caller's email change and
      deletion answer `403` and apply nothing. Verify: they fail on the blanket guard.
- [x] 9.2 Add the supporting unit tests in `src/lib/application/use_case/update_user.rs`: the field-aware accept/refuse
      matrix and the email uniqueness path. They carry no separate `proves` line; 9.1 owns the scenarios.
- [x] 9.3 Change `src/lib/application/use_case/update_user.rs` to replace the `user.id() == 0` guard with the
      field-aware rule (caller `0`, no role, no username) and keep the email uniqueness path. Verify 9.1 and 9.2 pass.
- [x] 9.4 Confirm the delete path in `src/lib/application/use_case/delete_user.rs` still refuses account `0`, and that
      the delta in `openspec/changes/add-root-admin-setup-token/specs/authorization/spec.md` removes the immutable
      sentence; verify the authorization suite stays green.

## 10. Documentation and the `SEC-001` removal

- [x] 10.1 Change `docs/development/TechnicalDesign.md`: delete the `SEC-001` row from § 8 and the "Secret-bearing
      structs" prose in § 1, keeping the definition where other rules cite it; move the "a raw secret in a request body
      is an input" note to Code Style Guidelines § 16. Verify: no dangling `SEC-001` reference remains and the Markdown
      gate passes.
- [x] 10.2 Change `docs/development/UseCases.md` UC-003 for the setup token, the reissue on restart and the setup
      operation that sets the password and optional email. Verify: the entry carries no default-password language and
      the Markdown gate passes.
- [x] 10.3 Verify the two new terms in `docs/development/Glossary.md` and the broadened **Site pepper** definition match
      the implementation, and that `proposal`, `design`, `tasks` and each delta carry their glossary reference. Verify:
      the Markdown gate passes.

## 11. Integration gates

- [x] 11.1 Run `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings` and
      `cargo clippy --all-targets -- -D warnings`; verify all are clean.
- [x] 11.2 Run `cargo test` and the coverage gate
      (`cargo llvm-cov --lib --ignore-filename-regex '…' --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80`);
      verify the new domain, use case and adapter code is covered.
- [x] 11.3 Run `script/rule_enforcer.sh check` and verify every registered rule passes.
- [x] 11.4 Run `script/system.sh` (image built with `test-seed` and `api-docs`) and verify the authentication, api, user
      and logging modules pass, including every scenario this change adds.
- [x] 11.5 Run `script/investigation.sh` and verify every new `SCN-*` identifier appears as proven in
      `docs/development/artifacts/RequirementsTraceabilityCoverage.html`.

## Workflow follow-up

- Archive the change with `openspec archive add-root-admin-setup-token` after review, then verify the archived specs.
