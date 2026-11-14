# Proposal

## Why

Logging is implemented end to end — a subscriber with a rotating file sink and a standard-output sink, a closed
security-event catalog, and 197 emission sites across the application and infrastructure layers — but no capability spec
describes it. The contract exists only in `docs/development/TechnicalDesign.md` § 9, whose catalog table is parsed by
`check_obs_006`, and the events themselves are scattered across six capability specs and one in-flight change as prose:
eight of the thirty-two catalogued events are named by a capability, `instance/state` and both identification specs
describe events without naming them, and the in-flight `upload` capability names none at all. A reader cannot tell which
capability owns an event, and the sinks — the file name, the rotation period, the pruning bound, which sink is coloured
— are specified nowhere.

## What Changes

- Add a `logging` capability that owns the whole logging contract: the two sinks and their rotation, the verbosity
  control and its non-suppressible floor, the layer that may emit, the log-once rule, log content, redaction, the five
  severities, the closed event catalog, and the trigger, level and fields of every catalogued event.
- Move every event requirement out of the capabilities that only happened to state it: `authentication`
  (`REQ-AUTHENTICATION-017`), `authorization` (`REQ-AUTHORIZATION-027` and `028`, retired and re-added without the
  event), `instance/state`, `library/media-ingest`, `library/music-identification` and `library/video-identification`.
  Each keeps the requirement whose subject is its own state; only the sentence whose subject is a log line moves.
- Own the meaning of the four `logging` settings — `level`, `rotation`, `max_files` and `ansi` — while `configuration`
  keeps how any setting is resolved from the defaults, the file and the environment. A refused value is refused at
  startup, naming the setting and never the value.
- Make the catalog normative in the specification. `TechnicalDesign.md` § 9.6 becomes a citation of the capability and
  `check_obs_006` parses the spec's table instead, so the catalog has one home; `OBS-008` and `check_obs_008` are
  deliberately untouched.
- Prove the scenarios black box. A new `test/system/logging_events.py` feature module carries every logging scenario,
  and the log assertions living in `test/system/authentication.py`, `test/configuration/instance_state.py` and
  `test/configuration/logging_settings.py` move into it; the two configuration modules stop asserting on log output, and
  `logging_settings.py` is deleted because it proved logging and nothing else.
- No behaviour change. Every requirement already holds in the server; the witnesses are relocated, not rewritten.

Out of scope:

- `OBS-008` — the external-parser trust boundary stays in `TechnicalDesign.md` § 9 with its rule and its check.
- Any log destination this server does not own: the container runtime's log collection, log shipping and retention.
- A setting's default, range or enum spelling; those stay in the domain constructors, `default_config.yaml` and the
  tests that own them.

## Capabilities

### New Capabilities

- `logging`: what reaches a sink and what must not — the two log sinks and their rotation, the verbosity control that
  cannot suppress a security event, the layer that may emit and the one that may not, log content and redaction, the
  five severities, and the closed catalog of security events with the trigger, level and fields of each.

### Modified Capabilities

- `authentication`: `REQ-AUTHENTICATION-017` and its five scenarios are removed; the catalog owns the events and their
  triggers. The stored-hash and redacted-`Debug` clauses of `REQ-AUTHENTICATION-006` stay.
- `authorization`: `REQ-AUTHORIZATION-027` and `REQ-AUTHORIZATION-028` are retired and re-added as
  `REQ-AUTHORIZATION-038` and `039` without their `authz_failed` clauses; the uniform, concealed denial and the listing
  filter stay. A renumber is required because the schema does not let a modified requirement drop a scenario.
- `instance/state`: the creation and secret-initialization event requirement is removed; identity persistence, privacy
  and atomic write stay.
- `library/media-ingest`: "Ingest reports aggregate counts only" is removed; what each count means stays.
- `library/music-identification`: "Identification events carry no content" is removed.
- `library/video-identification`: "Identification outcomes are observable without exposing content" is removed.

## Impact

- **Specs**: one capability is created and six change; `upload` gains its events without being modified.
- **Rules and docs**: `script/rule_enforcer.sh`'s `check_obs_006` parses the new catalog table; `TechnicalDesign.md` § 9
  and § 9.6 cite the capability; `docs/Permissions.md` points its `authz_failed` vocabulary at the catalog;
  `docs/development/Glossary.md` gains the terms the capability introduces.
- **Tests**: a new `test/system/logging_events.py`; log assertions move out of the authentication system module and the
  two configuration modules; `test/configuration/logging_settings.py` is deleted; Rust `#[traced_test]` coverage stays
  where the container cannot provoke the outcome.
- **API**: none. No route, representation or status changes.
- **Code**: none required for behaviour; the change re-homes the catalog, not the emissions.
