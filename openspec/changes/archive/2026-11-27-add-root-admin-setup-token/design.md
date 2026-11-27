# Design

## Context

See `proposal.md` — Why.

The bootstrap is one application use case and one print site:

- `InitializeRootAdmin` (`src/lib/application/use_case/initialize_root_admin.rs`) opens one write unit of work, looks up
  user `0`, and, when absent, writes the credential, the user, its `User` resource, and returns the generated password.
  `src/lib/app.rs` prints it with `println!` before the listener binds.
- `security.log_root_admin_password` gates that reveal, and a first start with it false fails startup.
- `identity_public_router` (`.../rest/handler/identity.rs`) holds login; `identity_routes` wraps it in the per-address
  limiter, while `identity_protected_router` sits behind `authenticate`.
- `PasswordPolicy` (`src/lib/domain/service/password_policy.rs`) is the credential policy.
- `UpdateUser` (`src/lib/application/use_case/update_user.rs`) refuses every update whose target is user `0`.
- `state.yaml` is write-once: `FileInstanceStateStore::save_locked` returns `Conflict` when the file exists, and
  `REQ-STATE-007-D` refuses any field beyond the version, the creation instant and the two secrets.
- `SEC-001` is prose in `docs/development/TechnicalDesign.md` § 8, repeated in § 1; no rule file references it.
- The system harness reads the one-time secret from standard output (`test/util/harness.py`), and every fixture uses
  `root_password()`.

## Goals / Non-Goals

**Goals:**

- One short, one-time, expiring token that a client scans or types, with no plaintext password ever stored or returned.
- A grant that survives a restart, is atomically single-use, and is reissued on demand without a datastore migration of
  existing instances.
- Reuse the existing identity route split, limiter, unit of work, password policy and email validation; no new token
  scheme, claim or session state.
- Replace `SEC-001` with requirements the system suite can prove.
- Make the root's own email a normal profile field without opening its identity.

**Non-Goals:**

- An HTML page or any second display surface; the token is revealed on standard output only.
- Any change to login, bearer tokens or credential rotation for other users.
- A recovery path that bypasses the datastore and the state file.
- A QR payload that carries a URL; the QR carries the token alone.

## Decisions

### 1. The setup token replaces the password reveal

**Decision:** the first start writes a credential holding the hash of a generated secret that is never revealed, then
returns a **Root Admin setup token** instead of a password. The token authorizes one operation, setting credential `0`
and the root's optional email; it never authenticates a request.

This is the only shape that keeps `REQ-AUTHENTICATION-006` intact: no plaintext password is persisted, returned or
logged, and the credential's secret is discarded before the process can reveal it. The setup operation is where the Root
Admin chooses its real password, through the existing `PasswordPolicy`.

**Alternatives considered:** a one-time envelope that decrypts the generated password under the token. Rejected: it
persists the password in a reversible form to satisfy a reveal that the setup operation removes the need for.

### 2. The token is derived from the site pepper

**Decision:** the token is

```
token = CrockfordBase32(HMAC-SHA256(pepper, "mnemorium/root-admin-setup/v1" || nonce))[..8]
```

printed as `XXXX-XXXX`. The **Setup grant** stores the nonce, the expiry and the consumption instant; it stores no token
and no verifier. Verification recomputes the token from the persisted nonce and the **Site pepper** and compares it in
constant time.

40 bits is safe only because three controls bound the search: a 30-minute window, a per-address budget, and a
process-wide budget. The derivation uses `hmac` 0.13 over the existing `sha2`, with a domain-separation prefix so the
pepper's other use (password hashing) cannot collide with it.

HMAC rather than a plain `Sha256(pepper || ...)`: HMAC is the standard keyed construction over the _same_ SHA-256
implementation, adding only the `ipad`/`opad` key handling. A raw secret-prefix hash is a homemade MAC with a
length-extension quirk: given `H(k || m)` one can compute `H(k || m || padding || extra)` without `k`. Here that is not
exploitable — the printed token is a 40-bit truncation, and an extended input names a nonce the server never stored —
but both are accidents. Lengthening the token, logging the full digest or accepting a nonce from the request would make
it live. HMAC removes the argument, and keeps the pepper explicitly a key rather than more message bytes.

The derivation is a **domain service**, `SetupTokenDeriver` in `src/lib/domain/service/setup_token.rs`: it is stateless,
pure and called by both `InitializeRootAdmin` (to reveal) and `SetupRootAdmin` (to verify), so it fits the
domain-service rule (`ARCH-005`) and owns its unit test module (`TEST-024`). The **Setup grant** is a domain model and
gets no dedicated test (`TEST-019`); its invariants are exercised through the application tests and the repository
integration test.

**Alternatives considered:**

- A random token with a stored hash. Rejected: it buys nothing here. The page that would need to redisplay the token is
  gone, and a hash adds a verifier to the datastore.
