# Design

## Context

See `proposal.md` — Why. The current state that shapes this change:

- `openspec/specs/configuration/loading/spec.md` holds five requirements without `REQ-*`/`SCN-*` identifiers and covers
  only resolution: layers, totality, file generation, environment overrides and immutability.
- The rest of the general contract lives in `TechnicalDesign.md` § 10: the environment naming rule, the file's refusal
  semantics, the boundary between configuration and the instance identity, and the two fixed locations; § 10.1 holds the
  startup-only setting registry.
- The implementation is unchanged by this design: `domain/model/configuration.rs` (defaults and validation),
  `domain/model/partial_configuration.rs` (the sparse layer), `application/use_case/load_configuration.rs` (the merge
  and the applied-setting list), and the two adapters `infrastructure/outbound/file_system/configuration_source.rs` and
  `infrastructure/outbound/envy/configuration.rs`.
- Proof already exists at three levels: Rust adapter and use-case tests, `test/configuration/*` black-box cases, and
  `test/system/*` scenario-proof modules with Allure labels. `TEST-053`, `TEST-054` and `TEST-055` define the
  configuration suite's remit; the system-test section says a scenario's proof is a system test whenever the container
  can observe the outcome, and a replaced case is deleted in the same change.
- OpenSpec has a first-class retirement path: a `REMOVED`-only delta for a capability, together with
  `retire_capabilities: true` in the change's `.openspec.yaml`, makes `openspec archive` delete the emptied spec.

## Goals / Non-Goals

**Goals:**

- One capability file that states the configuration mechanism once, with stable identifiers.
- Every requirement witnessed at the lowest level that can observe it, and no requirement that only a parameter can
  express.
- No drift between the spec and the suite: the mechanism cases move to the suite that proves scenarios, in the same
  change.

**Non-Goals:**

- Changing behaviour, settings, defaults or validation.
- Specifying any individual setting, its default, range or enum spelling.
- Building an operator configuration document, a scenario gate, the music-identification provider requirement, or the
  documentation alignment (`TEST-053`, § 10, the glossary), which a later change owns.

## Decisions

### D1. One capability, `configuration/spec.md`

Alternatives considered: a layer split (`loading`, `file`, `environment`, `scope`) and a facet split (`loading`,
`sources`, `refusal`, `scope`). Both were rejected on the same ground: the file and the environment differ in only their
input rules, while the refusal surface, the report and the boundary are shared. Splitting by layer restates the shared
rules twice; splitting by facet scatters what happens to one document or one variable across three files. A single file
can state the shared rule once (`REQ-CONFIGURATION-006`) and still keep the layers distinct in their own requirements.
It also matches the shape of `authentication`, which states one mechanism in one file.

The consequence is accepted deliberately: reading "what happens when my `config.yaml` is wrong" means reading three
requirements (002, 003, 004) rather than one file, and the capability's Purpose states the mechanism so the boundary is
visible before the requirements are read.

### D2. The requirement set

Nine requirements, each one mechanism. The mapping from the retired capability:

| New                     | Owns                                                                       | Absorbs                                                           |
| ----------------------- | -------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| `REQ-CONFIGURATION-001` | layers, precedence, totality                                               | `Configuration resolves in layers`, `Every setting has a default` |
| `REQ-CONFIGURATION-002` | fixed file location, sparse document at any depth                          | file-location and sparseness sentences of § 10 (new)              |
| `REQ-CONFIGURATION-003` | generation, never rewrite, unwritable path                                 | `A default configuration file is written when absent`             |
| `REQ-CONFIGURATION-004` | document refusal: unreadable, malformed, unknown key, wrong type           | new (was an implementation note in § 10)                          |
| `REQ-CONFIGURATION-005` | environment naming, override, discard, capability limit                    | `The environment overrides the file for the settings it names`    |
| `REQ-CONFIGURATION-006` | one refusal surface: setting named, value never carried, no partial config | the malformed-variable scenario, generalized                      |
| `REQ-CONFIGURATION-007` | applied overrides reported, without values                                 | the report scenario of the retired requirement                    |
| `REQ-CONFIGURATION-008` | immutable for the process, restart applies                                 | `Resolved configuration is immutable for the process`             |

