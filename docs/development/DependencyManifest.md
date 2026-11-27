# Dependency manifest

The direct dependencies of this project, grouped by source. The CI rules that keep them resolvable and trustworthy —
`DEPS-001`–`DEPS-004` — are in [TechnicalDesign.md](TechnicalDesign.md) § 5 (Dependencies & Dev Environment). Update the
tables when a dependency is added, removed or upgraded; transitive dependencies are out of scope (the lockfiles and
`cargo audit` cover those).

Versions are the resolved ones (lockfile or Nix store), not the declared constraints. When a tool is declared by more
than one source it is listed once, under the source that pins it (normally `devenv.nix`). Cells marked `TODO` still need
to be filled in.

Columns: **Name**, **Description**, **Version**, **License**.

## 1. Rust

Source: `Cargo.toml`.

---

### 1.1 Runtime

| Name                                                              | Description                                                                     | Version | License           |
| ----------------------------------------------------------------- | ------------------------------------------------------------------------------- | ------- | ----------------- |
| [anyhow](https://crates.io/crates/anyhow)                         | Flexible concrete error type built on `std::error::Error`.                      | 1.0.104 | MIT OR Apache-2.0 |
| [argon2](https://crates.io/crates/argon2)                         | Pure Rust implementation of the Argon2 password hashing function.               | 0.6.0   | MIT OR Apache-2.0 |
| [axum](https://crates.io/crates/axum)                             | HTTP routing and request handling library focused on ergonomics and modularity. | 0.8.9   | MIT               |
| [base64](https://crates.io/crates/base64)                         | Encodes and decodes base64 as bytes or UTF-8.                                   | 0.23.1  | MIT OR Apache-2.0 |
| [chrono](https://crates.io/crates/chrono)                         | Date and time library for Rust.                                                 | 0.4.45  | MIT OR Apache-2.0 |
| [email_address](https://crates.io/crates/email_address)           | RFC-compliant `EmailAddress` newtype.                                           | 0.2.9   | MIT               |
| [envy](https://crates.io/crates/envy)                             | Deserializes a struct from prefixed environment variables.                      | 0.4.2   | MIT               |
| [fast_qr](https://crates.io/crates/fast_qr)                       | QR code encoder whose terminal renderer prints the setup token.                 | 0.14.0  | MIT               |
| [governor](https://crates.io/crates/governor)                     | Rate-limiting quotas and keyed state behind the login limiters.                 | 0.10.4  | MIT               |
| [hmac](https://crates.io/crates/hmac)                             | Keyed-hash message authentication over SHA-256 for the setup-token derivation.  | 0.13.0  | MIT OR Apache-2.0 |
| [image](https://crates.io/crates/image)                           | Image decoding and metadata: dimensions and EXIF orientation.                   | 0.25.10 | MIT OR Apache-2.0 |
| [infer](https://crates.io/crates/infer)                           | Infers a file type from its magic number signature.                             | 0.22.0  | MIT               |
| [jsonwebtoken](https://crates.io/crates/jsonwebtoken)             | Creates and decodes JWTs in a strongly typed way.                               | 11.0.0  | MIT               |
| [notify](https://crates.io/crates/notify)                         | Cross-platform filesystem notification library.                                 | 8.2.1   | CC0-1.0           |
| [rand](https://crates.io/crates/rand)                             | Random number generators and other randomness functionality.                    | 0.10.2  | MIT OR Apache-2.0 |
| [reqwest](https://crates.io/crates/reqwest)                       | Higher level HTTP client library.                                               | 0.13.4  | MIT OR Apache-2.0 |
| [serde](https://crates.io/crates/serde)                           | Generic serialization and deserialization framework.                            | 1.0.229 | MIT OR Apache-2.0 |
| [serde_json](https://crates.io/crates/serde_json)                 | JSON serialization file format.                                                 | 1.0.151 | MIT OR Apache-2.0 |
| [sha2](https://crates.io/crates/sha2)                             | Pure Rust implementation of the SHA-2 cryptographic hash algorithms.            | 0.11.0  | MIT OR Apache-2.0 |
| [sqlx](https://crates.io/crates/sqlx)                             | Async, pure Rust SQL toolkit with compile-time checked queries and no DSL.      | 0.9.0   | MIT OR Apache-2.0 |
| [thiserror](https://crates.io/crates/thiserror)                   | Derives `std::error::Error` implementations.                                    | 2.0.20  | MIT OR Apache-2.0 |
| [tokio](https://crates.io/crates/tokio)                           | Event-driven, non-blocking I/O platform for asynchronous applications.          | 1.53.1  | MIT               |
| [tower_governor](https://crates.io/crates/tower_governor)         | Per-client rate limiting for Tower services, backed by `governor`.              | 0.8.0   | MIT OR Apache-2.0 |
| [tracing](https://crates.io/crates/tracing)                       | Application-level tracing for Rust.                                             | 0.1.44  | MIT               |
| [tracing-appender](https://crates.io/crates/tracing-appender)     | File appenders and non-blocking writers for `tracing`.                          | 0.2.5   | MIT               |
| [tracing-subscriber](https://crates.io/crates/tracing-subscriber) | Composing and implementing `tracing` subscribers.                               | 0.3.23  | MIT               |
| [utoipa](https://crates.io/crates/utoipa)                         | Compile-time generated OpenAPI documentation for Rust.                          | 6.0.0   | MIT OR Apache-2.0 |
| [utoipa-axum](https://crates.io/crates/utoipa-axum)               | Registers axum routes and collects their OpenAPI path items and schemas.        | 0.3.0   | MIT OR Apache-2.0 |
| [utoipa-scalar](https://crates.io/crates/utoipa-scalar)           | Serves the interactive Scalar API reference; optional, enabled by `api-docs`.   | 0.4.0   | MIT OR Apache-2.0 |
| [yaml_serde](https://crates.io/crates/yaml_serde)                 | YAML serialization and deserialization; the maintained fork of `serde_yaml`.    | 0.10.7  | MIT OR Apache-2.0 |

---

### 1.2 Development

| Name                                                  | Description                                                           | Version | License           |
| ----------------------------------------------------- | --------------------------------------------------------------------- | ------- | ----------------- |
| [mockall](https://crates.io/crates/mockall)           | Powerful mock object library for Rust.                                | 0.15.0  | MIT OR Apache-2.0 |
| [rstest](https://crates.io/crates/rstest)             | Fixture-based test framework with table-driven tests.                 | 0.26.1  | MIT OR Apache-2.0 |
| [tempfile](https://crates.io/crates/tempfile)         | Manages temporary files and directories.                              | 3.27.0  | MIT OR Apache-2.0 |
| [tower](https://crates.io/crates/tower)               | Modular, reusable components for building robust clients and servers. | 0.5.3   | MIT               |
| [tracing-test](https://crates.io/crates/tracing-test) | Captures `tracing` output for a test through one global subscriber.   | 0.2.6   | MIT               |
| [wiremock](https://crates.io/crates/wiremock)         | HTTP mocking to test Rust applications.                               | 0.6.5   | MIT OR Apache-2.0 |

---

## 2. Tooling

---

### 2.1 Nix / devenv

Source: `devenv.nix`.

| Name                                                              | Description                                                                                                           | Version | License |
| ----------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- | ------- | ------- |
| [betterleaks](https://github.com/dortort/betterleaks)             | Secret scanning in pre-commit and CI.                                                                                 | 1.8.1   | TODO    |
| [cargo-about](https://github.com/EmbarkStudios/cargo-about)       | Generates the third-party license notices in `THIRD_PARTY_NOTICES.txt`.                                               | 0.9.0   | TODO    |
| [cargo-deny](https://github.com/EmbarkStudios/cargo-deny)         | Audits `Cargo.lock` for advisories, licenses, bans, and sources.                                                      | 0.20.2  | TODO    |
| [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov)       | Coverage instrumentation and reporting for Rust.                                                                      | 0.9.0   | TODO    |
| [cargo-machete](https://github.com/bnjbvr/cargo-machete)          | Reports direct dependencies no source file uses; pre-commit and CI.                                                   | 0.9.2   | MIT     |
| [ffmpeg-headless](https://ffmpeg.org/)                            | Headless FFmpeg build providing the `ffprobe` binary.                                                                 | 6.1.2   | TODO    |
| [git](https://git-scm.com/)                                       | Version control.                                                                                                      | 2.55.0  | TODO    |
| [hadolint](https://github.com/hadolint/hadolint)                  | Dockerfile linter; the pre-commit hook and the Docker workflow.                                                       | 2.13.1  | GPL-3.0 |
| [llvm](https://llvm.org/)                                         | LLVM tools used by coverage (`llvm-cov`, `llvm-profdata`).                                                            | 21.1.8  | TODO    |
| [ls-lint](https://ls-lint.org/)                                   | File and directory naming linter.                                                                                     | 2.3.1   | TODO    |
| [nixfmt](https://github.com/NixOS/nixfmt)                         | Nix code formatter.                                                                                                   | 1.4.0   | TODO    |
| [nodejs](https://nodejs.org/)                                     | Node.js runtime for tooling.                                                                                          | 24.19.0 | TODO    |
| [prettier](https://prettier.io/)                                  | Markdown formatter.                                                                                                   | 3.8.3   | TODO    |
| [Python toolchain](https://www.python.org/)                       | Python runtime for tooling and tests.                                                                                 | 3.14.6  | TODO    |
| [Rust toolchain](https://www.rust-lang.org/)                      | Rust compiler and standard tooling.                                                                                   | 1.98.0  | TODO    |
| [schemathesis](https://github.com/schemathesis/schemathesis)      | Property-based API tests against the served OpenAPI document.                                                         | 4.33.1  | MIT     |
| [semgrep](https://semgrep.dev/)                                   | Static-analysis (SAST) scanner; CI and the `security:semgrep` task, with the repository rules in `security/semgrep/`. | 1.172.0 | TODO    |
| [shellcheck](https://github.com/koalaman/shellcheck)              | Shell script linter.                                                                                                  | 0.11.0  | TODO    |
| [shfmt](https://github.com/mvdan/sh)                              | Shell script formatter.                                                                                               | 3.13.1  | TODO    |
| [sqlfluff](https://sqlfluff.com/)                                 | SQL linter and formatter (SQLite dialect).                                                                            | 4.3.0   | TODO    |
| [sqlite](https://sqlite.org/)                                     | SQLite command-line shell.                                                                                            | 3.51.2  | TODO    |
| [sqlx-cli](https://github.com/launchbadge/sqlx)                   | SQLx migrations and database CLI.                                                                                     | 0.9.0   | TODO    |
| [taplo](https://taplo.tamasfe.dev/)                               | TOML formatter.                                                                                                       | 0.10.0  | TODO    |
| [xdg-utils](https://www.freedesktop.org/wiki/Software/xdg-utils/) | Desktop integration helpers (opens reports in a browser).                                                             | 1.2.1   | TODO    |
| [yamllint](https://github.com/adrienverge/yamllint)               | YAML linter.                                                                                                          | 1.37.1  | TODO    |

The repository rules in `security/semgrep/` complement the registry packs with the checks that name this server's own
sinks: SQL interpolation, process spawns, secret-bearing logs, JWT validation, path joins, file modes, weak randomness
and password hashing, direct stdout, body limits and client-sized allocations. A rule's `severity` is its gate: an
`ERROR` finding fails the `semgrep` CI job, while a `WARNING` finding is reported in the SARIF artifact without failing
until the finding it names is remediated.

---

### 2.2 Python

Source: `requirements.txt`.

| Name                                                         | Description                                                       | Version | License      |
| ------------------------------------------------------------ | ----------------------------------------------------------------- | ------- | ------------ |
| [Jinja2](https://pypi.org/project/Jinja2/)                   | Template engine the coverage report renders with.                 | 3.1.6   | BSD-3-Clause |
| [Markdown](https://pypi.org/project/Markdown/)               | Renders the specification's Markdown in the coverage report.      | 3.11    | BSD-3-Clause |
| [mkdocs](https://pypi.org/project/mkdocs/)                   | Static site generator for the documentation site.                 | 1.6.0   | BSD-2-Clause |
| [mkdocs-material](https://pypi.org/project/mkdocs-material/) | Material Design theme for MkDocs.                                 | 9.7.7   | MIT          |
| [mkdocs_puml](https://pypi.org/project/mkdocs-puml/)         | Renders PlantUML diagrams from fenced blocks.                     | 2.3.0   | MIT          |
| [pytest](https://pypi.org/project/pytest/)                   | Python testing framework used by the black-box suites.            | 9.1.1   | MIT          |
| [PyYAML](https://pypi.org/project/PyYAML/)                   | YAML parser the configuration harness renders `config.yaml` with. | 6.0.3   | MIT          |
| [requests](https://pypi.org/project/requests/)               | HTTP library for Python.                                          | 2.34.2  | Apache-2.0   |
| [ruff](https://pypi.org/project/ruff/)                       | Fast Python linter and formatter.                                 | 0.16.5  | MIT          |
| [vulture](https://pypi.org/project/vulture/)                 | Finds unused functions, constants and attributes in `test/`.      | 2.16    | MIT          |

---

### 2.3 CI

Source: `.github/workflows/*`.

| Name                                                                                                                   | Description                                              | Version | License    |
| ---------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------- | ------- | ---------- |
| [@opencode/cli](https://www.npmjs.com/package/@opencode/cli)                                                           | Runs the unattended triage sweep in the triage workflow. | 2.0.22  | TODO       |
| [@semantic-release-plus/docker](https://www.npmjs.com/package/@semantic-release-plus/docker)                           | TODO                                                     | 3.1.3   | TODO       |
| [@semantic-release/changelog](https://www.npmjs.com/package/@semantic-release/changelog)                               | TODO                                                     | 7.0.0   | TODO       |
| [@semantic-release/exec](https://www.npmjs.com/package/@semantic-release/exec)                                         | TODO                                                     | 7.1.0   | TODO       |
| [@semantic-release/git](https://www.npmjs.com/package/@semantic-release/git)                                           | TODO                                                     | 11.0.1  | TODO       |
| [actions/cache](https://github.com/actions/cache)                                                                      | Caches the local BuildKit layer cache for image builds.  | v4.3.0  | MIT        |
| [actions/checkout](https://github.com/actions/checkout)                                                                | TODO                                                     | v7.0.1  | TODO       |
| [actions/create-github-app-token](https://github.com/actions/create-github-app-token)                                  | TODO                                                     | v3.2.0  | TODO       |
| [actions/setup-node](https://github.com/actions/setup-node)                                                            | TODO                                                     | v7.0.0  | TODO       |
| [actions/setup-python](https://github.com/actions/setup-python)                                                        | TODO                                                     | v7.0.0  | TODO       |
| [actions/upload-artifact](https://github.com/actions/upload-artifact)                                                  | TODO                                                     | v7      | TODO       |
| [amannn/action-semantic-pull-request](https://github.com/amannn/action-semantic-pull-request)                          | TODO                                                     | v6      | TODO       |
| [astral-sh/ruff-action](https://github.com/astral-sh/ruff-action)                                                      | Runs `ruff` in the Python workflow.                      | v3.6.1  | Apache-2.0 |
| [cachix/install-nix-action](https://github.com/cachix/install-nix-action)                                              | TODO                                                     | v31     | TODO       |
| [conventional-changelog-conventionalcommits](https://www.npmjs.com/package/conventional-changelog-conventionalcommits) | TODO                                                     | 9.3.1   | TODO       |
| [DavidAnson/markdownlint-cli2-action](https://github.com/DavidAnson/markdownlint-cli2-action)                          | TODO                                                     | v24     | TODO       |
| [docker/build-push-action](https://github.com/docker/build-push-action)                                                | Builds the test image with a local BuildKit cache.       | v7      | Apache-2.0 |
| [docker/login-action](https://github.com/docker/login-action)                                                          | TODO                                                     | v4      | TODO       |
| [docker/setup-buildx-action](https://github.com/docker/setup-buildx-action)                                            | BuildKit builder setup for cached image builds.          | v4      | Apache-2.0 |
| [dortort/betterleaks-action](https://github.com/dortort/betterleaks-action)                                            | TODO                                                     | v0.1.0  | TODO       |
| [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain)                                                    | TODO                                                     | 1.98.0  | TODO       |
| [hadolint/hadolint-action](https://github.com/hadolint/hadolint-action)                                                | Lints `Dockerfile` in the Docker workflow.               | v3.5.0  | MIT        |
| [ibiqlik/action-yamllint](https://github.com/ibiqlik/action-yamllint)                                                  | Lints YAML in the YAML workflow.                         | v3.1.1  | MIT        |
| [ls-lint/action](https://github.com/ls-lint/action)                                                                    | TODO                                                     | v2      | TODO       |
| [semantic-release](https://www.npmjs.com/package/semantic-release)                                                     | TODO                                                     | 25.0.9  | TODO       |
| [semgrep/semgrep](https://hub.docker.com/r/semgrep/semgrep)                                                            | Image the Docker workflow's scan runs.                   | 1.172.0 | TODO       |
| [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache)                                                          | TODO                                                     | v2      | TODO       |
| [taiki-e/install-action](https://github.com/taiki-e/install-action)                                                    | TODO                                                     | v2      | TODO       |

`@opencode/cli` is installed by the triage workflow at the exact version `2.0.22` on `actions/setup-node` (`v7.0.0`,
Node `24.19.0`). Its transitive dependency tree resolves fresh from the npm registry, and its postinstall script
downloads a platform-native `opencode` binary; neither is lockfile-pinned. The npm tree and the postinstall binary are
documented `DEPS-003` residuals; the `setup-node` install of the dev environment's Node is a documented `DEPS-004`
residual, because the workflow restates the toolchain rather than resolving it through `devenv.lock`. The other
workflows provision Python and Node with `actions/setup-python` and `actions/setup-node`.

The `yaml` workflow does not provision a Python toolchain: `ibiqlik/action-yamllint` shells out to the `yamllint` the
`ubuntu-latest` image ships (1.38.0), which the workflow selects by pinning the action's commit rather than the linter's
version. The dev environment resolves yamllint from the locked nixpkgs input instead, so a new yamllint release reaches
CI before it reaches the pre-commit hook — a `DEPS-004` residual.

---

## 3. Container images

Source: `Dockerfile`.

| Name                                                       | Description                                       | Version         | License |
| ---------------------------------------------------------- | ------------------------------------------------- | --------------- | ------- |
| [alpine](https://hub.docker.com/_/alpine)                  | Alpine Linux base image (runtime stage).          | 3.24            | TODO    |
| [cargo-chef](https://github.com/LukeMathWalker/cargo-chef) | Caches Rust dependency builds for Docker layers.  | 0.1.78          | TODO    |
| [ffmpeg](https://ffmpeg.org/)                              | Runtime media tooling; ships `ffprobe` on `PATH`. | 8.1.3-r0        | TODO    |
| [rust](https://hub.docker.com/_/rust)                      | Rust build image (build stage).                   | 1.98-alpine3.21 | TODO    |

---

## 4. Runtime system binaries

Source: the runtime stage of `Dockerfile`.

| Name                                       | Description                                                                                  | Provided by                                                              | License |
| ------------------------------------------ | -------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ | ------- |
| [ffprobe](https://ffmpeg.org/ffprobe.html) | Video metadata probe resolved from `PATH` by `media_probe.rs` on every `video/*` completion. | The `ffmpeg` package (`ffmpeg=8.1.3-r0`) installed by the runtime stage. | TODO    |

A binary a shipped container resolves from `PATH` at run time is a runtime dependency, not a build one: the runtime
stage image must provide it. `ffprobe` ships with `ffmpeg`; the runtime stage installs `ffmpeg=8.1.3-r0`.
