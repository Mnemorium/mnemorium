# Design

## Context

See `proposal.md` — Why.

Authentication is implemented in four pieces that already work:

- `InitializeRootAdmin` (`src/lib/application/use_case/initialize_root_admin.rs`) creates user `0` / `root` / `Admin`
  and its credential at startup, before the router is built.
- `LoginUser` (`src/lib/application/use_case/login_user.rs`) looks up the user, verifies the password through the
  `PasswordHasher` port, and issues a token through the `TokenProvider` port.
- `JwtTokenProvider` (`src/lib/infrastructure/outbound/jwt/token_provider.rs`) signs HS256 tokens carrying only `sub`
  and `exp`, using the instance identity's signing secret and the configured `security.jwt.ttl`.
- `authenticate` (`src/lib/infrastructure/inbound/rest/middleware/auth.rs`) parses `Authorization: Bearer`, validates
  the token, and attaches `AuthenticatedUser`.

The Root Admin short-circuits authorization (`Action::CredentialPatch.root_access() == RootAccess::All`), and
`PatchCredential` decides the action against `Global` and then loads any credential by path identifier.

The login limiter (`src/lib/infrastructure/inbound/rest/middleware/rate_limit.rs`) keys only on the client address, so
attempts against one account from many addresses are not slowed. The only token issuance is login; no endpoint
regenerates one today, so a caller cannot extend a session without resubmitting credentials.

The project's spec tree keeps `identity/authorization`, `user/account-creation` and `instance/state`. This change adds a
sibling top-level capability, `authentication`, as requested, rather than nesting it under `identity/`. Route names and
Rust modules stay under `identity/*`; the capability name describes the behavior, not the URL.

## Goals / Non-Goals

**Goals:**

- Publish one normative capability spec for Root Admin bootstrap, login, bearer-token validation and credential
  rotation.
- Correct the four defects the spec would otherwise bless: a grantable credential takeover, a 60-second expiry grace, an
  unrecoverable first-start password, and a `401` without a Bearer challenge.
- Add an account dimension to login throttling, keyed by the resolved user identifier.
- Let an authenticated caller extend its stateless session by regenerating its token.
- Own the general password policy in `authentication`, with account creation and rotation referencing it.
- Prove the capability's scenarios black box, in a new `test/system/` suite keyed to the spec.
- Keep the change test-first and small enough to review in one pass.

**Non-Goals:**

- Account deletion, the deleted-caller rule, or moving `identity/authorization`'s requirement for it. That work may
  already be in flight and is tracked separately.
- Logout, refresh tokens, revocation, or a token-version claim.
- Self-service password change; no endpoint exposes it today.
- Renaming `/api/v1/identity/*` routes or `handler/identity/*` modules.

## Decisions

### 1. `credential.patch` is structurally Root-Admin-only

**Decision:** make `Action::CredentialPatch` non-configurable and deny it to every non-root caller inside
`Authorization`, regardless of stored permissions.

Both layers are required:

- `Action::is_configurable()` excludes `CredentialPatch`, mirroring `PermissionManage`, so `Permission::try_new` rejects
  a new override with `422` and the action disappears from the configurable defaults representation.
- `Authorization::allows` denies `CredentialPatch` to a non-root caller before consulting overrides. This matters
  because `effective_scope` trusts any stored `Permission` row; a row written before this change would otherwise keep
  granting the action. The Root Admin short-circuit still allows it.

**Alternatives considered:**

- _Keep it configurable and add a self-or-root guard._ Rejected: no self-service endpoint exists, so the grant would be
  meaningless for a Standard user, and "any credential" would remain a takeover grant.
- _Split the action into `credential.patch` (self) and `credential.manage` (any)._ Rejected: new API surface with no
  client need, and it would still need a structural rule.

**Consequence:** `docs/Permissions.md` changes `credential.patch` to non-configurable, and the permission representation
no longer lists it under `defaults`.

### 2. Expiry is enforced at `exp`, with no grace period

**Decision:** set `validation.leeway = 0` in `JwtTokenProvider::validate`.

`jsonwebtoken 11` defaults to 60 seconds of leeway, so a token is currently accepted for a minute after it expires. The
issuer and validator run in the same process against the same clock, so the tolerance buys nothing and contradicts the
contract. A future multi-node deployment must revisit this with an explicit clock strategy rather than an implicit
default.