`REQ-CONFIGURATION-006` states the **Redaction** rule for the whole refusal surface: the setting is named, the value is
not, and no layer may serve a partial configuration.

Deliberately left outside the capability, with the owner that already states it:

- Provider list shape, index order and the invalid-provider refusal: `library/video-identification`.
- The library root's runtime behaviour and the startup wait: `library/media-ingest`.
- The instance identity's secrets and their ordering against configuration: `instance/state`. The capability's Purpose
  states that the identity's secrets and the fixed locations are outside it, without restating any identity scenario.
- Atomic write, file mode, YAML parser and log target: not observable, and they stay in § 10.
- The startup-only registry: § 10.1 remains the registry; the spec keeps only the observable rule in
  `REQ-CONFIGURATION-008`.

### D3. Retire `configuration/loading` through OpenSpec's own mechanism

The change carries a `REMOVED`-only delta at `specs/configuration/loading/spec.md` with a Reason and Migration per
requirement, and declares `retire_capabilities: true` in `.openspec.yaml`. On archive, `configuration/spec.md` is
created from the ADDED requirements and the emptied `configuration/loading/spec.md` is deleted.

Alternatives considered: deleting `openspec/specs/configuration/loading/spec.md` in an apply task (rejected: the removal
would be invisible in the change, and `openspec archive` refuses an unmarked `REMOVED`-only delta); leaving a
requirement-less spec behind (rejected: `openspec validate` rejects a spec with no requirements).

### D4. The mechanism cases move to the system suite

`test/configuration/config_layers.py` is already scenario-shaped: each case names one general behaviour and observes it
through a cheap probe (`expires_in`, one escape byte, a burst of logins). That is a system test without labels. The
cases move to `test/system/configuration.py`, one module per feature area, and `config_layers.py` is deleted in the same
change, per the system-test rule that a replaced case is deleted so one failure never reports twice.

The per-setting modules stay, because `TEST-053` and `TEST-055` give the configuration suite a different remit: prove
the effect of a valid value, one question per setting, and leave parsing and rejection to the Rust tests. Concretely:

- `test/configuration/asset_settings.py`, `logging_settings.py`, `library_settings.py` — unchanged.
- `test/configuration/test_harness.py` — keeps its three-file start-up smoke and drops the generated-file assertion,
  which the system suite's `SCN-CONFIGURATION-003-A` owns.
- `test/configuration/instance_state.py` and `test_scenarios.py` — unchanged. The state suite keeps its gate because its
  scenarios are proven there.

`TEST-053` is not amended here: the documentation surface is deferred, so the module list keeps naming
`config_layers.py` until then.

### D5. Witnesses per requirement

Every requirement has a witness at the lowest level that can observe it. Rust tests are cited, not rewritten, except
where the table marks a new case.

