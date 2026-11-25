# Design

## Context

See `proposal.md` — Why. The constraints that shape the approach:

- `docs/development/TechnicalDesign.md` § API owns 47 rules (`API-001`–`API-047`) and six declaration details plus five
  reference sections. Thirty-five of the rules have a `check_api_0NN` function and a `rule/api-0NN.yml` ast-grep rule;
  four (`API-016`, `017`, `018`, `026`) are review-only. The Rust section ends at `STY-RUST-090`, so appending
  `STY-RUST-091` onward keeps the per-prefix sequence gap-free.
- The rulebook's own anatomy (`How this document is extended`, `Section anatomy`, `Rule model`) makes a section's rules
  normative statements and its numbered details non-normative. A rule must be mechanisable; a requirement need not be.
  Section numbers are append-only, and a removed section causes the following numbers to shift once.
- `openspec/specs/api/spec.md` already states the client-observable contract in 23 requirements. `REQ-API-007`
  (`SCN-API-007-A`/`C`), `REQ-API-012`, `REQ-API-019`, `REQ-API-021`, `REQ-API-022` and `REQ-API-023` currently have no
  runtime proof, which is why deleting the rulebook's restatements without a replacement proof would lower coverage.
- The API suite is `script/api_fuzz.sh`: Schemathesis reads `/doc/openapi.json` and drives all 66 operations without a
  test naming a route. Its custom checks are pure bodies in `test/api/checks.py` registered by thin wrappers in
  `test/api/hooks.py`; its scenario claims live in the hand-maintained `test/api/scenarios.toml`, because none of the
  run is a pytest case. The suite image is built with `api-docs`.
- `test/system/web` generated client is produced from the served document, so a system case can only drive what the
  document declares. `test/system/api.py` already owns the hypermedia cases, including `SCN-API-001-C` reading
  `/doc/openapi.json`.
- `script/rule_enforcer.sh` holds a `RULES` registry of `ID:function` entries, the `need_dir`/`scan_short` helpers, and
  the `uncovered` command. `rule_enforcer.yaml` holds the per-rule configuration blocks (`API-028.non_hal_bodies`,
  `API-030.wrapper_keys`, `API-032.allowed_locations`). `script/rule_enforcer.sh check` must pass at every commit.
- `script/investigation.sh` regenerates `docs/development/artifacts/RequirementsTraceabilityCoverage.md` from
  `openspec/specs/**`, `@scenario` decorators, `// proves:` comments and `test/api/scenarios.toml`. `api` is at 27/69
  proven.
- The served API is unchanged: this change moves where the contract is written and how it is proven, and does not alter
  a route, payload, status or header.

## Goals / Non-Goals

**Goals:**

- Give every client-observable API statement exactly one normative home: `openspec/specs/api/spec.md`.
- Keep the source-shape conventions in the Rust section, under one prefix, with their existing checks renamed rather
  than rewritten.
- Prove the broad contract through the API suite and the document test, so a rule is not reintroduced to "enforce" a
  requirement.
- Keep traceability whole: every api scenario that loses its rulebook claim gains a suite, document, system or Rust
  proof, except the one recorded residual.

**Non-Goals:**

- No handler signature, use-case, payload or route refactor beyond the query extractor `STY-RUST-122` requires. Any
  other defect the new proof exposes is fixed at its source, not by changing the contract.
- No new capability and no rewrite of existing api requirement or scenario text; `REQ-API-024` is additive.
- No check invented for `STY-RUST-106`, `107`, `108` or `116`; review-only is the sanctioned state.
- No normative statement added to the rulebook for runtime behaviour; a missing behaviour is fixed in the handler or the
  document.

## Decisions

### 1. Split by observability, not by topic

A statement is a rule when it constrains the source independently of what the server returns; it is a requirement when a
client observes it in a response or in the published document. This is why `API-001`–`API-027` (declaration shape),
`API-030`/`031`/`032` (payload struct shape) and `API-037` (`ApiError` argument shape) move to the Rust section, while
`API-028`/`029`/`033`–`036`/`038`–`047` (media type, self links, status meanings, wire formats, headers, binary bodies)
are deleted as restatements of the already-normative spec.

**Alternative considered:** keep the API section and mark its requirement rows review-only. Rejected: it keeps two
normative statements of one contract, which is the drift the change exists to remove, and leaves 35 checks enforcing
declarations that the document test can prove from the served document itself.

### 2. Proof is chosen by breadth

