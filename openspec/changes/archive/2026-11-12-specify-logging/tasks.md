# Tasks

Schemathesis already proves a requirement's declarative contract against the served document: every declared status,
media type, response header, envelope shape and `_links`/`_embedded` field belongs to the API fuzz test
(`script/api_fuzz.sh`). Do not add a system case that restates it — a system case proves only the effect the fuzzer
cannot observe, and the task names the fuzz operation that covers the rest.

Do not run the E2E suite (`script/e2e.sh` or `pytest test/e2e`), the configuration suite (`script/configuration.sh`) or
`devenv test` in this change, with no exceptions.

## 1. Move the catalog and re-point its check

- [x] 1.1 Replace the catalog table in `docs/development/TechnicalDesign.md` § 9.6 with a citation of `REQ-LOGGING-010`;
      verify with `grep -n 'authn_succeeded' docs/development/TechnicalDesign.md` that the section holds no event row,
      and that the citation names the capability.
- [x] 1.2 Re-point `check_obs_006` in `script/rule_enforcer.sh` at `openspec/specs/logging/spec.md`, anchored on the
      `The security-event catalog:` line and stopping at the next `### Requirement:`; verify the anchor occurs once
      (`grep -c`) and that the table holds exactly 32 rows with the event, severity, fields and recorded-when columns.
- [x] 1.3 Update `docs/development/TechnicalDesign.md` § 9's introduction and the `OBS-001`–`OBS-007` rows to cite the
      logging capability instead of restating it, leaving `OBS-008` untouched; verify every logging row names the
      capability and `openspec validate specify-logging --strict` is clean.

## 2. Prove the sinks and the mechanism in the system suite

- [x] 2.1 Move the `security_events` helper out of `test/system/authentication.py` into `test/system/conftest.py` and
      verify `MNEMORIUM_IMAGE=mnemorium:latest pytest test/system/authentication.py -q` still passes.
- [x] 2.2 Add `test/system/logging_events.py` with the `REQ-LOGGING-001` sink and colour scenarios and the
      `REQ-LOGGING-002` rotation and pruning scenarios, moved from `test/configuration/logging_settings.py` and
      re-labelled `epic="Logging"`, `feature` and `story`; verify the module passes, then delete the configuration
      module because every case in it has moved.
- [x] 2.3 Prove `REQ-LOGGING-003` in `test/system/logging_events.py`, using the harness's start-expecting-failure path:
      an empty value, `off`/`none` and a reserved-target directive each refuse startup naming the setting and never echo
      the value.
- [x] 2.4 Prove `REQ-LOGGING-004` non-suppression in `test/system/logging_events.py`: a quiet verbosity admits only
      `error`, a login succeeds, and both an `info` event and a `debug` catalogued event still reach the sinks.
- [x] 2.5 Prove `REQ-LOGGING-005` layers: verify `grep -rn 'target: "security"' src/lib/domain` finds nothing, and add
      the "a domain rejection is recorded by its caller" scenario as a `#[traced_test]` asserting exactly one event for
      a rejection the domain returns as a value; verify `cargo test` passes.
- [x] 2.6 Prove `REQ-LOGGING-006` once-only: assert exactly one event for a denial and a rejected login through the
      system suite, and for a dependency fault at unit level; verify both pass.
- [x] 2.7 Prove `REQ-LOGGING-007` content: a request whose query string carries a value traces the path and not the
      query, and a file-storage failure names no file, in `test/system/logging_events.py`; add the framework-error
      classification case where only a unit test can provoke it; verify both pass.
- [x] 2.8 Prove `REQ-LOGGING-008` redaction: move the login and token assertions into `test/system/logging_events.py`,
      move the identity-secret assertions out of `test/configuration/instance_state.py` into the same module, and add
      the placeholder-debug case at unit level; verify no sink holds a password, token, pepper or signing secret.
- [x] 2.9 Prove `REQ-LOGGING-009` severity: an expected business rejection records `warn` or `info` and a dependency
      fault records `error`; verify both through the system suite.
- [x] 2.10 Prove `REQ-LOGGING-011` injection: a rejected login claiming an identity with a newline keeps the record on
      one line and an over-long identity is truncated to the bound; verify the two cases pass.

## 3. Prove every catalog group at the trigger it names

- [x] 3.1 Move the four `REQ-AUTHENTICATION-017` cases from `test/system/authentication.py` into
      `test/system/logging_events.py`, re-labelled to `REQ-LOGGING-012`, and verify each event is emitted exactly once
      with the declared fields.
- [x] 3.2 Prove `REQ-LOGGING-013`: one `authz_failed` per denial, none for a filtered listing, and one
      `suspicious_business_logic` for an upload action taken out of order; verify the three cases pass.
- [x] 3.3 Prove `REQ-LOGGING-014` at application and adapter level: `port_fault` for a dependency failure,
      `constraint_rejected` for an expected constraint and `application_error` for an originated error, each exactly
      once; verify `cargo test` passes.
- [x] 3.4 Prove `REQ-LOGGING-015`: move the lifecycle assertion out of `test/configuration/logging_settings.py` and the
      identity-creation assertions out of `test/configuration/instance_state.py` into `test/system/logging_events.py`;
      add the `user_admin` case; verify creation emits `system_object` plus one `secret_initialized` per generated
      secret and reuse emits neither, and confirm by `grep` that no event assertion remains in the configuration
      modules.
- [x] 3.5 Prove `REQ-LOGGING-016`: an ingest reports the seven counts and no file name or path, a degraded observer
      reports `ingest_fault`, and a skipped provider reports its classification only; verify the system cases pass and
      keep the provider-failure case at unit level where the container cannot provoke it.
- [x] 3.6 Prove `REQ-LOGGING-017`: an upload lifecycle writes `upload` per outcome and a reaping run writes
      `upload_reap` with counts only; verify no file name or path appears.
- [x] 3.7 Prove `REQ-LOGGING-018`: a personal-data read at `debug` records the actor and the object and no personal
      value; verify the case passes.

## 4. Point the project's vocabulary at the capability

- [x] 4.1 Update `docs/development/Glossary.md`: cite the logging requirements in the `Redaction`, `Security event`,
      `Non-suppressible event`, `Log injection` and `Log rotation` entries, and add a term for the closed event catalog
      if the spec's vocabulary needs one; verify `markdownlint-cli2 '**/*.md'` passes.
- [x] 4.2 Point the `authz_failed` field vocabulary in `docs/Permissions.md` at the logging catalog instead of § 9.6;
      verify every cited path resolves.
- [x] 4.3 Update `docs/development/Overview.md`'s description of `src/lib/infrastructure/logging.rs` to name the
      capability it implements; verify its links resolve.

## 5. Integration checks

- [x] 5.1 Run `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check` and `cargo test`,
      and confirm each passes.
- [x] 5.2 Run `script/system.sh` and confirm `test/system/logging_events.py` passes alongside every other feature
      module, with no scenario now proven at two levels.
- [x] 5.3 Run `openspec validate specify-logging --strict` and confirm it passes.

## Workflow follow-up

- Archive the change after the project's review requirements are satisfied.
- Verify the archived result: `openspec/specs/logging/spec.md` exists with the catalog, and `TechnicalDesign.md` § 9.6
  cites it.
