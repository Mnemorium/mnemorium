# Tasks

Schemathesis already proves a requirement's declarative contract against the served document: every declared status,
media type, response header, envelope shape and `_links`/`_embedded` field belongs to the API fuzz test
(`script/api_fuzz.sh`). Do not add a system case that restates it — a system case proves only the effect the fuzzer
cannot observe, and the task names the fuzz operation that covers the rest.

Do not run the E2E suite (`script/e2e.sh` or `pytest test/e2e`), the configuration suite (`script/configuration.sh`) or
`devenv test` in this change, with no exceptions.

## 1. Pre-flight and change metadata

- [x] 1.1 Confirm `specify-logging` has archived and removed its requirement from `instance/state`, otherwise stop and
      coordinate the sequencing from design D8. Verify: `openspec show instance/state --type spec --json --no-scenarios`
      lists the five state requirements and no "Identity creation is reported" requirement, and `openspec list --json`
      no longer lists `specify-logging`.
- [x] 1.2 Confirm `openspec/changes/specify-state/.openspec.yaml` declares `retire_capabilities: true`; if it does not,
      stop and fix the scaffold before writing the delta. Verify: `openspec validate specify-state --strict` passes.

## 2. Move the proof to the system suite

- [x] 2.1 Create `test/system/state.py` by moving the state cases from `test/configuration/instance_state.py`, dropping
      the creation-event and redaction cases that `specify-logging` owns. Resolve `/health` and the current-user route
      through `sut.index()` and representation links; use the `sut` fixture directly because each case starts, stops or
      restarts its own container. Add `epic("State")`, `feature("REQ-STATE-00n")` and `story("SCN-STATE-00n-x")` to
      every case. Verify: `pytest test/system/state.py` collects the module and each case reports its expected outcome;
      the new privacy case fails for the expected reason, while an already-enforced case passes and records that the
      specification was the gap.
- [x] 2.2 Add the scenario cases the spec now declares: the split model cases (`SCN-STATE-001-A/B`, `SCN-STATE-002-A/B`,
      `SCN-STATE-003-A/B`), malformed pepper and signing secret seeds (`SCN-STATE-004-E`), fixed `state.yaml` path
      (`SCN-STATE-007-E`), a 0644 restored state repaired (`SCN-STATE-008-B`), a 0600 restored state accepted
      (`SCN-STATE-008-C`), a 0600 creation mode (`SCN-STATE-008-A`), and a state that cannot be made private refused
      (`SCN-STATE-008-D`). For an already-enforced behaviour, confirm the case passes against current code and record
      that the gap was the specification, not the server. Verify: the 0644 repair case fails before task 3.2 and passes
      after it; every other new case passes or is proven by a Rust test.
- [x] 2.3 Delete `test/configuration/instance_state.py` and `test/configuration/test_scenarios.py`; drop the
      `state_scenario` marker from `pytest.ini` and update `test/configuration/test_harness.py`'s "instance state spec"
      wording. Verify: `grep -r "instance_state.py\|test_scenarios.py" test/configuration` returns nothing and the
      directory no longer holds a state module.

## 3. Repair-or-refuse privacy for a restored state

- [x] 3.1 Write the failing tests first: a unit test of the private-file predicate with a 0600 file, a 0644 file, a
      directory and a file owner that differs from the directory owner; and a system case that starts with a valid
      `state.yaml` at mode 0644, expects startup success, and asserts the mode becomes `0600`. Verify: the predicate
      test fails because no predicate exists, and the system case fails because the mode stays `0644`.
- [x] 3.2 Implement the check in `FileInstanceStateStore::load` on the opened descriptor: require a regular file owned
      by the working-directory owner, and when `mode & 0o077 != 0`, call `file.set_permissions(0600)` before reading.
      Add `InstanceStateStoreError::Insecure` and map it to a `LoadStateError::StateInsecure` whose message names
      `state.yaml` and the `chmod`/`chown` remedy, never a secret. Verify: the 0644 case starts with mode `0600`, the
      0600 case starts unchanged, and the foreign-owner predicate and unrepairable path fail startup.
- [x] 3.3 Restart the repaired instance and assert the state is reused unchanged with mode `0600`. Verify: the second
      start is healthy, the secrets are unchanged, and no second repair is needed.

## 4. Testable datastore presence

- [x] 4.1 Write the failing application test first: a `DatastorePresence` mock returns an unreadable error and
      `LoadState` must report `LoadStateError::DatastoreUnreadable` without touching the store or writing a state. Add
      the outbound port trait in `src/lib/domain/port/` and thread it through the constructor and tests. Verify: the
      test fails before the mapping is implemented.