| Proof                                         | Owns                                                      | Why                                                                                          |
| --------------------------------------------- | --------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| API suite (`checks.py`/`hooks.py`)            | a property that quantifies over every published operation | the fuzz run reads the document and drives all 66 operations without naming one              |
| Document test (`test/system/api_document.py`) | a property of the document as a whole                     | one loop over `/doc/openapi.json` is cheaper and more complete than a per-endpoint assertion |
| System case in the owning capability          | one exact endpoint, fixture or second principal           | the fuzz fixture cannot create the state or the caller                                       |
| Rust test with `// proves:`                   | a source-level rule the handler owns                      | it already asserts the behaviour; only the declaration was missing                           |

**Alternative considered:** one new system case per deleted rule. Rejected: most properties are document-wide or
per-operation, and enumerating endpoints re-creates the coupling to routes the fuzz suite avoids.

### 3. Append the moved rules as `STY-RUST-091`–`STY-RUST-121`

| Old                                                          | New            | Check action |
| ------------------------------------------------------------ | -------------- | ------------ |
| `API-001` annotate each handler with `#[utoipa::path]`       | `STY-RUST-091` | rename       |
| `API-002` one `utoipa::OpenApi` aggregator; routes collected | `STY-RUST-092` | rename       |
| `API-003` `operation_id` declared and unique                 | `STY-RUST-093` | rename       |
| `API-004` HTTP method declared                               | `STY-RUST-094` | rename       |
| `API-005` `path` declared                                    | `STY-RUST-095` | rename       |
| `API-006` `tag` declared (context or `system`)               | `STY-RUST-096` | rename       |
| `API-007` `request_body` declared for a body                 | `STY-RUST-097` | rename       |
| `API-008` `params` declared                                  | `STY-RUST-098` | rename       |
| `API-009` `responses` declared, one row per status           | `STY-RUST-099` | rename       |
| `API-010` `security` declared on every operation             | `STY-RUST-100` | rename       |
| `API-011` `summary` declared                                 | `STY-RUST-101` | rename       |
| `API-012` parameter `name`                                   | `STY-RUST-102` | rename       |
| `API-013` parameter `in`                                     | `STY-RUST-103` | rename       |
| `API-014` parameter `parameter_type`                         | `STY-RUST-104` | rename       |
| `API-015` parameter `description`                            | `STY-RUST-105` | rename       |
| `API-016` constraint attributes used                         | `STY-RUST-106` | none         |
| `API-017` reused `IntoParams` struct                         | `STY-RUST-107` | none         |
| `API-018` `example` provided                                 | `STY-RUST-108` | none         |
| `API-019` request `content_type`                             | `STY-RUST-109` | rename       |
| `API-020` request body schema                                | `STY-RUST-110` | rename       |
| `API-021` response `status`                                  | `STY-RUST-111` | rename       |
| `API-022` response `description`                             | `STY-RUST-112` | rename       |
| `API-023` response `body`                                    | `STY-RUST-113` | rename       |
| `API-024` response `content_type`                            | `STY-RUST-114` | rename       |
| `API-025` response `headers`                                 | `STY-RUST-115` | rename       |
| `API-026` response `links`                                   | `STY-RUST-116` | none         |
| `API-027` every returned response declared                   | `STY-RUST-117` | rename       |
| `API-030` no wrapper member at the payload root              | `STY-RUST-118` | rename       |
| `API-031` a `_links` field's type holds only `Link` members  | `STY-RUST-119` | rename       |
| `API-032` `_embedded` only at a recorded location            | `STY-RUST-120` | rename       |
| `API-037` an `ApiError` message argument is a string literal | `STY-RUST-121` | rename       |

`STY-RUST-122` is new rather than moved: it requires every endpoint handler to extract the query (`<Context>Query` with
`serde(deny_unknown_fields)` where it takes parameters, the shared `NoParametersQuery` where it takes none), which is
what makes `REQ-API-022` true on the 70 operations the probe found to be open. Its detail is § 20.

The four reworded rows are phrased as struct/argument shape, never as response behaviour: `STY-RUST-118` says a HAL
payload struct declares no wrapper member; `STY-RUST-119` says a `_links` field's type holds only `Link` members;
`STY-RUST-120` says a payload struct uses `_embedded` only at a recorded location; `STY-RUST-121` says an `ApiError`
message argument is a string literal.

**Alternative considered:** a new `STY-API` prefix. Rejected: the rule model gives each section one prefix and these are
Rust source conventions; `STY-RUST` is the section that owns them.

### 4. Delete sixteen restatements and re-home one lost clause