- A raw secret-prefix SHA-256. Rejected: it works today only because of the truncation and the fixed nonce, and every
  review would have to re-litigate length extension.
- A hand-rolled HMAC over `sha2`. Rejected: the keyed construction is exactly the place not to write it yourself.
- Argon2 for the token verifier. Rejected: the token is 40 bits, not a user password, and the throttle makes offline
  work irrelevant. Anyone holding both the datastore and `state.yaml` can already forge a root bearer token with the
  signing secret.
- Storing the token in `state.yaml`. Rejected by Decision 3.

### 3. The grant is a SQLite row, not instance state

**Decision:** a new `root_admin_setup` table with one row (`id = 0`): `nonce`, `expires_at`, `consumed_at`. The identity
unit of work gains a setup-grant repository; the SQLx adapter implements a conditional consume:

```sql
UPDATE root_admin_setup SET consumed_at = :now
WHERE id = 0 AND consumed_at IS NULL AND expires_at > :now
```

A zero-row result is the uniform refusal, and the surrounding transaction rolls the credential and email writes back.

`state.yaml` cannot host this. Its save path refuses to replace an existing identity, `REQ-STATE-007-D` refuses extra
fields, and it has no transaction to make a consume atomic against a concurrent request. The state file's job is the
identity, not a mutable workflow record; the datastore already has transactions, migrations and a reaper-friendly shape.

**Alternatives considered:** a second mutable file beside `state.yaml`. Rejected: it reimplements locking and atomic
writes for one row, and a crash between the credential write and the consume would leave a live token after setup.

### 4. A start reissues while setup is pending

**Decision:** `InitializeRootAdmin` returns one of three outcomes:

```
no user 0            -> create user + credential + grant, return the token
user 0, grant open   -> replace the nonce and expiry, return the new token
user 0, grant closed -> return nothing
```

The reveal guard covers both issuing branches: a start that would issue a token while `security.root_admin_setup.reveal`
is false fails, naming the setting (SCN-AUTHENTICATION-003-D). Otherwise a pending instance could lose its only way in
by restarting.

Reissue invalidates the previous token, so an operator who restarts after losing the console output gets a fresh one. A
datastore created by an older build has user `0` and no grant row; the absent row is read as "setup complete" so an
upgrade never exposes a fresh token for an initialized instance.

**Alternative considered:** keep the grant until it expires and never replace it on restart. Rejected: a lost or expired
token would need filesystem surgery; a restart is the natural recovery the operator already has.

### 5. Setup is a public operation with the token in the body

**Decision:** `POST /api/v1/identity/setup` is public, takes `{ "token", "password", "email" }` (`email` optional), and
answers `204 No Content`. The token travels in the body, like a login password, so `authenticate` keeps meaning "session
bearer" and no second security scheme is introduced. The handler maps:

| Outcome                         | Status                      |
| ------------------------------- | --------------------------- |
| setup applied                   | `204 No Content`            |
| unknown, expired or used token  | `401 Unauthorized`, uniform |
| password fails the policy       | `422 Unprocessable Content` |
| email another account holds     | `409 Conflict`              |
| address or process budget spent | `429 Too Many Requests`     |

`401` follows login's precedent (`REQ-AUTHENTICATION-005`) rather than `REQ-API-013`'s body-reference rule, because the
token is a credential, not a resource reference.

The per-address budget is the existing `GovernorLayer` applied to the route; the process-wide budget is a new
`SetupAttemptLimit` keyed by a constant, mirroring `LoginAttemptLimit`. Both emit the catalogued `rate_limit_exceeded`.
The index gains a `setup` link, so a client holding only the token can discover the route.

**Alternative considered:** a bearer header carrying the setup token. Rejected: it would make the token look like a
session token to the middleware, the generated document, and every reader of the endpoint.

### 6. Setup writes the credential, the email and the grant in one unit of work

**Decision:** a new `SetupRootAdmin` application use case owns the operation:

1. begin a write unit of work and read the grant; refuse when it is absent, expired or consumed;
2. normalize and compare the submitted token to the derived one, in constant time;
3. validate the password with `PasswordPolicy` and the email with the account rules, including the case-insensitive
   uniqueness check the update path already uses;
4. hash the password with the existing `PasswordHasher` (pepper included);
5. replace credential `0`'s hash and set user `0`'s email when one is supplied;
6. consume the grant conditionally and commit; a zero-row consume is the uniform refusal and rolls everything back.

The consume is last so a lost race cannot leave a live token after a successful setup, and a failed write leaves the
grant usable (SCN-AUTHENTICATION-021-C, -D, -E). The root's username and role are not touched.

**Alternative considered:** reuse `PatchCredential` and `UpdateUser` from the setup use case. Rejected: both authorize a
caller, and the setup has none; calling them would need a synthetic root caller and a second transaction boundary.

### 7. The root maintains its own email