**Alternative considered:** document the 60-second tolerance as deliberate clock-skew handling. Rejected: it weakens the
contract to preserve a framework default nobody chose.

### 3. The bootstrap refuses to withhold its only reveal

The generated password's shape is fixed by `RandomPasswordGenerator`: 20 characters, 19 alphanumeric plus one symbol
from `!@#$%^&*()-_=+`. The spec states that shape directly rather than delegating to the account password policy,
because the server controls the generated value; the policy remains the rule for user-chosen passwords at creation and
rotation.

**Decision:** extend the `InitializeRootAdmin` input with the reveal decision; when it would create the account and the
reveal is disabled, return a dedicated business error before generating or writing anything, and let `app.rs` fail
startup with a message naming `security.log_root_admin_password`.

The use case is the only place that knows whether creation is needed, so the guard belongs there rather than in
configuration loading (which cannot know whether user `0` exists) or in `app.rs` (which would duplicate the lookup). The
guard runs after the existence check and before the password is generated, so the rollback leaves no user and no
credential.

**Alternatives considered:**

- _Always print on first creation, ignoring the flag._ Rejected: it silently changes a documented and tested setting.
- _Add an operator-supplied seed password._ Rejected: new configuration surface and secret handling outside this
  change's scope.

### 4. Every `401` carries `WWW-Authenticate: Bearer`

**Decision:** add the header centrally in `ApiError::into_response` when the status is `401`.

Centralizing covers the auth middleware and login failures alike, and `hal_errors` already preserves `WWW-Authenticate`
while rewrapping the body. The header carries the scheme only; error codes such as `invalid_token` are not needed by any
current client.

**Alternative considered:** set the header only in `authenticate`. Rejected: it would leave the login `401` and any
future `401` producer inconsistent.

### 5. Tokens stay stateless and unrevalidated

**Decision:** document "valid until expiry" as the contract; do not add logout, revocation, or a credential version
claim.

A single-container server with no session store gets a simple, restart-safe contract this way. The cost is that a
rotated credential does not evict tokens already issued. The configured TTL bounds that window, and the spec states the
behavior explicitly so it cannot be mistaken for a bug. Regeneration (Decision 7) is a re-issue, not a revocation; it
extends the window without adding state.

**Alternative considered:** add a credential version or `updated_at` claim and compare it on validation. Rejected for
this change: it adds a datastore read to every request and a migration, and it is better designed alongside a real
session or logout feature.

### 6. Login is throttled on two independent keys

**Decision:** keep the per-client `GovernorLayer` on the login route and add a per-account check keyed by the resolved
user identifier. The check runs inside `LoginUser`, after the username resolves and before password verification,
because only the use case knows the identifier. A new outbound `LoginAttemptLimiter` port returns the wait time; the use
case answers `RateLimited`, and `post_login` maps it to `429` with `Retry-After` and emits `rate_limit_exceeded`. The
composition root builds the stateful limiter once and injects it into the identity use-case factory.

A username that resolves to no account has no identifier, so the adapter keys the same quota on a `SHA-256` digest of
the submitted value. The digest is never logged, and it gives the unknown branch the same throttle behavior, so a `429`
cannot be used to test whether an account exists. Both stores are pruned by the existing periodic task; the keyed store
uses `retain_recent`, which gives `RateLimitCleanup::merge` its first caller.

Why not check in the handler: the identifier is only known after the use case resolves the user, and resolving it a
second time would duplicate the lookup and split the identity rule. A second governor layer has the same problem:
`KeyExtractor` sees headers and extensions, not the resolved account.

**Alternative considered:** key on the submitted username in the handler. Rejected: the requirement binds the quota to
the account, not the string, so a renamed username keeps its bucket and the raw value never becomes a key.

**Trade-off:** a caller who knows a username can spend its quota and deny that user. The quota is the same as the client
burst, and the spec states the account key so the behavior is deliberate rather than accidental.

### 7. Token regeneration re-issues from the authenticated subject

**Decision:** add `POST /api/v1/identity/token`, protected by `authenticate`, backed by a `RegenerateToken` use case
that loads the caller, issues a fresh token through `JwtTokenProvider::issue`, returns the same representation as login
(extracted to a shared `TokenResponse`), and emits one `token_regenerated` with the actor.