| Deleted   | Requirement that owns it | New proof                                                  |
| --------- | ------------------------ | ---------------------------------------------------------- |
| `API-028` | `REQ-API-003`            | `content_type_conformance` (existing)                      |
| `API-029` | `REQ-API-004`            | `valid_hal_links` + `valid_canonical_self`                 |
| `API-033` | `REQ-API-006`            | `valid_collection_links`                                   |
| `API-034` | `REQ-API-021`            | extended `valid_hal_links` over every link                 |
| `API-035` | `REQ-API-002-D`          | `hypermedia_body_check` (new probe)                        |
| `API-036` | `REQ-API-008`            | envelope checks + system cases for `404`/`405`/`413`/`401` |
| `API-038` | `REQ-API-011`            | `negative_data_rejection` + `request_body_contract_check`  |
| `API-039` | `REQ-API-004`            | `valid_canonical_self`                                     |
| `API-040` | `REQ-API-008-C`          | `valid_hal_links` over every `4xx`                         |
| `API-041` | `REQ-API-013`            | capability system cases (authorization, gallery)           |
| `API-042` | **new `REQ-API-024`**    | `negative_data_rejection` over declared bounds             |
| `API-043` | `REQ-API-017`            | document test                                              |
| `API-044` | `REQ-API-018`            | document test                                              |
| `API-045` | `REQ-API-019`            | document test + `valid_declared_headers`                   |
| `API-046` | `REQ-API-020`            | document test                                              |
| `API-047` | `REQ-API-021`            | extended `valid_hal_links`                                 |

`REQ-API-024` is added rather than folded into an existing requirement because a declared bound being enforced is a
distinct client-observable promise; the delta keeps every existing requirement block untouched.

**Alternative considered:** make `API-003` (`operation_id` uniqueness) and `API-018` (examples) requirements too.
Rejected for this change: they constrain the build and the document, not a response; a normative operation-id contract
is a possible follow-up (`REQ-API-025`), not part of removing restatements.

### 5. Non-normative HTTP reference moves to `docs/development/HttpApi.md`

The new document keeps the current anchors: `#openapi-generation`, `#response-codes`, `#error-public-payload`,
`#hal-payload-guidelines`, `#asset-content-retrieval`, and the per-method and `#search-and-filtering` subsections. It is
retitled as implementation notes, added to `mkdocs.yml` under Development, and added to the `AGENTS.md` router.
`DOC-004`, `DOC-006` and `DOC-007` point at the API specification for behaviour and at `HttpApi.md` for the reference.

**Alternative considered:** fold it into the Rust details. Rejected: the response-code table, error envelope example,
HAL samples and content-serving table are HTTP reference, not Rust source conventions, and the Rust section should stay
about how code is written.

### 6. Renumber only the top-level sections that follow the deleted one

| Was                                | Becomes |
| ---------------------------------- | ------- |
| § 3 API                            | —       |
| § 4 Persistence                    | § 3     |
| § 5 Testing                        | § 4     |
| § 6 Dependencies & Dev Environment | § 5     |
| § 7 Documentation                  | § 6     |
| § 8 Logging & Observability        | § 7     |
| § 9 Security                       | § 8     |

Only the named citations are swept — `§ 3 (API)`, `§ 4 (Persistence)`, `§ 5 (Testing)`,
`§ 6 (Dependencies & Dev Environment)`, `§ 7 (Documentation)`, `§ 8 (Logging & Observability)`, `§ 9 (Security)`. A bare
`§ N` that names a detail (`§ 8.1` Repository, `§ 10.1` startup-only settings, `§ 11` Visibility) is unchanged.

**Alternative considered:** keep a rule-free "API" heading to preserve the numbers. Rejected: it violates the section
anatomy and leaves an empty section in the router.

### 7. New runtime proof lands before the source checks are deleted

The order is: specification delta and new tests first, then the rulebook rewrite and enforcer deletion, then the
citation sweep. This is what makes deleting `API-036` and `API-040` safe:

- `API-036` proved `hal_errors` is the outermost layer; the runtime envelope checks plus system cases for an unknown
  route (`404`), a wrong method (`405`), an oversized body (`413`) and the authentication middleware's `401` replace it.
- `API-040` proved the envelope reads `uri().path()`; extended `valid_hal_links` asserts a `4xx` self equals the request
  path with the query dropped, and `SCN-API-008-C` keeps the system-level claim.
- `API-029`'s "every representation declares a `_links` field" gate is accepted as a per-capability contract; the
  runtime check covers a representation whenever it is returned.

### 8. API suite: five checks and three probes

