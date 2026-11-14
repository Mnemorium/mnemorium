# Tasks

## 1. Move the shared harness

- [x] 1.1 Move `test/configuration/harness.py` to `test/util/harness.py`; set `pythonpath = test/util` and drop
      `test/configuration` from `testpaths` in `pytest.ini`; rewrite the module docstring and the `pytest.ini` header
      comment so they name the black-box suites, not the configuration suite. Verify:
      `pytest test/system --collect-only -q` and `pytest test/configuration --collect-only -q` both collect, and
      `grep -r "test/configuration" pytest.ini` finds only the removed `testpaths` entry.
- [x] 1.2 Run the system suite against the moved harness before deleting anything. Verify: `script/system.sh` passes, or
      `pytest test/system -q` passes when an image is already built.
- [x] 1.3 Update the `harness.py` rule citations (`TEST-056`, `TEST-057`, `TEST-058`) to the rehomed system-test rules
      from group 5. Verify: every `TEST-*` the module names exists in `docs/development/TechnicalDesign.md`.

## 2. Move the provider cases

- [x] 2.1 Add the empty-list, configured-list and key-not-logged cases to `test/system/metadata_enrichment.py`; use
      `sut` and `start_instance` directly and label the configured-list case `epic("Metadata enrichment")`,
      `feature("REQ-METADATA-ENRICHMENT-029")`, `story("SCN-METADATA-ENRICHMENT-029-A")`, and the key case
      `feature("REQ-METADATA-ENRICHMENT-016")`, `story("SCN-METADATA-ENRICHMENT-016-D")`. Verify: the module passes, and
      the sentinel key is absent from both `logs()` and `log_text()`.
- [x] 2.2 Add the Rust adapter witness in `src/lib/infrastructure/outbound/reqwest/tmdb.rs`: build
      `TmdbVideoMetadataProvider::with_endpoints` against `wiremock`, assert `received_requests()` is empty, call
      `search`, and assert one request; label it `#[allure_test]`, `epic("Metadata enrichment")`,
      `feature("REQ-METADATA-ENRICHMENT-029")`, `story("SCN-METADATA-ENRICHMENT-029-A")`. Verify:
      `cargo test tmdb_adapter_makes_no_request` passes.
- [x] 2.3 Update the `test/system/metadata_enrichment.py` module docstring: it no longer proves only a provider-less
      container, because the moved cases configure providers. Verify: the docstring names the configured-provider cases
      and still points provider-backed mapping to the Rust tests.

## 3. Move the startup cases

- [x] 3.1 Confirm `finish-spec-refactor` has landed `test/system/media_ingest.py` and that it covers the root present,
      absent and late. If it has not, stop and land that change first. Verify: the module exists and its startup cases
      collect.
- [x] 3.2 Confirm the startup cases in `test/system/media_ingest.py` cover the required-absent, optional-absent and
      present-root behaviours; add or extend a case there if one is missing, using `sut.start(create_media=False, ...)`,
      and label it `epic("Media ingest")`, `feature("REQ-MEDIA-INGEST-002")` and the matching `SCN-MEDIA-INGEST-002-*`.
      Verify: the module passes and no configuration case is still the only witness.
- [x] 3.3 Reconcile the setting-name assertion: keep `"fail_without_media_root" in failure.output` only if
      `finish-spec-refactor`'s `REQ-MEDIA-INGEST-002` names the setting; otherwise drop the assertion and record the
      wording gap on that change. Verify: the moved case and the spec agree.
- [x] 3.4 Delete the moved cases from `test/configuration/library_settings.py`; delete the file once empty. Verify:
      `grep -r "library_settings" test/` returns nothing.

## 4. Delete the suite and its tooling

- [x] 4.1 Delete `test/configuration/test_harness.py` and the now-empty `test/configuration/` directory. Verify:
      `test/configuration/` is absent and `pytest test/system --collect-only -q` still collects.
- [x] 4.2 Delete `script/configuration.sh`; remove `tasks."test:configuration"` from `devenv.nix`; remove the “Run
      configuration tests” step from `.github/workflows/ci.yml`. Verify:
      `grep -r "configuration.sh\|test:configuration" script devenv.nix .github` returns nothing, and
      `shellcheck script/*.sh` and `yamllint .github/workflows/ci.yml` pass.
- [x] 4.3 Remove `test-config` from the gatekeeper's `scopes` list and its row from the test command table in
      `TechnicalDesign.md`. Verify: `grep -r "test-config" .github docs` returns nothing.
- [x] 4.4 Remove “the configuration suite” from the test-level rule in `openspec/config.yaml`. Verify:
      `openspec validate remove-configuration-test-suite` passes and the rule lists only the surviving levels.
- [x] 4.5 Reconcile `pytest.ini` with `remove-e2e-test-suite`: the final `testpaths` is `test/system` and the final
      `pythonpath` is `test/util`; that change's build-hint edit to `script/configuration.sh` is dropped because this
      change deletes the file. Verify: `pytest --collect-only -q` collects only the system suite.

## 5. Documentation

- [x] 5.1 Merge § Configuration test into § System test: keep the harness rules as `TEST-056`–`TEST-059`, retire
      `TEST-053` and `TEST-055`, and update the test command table. Verify: `markdownlint-cli2 '**/*.md'` and
      `prettier --check` pass, and no link targets `#configuration-test`.
- [x] 5.2 Update `test/system/conftest.py` and `test/system/configuration.py` docstrings that still mention the
      configuration suite. Verify: `grep -r "configuration suite" test/` returns nothing.

## 6. Integration check

- [ ] 6.1 Run `script/rule_enforcer.sh check`, the Rust witness, `script/system.sh` and the Markdown gates. Verify: all
      pass, and the Allure report lists the moved cases under the Metadata enrichment and Media ingest epics.
- [x] 6.2 Confirm the old suite leaves no trace. Verify:
      `grep -r "test/configuration\|configuration.sh\|test-config" . --include=*.py --include=*.sh --include=*.yml --include=*.nix --include=*.md --include=*.ini`
      returns nothing outside `openspec/changes/archive/`.

## Workflow follow-up

- Archive the change once the project's review requirements are satisfied.
- Verify the archived result: `openspec/specs/metadata-enrichment/spec.md` carries `REQ-METADATA-ENRICHMENT-029`, and
  `test/configuration/` is absent.
