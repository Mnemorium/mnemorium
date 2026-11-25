# Tasks

Each numbered group is one reviewable, separately committable chunk. The order is deliberate: the runtime proof lands
before the rulebook checks are deleted, so no scenario loses its only claim in between.

Three rules govern every test this change touches:

- Write the failing proof first, watch it fail for the expected reason, then make it pass. Put each proof at the lowest
  level that can observe the behaviour, and never re-assert at a higher level what a lower one already owns.
- Declare every scenario a pytest case proves with `@scenario("SCN-…", TestType.…)`; a Rust test uses `// proves: SCN-…`
  above its attributes. A scenario proven by the fuzz run goes in `test/api/scenarios.toml` with its kind from the
  closed set.
- Read `rule/*.yml` and `rule_enforcer.yaml` with the read, grep or glob tools, never the shell; `opencode.json` denies
  a shell command that names either path.

## 1. API-suite checks and probes

- [x] 1.1 Keep no separate unit module for the pure check bodies: the fuzz suite and the document test in
      `test/system/api.py` own every one of those cases, and the intermediate `test/api/checks_test.py` was removed on
      request.
- [x] 1.2 Extend `valid_hal_links` and `valid_collection_links` in `test/api/checks.py` to the behaviour the unit cases
      name, keeping their existing signatures. Verify `pytest test/api/checks_test.py` passes.
- [x] 1.3 Add `valid_embedded_payloads`, `valid_success_responses` and `valid_declared_headers` to `test/api/checks.py`,
      with the small standard-header allowlist for the header check. Verify their unit cases pass.
- [x] 1.4 Register the three new checks and add the `undeclared_query_check`, `protected_operation_check` and
      `hypermedia_body_check` probes in `test/api/hooks.py`; reuse the once-per-operation guard and the credential rules
      `request_body_contract_check` already applies. Verify `python -m py_compile test/api/checks.py test/api/hooks.py`
      succeeds and the registered names match the manifest.
- [x] 1.5 Rewrite the manifest header in `test/api/scenarios.toml` to name each check and probe, then add the 20
      entries: `SCN-API-002-D`, `005-C`, `006-B`, `009-A`, `016-A`, `016-B`, `016-C`, `016-E`, `019-A`, `021-A`, `021-B`
      as `Business rule`; `007-A`, `012-A`, `012-C` as `Positive`; `007-C` as `Boundary`; `009-B`, `023-A` as
      `Security`; `022-A`, `024-A`, `024-B` as `Negative`. Verify
      `python -c "import tomllib, pathlib; tomllib.loads(pathlib.Path('test/api/scenarios.toml').read_text())"` succeeds
      and the file still parses under `test/util/investigation.py`.

## 2. Document-wide and system proof

- [x] 2.1 Add the five document-wide cases to `test/system/api.py` (`001-C`/`014-A`/`014-B`, `017-A`/`B`, `018-A`/`B`,
      `019-B`/`C`, `020-A`/`B`), each reading `/doc/openapi.json` once and looping its paths, with the matching
      `@scenario` decorators. Verify the cases pass against a seeded `api-docs` image (`NO_BUILD=1 script/system.sh`
      after the image exists).
- [x] 2.2 Add the missing system cases to `test/system/api.py`: `SCN-API-008-A` (an unknown route under `/api/v1`
      answers the envelope) and `SCN-API-009-C` (a login burst answers `429` with `Retry-After`), and cover `405` plus
      `Allow` on a declared path with a wrong method (`DELETE /health`). Verify the module passes.
- [x] 2.3 Stack the declarations a deleted check was the only claim on: `SCN-API-012-B` on the music album/medium create
      cases, `SCN-API-013-A`/`B` on the gallery/authorization cases, `SCN-API-011-C` on the music/upload conflict/gone
      cases, and `SCN-API-004-C`/`D` on the upload-completion and login/token/health cases. Verify no module declares
      the same scenario twice and the cases pass.
- [x] 2.4 Add `// proves:` comments in `src/lib/infrastructure/inbound/rest/api_error.rs` (`SCN-API-010-A`, `010-B`,
      `011-B`) and `src/lib/infrastructure/inbound/rest/middleware/hal_errors.rs` (`SCN-API-008-B`) above the tests that
      already assert the behaviour. Verify `cargo test` passes and `script/investigation.sh` maps the four scenarios.
- [x] 2.5 Record `SCN-API-014-C` as the traceability residual in the API-fuzz section of `docs/development/Test.md`,
      naming the reason (the suite image is built with `api-docs`). Verify the sentence survives `markdownlint-cli2`.

## 3. Specification, reference document and rulebook

- [x] 3.1 Append `REQ-API-024` with `SCN-API-024-A`/`B` to `openspec/specs/api/spec.md`, exactly as the change delta
      states, without touching any existing requirement. Verify
      `openspec validate     dissolve-technical-design-api-section` passes and `script/investigation.sh` reports 24 api
      requirements and 71 scenarios.
- [x] 3.2 Create `docs/development/HttpApi.md` from the deleted reference material: OpenAPI generation, the
      response-code table, the error envelope, the HAL guidelines and per-method samples, search and filtering, and
      asset content retrieval. Keep the existing anchors and retitle the material as implementation notes. Verify
      `markdownlint-cli2 'docs/development/HttpApi.md'` passes and every moved anchor still resolves.
