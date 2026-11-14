# Design

## Context

See [proposal.md](proposal.md) — Why. The suite being removed is already hollow: `asset_settings.py`,
`logging_settings.py`, `security_settings.py`, `instance_state.py`, `config_layers.py` and `test_scenarios.py` are gone,
and only `library_settings.py` and `test_harness.py` remain beside `harness.py`.

`harness.py` is not configuration-specific. `pytest.ini` sets `pythonpath = test/configuration` and
`addopts = -p harness`, and 20 system modules import `from harness` for `Sut`, `link_of`, `register`, `begin_upload`,
`start_instance` and the rest. The system suite's `conftest.py` also imports it for the `server` and `root_token`
fixtures. Removing the directory without moving the module breaks the system suite at import time.

`finish-spec-refactor` is already in flight. It creates the top-level `media-ingest` capability, retires
`library/media-ingest`, and plans `test/system/media_ingest.py` with the startup window. This change therefore does not
re-author that capability; it consumes the system module and removes the replaced configuration cases.

## Goals / Non-Goals

**Goals:**

- One black-box harness for the system suite, at a neutral path.
- No configuration-suite case left without a system home.
- One CI job, one runner script, one container prefix and one set of test rules.
- `REQ-METADATA-ENRICHMENT-029` witnessed at the level that can observe it.

**Non-Goals:**

- Re-authoring `media-ingest` or changing its startup wording; `finish-spec-refactor` owns that delta.
- Changing any production behaviour, route, payload, status, schema or migration.
- Rewriting the system suite's existing cases beyond the moved provider and startup cases.

## Decisions

**The harness moves to `test/util/harness.py`, not into `test/system/`.** `pytest.ini` becomes `pythonpath = test/util`
and keeps `addopts = -p harness`; `testpaths` drops `test/configuration`. The module matches none of the `python_files`
patterns, so it is not collected as a test. Keeping it under `test/system/` would force an exclusion in
`test/system/conftest.py`'s collector, which currently collects every unprefixed module in that directory. Alternative
considered: fold the harness into `test/system/conftest.py`; rejected because the `sut` and `image` fixtures and the
shared helpers are useful outside that one suite and the module is already a plugin, not a conftest.

**The provider cases move to `test/system/metadata_enrichment.py`; the startup cases move to
`test/system/media_ingest.py`.** The provider cases become labelled system cases: the empty list, the configured list,
and the key absent from both sinks. The startup cases become labelled `media_ingest` cases using
`sut.start(create_media=False, ...)`. `test_harness.py` is deleted outright: its health, state and datastore assertions
are already made by `test/system/configuration.py` and `test/system/state.py`.

**`REQ-METADATA-ENRICHMENT-029` is proved twice, at two levels.** The container can observe that a configured list
starts; it cannot observe the absence of a request. The Rust case builds `TmdbVideoMetadataProvider::with_endpoints`
against `wiremock`, asserts `received_requests()` is empty, then calls `search` and asserts one request. The requirement
states the behaviour once; the two witnesses prove the two observable halves. Alternative considered: inject a provider
builder into `RuntimeLibraryUseCaseFactory` and assert it is not called at startup; rejected as a production seam for a
property the adapter test plus the composition code already establish.

**The stale and duplicate cases go, not into a spec.** `test_a_wait_beyond_one_day_is_rejected_before_startup` is a
domain-invalid value rejection, which `TEST-055` assigns to Rust and
`load_configuration_startup_rejects_an_oversized_wait` already proves; the 86400 bound is deliberately not a spec value.
`test_the_configuration_file_is_never_rewritten` duplicates `test/system/configuration.py`'s existing-file case. Neither
is moved.

**The test rules are rehomed by ID, not renumbered.** `TEST-053` (one module per configuration section) and `TEST-055`
(the configuration suite asserts valid values only) are retired because their subject is the suite being removed.
`TEST-056` (sleep bound), `TEST-057` (operator prose), `TEST-058` (container isolation) and `TEST-059` (refusal effect)
still describe the black-box harness and move under § System test. The `test-config` pull-request scope and its
path-table row are removed. Alternative considered: keep the IDs under a new “Black-box test” heading; rejected because
the system test section already owns the harness and a third heading would split one doctrine.

## Risks / Trade-offs

- **Deleting the harness before moving it breaks 20 system modules** → Move `harness.py` first, update `pytest.ini`, and
  run `pytest test/system --collect-only` before deleting `test/configuration/`.
- **The startup setting-name assertion has no spec clause** → `finish-spec-refactor`'s `REQ-MEDIA-INGEST-002` currently
  says only “naming the absent root”. When the startup cases move, either keep the assertion only if that delta names
  the setting, or drop the assertion and record the wording gap on `finish-spec-refactor`. Do not add a second
  `media-ingest` delta here.
- **The moved provider cases need a configured container** → `test/system/metadata_enrichment.py` currently uses the
  `server` fixture; the new cases use `sut` and `start_instance` directly, so the module's provider-less docstring must
  be updated.
- **CI loses the configuration job** → The same cases are in the system job before the suite is deleted; verify the
  system suite is green with the moved cases, then remove the job.
- **Retiring the `test-config` scope rejects a PR that still uses it** → Remove it from the gatekeeper's `scopes` list
  and the path table in the same change; historical PRs are unaffected.
- **`remove-e2e-test-suite` edits the same files** → `pytest.ini` loses `test/e2e` there and `test/configuration` here,
  and `script/configuration.sh` is deleted here, so that change's build-hint edit to it is dropped if this lands first.
  Sequence the two and reconcile `pytest.ini` once, so the final `testpaths` is `test/system` and the final `pythonpath`
  is `test/util`.

## Migration Plan

1. Move `harness.py` and update `pytest.ini`; verify the system suite collects and passes.
2. Move the provider cases and add the Rust witness; verify `test/system/metadata_enrichment.py` and the Rust test.
3. Move the startup cases into the `test/system/media_ingest.py` created by `finish-spec-refactor`; delete the replaced
   configuration cases.
4. Delete `test/configuration/`, `script/configuration.sh`, the devenv task and the CI step; retire the `test-config`
   scope.
5. Merge the documentation and the task rule; run `script/rule_enforcer.sh check`, the Markdown gates and `devenv test`.