| New or extended check             | Proves                                                                                                                                                                                                                                         |
| --------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `valid_hal_links` (extend)        | `REQ-API-003`/`004`/`008-C`/`021`: every link non-empty; a templated link keeps `{}` and only a templated link sets `templated`; every plain `href` starts with `/`; a `4xx` self is the request path                                          |
| `valid_collection_links` (extend) | `REQ-API-006-A`/`B`, `007-A`/`B`/`C`: `search` templated and names the collection; root members do not repeat the query parameters; a page without `limit` has at most 20 items; `next` advances by the limit; `prev` is absent at offset 0    |
| `valid_embedded_payloads` (new)   | `REQ-API-005-C`, `016-A`/`B`/`C`/`E`: `_embedded` only on the four catalog listings; when present, `_embedded.items` exists and no root `items`; each item has `id`, `name`, a cover identifier, `self` and `cover`, and no detail-only member |
| `valid_success_responses` (new)   | `REQ-API-012-A`/`C`: `201` carries `Location`; `204` carries no body                                                                                                                                                                           |
| `valid_declared_headers` (new)    | `REQ-API-009-B`, `019-A`: every emitted response header is declared on that status, with a small standard-header allowlist; a `401` carries `WWW-Authenticate: Bearer`                                                                         |

| New probe                   | Proves          | Behaviour                                                                                                             |
| --------------------------- | --------------- | --------------------------------------------------------------------------------------------------------------------- |
| `undeclared_query_check`    | `REQ-API-022`   | derive a case, append a random undeclared query parameter, expect `400`                                               |
| `protected_operation_check` | `REQ-API-023`   | for every operation declaring `bearer_auth`, derive a request with no `Authorization`, expect `401` and the challenge |
| `hypermedia_body_check`     | `REQ-API-002-D` | on a valid JSON body, add `_links`/`_embedded`, replay, expect the positive outcome                                   |

`wrong_method_check` is not written as a probe: the `405` behaviour is cheaper and less generator-coupled as a system
case on `DELETE /health`, which also carries `Allow` (`REQ-API-009`).

### 9. Document-wide proof is one Python module

`test/system/api_document.py` reads `/doc/openapi.json` once per case and loops its paths; no test names an endpoint:

| Case                                                                                                           | Scenarios                 |
| -------------------------------------------------------------------------------------------------------------- | ------------------------- |
| `test_the_document_is_the_contract` — one server whose path is `/api/v1`, relative paths, every route declared | `001-C`, `014-A`, `014-B` |
| `test_every_date_and_timestamp_declares_its_format` — `format: date`/`date-time`, UTC `Z`                      | `017-A`, `017-B`          |
| `test_every_closed_vocabulary_is_an_enum` — request properties and path/query parameters                       | `018-A`, `018-B`          |
| `test_every_published_header_declares_its_schema` — `Location`, `WWW-Authenticate`, content headers            | `019-B`, `019-C`          |
| `test_content_bodies_are_binary_and_wildcard` — `format: binary`, `*/*`                                        | `020-A`, `020-B`          |

`SCN-API-014-C` (documentation off by default) cannot be proven by the suite image, which is built with `api-docs`; it
is recorded as the traceability residual. The alternative — a CI assertion that `api-docs` is not a default Cargo
feature — is deferred rather than smuggled into this change as a shell rule.

### 10. Existing tests take the declarations a deleted check was the only claim on

- `src/lib/infrastructure/inbound/rest/api_error.rs`: the tests asserting a client message echoes neither the rejected
  value nor the expected type name get `// proves: SCN-API-010-A` and `SCN-API-010-B`; the unclassified-rejection test
  gets `SCN-API-011-B`.
- `src/lib/infrastructure/inbound/rest/middleware/hal_errors.rs`: the envelope tests get `SCN-API-008-B`.
- Capability system cases gain `@scenario`: `SCN-API-012-B` on the music-album/medium cases, `SCN-API-013-A`/`B` on the
  gallery/authorization cases, `SCN-API-011-C` on the music/upload cases, and `SCN-API-004-C`/`D` on the upload
  completion and login/token/health cases.

No new Rust test module is added to `rest.rs`: the document is proven by the system suite, as an earlier API change
already established.

### 11. Enforcer: delete eight, rename twenty-seven, move two configuration blocks

- Delete `check_api_028`, `029`, `033`, `034`, `035`, `036`, `038`, `040`, their `rule/api-0NN.yml` files and their
  `RULES` entries.
- Rename `check_api_0NN` to `check_sty_rust_0MM` and `rule/api-0NN.yml` to `rule/sty-rust-0MM.yml` for the 27 mapped
  rows, updating every `need_dir`/`need_file`/diagnostic string so no message cites `API-0xx`.