The previous token is not touched: statelessness means it stays valid until its own `exp`. There is no refresh token and
no stored session; the endpoint simply re-issues the caller's access token. The use case loads the caller so a token for
a deleted account cannot mint a successor.

**Alternative considered:** a refresh-token grant with a separate long-lived credential. Rejected: it adds a second
secret to store, rotate and revoke, which this server deliberately does not have.

**Trade-off:** a holder can keep regenerating before expiry, so a stolen token can be kept alive indefinitely. The
endpoint is protected by the same token; an absolute session cap (`iat` plus a maximum age) is the natural follow-up if
that window becomes unacceptable.

### 8. The general password policy lives in `authentication`

**Decision:** move the normative password rule — at least 8 characters and at least one non-alphanumeric character —
into `authentication` (`REQ-AUTHENTICATION-018`) and remove the requirement from `user/account-creation`, which no
longer states or references it. Rotation already lives here, and `REQ-AUTHENTICATION-018` applies the rule to every
place a password is set, creation included.

The rule governs every place a password is set, so two owners would drift. `PasswordPolicy` in the domain already
implements it once; this is a spec-ownership move, not a code change.

**Alternative considered:** leave a delegating requirement in `user/account-creation` that points at `authentication`.
Rejected: it keeps a second requirement that owns nothing, and the creation delta must then stay in sync with a rule it
does not define.

**Alternative considered:** keep the policy in `user/account-creation` and have `authentication` reference it. Rejected:
the capability that owns credentials and rotation should own the rule they enforce.

### 9. Spec scenarios are proven black box, in a system suite

**Decision:** add `test/system/`, a black-box suite with one module per feature area — `authentication.py` first —
driven through the same `harness` plugin the configuration suite uses (`Sut`, `sut`, `hex64`): each test starts its own
container on a random loopback port with its own working directory, and reads only what an operator can — HTTP
responses, standard output, the log sink, and the working directory. Each test names the scenario it proves through the
Allure `story` label, so the spec's scenario IDs are what the report groups on and one test carrying two `story` labels
proves two scenarios.

The suite is not a new harness. `pytest.ini` already loads `harness` as a plugin for every suite, and `Sut` already
starts a container, waits for health, reads its files, and starts one that is expected to fail — which is exactly what a
bootstrap scenario needs.

**Priority:** a scenario's proof is a system test whenever the container can observe the outcome. A Rust test follows
for what the black box cannot reach, and beside an observable outcome where the spec states a mechanism the container
cannot inspect:

- a failure injected into a port — the bootstrap rollback (`SCN-001-C`, `-D`) and the rotation commit failure
  (`SCN-015-B`), with no system test;
- the decoy-hash branch of `SCN-005-A`, where response timing is not assertable — it has no system test;
- the `Debug` redaction (`SCN-006-B`) and the generator's sampling distribution (`SCN-002-A`), where the system test
  proves the outcome and the unit test inspects the mechanism.

**Conformance belongs to the API suite.** A declared status, content type, body schema or response header is checked by
Schemathesis on every operation it reaches, so no task writes that assertion again. A task writes only what the schema
cannot express: which caller is refused, which password is rejected, what state changed, what the log carries, or that a
field is absent. A scenario whose whole content is "a guarded operation answers `401`" is owned by the API suite and has
no system test.

**A scenario moves; it is not copied.** When the system suite takes a scenario over from `test/e2e/` or
`test/configuration/`, the pre-existing case is deleted in the same change — otherwise one failure reports twice and one
of the copies rots. This change deletes `test/e2e/identity/uc002_user_authentication.py` and
`test/configuration/security_settings.py` once their cases are replaced, the two password-policy rows in
`test/e2e/user/uc001_create_a_user_account.py`, and the seventeen per-endpoint `requires_credentials` cases in
`test/e2e/library/*`. The last group is the API suite's: Schemathesis calls every operation against its declared
`security` and its declared `401`, so a per-endpoint E2E copy asserts the same route guard at a slower level.

**Why:** the spec is the product contract, so its proof must be the test that fails when the contract breaks. A Rust
test that asserts the same outcome also fixes the internal route; it can stay green while the observable behaviour
changes or disappears. The suite also gives every later capability one place to prove its scenarios instead of growing
the use-case-keyed E2E files.

**Alternatives considered:**

