# Dependencies

Direct dependencies of this project, grouped by source. Update the tables when a dependency is added, removed, or
upgraded; transitive dependencies are out of scope (the lockfiles and `cargo audit` cover those).

Versions are the resolved ones (lockfile or Nix store), not the declared constraints. When a tool is declared by more
than one source it is listed once, under the source that pins it (normally `devenv.nix`). Cells marked `TODO` still need
to be filled in.

Columns: **Name**, **Description**, **Version**, **License**.

## Rust

Source: `Cargo.toml`.

### Runtime

| Name                                                              | Description                                                                     | Version | License                            |
| ----------------------------------------------------------------- | ------------------------------------------------------------------------------- | ------- | ---------------------------------- |
| [anyhow](https://crates.io/crates/anyhow)                         | Flexible concrete error type built on `std::error::Error`.                      | 1.0.104 | MIT OR Apache-2.0                  |
| [argon2](https://crates.io/crates/argon2)                         | Pure Rust implementation of the Argon2 password hashing function.               | 0.6.0   | MIT OR Apache-2.0                  |
| [axum](https://crates.io/crates/axum)                             | HTTP routing and request handling library focused on ergonomics and modularity. | 0.8.9   | MIT                                |
| [chrono](https://crates.io/crates/chrono)                         | Date and time library for Rust.                                                 | 0.4.45  | MIT OR Apache-2.0                  |
| [config](https://crates.io/crates/config)                         | Layered configuration system for Rust applications.                             | 0.15.25 | MIT OR Apache-2.0                  |
| [email_address](https://crates.io/crates/email_address)           | RFC-compliant `EmailAddress` newtype.                                           | 0.2.9   | MIT                                |
| [infer](https://crates.io/crates/infer)                           | Infers a file type from its magic number signature.                             | 0.22.0  | MIT                                |
| [jsonwebtoken](https://crates.io/crates/jsonwebtoken)             | Creates and decodes JWTs in a strongly typed way.                               | 11.0.0  | MIT                                |
| [md-5](https://crates.io/crates/md-5)                             | MD5 hash function.                                                              | 0.11.0  | MIT OR Apache-2.0                  |
| [moka](https://crates.io/crates/moka)                             | Fast, concurrent cache library inspired by Java Caffeine.                       | 0.12.16 | (MIT OR Apache-2.0) AND Apache-2.0 |
| [rand](https://crates.io/crates/rand)                             | Random number generators and other randomness functionality.                    | 0.10.2  | MIT OR Apache-2.0                  |
| [reqwest](https://crates.io/crates/reqwest)                       | Higher level HTTP client library.                                               | 0.13.4  | MIT OR Apache-2.0                  |
| [serde](https://crates.io/crates/serde)                           | Generic serialization and deserialization framework.                            | 1.0.229 | MIT OR Apache-2.0                  |
| [serde_json](https://crates.io/crates/serde_json)                 | JSON serialization file format.                                                 | 1.0.151 | MIT OR Apache-2.0                  |
| [sqlx](https://crates.io/crates/sqlx)                             | Async, pure Rust SQL toolkit with compile-time checked queries and no DSL.      | 0.9.0   | MIT OR Apache-2.0                  |
| [thiserror](https://crates.io/crates/thiserror)                   | Derives `std::error::Error` implementations.                                    | 2.0.20  | MIT OR Apache-2.0                  |
| [tokio](https://crates.io/crates/tokio)                           | Event-driven, non-blocking I/O platform for asynchronous applications.          | 1.53.1  | MIT                                |
| [tower](https://crates.io/crates/tower)                           | Modular, reusable components for building robust clients and servers.           | 0.5.3   | MIT                                |
| [tracing](https://crates.io/crates/tracing)                       | Application-level tracing for Rust.                                             | 0.1.44  | MIT                                |
| [tracing-appender](https://crates.io/crates/tracing-appender)     | File appenders and non-blocking writers for `tracing`.                          | 0.2.5   | MIT                                |
| [tracing-subscriber](https://crates.io/crates/tracing-subscriber) | Composing and implementing `tracing` subscribers.                               | 0.3.23  | MIT                                |
| [utoipa](https://crates.io/crates/utoipa)                         | Compile-time generated OpenAPI documentation for Rust.                          | 5.5.0   | MIT OR Apache-2.0                  |

### Development

| Name                                          | Description                                           | Version | License           |
| --------------------------------------------- | ----------------------------------------------------- | ------- | ----------------- |
| [mockall](https://crates.io/crates/mockall)   | Powerful mock object library for Rust.                | 0.15.0  | MIT OR Apache-2.0 |
| [rstest](https://crates.io/crates/rstest)     | Fixture-based test framework with table-driven tests. | 0.26.1  | MIT OR Apache-2.0 |
| [tempfile](https://crates.io/crates/tempfile) | Manages temporary files and directories.              | 3.27.0  | MIT OR Apache-2.0 |
| [wiremock](https://crates.io/crates/wiremock) | HTTP mocking to test Rust applications.               | 0.6.5   | MIT OR Apache-2.0 |

### Build

| Name                                  | Description                                                                   | Version | License           |
| ------------------------------------- | ----------------------------------------------------------------------------- | ------- | ----------------- |
| [sqlx](https://crates.io/crates/sqlx) | Async Rust SQL toolkit; used at build time to run embedded SQLite migrations. | 0.9.0   | MIT OR Apache-2.0 |

## Tooling

### Nix / devenv

Source: `devenv.nix`.

| Name                                                              | Description                                                | Version | License |
| ----------------------------------------------------------------- | ---------------------------------------------------------- | ------- | ------- |
| [betterleaks](https://github.com/dortort/betterleaks)             | Secret scanning in pre-commit and CI.                      | 1.8.1   | TODO    |
| [cargo-audit](https://github.com/rustsec/rustsec)                 | Audits `Cargo.lock` for known vulnerabilities.             | 0.22.2  | TODO    |
| [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov)       | Coverage instrumentation and reporting for Rust.           | 0.9.0   | TODO    |
| [git](https://git-scm.com/)                                       | Version control.                                           | 2.55.0  | TODO    |
| [llvm](https://llvm.org/)                                         | LLVM tools used by coverage (`llvm-cov`, `llvm-profdata`). | 21.1.8  | TODO    |
| [ls-lint](https://ls-lint.org/)                                   | File and directory naming linter.                          | 2.3.1   | TODO    |
| [nixfmt](https://github.com/NixOS/nixfmt)                         | Nix code formatter.                                        | 1.4.0   | TODO    |
| [nodejs](https://nodejs.org/)                                     | Node.js runtime for tooling.                               | 24.19.0 | TODO    |
| [prettier](https://prettier.io/)                                  | Markdown formatter.                                        | 3.8.3   | TODO    |
| [Python toolchain](https://www.python.org/)                       | Python runtime for tooling and tests.                      | 3.14.6  | TODO    |
| [Rust toolchain](https://www.rust-lang.org/)                      | Rust compiler and standard tooling.                        | 1.98.0  | TODO    |
| [shellcheck](https://github.com/koalaman/shellcheck)              | Shell script linter.                                       | 0.11.0  | TODO    |
| [shfmt](https://github.com/mvdan/sh)                              | Shell script formatter.                                    | 3.13.1  | TODO    |
| [sqlfluff](https://sqlfluff.com/)                                 | SQL linter and formatter (SQLite dialect).                 | 4.3.0   | TODO    |
| [sqlite](https://sqlite.org/)                                     | SQLite command-line shell.                                 | 3.51.2  | TODO    |
| [sqlx-cli](https://github.com/launchbadge/sqlx)                   | SQLx migrations and database CLI.                          | 0.9.0   | TODO    |
| [taplo](https://taplo.tamasfe.dev/)                               | TOML formatter.                                            | 0.10.0  | TODO    |
| [xdg-utils](https://www.freedesktop.org/wiki/Software/xdg-utils/) | Desktop integration helpers (opens reports in a browser).  | 1.2.1   | TODO    |
| [yamllint](https://github.com/adrienverge/yamllint)               | YAML linter.                                               | 1.37.1  | TODO    |

### Python

Source: `requirements.txt`.

| Name                                                         | Description                                       | Version | License      |
| ------------------------------------------------------------ | ------------------------------------------------- | ------- | ------------ |
| [mkdocs](https://pypi.org/project/mkdocs/)                   | Static site generator for the documentation site. | 1.6.0   | BSD-2-Clause |
| [mkdocs-material](https://pypi.org/project/mkdocs-material/) | Material Design theme for MkDocs.                 | 9.7.7   | MIT          |
| [mkdocs_puml](https://pypi.org/project/mkdocs-puml/)         | Renders PlantUML diagrams from fenced blocks.     | 2.3.0   | MIT          |
| [neoteroi-mkdocs](https://pypi.org/project/neoteroi-mkdocs/) | MkDocs plugins, including OpenAPI rendering.      | 1.2.0   | MIT          |
| [pytest](https://pypi.org/project/pytest/)                   | Python testing framework used by the E2E suite.   | 9.1.1   | MIT          |
| [requests](https://pypi.org/project/requests/)               | HTTP library for Python.                          | 2.34.2  | Apache-2.0   |
| [ruff](https://pypi.org/project/ruff/)                       | Fast Python linter and formatter.                 | 0.16.5  | MIT          |

### Node

Source: `.opencode/package.json`.

| Name                                                                     | Description                                                                          | Version | License |
| ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------ | ------- | ------- |
| [@opencode-ai/plugin](https://www.npmjs.com/package/@opencode-ai/plugin) | OpenCode plugin SDK; provides the `tool` API used by the repository's tools.         | 1.18.29 | MIT     |
| [@opencode/plugin](https://www.npmjs.com/package/@opencode/plugin)       | OpenCode plugin runtime; provides the `Plugin` API used by the repository's plugins. | 2.0.14  | MIT     |

### CI

Source: `.github/workflows/*`.

| Name                                                                                                                   | Description | Version | License |
| ---------------------------------------------------------------------------------------------------------------------- | ----------- | ------- | ------- |
| [@semantic-release-plus/docker](https://www.npmjs.com/package/@semantic-release-plus/docker)                           | TODO        | 3.1.3   | TODO    |
| [@semantic-release/changelog](https://www.npmjs.com/package/@semantic-release/changelog)                               | TODO        | 7.0.0   | TODO    |
| [@semantic-release/exec](https://www.npmjs.com/package/@semantic-release/exec)                                         | TODO        | 7.1.0   | TODO    |
| [@semantic-release/git](https://www.npmjs.com/package/@semantic-release/git)                                           | TODO        | 11.0.1  | TODO    |
| [actions/checkout](https://github.com/actions/checkout)                                                                | TODO        | v6      | TODO    |
| [actions/create-github-app-token](https://github.com/actions/create-github-app-token)                                  | TODO        | v3.2.0  | TODO    |
| [actions/setup-node](https://github.com/actions/setup-node)                                                            | TODO        | v4      | TODO    |
| [actions/setup-node](https://github.com/actions/setup-node)                                                            | TODO        | v7      | TODO    |
| [actions/setup-python](https://github.com/actions/setup-python)                                                        | TODO        | v5      | TODO    |
| [actions/upload-artifact](https://github.com/actions/upload-artifact)                                                  | TODO        | v4      | TODO    |
| [amannn/action-semantic-pull-request](https://github.com/amannn/action-semantic-pull-request)                          | TODO        | v6      | TODO    |
| [anomalyco/opencode/github](https://github.com/anomalyco/opencode)                                                     | TODO        | latest  | TODO    |
| [cachix/install-nix-action](https://github.com/cachix/install-nix-action)                                              | TODO        | v31     | TODO    |
| [conventional-changelog-conventionalcommits](https://www.npmjs.com/package/conventional-changelog-conventionalcommits) | TODO        | 9       | TODO    |
| [DavidAnson/markdownlint-cli2-action](https://github.com/DavidAnson/markdownlint-cli2-action)                          | TODO        | v24     | TODO    |
| [docker/login-action](https://github.com/docker/login-action)                                                          | TODO        | v4      | TODO    |
| [dortort/betterleaks-action](https://github.com/dortort/betterleaks-action)                                            | TODO        | v0.1.0  | TODO    |
| [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain)                                                    | TODO        | 1.98.0  | TODO    |
| [ls-lint/action](https://github.com/ls-lint/action)                                                                    | TODO        | v2      | TODO    |
| [semantic-release](https://www.npmjs.com/package/semantic-release)                                                     | TODO        | 25.0.9  | TODO    |
| [semantic-release-openapi](https://www.npmjs.com/package/semantic-release-openapi)                                     | TODO        | 2.3.6   | TODO    |
| [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache)                                                          | TODO        | v2      | TODO    |
| [taiki-e/install-action](https://github.com/taiki-e/install-action)                                                    | TODO        | v2      | TODO    |
| [tj-actions/changed-files](https://github.com/tj-actions/changed-files)                                                | TODO        | v47.0.6 | TODO    |

## Container images

Source: `Dockerfile`.

| Name                                                       | Description                                      | Version         | License |
| ---------------------------------------------------------- | ------------------------------------------------ | --------------- | ------- |
| [alpine](https://hub.docker.com/_/alpine)                  | Alpine Linux base image (runtime stage).         | 3.21            | TODO    |
| [cargo-chef](https://github.com/LukeMathWalker/cargo-chef) | Caches Rust dependency builds for Docker layers. | latest          | TODO    |
| [rust](https://hub.docker.com/_/rust)                      | Rust build image (build stage).                  | 1.98-alpine3.21 | TODO    |
