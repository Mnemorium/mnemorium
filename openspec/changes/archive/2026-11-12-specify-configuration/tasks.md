# Tasks

Schemathesis already proves a requirement's declarative contract against the served document: every declared status,
media type, response header, envelope shape and `_links`/`_embedded` field belongs to the API fuzz test
(`script/api_fuzz.sh`). Do not add a system case that restates it — a system case proves only the effect the fuzzer
cannot observe, and the task names the fuzz operation that covers the rest.

Do not run the E2E suite (`script/e2e.sh` or `pytest test/e2e`), the configuration suite (`script/configuration.sh`) or
`devenv test` in this change, with no exceptions.

No production change is expected. A moved case must pass unchanged; a new case that fails proves either a missing
observable or a wrong requirement, and stops the task rather than prompting a code change.

The moved proof lives in the system suite, and the remaining test edits are verified by collection, not by re-running a
suite this change does not own.

## 1. Prove the mechanism in the system suite

- [x] 1.1 Create `test/system/configuration.py` with the mechanism cases from `test/configuration/config_layers.py`,
      adding the defaults case the harness held: defaults when nothing is supplied with the generated file
      (`SCN-CONFIGURATION-001-A`, `SCN-CONFIGURATION-003-A`), file over default (`SCN-CONFIGURATION-001-B`), environment
      over file (`SCN-CONFIGURATION-001-C`), a partial file (`SCN-CONFIGURATION-001-D`), a file never rewritten
      (`SCN-CONFIGURATION-003-B`, `SCN-CONFIGURATION-003-C`), the applied-override report (`SCN-CONFIGURATION-007-A` to
      `-C`), and a runtime edit until restart (`SCN-CONFIGURATION-008-A`, `SCN-CONFIGURATION-008-B`). Every case carries
      `epic("Configuration")`, `feature("REQ-CONFIGURATION-00n")` and `story("SCN-CONFIGURATION-00n-x")`. Verify:
      `./script/system.sh` passes and the Allure report lists the cases under the Configuration epic.

- [x] 1.2 Delete `test/configuration/config_layers.py` so no mechanism case reports twice. Verify:
      `grep -r "config_layers" test/` returns nothing and `python3 -m pytest test/configuration --collect-only -q`
      collects the remaining modules without error.

- [x] 1.3 Drop the generated-file assertion from `test/configuration/test_harness.py`, keeping its three-file start-up
      smoke. Verify: the module no longer asserts `exists(CONFIG_PATH)` and its import list no longer names
      `CONFIG_PATH`, and `python3 -m pytest test/configuration --collect-only -q` still succeeds.

## 2. Prove the unreadable document in the adapter

- [x] 2.1 Add `configuration_source_unreadable_path_is_rejected` to the test module of
      `src/lib/infrastructure/outbound/file_system/configuration_source.rs`, using a directory in place of
      `config.yaml`, asserting the error names the path and carries no document content (`REQ-CONFIGURATION-004` /
      `SCN-CONFIGURATION-004-D`), labelled with `#[allure_test]`, `epic("Configuration")` and the requirement and
      scenario identifiers. Verify: `cargo test configuration_source_unreadable_path_is_rejected` passes; if it fails,
      stop and revise `REQ-CONFIGURATION-004` or the observation before continuing.

## 3. Check the retirement before archiving

- [x] 3.1 Confirm the retirement delta names exactly the five requirements the main spec holds. Verify:
      `openspec show configuration/loading --type spec --json --no-scenarios` lists five requirement names, each appears
      under `## REMOVED Requirements` in `specs/configuration/loading/spec.md`, and
      `openspec validate specify-configuration` passes.

## 4. Run the gates

- [x] 4.1 Run `script/rule_enforcer.sh check` and the markdown gates (`markdownlint-cli2 '**/*.md'` and
      `prettier --check`); verify both pass.

## Workflow follow-up

- Archive the change once the project's review requirements are satisfied. `.openspec.yaml` declares
  `retire_capabilities: true`, so `openspec archive specify-configuration` creates
  `openspec/specs/configuration/spec.md` and deletes the emptied `configuration/loading` spec.
- Verify the archived result: `openspec/specs/configuration/loading/` is gone and `openspec/specs/configuration/spec.md`
  carries the eight `REQ-CONFIGURATION-*` requirements.
- Documentation alignment is deferred to a later change: `TEST-053` still lists `config_layers.py`, § 10 still restates
  the contract, and the glossary does not yet carry the configuration vocabulary.
- A later change that moves more configuration cases into the system suite deletes the replaced case in the same change.