- [x] 4.2 Add the file adapter that maps `NotFound` to `Ok(false)` and every other `try_exists` failure to `Err`, wire
      it in `src/lib/app.rs`, and map the port error in `LoadState`. Verify:
      `cargo test load_state_datastore_probe_failure_is_reported` passes and the existing `LoadState` tests still pass.

## 5. Atomic write and decode-level proofs

- [x] 5.1 Write the failing adapter test first: after a valid `state.yaml` exists, a write whose final rename fails must
      leave the previous state loadable and no partial `state.yaml` behind. Extract `write_atomic_with` with the rename
      passed in, keep `write_atomic` calling it with `std::fs::rename`, and make the test inject a failing rename.
      Verify: the test fails before the extraction and passes after it.
- [x] 5.2 Extend the adapter tests for the new scenarios: unsupported version 0 and 2, unknown field, oversized
      document, truncated document. Verify: each test names its failure and the document stays unchanged.

## 6. Make the proof whole

- [x] 6.1 Add the three Allure labels to every `test/system/state.py` case, one `story` per scenario. Verify: the Allure
      report lists the State epic, and `grep` finds no state case without `epic`, `feature` and `story`.
- [x] 6.2 Remove the logging helpers and cases from the moved module: `_creation_events`, `_secret_events`, the
      "creation is reported" and "seeded secret reports no generation" cases, and the secret-sink case. Verify: no state
      test references `secret_initialized`, `system_object`, or `REQ-LOGGING-*`; the logging suite owns them.

## 7. Specification and documentation alignment

- [x] 7.1 Update `docs/development/TechnicalDesign.md` § 10.2 to point at `openspec/specs/state/spec.md`, state the
      fail-closed privacy rule, and record that the configuration ordering is design, not a state requirement. Verify:
      every cited requirement ID exists in the change's `specs/state/spec.md`, and the section no longer names
      `instance/state` as the owner of configuration ordering.
- [x] 7.2 Update `docs/development/TechnicalDesign.md` § 5: `TEST-053` drops the state module and its gate, keeping any
      configuration-module changes `specify-configuration` has already made; `TEST-054` states the system suite's
      Allure-label discipline in place of a scenario map or meta-test; and `TEST-055` no longer lists a malformed seed
      as a configuration input. Update the configuration-test and system-test narratives to name `test/system/state.py`
      and the removed configuration cases. Verify: the module lists match `ls test/configuration` and `ls test/system`,
      and the `TEST-054` wording names the Allure labels rather than a meta-test.
- [x] 7.3 Update `docs/development/Overview.md` § Startup, `docs/development/Glossary.md` (instance state, site pepper,
      token signing secret, instance root), `README.md` (backup and restore at mode `0600`, chown to the server
      account), `pytest.ini` and `test/configuration/test_harness.py`. Verify: `markdownlint-cli2` and
      `prettier     --check` pass; the glossary terms are used with the same meaning in the spec.
- [x] 7.4 Coordinate with the owners of `specify-configuration` and `specify-logging`: confirm no surviving statement
      assigns the state gate or the configuration ordering to `instance/state`, and that the logging delta no longer
      targets `instance/state`. Verify:
      `grep -rn "instance/state" openspec/changes/specify-configuration     openspec/changes/specify-logging` returns
      only historical/migration notes, or the owners have updated them.

## 8. Integration gates

- [x] 8.1 Run `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`,
      `cargo clippy --all-targets -- -D warnings`, `cargo test` and the coverage command from `AGENTS.md`. Verify: all
      pass at or above the 80% gate.
- [x] 8.2 Run `pytest test/system`. Verify: the system suite passes.
- [x] 8.3 Run `markdownlint-cli2 '**/*.md'`, `prettier --check '**/*.md'` and
      `openspec validate specify-state --strict`. Verify: all gates pass and the spec parses with the eight
      requirements.
- [x] 8.4 Run `cargo deny check` to confirm the change adds no dependency edge. Verify: the dependency gate is clean.

## Workflow follow-up

- Archive `specify-state` with `retire_capabilities: true` after `specify-logging` is archived and the configuration
  owner has reconciled its stale statements.
- Verify the archived result: `openspec/specs/state/spec.md` carries the eight `REQ-STATE-*` requirements,
  `openspec/specs/instance/state/` is gone, and `openspec list --specs` reports `state` once.