- `rule_enforcer.yaml`: delete the `API-028` block (`non_hal_bodies` disappears with it); rename `API-030` to
  `STY-RUST-118` and `API-032` to `STY-RUST-120`, moving `wrapper_keys` and `allowed_locations` unchanged.
- `sgconfig.yaml` is untouched: it globs `rule/`.

Expected state: `script/rule_enforcer.sh list` has no `API-` row, the registry drops by 8, and `uncovered` lists
`STY-RUST-106`, `107`, `108`, `116`.

### 12. Citation sweep is rule-scoped, never a blind substitution

Sweep only rule citations with `(?<!REQ-)(?<!SCN-)API-0[0-9]{2}`, using the map in decision 3 and the deletions in
decision 4. `openspec/changes/archive/**` is historical and is never touched. `openspec/specs/api/spec.md`,
`test/api/scenarios.toml`, `test/system/api.py` and `test/api/checks.py` use `REQ-`/`SCN-` identifiers and are edited
only where a docstring names a deleted rule.

The agent prompts that describe the removed section change with it:

- `.opencode/agents/api-architect.md`: authority becomes the `api` specification for behaviour and
  `STY-RUST-091`–`STY-RUST-122` plus `docs/development/HttpApi.md` for declaration and document style.
- `.opencode/agents/security-specialist.md`: the `API-040` citation becomes `REQ-API-008`.
- `AGENTS.md`: the API router entry points at the specification, the Rust rule range and `HttpApi.md`; the Special Rules
  bullet on `#[utoipa::path]` cites `STY-RUST-091`/`094`/`095`.

## Risks / Trade-offs

- **Deleting `API-036`/`API-040` removes a source-level guarantee of the envelope's outermost position and path-only
  self.** → the runtime envelope checks and the system cases for `404`, `405`, `413` and `401` land first; the change
  does not delete the checks until those exist.
- **Deleting `API-029` loses the "every representation declares `_links`" gate for a representation no test returns.** →
  accepted: that is a per-capability contract and `REQ-API-003` is the requirement; the runtime check covers every
  representation the suite receives.
- **Renumbering six top-level sections touches ~40 named citations across 15 files.** → do it in one commit and use the
  `§ ?N (Name)` grep in the verification section as the gate; never rewrite a detail number.
- **Four moved conventions have no check and will appear in `uncovered`.** → intended state for review-only rules; do
  not invent a check to empty the list.
- **`valid_declared_headers` may flag a legitimate header the document does not declare** (a proxy or framework header).
  → keep a small standard-header allowlist and assert only the headers the serving path emits; report an undeclared
  application header as a defect.
- **The closed-query probe exposes 70 operations that ignore an undeclared parameter.** → the missing behaviour is fixed
  where it belongs: every handler extracts the query, its own or `NoParametersQuery`, and `STY-RUST-122` keeps a new
  handler from omitting it. This is the one production change the runtime proof forces.
- **A once-per-operation probe can be skipped or replayed** if the first authenticated case is rejected. → reuse the
  guard `request_body_contract_check` already uses (skip a `401`/`403` first case, record the operation as probed before
  calling).
- **The document test may expose real drift** (a missing `format`, a non-enum vocabulary, a binary body published as an
  integer array). → treat each as a defect fixed in the handler or derive within this change; the served contract is not
  weakened to make the test pass.
- **The traceability matrix will still show `SCN-API-014-C`.** → record it as a residual with its reason (no suite image
  without `api-docs`) rather than claiming a proof that does not exist.
- **The API suite is the proof, not the enforcer.** → if a runtime behaviour is missing, fix the handler or the
  document; never reintroduce a rulebook row to "enforce" a requirement.

## Migration Plan

No deploy or data migration: no runtime behaviour, schema or API shape changes. The implementation lands in five
commits, each green before the next:

1. `api` specification delta and the new/extended API-suite checks, hooks, manifest entries and document test, plus the
   system/Rust declarations.
2. `docs/development/HttpApi.md`, the `TechnicalDesign.md` section deletion, the 31 Rust rows/details and the section
   renumbering.
3. `rule/*.yml`, `script/rule_enforcer.sh` and `rule_enforcer.yaml` deletion/rename.
4. The citation and prompt sweep.
5. Regenerated traceability matrix and the full gate.

Rollback is a revert of those commits; no stored state is touched.

## Open Questions

- Should `operation_id` uniqueness (`STY-RUST-093`) become normative as `REQ-API-025`? Deferred: it is a build/document
  convention, and a follow-up can decide without changing this change's design or task breakdown.