**Decision:** replace the blanket `user.id() == 0` guard in `UpdateUser` with a field-aware rule: an update to account
`0` is accepted only when the caller is `0`, `role` is absent and `username` is absent; the email is then written
through the ordinary uniqueness path. Deletion keeps its own refusal.

The root is the only account with no administrator above it, so "untouchable" cannot mean "cannot maintain its own
contact address". Its username is fixed by `REQ-AUTHENTICATION-001`, and its role is the top of the hierarchy; letting
either move would let the instance lose its root. `REQ-AUTHORIZATION-017` loses the sentence that made the whole account
immutable, because account administration, not the authorization decision, owns that rule.

### 8. `SEC-001` is replaced by requirements

**Decision:** delete the `SEC-001` row from `docs/development/TechnicalDesign.md` § 8 and the "Secret-bearing structs"
prose from § 1, keeping only the definition where other rules cite it. Move the "a raw secret in a request body is an
input, not a secret-bearing struct" note to Code Style Guidelines § 16 (Request body declaration). Add `REQ-API-025`
(the public surface is exactly the probe, the index, login and setup; a public response carries no secret except login's
own token) and `REQ-AUTHENTICATION-020`/`-021` (one-time, expiring, rate-limited setup; the only bearerless credential
mutation). The system suite proves both, so the rule stops being prose only.

No `rule/*.yml` or `rule_enforcer.yaml` entry references `SEC-001`; `STY-RUST-100` still enforces the `security`
declaration on every operation and needs no change.

### 9. Configuration and dependencies

**Decision:** replace `security.log_root_admin_password` with:

```yaml
security:
  root_admin_setup:
    reveal: true
    ttl_seconds: 1800
```

`ttl_seconds` is bounded (1 to 86400) and read when a token is issued. The removed key is a breaking change: an existing
`config.yaml` that still names it fails startup under `REQ-CONFIGURATION-004`, naming the key, so the operator must
update the file. `security.root_admin_setup.reveal` joins the startup-only registry.

Two crates join `Cargo.toml` and `docs/development/DependencyManifest.md`: `hmac` 0.13.0 (MIT OR Apache-2.0) for the
derivation, and `fast_qr` 0.14.0 (MIT) for the terminal QR. The QR renderer is a pure outbound adapter under
`src/lib/infrastructure/outbound/qr/`, called by `app.rs` beside the text line; no port is needed because nothing
injects it.

**Alternative considered:** `qrcode` 0.15.0 (MIT OR Apache-2.0). Also viable; `fast_qr` is chosen for its terminal
renderer and its lack of an image dependency.

### 10. Observability and redaction

**Decision:** reuse the catalog. A successful setup emits `credential_rotated` with actor `0`; a refused token emits
`authn_failed` with one reason, so the event does not disclose which check failed; a throttled request emits
`rate_limit_exceeded` with route `identity_setup`; the root creation keeps its `system_object` event. The setup
command's token and password fields redact their `Debug` representation, like the login request.

No new event is added, so `REQ-LOGGING-010`'s closed catalog is unchanged.

## Risks / Trade-offs

- **[Lockout]** A 40-bit token cannot be recovered from the datastore, and a lost token strands a pending instance →
  every start while setup is pending reissues and prints; a pending start with the reveal disabled fails instead of
  issuing silently; the operator can always restart to get a new token.
- **[Exposure]** The setup endpoint is public → the token is the only authority, the 30-minute window and the
  process-wide budget bound guessing, and no page displays the token to a passer-by.
- **[Token in container logs]** The token is printed to standard output, which Docker retains → it is one-time and
  expires; this is the same channel the permanent password used before, with a smaller blast radius.
- **[Suite churn]** Every system fixture reads the password → the harness performs the setup once per instance and
  caches the chosen password, so cases keep calling `root_password()`.
- **[Breaking configuration]** The renamed keys fail an untouched `config.yaml` → the upgrade note and the startup error
  name the key.
- **[Root email change]** A caller that relied on `PATCH /user/0` always failing now succeeds for an email-only update →
  the behavior is specified and tested, and identity fields still refuse.
- **[Dependency pairing]** `hmac` 0.13 requires `digest` 0.11, which `sha2` 0.11 already uses → verify the resolved tree
  with `cargo tree -d` and keep `cargo deny` clean.

## Migration Plan

1. Ship the migration (`create_root_admin_setup`) and the code. No backfill: an existing `mnemorium.db` has no grant
   row, which reads as setup complete.
2. Update `config.yaml`: delete `security.log_root_admin_password`, add `security.root_admin_setup.reveal` and
   `.ttl_seconds`. A stale key fails startup naming it, so the failure is explicit.
3. Start the instance, read the token and scan or type it, submit the setup, then verify a normal login.
4. Rollback: the old binary ignores the new table; revert the configuration keys and the binary together, because the
   old binary rejects the new keys and the new binary rejects the old one until the file is updated.

## Open Questions

None.