| Requirement         | Witness                                                                                                          |
| ------------------- | ---------------------------------------------------------------------------------------------------------------- |
| 001-A               | `test/system/configuration.py`, defaults case (from `test_harness.py`)                                           |
| 001-B, 001-C, 001-D | `test/system/configuration.py` (from `config_layers.py`)                                                         |
| 002-A, 002-B        | `test/system/configuration.py` (partial file); `load_configuration_file_layer_overrides_the_defaults`            |
| 002-C               | `environment_configuration_unknown_names_are_discarded`                                                          |
| 003-A               | `test/system/configuration.py` (generated file)                                                                  |
| 003-B, 003-C        | `test/system/configuration.py` (never rewritten)                                                                 |
| 003-D               | `configuration_source_unwritable_path_fails`                                                                     |
| 004-A               | `configuration_source_unknown_file_key_is_rejected`, `configuration_source_removed_storage_root_key_is_rejected` |
| 004-B               | `configuration_source_malformed_document_is_rejected`                                                            |
| 004-C               | `configuration_source_mistyped_file_value_is_rejected`                                                           |
| 004-D               | new adapter case: a directory in place of the file                                                               |
| 005-A               | `test/system/configuration.py` (environment over file)                                                           |
| 005-B               | `environment_configuration_unknown_names_are_discarded`                                                          |
| 005-C               | `environment_configuration_video_provider_settings_are_not_overridable`                                          |
| 006-A               | `load_configuration_*_names_the_setting` cases                                                                   |
| 006-B               | `environment_configuration_badly_typed_value_names_the_variable`                                                 |
| 006-C               | the sentinel assertions in `load_configuration.rs` and the `configuration_source.rs` adapter                     |
| 007-A, 007-B, 007-C | `test/system/configuration.py` (report cases)                                                                    |
| 008-A, 008-B        | `test/system/configuration.py` (restart case)                                                                    |

### D6. Labels, and no scenario gate

Each system case carries `epic("Configuration")`, `feature("REQ-CONFIGURATION-00n")` and
`story("SCN-CONFIGURATION-00n-x")`, matching the existing bare-identifier style (`REQ-AUTHORIZATION-031`).

No gate is added. The repo gates exactly one capability — `instance/state` — because its scenarios are proven by the
configuration suite, which has no label discipline of its own. `authentication`, `api` and `identity/authorization` have
system-suite proofs and no gate; `configuration` joins them. The alternative — a `CONFIGURATION_SCENARIOS` map mirroring
`STATE_SCENARIOS` in `test/system/` — remains available and needs no spec change, so it is deferred rather than
rejected.

Two key shapes coexist and never meet: Allure labels take bare identifiers; the `STATE_SCENARIOS` map takes
`"<Requirement title> / <Scenario title>"`. Any future gate for the system suite must choose one.

### D7. Documentation is deferred

The documentation surface is not part of this change. `TechnicalDesign.md` § 5 (`TEST-053` module list) and § 10, and
`docs/development/Glossary.md`, keep their current text, so the design document lags the suite until a later change
aligns it. The lag is deliberate: the documentation redesign is a separate concern, and this change should not decide
its shape.

## Risks / Trade-offs

- **Retirement depends on the archive marker.** → The marker is in `.openspec.yaml` and `openspec validate` passes. If
  the archive still refuses, the old spec holds only Purpose plus the five removed requirements, so the merge accounts
  for all content; deleting the file by hand and rerunning archive is the fallback.
- **The moved cases must keep their observables.** → The cases are moved verbatim, not rewritten; only decorators are
  added. A case that cannot be observed black box stays in Rust and is cited.
- **Moving precedence proof out of the configuration suite weakens its locality.** → This design records the split and
  the suite keeps its per-setting remit; the system suite is the only place that proves scenarios. The lagging
  `TEST-053` entry is settled by the deferred documentation change.
- **The spec is ungated and can drift.** → Accepted; the Allure report shows a missing label, and the gate can be added
  later without touching the spec.
- **`REQ-CONFIGURATION-004-D` has no witness today.** → The apply change adds one adapter case using a directory in
  place of the file; it needs no production change.
- **The naming rule is now a stated contract.** → "Single underscores" is operator-facing; changing it later is a
  breaking change, which is the correct place to record it.

## Migration Plan

1. Apply: add `test/system/configuration.py`, delete `test/configuration/config_layers.py`, trim the generated-file
   assertion from `test_harness.py`, and add the unreadable-file adapter case.
2. Archive: `openspec archive specify-configuration` creates `openspec/specs/configuration/spec.md` and deletes the
   retired `configuration/loading` spec.
3. Rollback: revert the change. There is no runtime state, no migration and no persistence to undo.

## Open Questions

None. The gate decision is recorded in D6 and is reversible without a spec change.
