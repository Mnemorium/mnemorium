# Proposal

## Why

The first start reveals the **Root Admin**'s only credential as a 20-character password on standard output. Moving that
value to a phone or desktop client means copying it by hand from a console, and what it delivers is a permanent password
that nobody can inspect, retype or scan. The handoff should instead be a short, one-time, expiring token that a client
can scan or type, exchange once for the credentials it chooses, and never see again.

Two adjacent inconsistencies should close in the same change. `REQ-AUTHORIZATION-017` calls the root's account
immutable, yet the root should maintain its own email; the code refuses every update to account 0, so the setup email
would otherwise need a second, unexplained carve-out. And `SEC-001` carries HTTP-boundary rules — the closed set of
public operations, the rule that a public response never carries a secret — that the capability specifications never
state, so the design document is the only place they exist and nothing tests them.

## What Changes

- Replace the generated-password reveal with a **Root Admin setup token**: eight Crockford base32 characters, printed as
  `XXXX-XXXX` and as a machine-scannable QR code, single-use, rate-limited and expiring after 30 minutes by default.
- Derive the token from the instance's **Site pepper** and a stored nonce, so the **Setup grant** holds no token
  material and a datastore copy alone neither reveals the token nor verifies a guess.
- Give the Root Admin credential the hash of a generated secret that is never revealed. No plaintext password exists at
  any point, and the current account has no usable login until setup completes.
- Reissue the token on every start while setup is pending, replacing the previous one; once setup completes, a start
  issues and reveals nothing.
- Add the public `POST /api/v1/identity/setup` operation: it accepts the token, a password satisfying the credential
  policy and an optional email, and answers `204 No Content`. A successful call replaces credential 0, stores the email,
  and consumes the grant in one transaction; a rejected password or a taken email leaves the grant usable.
- Let the root update its own email through `PATCH /api/v1/user/{id}`, while its username and role stay fixed and no
  caller may delete it.
- **BREAKING** for operators: replace `security.log_root_admin_password` with `security.root_admin_setup.reveal` and
  `security.root_admin_setup.ttl_seconds`, and stop printing a password. A first start with the reveal disabled still
  fails, naming the setting.
- Remove `SEC-001` from `docs/development/TechnicalDesign.md` § 8 and the secret-bearing-struct prose in § 1. Move the
  "a raw secret in a request body is an input" note to § 1 Code Style Guidelines, under § 16 Request body declaration.
- Add the requirements the deleted rule carried: the public surface is closed to the probe, the **API index**, login and
  setup, and setup is the only credential mutation reachable without a bearer token.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `authentication`: the bootstrap creates a **Setup grant** instead of revealing a password; the token's shape, one-time
  lifecycle, derivation, rate limit and the setup operation that replaces the Root Admin credential and optional email.
- `api`: the **API index** reaches the setup operation, and the public surface is a closed set that returns no secret
  material.
- `user`: the root may update its own email; its username, role and existence remain fixed.
- `authorization`: `REQ-AUTHORIZATION-017` drops the sentence that the root's account is immutable, because account
  administration owns that rule.
- `logging`: the setup token is a secret; only the one-time standard-output reveal carries it and no event records it.

## Impact

- **API**: `POST /api/v1/identity/setup` is added as a public operation; the index gains a `setup` link;
  `PATCH /api/v1/user/0` accepts an email-only update when the caller is the root. All other routes are unchanged.
- **Code**: `src/lib/application/use_case/initialize_root_admin.rs` and its port and response; a new setup use case,
  port and SQLx repository; a migration for the setup grant; `src/lib/infrastructure/inbound/rest/handler/identity.rs`
  and a new `post_setup.rs`; `src/lib/app.rs` (QR reveal and state); `src/lib/application/use_case/update_user.rs`
  (field-aware root guard); `src/lib/domain/service/setup_token.rs` (the derivation service); the identity use-case
  factory.
- **Configuration**: `security.log_root_admin_password` is removed in favour of `security.root_admin_setup.reveal` and
  `security.root_admin_setup.ttl_seconds`; `config.yaml` and the startup-only settings registry follow.
- **Dependencies**: two crates, an HMAC implementation and a QR encoder, recorded in
  `docs/development/DependencyManifest.md`.
- **Tests**: the system harness exchanges the token once per instance and caches the chosen password; the bootstrap,
  setup and root-email scenarios are added to `test/system/authentication.py` and `test/system/user.py`; the
  20-character password-shape case is replaced.
- **Docs**: `docs/development/Glossary.md` gains `Root Admin setup token` and `Setup grant`; `Site pepper` broadens;
  `docs/development/TechnicalDesign.md` § 1, § 8 and the settings registry change.
