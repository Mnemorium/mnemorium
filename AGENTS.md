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
| OpenAPI   | `cargo run --bin openapi_gen` (writes `docs/development/api/openapi.json`)                                                           |
| Full gate | `devenv test` (all pre-commit hooks)                                                                                                 |

For Markdown, use the `markdownlint` and `markdownfmt` tools rather than
invoking the underlying binaries directly.

Testing strategy and conventions live in `docs/development/TechnicalDesign.md`
§ 5.

## Code Style

Rust and SQL conventions and guidelines live in `docs/development/TechnicalDesign.md`
§ 1.

## Architecture

- **Layout** — module and layer map: `docs/development/Overview.md`.
- **Use Cases** — catalog and conventions: `docs/development/UseCases.md`; each
  use case maps to one file in `src/lib/application/use_case/`.
- **API** — REST and OpenAPI guidelines: `docs/development/TechnicalDesign.md` § 3. The
  spec is generated from source (see Build/Test Commands above).

## Dependencies

See `docs/development/TechnicalDesign.md` § 6 for the direct dependencies and tools this
project uses.

When adding a crate or tool: reuse an existing adapter before adding one, never
use wildcard dependency versions, and keep `cargo audit` clean.

## Agent skills

### Issue tracker

Issues are tracked on GitHub Issues in the `Mnemorium/mnemorium` repository.
See `docs/development/IssueTracking.md`.

### Triage labels

The state labels are `needs-triage`, `needs-info`, `ready-for-agent`,
`ready-for-human`, and `wontfix`. See `docs/development/IssueTracking.md`.

### Domain docs

Single-context repository: there is no `CONTEXT.md` and no `docs/adr/`. The
domain language lives in `docs/development/Glossary.md` and the use-case catalog
in `docs/development/UseCases.md`.

## Agents and skills

- **Agents** — `.opencode/agent/`: `rust-dev`, `database-engineer`, `qa-e2e`,
  `devops`, `reviewer`, and `review-orchestrator`.
- **Skills** — `.opencode/skills/`: `review-pr` and `create-issue`.

## Special Rules

- Respect the hexagonal dependency direction: `domain` ← `application` ←
  `infrastructure`. Nothing points outward; the domain must not import
  infrastructure.
- Any Rust change that affects the API must regenerate and commit
  `docs/development/api/openapi.json`.
- Migrations are append-only: never edit a migration that has been merged.