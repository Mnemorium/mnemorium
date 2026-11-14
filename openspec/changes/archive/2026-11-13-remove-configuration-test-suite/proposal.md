# Proposal

## Why

The configuration test suite is the last black-box suite that duplicates the system suite's remit. Every per-setting
module except `library_settings.py` has already been retired into `test/system/`, and the suite's `harness.py` is now
the system suite's own plugin: `pytest.ini` puts `test/configuration` on `pythonpath` and 20 system modules import
`from harness`. Keeping the suite keeps a second CI job, a second runner script, a separate container prefix and a set
of test rules (`TEST-053`, `TEST-055`–`TEST-059`) whose subjects have moved. The remaining `library_settings.py` cases
either duplicate a system case or belong to **Metadata enrichment** and media ingest.

## What Changes

- Move `test/configuration/harness.py` to `test/util/harness.py`; set `pythonpath = test/util` and drop
  `test/configuration` from `testpaths` in `pytest.ini`.
- Move the provider settings cases from `test/configuration/library_settings.py` into
  `test/system/metadata_enrichment.py`. Add `REQ-METADATA-ENRICHMENT-029` for the behaviour those cases headline: a
  valid provider list is accepted at startup without contacting a provider, and an adapter makes no request until a read
  uses it. Prove the adapter half with a Rust `wiremock` case. Add `SCN-METADATA-ENRICHMENT-016-D` for the clean-start
  key case, which the existing refusal scenarios do not cover.
- Move the **Library root** startup cases into the `test/system/media_ingest.py` module that `finish-spec-refactor`
  creates, and delete the replaced configuration cases in the same change.
- Delete `test/configuration/test_harness.py`: its empty-configuration smoke is covered by
  `test/system/configuration.py` and `test/system/state.py`.
- Delete `test/configuration/`, `script/configuration.sh`, the `test:configuration` devenv task and the “Run
  configuration tests” CI step; retire the `test-config` pull-request scope and its path-table row.
- Merge § Configuration test into § System test in `TechnicalDesign.md`; rehome or retire `TEST-053` and
  `TEST-055`–`TEST-059`; drop “the configuration suite” from the task rule in `openspec/config.yaml`.
- **BREAKING**: none. No route, payload, status, schema, migration or runtime behaviour changes.

## Capabilities

### New Capabilities

None. This is a test-infrastructure change.

### Modified Capabilities

- `metadata-enrichment`: adds `REQ-METADATA-ENRICHMENT-029`, a valid provider list is accepted at startup without a
  provider request, and `SCN-METADATA-ENRICHMENT-016-D`, a clean start writes no key.

## Impact

- **Tests**: `test/configuration/` is removed; `test/util/harness.py` is added; `test/system/metadata_enrichment.py` is
  extended; the **Library root** startup cases move to the `test/system/media_ingest.py` module created by
  `finish-spec-refactor`.
- **Tooling**: `pytest.ini`, `script/configuration.sh`, `devenv.nix`, `.github/workflows/ci.yml`, and the `test-config`
  scope in the pull-request gatekeeper.
- **Docs**: `docs/development/TechnicalDesign.md` § 5 (Configuration test, System test, test command table) and
  `openspec/config.yaml` (tasks rule).
- **Source**: one test-only Rust case under `src/lib/infrastructure/outbound/reqwest/`; no production change.
- **Sequencing**: lands after `finish-spec-refactor`, which creates the `media-ingest` capability and its system module.
  The startup wording in `REQ-MEDIA-INGEST-002` remains that change's responsibility.
