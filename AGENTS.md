# AGENTS.md

## Introduction

Mnemorium is a self-hosted media server designed for home use.

This repository is the **server backend** of a client-server architecture: it
exposes the whole application to clients through a REST API. It is written in
Rust, uses **axum** for HTTP and **sqlx + SQLite3** as the single datastore.

Design goals:

- One Docker container ships the entire backend — no external services.
- REST-first: clients talk to the server exclusively over HTTP.

This file is a router: it summarizes and points to the canonical documentation
under `docs/development/`. When a doc moves, update the link here.

## Build/Test Commands

Shell tool calls run inside the dev environment automatically
(`.opencode/plugins/devenv-shell.ts`); a terminal session needs `devenv shell`
first.

| Task      | Command                                                                                                                              |
| --------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| Build     | `cargo build` (server: `cargo build --bin server`)                                                                                   |
| Run       | `cargo run --bin server` (listens on `:4080`; health `GET /health`)                                                                  |
| Seeded run | `MNEMORIUM_SEED_PROFILE=api cargo run --features test-seed --bin server` (creates `media/` when absent; seeds the named profile, prints demo credentials; a boot naming no profile starts unseeded) |
| Test      | `cargo test` (unit + integration)                                                                                                    |
| System    | `script/system.sh` (builds the image with `test-seed` and `api-docs`, regenerates the suite's typed client, then runs `test/system`; `NO_BUILD=1` reuses the image, `PYTEST_WORKERS=4` parallelises, `IMAGE=...` picks the tag) |
| Client    | `script/openapi_client.sh` or `devenv tasks run system:client` (regenerates `test/generated/` from the image under test; the package is never committed) |
| Investigation | `script/investigation.sh` (regenerates `docs/development/artifacts/RequirementsTraceabilityCoverage.html` from the specifications, the suites' `@scenario`/`// proves:` annotations and the API-suite manifest; needs no container and no pytest run) |
| Coverage  | `cargo llvm-cov --lib --ignore-filename-regex 'src/lib/infrastructure/inbound/rest/handler\|src/lib/infrastructure/seed.rs' --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80` (gate ≥ 80%; the REST handler tree is covered by the system suite and Schemathesis instead, and the seed file by every suite run and the performance dataset gate) |
| Lint      | `cargo clippy --all-targets --all-features -- -D warnings` and `cargo clippy --all-targets -- -D warnings`; `cargo fmt --all -- --check` |
| All rules | `script/rule_enforcer.sh check` (every registered rule; CI and the pre-commit hook run this)                                         |
| One rule  | `script/rule_enforcer.sh check STY-RUST-001` (IDs from `script/rule_enforcer.sh list`; a bare ID works too)                          |
| Full gate | `devenv test` (all pre-commit hooks)                                                                                                 |

`git commit` runs the repository's commit hooks, which need the dev
environment's tooling.

For Markdown, use the `lint:md` and `fmt:md` devenv tasks, or run
`markdownlint-cli2 '**/*.md'` / `prettier`; `devenv test` runs both gates.

Testing strategy and conventions live in `docs/development/TechnicalDesign.md`
§ 4; the black-box performance suite, its volume dataset and its baseline live in
`docs/development/Test.md`; shared Rust test fixtures live in an inline `#[cfg(test)] mod test_helpers`,
crate-wide in `src/lib/lib.rs` and module-scoped in the roots `rule_enforcer.yaml`
lists under `TEST-044`.

## Code Style

Rust and SQL conventions and guidelines live in `docs/development/TechnicalDesign.md`
§ 1.

## Architecture

- **Layout** — module and layer map: `docs/development/Overview.md`.
- **Use Cases** — business use-case catalog, grouped by bounded context with its package diagrams:
  `docs/development/UseCases.md`; its entry format is `TechnicalDesign.md` § 5 (Business use case format),
  `DOC-005`, `DOC-007` and `DOC-008`.
- **API** — the wire contract is `openspec/specs/api/spec.md`; how a handler and payload are declared is the Rust
  rules `STY-RUST-091`–`STY-RUST-122` in `docs/development/TechnicalDesign.md`, and the non-normative HTTP reference
  is `docs/development/HttpApi.md`. The served specification is generated from source (see Build/Test Commands above).
- **Implementation & review agents** — the implementation loop and the specialist
  review panel are defined in `.opencode/agents/execution-governor.md` and
  `.opencode/agents/lead-reviewer.md`.

## Dependencies

See `docs/development/TechnicalDesign.md` § 5 for the rules that govern dependencies and tools,
and `docs/development/DependencyManifest.md` for the direct dependencies and tools this project
uses.

When adding a crate or tool: reuse an existing adapter before adding one, never
use wildcard dependency versions, and keep `cargo deny` clean.

## Documentation

See `docs/development/TechnicalDesign.md` § 6 (Documentation), `DOC-001` for the
code-example conventions: an example cites the source it restates, tracks that
code, and marks elided regions with `// [...]`.

The permissions guide — the action catalog, the five scopes, the resolution
order, the Root Admin and upload rules, denial semantics and the administration
API — is `docs/Permissions.md`.

## Logging

See `docs/development/TechnicalDesign.md` § 7 (Logging & Observability),
`OBS-001`–`OBS-008` for what the server logs, from which layer, at what severity,
and what must never reach a log sink.

## Security

See `docs/development/TechnicalDesign.md` § 8 (Security) for the definition of a
secret-bearing struct, and `REQ-API-025` in `openspec/specs/api/spec.md` for the
closed public surface that protects it.

## Special Rules

- Respect the hexagonal dependency direction: `domain` ← `application` ←
  `infrastructure`. Nothing points outward; the domain must not import
  infrastructure.
- Any Rust change that affects the API must keep the handler's
  `#[utoipa::path(...)]` declaration and the context route registration in step
  (`STY-RUST-091`, `STY-RUST-094`, `STY-RUST-095`); the served specification is
  generated from them.
- Migrations are mutable until release — see
  `docs/development/TechnicalDesign.md` § 3 (Persistence), `PERS-014`.
- Read `rule/*.yml` and `rule_enforcer.yaml` with the read, grep, or glob tools,
  never the shell: `opencode.json` denies any shell command that names either
  path, even a read-only one.
