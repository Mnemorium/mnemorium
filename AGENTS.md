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

Enter the environment first: `devenv shell`.

| Task      | Command                                                                                                                              |
| --------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| Build     | `cargo build` (server: `cargo build --bin server`)                                                                                   |
| Run       | `cargo run --bin server` (listens on `:4080`; health `GET /health`)                                                                  |
| Test      | `cargo test` (unit + integration)                                                                                                    |
| Coverage  | `cargo llvm-cov --lib --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80` (gate ≥ 80%)                          |
| E2E       | `pytest -p no:cacheprovider test/e2e` (needs the Docker server named `mnemorium-e2e`)                                                |
| Lint      | `cargo clippy --all-targets --all-features -- -D warnings`; `cargo fmt --all -- --check`                                             |
| OpenAPI   | `cargo run --bin openapi_gen` (writes `docs/openapi.json`)                                                           |
| Full gate | `devenv test` (all pre-commit hooks)                                                                                                 |

Run `git commit` through the environment as well, prefixed with `devenv shell --`
(e.g. `devenv shell -- git commit`), because the repository's commit hook needs
tooling that is only available inside the development environment.

For Markdown, use the `lint:md` and `fmt:md` devenv tasks, or run
`markdownlint-cli2` / `prettier` through `devenv shell --`; `devenv test` runs
both gates.

Testing strategy and conventions live in `docs/development/TechnicalDesign.md`
§ 5; shared Rust test fixtures live in the single `#[cfg(test)] mod
test_helpers` in `src/lib/lib.rs` (`TEST-046`).

## Code Style

Rust and SQL conventions and guidelines live in `docs/development/TechnicalDesign.md`
§ 1.

## Architecture

- **Layout** — module and layer map: `docs/development/Overview.md`.
- **Use Cases** — business use-case catalog and conventions: `docs/development/UseCases.md`;
  each entry lists the application use-case file(s) in
  `src/lib/application/use_case/` that implement it.
- **API** — REST and OpenAPI guidelines: `docs/development/TechnicalDesign.md` § 3. The
  spec is generated from source (see Build/Test Commands above).
- **Implementation & review agents** — the implementation loop and the specialist
  review panel are defined in `.opencode/agents/execution-governor.md` and
  `.opencode/agents/lead-reviewer.md`.

## Dependencies

See `docs/development/TechnicalDesign.md` § 6 for the direct dependencies and tools this
project uses.

When adding a crate or tool: reuse an existing adapter before adding one, never
use wildcard dependency versions, and keep `cargo deny` clean.

## Repository Governance

See `docs/development/TechnicalDesign.md` § 7 (Repository Governance), `GOV-001`
for the commit and pull-request scope set and the paths each scope owns.

## Documentation

See `docs/development/TechnicalDesign.md` § 8 (Documentation), `DOC-001` for the
code-example conventions: an example cites the source it restates, tracks that
code, and marks elided regions with `// [...]`.

## Logging

See `docs/development/TechnicalDesign.md` § 9 (Logging & Observability),
`OBS-001`–`OBS-007` for what the server logs, from which layer, at what severity,
and what must never reach a log sink.

## Special Rules

- Respect the hexagonal dependency direction: `domain` ← `application` ←
  `infrastructure`. Nothing points outward; the domain must not import
  infrastructure.
- Any Rust change that affects the API must regenerate and commit
  `docs/openapi.json`.
- Migrations are mutable until release — see
  `docs/development/TechnicalDesign.md` § 4 (Persistence), `PERS-014`.