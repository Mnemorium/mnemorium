# Proposal

## Why

`docs/development/TechnicalDesign.md` § 3 (API) mixes three kinds of statement: requirements a client observes, which
`openspec/specs/api/spec.md` already states normatively; source-shape conventions, which belong with the other Rust
rules; and non-normative HTTP reference (examples, the response-code table, HAL samples). Keeping the requirement half
in two places lets the rulebook and the specification drift, and it spends 35 mechanical checks enforcing declarations
rather than behaviour. The API suite drives all 66 published operations from the served document without naming a route,
so the broad contract can be proven there, and only the specifics that need an exact fixture stay in the system suite.

## What Changes

- Delete `## API` from `TechnicalDesign.md`: its 47-row rule table, the six declaration details, and the
  `Response codes`, `Error public payload`, `OpenAPI generation`, `HAL payload guidelines` and `Asset content retrieval`
  reference sections. Renumber the top-level sections that follow it.
- Move 31 rules into the Rust section as `STY-RUST-091`–`STY-RUST-121`, phrased as source conventions rather than
  contract statements. Twenty-seven keep a check under its new name; `106`, `107`, `108` and `116` stay review-only and
  appear in `script/rule_enforcer.sh uncovered`. Add `STY-RUST-122`: every endpoint handler extracts the query — its own
  `<Context>Query`, or the shared `NoParametersQuery` when it declares none — so `REQ-API-022` holds on every operation
  instead of only the query-declaring ones.
- Delete 16 rules that restate a requirement (`API-028`, `029`, `033`–`036`, `038`–`047`). Their proof moves to the API
  suite, a new document test, or an existing capability's system case.
- Add `REQ-API-024` — a declared constraint is enforced — to the `api` specification, absorbing `API-042`. No existing
  requirement or scenario text changes.
- Create `docs/development/HttpApi.md` for the non-normative HTTP reference, keep its anchors working, and link it from
  `AGENTS.md` and `mkdocs.yml`.
- Prove the contract at runtime: extend `test/api/checks.py` (`valid_hal_links`, `valid_collection_links`) and add
  `valid_embedded_payloads`, `valid_success_responses` and `valid_declared_headers`; add the `undeclared_query_check`,
  `protected_operation_check` and `hypermedia_body_check` probes; record each proven scenario in
  `test/api/scenarios.toml`.
- Add `test/system/api_document.py` for the document-wide properties (base, wire formats, enums, headers, binary
  bodies), add the missing envelope and rate-limit system cases, and stack `@scenario`/`// proves:` declarations on
  existing tests where a deleted check was the only claim.
- Delete eight API checks and rename 27 in `script/rule_enforcer.sh`, rename `rule/api-*.yml` to `rule/sty-rust-*.yml`,
  and update the three API blocks in `rule_enforcer.yaml`.
- Sweep rule and section citations across `src/`, `docs/`, `.opencode/`, `AGENTS.md` and `.github/`; update
  `.opencode/agents/api-architect.md` and `.opencode/agents/security-specialist.md` to their new authorities.
- **BREAKING**: none. No route, payload, status, header or served document is intended to change; any drift the new
  proof exposes is a defect fixed within this change, not a contract change.

## Capabilities

### New Capabilities

None. The change reorganises where the contract is written and how it is proven; it introduces no new system behaviour.

### Modified Capabilities

- `api`: adds `REQ-API-024` with `SCN-API-024-A`/`SCN-API-024-B`, absorbing the deleted `API-042`. Every existing
  requirement and scenario keeps its identifier and text; the new runtime proof closes scenarios that were already
  specified but unproven.

## Impact

- **Specs**: one delta requirement in `api`; no capability is created, renamed or retired.
- **Rulebook**: `TechnicalDesign.md` loses a section and gains 32 Rust rows plus their details; a new
  `docs/development/HttpApi.md` carries the reference material.
- **Checks**: 8 API checks (35 registered) are deleted, 27 are renamed to `check_sty_rust_091`–`114`/`118`–`121`, a new
  `check_sty_rust_122` is added, and `rule_enforcer.yaml` loses the `non_hal_bodies` setting while `wrapper_keys` and
  `allowed_locations` move to their new IDs.
- **Tests**: `test/api/{checks,hooks,scenarios.toml}` grow; the document-wide cases land in `test/system/api.py`;
  existing capability modules gain stacked scenario declarations and `// proves:` comments.
- **Production**: every handler that declares no query parameters extracts `NoParametersQuery`, so an undeclared query
  parameter answers `400` on all 70 operations the new probe found open (`STY-RUST-122`). The medium type is published
  as an enum while the handler still validates the raw value (`REQ-API-018`). No route, payload or response shape
  changes.
- **Docs and prompts**: `AGENTS.md`, `mkdocs.yml`, `.opencode/agents/api-architect.md`,
  `.opencode/agents/security-specialist.md` and the ~40 named section citations in `docs/`, `src/` and `.github/` are
  updated in the same change.
- **Traceability**: `api` moves from 27/69 proven scenarios to 70/71, with `SCN-API-014-C` the recorded residual (no
  suite image is built without `api-docs`).