- _Keep labelling `test/e2e/` and `test/configuration/` tests with scenario IDs._ Rejected: those suites are keyed to
  use cases and settings, so a scenario with no journey has no home and a scenario touched by two journeys has two.
  `test/configuration/instance_state.py` already proved the spec-keyed mapping works for a whole capability.
- _Wait for a dedicated consumer._ Rejected: bootstrap, login and rotation are the largest black boxes in the server,
  and the suite is a folder, a label convention and a script.

**Relationship to the other suites:** `test/system/` owns scenario outcomes, `test/e2e/` keeps the use-case journeys,
and `test/configuration/` keeps the per-setting effects that no spec scenario states.
`test/configuration/instance_state.py` keeps its `instance/state` cases even where they incidentally observe the Root
Admin (a fresh datastore re-seeds it), because each is the proof of an `instance/state` scenario in the suite that owns
that capability. A scenario that only a Rust test can reach — an injected failure or the decoy-hash branch — carries the
same `story` label on that Rust test, so the report still indexes it to the scenario. Two scenarios that state the same
outcome (the pre-rotation token staying valid, `SCN-AUTHENTICATION-012-A` and `-013-C`) are two `story` labels on one
test, not two tests.

**Trade-off:** one container per test is slower than a Rust test. The harness bounds startup at 20 s, and the CI
`system` job already builds the image and runs for up to 30 minutes; the suite runs there after the configuration suite
against the same image.

## Risks / Trade-offs

- **A second black-box suite duplicates assertions** that `test/e2e/` and `test/configuration/` already make, so one
  change could make the same failure twice. Mitigation: each scenario carries its `story` label on exactly one test, and
  a replaced case is deleted with its replacement (Decision 9).
- **Existing `credential.patch` overrides** survive in the datastore after the action becomes non-configurable. The
  structural guard neutralizes them, but they remain visible no longer. Mitigation: the domain guard is the enforcement
  point; a cleanup is unnecessary unless an operator reports a stale row.
- **Zero leeway and clock skew** could reject a valid token in a future multi-node deployment. Mitigation: record the
  assumption here and revisit when the server becomes multi-instance.
- **First-start behavior changes** for `log_root_admin_password: false`. Mitigation: the failure is explicit, names the
  setting, and leaves the datastore untouched; existing instances are unaffected because user `0` already exists.
- **No revocation** leaves a compromised token valid until `exp`. Mitigation: short default TTL and an explicit spec
  statement; a token-version design can follow if needed.
- **Account quota enables targeted lockout.** A caller who knows a username can spend the account's quota and deny that
  user. Mitigation: the short configured period bounds the window; a future change can use a higher account burst or
  count only failed attempts.
- **Sliding session.** A holder can regenerate before expiry and keep a stolen token alive indefinitely. Mitigation:
  regeneration still requires a valid token; an absolute session cap (`iat` plus a maximum age) is the natural
  follow-up.
- **Policy ownership moved across capabilities.** `user/account-creation` loses its password-policy requirement, so the
  archive must remove it there and the numeric rule must appear only in `authentication`. Mitigation: the removal delta
  records the reason and migration, and the verification tasks check that no copy remains.
- **Naming split** between the `authentication` capability and the `identity` routes/modules. Mitigation: state it in
  the spec's context; a rename can be a separate, mechanical change.

## Migration Plan

No datastore migration is required.

1. Ship the code and the new spec; sync the delta into `openspec/specs/authentication/spec.md`.
2. Existing instances already have user `0`, so the bootstrap guard is inert; their issued tokens keep working until
   expiry.
3. A fresh instance with `security.log_root_admin_password: false` now fails at startup; the operator enables the reveal
   for the first start.
4. The per-account limiter uses the existing `security.rate_limit` burst and period, so no configuration change is
   required; it takes effect on restart.
5. The regeneration endpoint is additive; clients that keep re-authenticating are unaffected.
6. The password-policy requirement moves to `authentication`; the 422 behavior is unchanged, so no client or data change
   is required.
7. The system suite runs against the image like the configuration suite and needs no code change; `pytest.ini`, the CI
   `system` job and the documentation gain it, and it starts with `authentication` only.
8. The E2E and configuration cases it replaces are deleted with it, so no scenario is proven twice; a revert restores
   them with the code.
9. Rollback is a code revert; no data is altered by the fixes.