- [x] 3.3 Delete `## API` from `docs/development/TechnicalDesign.md` (table, details `#### 1.`–`#### 6.`, and the five
      reference sections), append `STY-RUST-091`–`STY-RUST-122` to the Rust table with their details as `§ 12` onward,
      and renumber the following top-level sections (Persistence → 3, Testing → 4, Dependencies → 5, Documentation → 6,
      Logging → 7, Security → 8). Point `DOC-004`, `DOC-006` and `DOC-007` at the API specification and `HttpApi.md`.
      Verify `script/rule_enforcer.sh check` passes, no `API-0` row remains, and the detail anchors still resolve.
- [x] 3.4 Update `AGENTS.md` (the API router entry and the `#[utoipa::path]` Special Rule) and add `HttpApi.md` to the
      Development nav in `mkdocs.yml`. Verify both links resolve and `markdownlint-cli2 '**/*.md'` passes.

- [x] 3.5 Add `STY-RUST-122`: every endpoint handler extracts the query — its own `<Context>Query` or the shared
      `NoParametersQuery` — with `rule/sty-rust-122.yml`, `check_sty_rust_122`, a `§ 20` detail, and the empty
      `Query<NoParametersQuery>` on every handler that declares none. Verify `script/rule_enforcer.sh check` passes and
      the fuzz probe answers `400` on all 70 previously-open operations.

## 4. Rule enforcer

- [x] 4.1 Delete `check_api_028`, `029`, `033`, `034`, `035`, `036`, `038` and `040`, their `rule/api-0NN.yml` files and
      their `RULES` entries. Verify `script/rule_enforcer.sh check` passes and the eight files are gone.
- [x] 4.2 Rename the 27 remaining API checks and rule files to `check_sty_rust_0MM` / `rule/sty-rust-0MM.yml` per the
      design's map, updating every `need_dir`/`need_file`/diagnostic string so no message cites `API-0xx`. Verify
      `script/rule_enforcer.sh list` has no `API-` row and `script/rule_enforcer.sh check` passes.
- [x] 4.3 In `rule_enforcer.yaml`, delete the `API-028` block and rename `API-030` to `STY-RUST-118` and `API-032` to
      `STY-RUST-120`, keeping `wrapper_keys` and `allowed_locations` unchanged. Verify
      `script/rule_enforcer.sh check STY-RUST-118` and `STY-RUST-120` both pass.
- [x] 4.4 Verify the sanctioned uncovered set: `script/rule_enforcer.sh uncovered` lists `STY-RUST-106`, `107`, `108`
      and `116` and no `API-*`, and the registry holds 27 fewer API entries and the rulebook has no API row.

## 5. Citations and prompts

- [x] 5.1 Sweep rule citations only, with `(?<!REQ-)(?<!SCN-)API-0[0-9]{2}`, across `src/`, `docs/`, `rule/`, `script/`,
      `rule_enforcer.yaml`, `.opencode/`, `.github/` and `AGENTS.md`, using the move/delete map; never touch
      `openspec/changes/archive/**`. Verify
      `grep -rnP '(?<!REQ-)(?<!SCN-)API-0[0-9]{2}' src docs rule script rule_enforcer.yaml .opencode .github AGENTS.md`
      returns nothing outside the archive.
- [x] 5.2 Rewrite `.opencode/agents/api-architect.md` so its authority is the `api` specification for behaviour and
      `STY-RUST-091`–`122` plus `HttpApi.md` for declaration/document style; update the `API-040` citation in
      `.opencode/agents/security-specialist.md` to `REQ-API-008`. Verify no prompt still describes the deleted section.
- [x] 5.3 Sweep named top-level section citations (`§ 3 (API)` through `§ 9 (Security)`) to the renumbered values; leave
      every bare detail number (`§ 8.1`, `§ 10.1`, `§ 11`) unchanged. Verify
      `grep -rnE '§ ?[0-9]+ \((API|Persistence|Testing|Dependencies & Dev Environment|Documentation|Logging & Observability|Security)\)'`
      finds no stale number.

## 6. Integration verification

- [x] 6.1 Run `script/rule_enforcer.sh check`, `script/rule_enforcer.sh uncovered` and `script/rule_enforcer.sh list`
      and confirm the counts: no API rule, four sanctioned uncovered Rust rules, and the registry dropped by 8 API
      entries while gaining `STY-RUST-122`.
- [x] 6.2 Run `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings` (and the
      without-features pass), `cargo test`, and `markdownlint-cli2 '**/*.md'`.
- [x] 6.3 Run `script/investigation.sh` and confirm the `api` row: 24 requirements, 71 scenarios, 70 proven, with
      `SCN-API-014-C` as the documented residual.
- [x] 6.4 Run `script/api_fuzz.sh` and confirm every registered check and probe completes without a failure.
- [x] 6.5 Run `script/system.sh` (or `NO_BUILD=1 script/system.sh`) and confirm the system suite, including
      `api_document.py` and the new envelope/rate-limit cases, passes.

## Workflow follow-up

- Archive the change after the project's review requirements are satisfied.
- Re-run `script/investigation.sh` after archiving and confirm the matrix is unchanged.
