# Proposal

## Why

The configuration contract has outgrown its capability. `configuration/loading` is named for one verb and states five
requirements, while the general contract lives only in `TechnicalDesign.md` § 10: the file's fixed location and refusal
semantics, what each layer may carry, the single refusal surface, immutability, and the boundary that keeps secrets and
locations out of configuration.

A capability named for loading invites the opposite of what configuration needs: a requirement per setting. Stating the
mechanism once, under `configuration`, keeps the specification at the level where one setting is indistinguishable from
another, and leaves each setting's meaning with the capability that consumes it.

## What Changes

- **One configuration capability.** `configuration/loading` becomes `configuration`, whose delta states eight
  requirements: layered resolution and totality, the file's fixed location and sparse document, its generation, its
  refusal semantics, the environment's naming, discard and capability rules, the single refusal contract, the
  applied-override report, and immutability. Its Purpose keeps the instance identity's secrets and the fixed locations
  outside the capability.
- **No setting is specified.** No requirement names a setting path, a default, a range or an enum spelling; those stay
  in the domain constructors, `default_config.yaml` and the tests that own them.
- **The loading capability is retired.** Its five requirements are absorbed into `configuration`, with the migration
  recorded per requirement. The change declares `retire_capabilities: true` so the archive deletes the emptied spec
  instead of leaving a requirement-less file.
- **The mechanism tests move to the system suite.** The six cases of `test/configuration/config_layers.py` are already
  scenario-shaped: each names one general behaviour and probes it through a cheap observable. They move to a new
  `test/system/configuration.py`, labelled `epic`/`feature`/`story` against the new requirement and scenario
  identifiers, and `config_layers.py` is deleted in the same change so one failure never reports twice.
- **One duplicate case goes.** `test/configuration/test_harness.py` drops its generated-file assertion, which the system
  suite takes over. The module keeps its three-file start-up smoke.
- **No behaviour change.** Every requirement already holds in the server; the Rust and suite witnesses are cited, not
  rewritten.

Out of scope:

- Documentation alignment: `TEST-053`, `TechnicalDesign.md` § 10 and the glossary are deferred to a later change, so the
  design document will lag this change until then.

- A requirement per setting, default, range or enum value.
- `library/music-identification`'s missing provider-list and invalid-configuration requirements; under the seam this
  change establishes, that gap belongs to the identification capability.
- The `library.metadata.video.providers` cases of `test/configuration/library_settings.py`: they prove identification
  and ingest scenarios, not configuration's.
- A scenario gate for `configuration`; the system-proven capabilities (`authentication`, `api`,
  `identity/authorization`) are ungated, and this one follows them. `instance/state` keeps its gate because its
  scenarios are proven by the configuration suite.
- The README's operator-configuration link, which needs an operator document rather than a specification.

## Capabilities

### New Capabilities

- `configuration`: how a complete configuration is resolved from the built-in defaults, the configuration file and the
  environment; what each layer may carry; how a rejected input fails; and what is deliberately not a setting.

### Modified Capabilities

- `configuration/loading`: retired. Its five requirements are removed and absorbed into `configuration`, with the reason
  and the migration recorded per requirement.

## Impact

- **Specs**: `openspec/specs/configuration/spec.md` is created; `openspec/specs/configuration/loading/spec.md` is
  retired.
- **Tests**: `test/system/configuration.py` is created; `test/configuration/config_layers.py` is deleted;
  `test/configuration/test_harness.py` loses one case.
- **Deferred**: `docs/development/TechnicalDesign.md` § 5 (`TEST-053`) and § 10, and `docs/development/Glossary.md`,
  keep their current text until the documentation change.
- **Unchanged**: Rust source, the REST API and its OpenAPI document, migrations, dependencies, the container image and
  the command line.
