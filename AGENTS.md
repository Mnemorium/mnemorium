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

| Task      | Command                                                                                                 |
| --------- | ------------------------------------------------------------------------------------------------------- |
| Build     | `cargo build` (server: `cargo build --bin server`)                                                      |
| Run       | `cargo run --bin server` (listens on `:4080`; health `GET /health`)                                     |
| Test      | `cargo test` (unit + integration)                                                                       |
| Coverage  | `cargo llvm-cov --lib` (gate ≥ 80% lines/regions/functions)                                             |
| E2E       | `pytest test/e2e` (needs the Docker server named `mnemorium-e2e`)                                       |
| Lint      | `cargo clippy --all-targets --all-features -- -D warnings`; `cargo fmt --all -- --check`                |
| OpenAPI   | `cargo run --bin openapi_gen` (writes `docs/development/api/openapi.json`)                              |
| Full gate | `devenv test` (all pre-commit hooks)                                                                    |

For Markdown, use the `markdownlint` and `markdownfmt` tools rather than
invoking the underlying binaries directly.

Testing strategy and conventions live in `docs/development/Test.md`.

## Code Style

Rust and SQL conventions and guidelines live in `docs/development/StyleGuide.md`.

## Architecture

- **Layout** — module and layer map: `docs/development/Overview.md`.
- **Use Cases** — catalog and conventions: `docs/development/UseCases.md`; each
  use case maps to one file in `src/lib/application/use_case/`.
- **API** — REST and OpenAPI guidelines: `docs/development/api/Overview.md`. The
  spec is generated from source (see Build/Test Commands above).

## Dependencies

See `docs/development/Dependencies.md` for the direct dependencies and tools this
project uses.

When adding a crate or tool: reuse an existing adapter before adding one, never
use wildcard dependency versions, and keep `cargo audit` clean.

## Special Rules

- Respect the hexagonal dependency direction: `domain` ← `application` ←
  `infrastructure`. Nothing points outward; the domain must not import
  infrastructure.
- Any Rust change that affects the API must regenerate and commit
  `docs/development/api/openapi.json`.
- Migrations are append-only: never edit a migration that has been merged.
