<!-- TODO: add the header assets at .github/assets/logo.png and .github/assets/banner.png (referenced below). -->
<!-- The first line is an HTML comment so markdownlint MD041 stays happy. -->

<a id="readme-top"></a>

<div align="center">
  <a href="https://github.com/Mnemorium/mnemorium">
    <img src=".github/assets/logo.png" alt="Mnemorium logo" width="120" />
  </a>

  <h1 align="center">Mnemorium</h1>

  <p align="center">
    TODO: one-line description of Mnemorium.
  </p>
</div>

<!-- TODO: add the banner asset at .github/assets/banner.png. -->
<p align="center">
  <img src=".github/assets/banner.png" alt="Mnemorium banner" width="100%" />
</p>

<p align="center">
  <a href="https://github.com/Mnemorium/mnemorium/actions/workflows/ci.yml">
    <img src="https://github.com/Mnemorium/mnemorium/actions/workflows/ci.yml/badge.svg" alt="Continuous integration" />
  </a>
  <a href="https://github.com/Mnemorium/mnemorium/releases">
    <img src="https://img.shields.io/github/v/release/Mnemorium/mnemorium" alt="Latest release" />
  </a>
  <a href="https://github.com/Mnemorium/mnemorium/blob/main/LICENSE">
    <img src="https://img.shields.io/badge/license-MIT-yellow.svg" alt="License: MIT" />
  </a>
  <a href="https://www.rust-lang.org/">
    <img src="https://img.shields.io/badge/rust-1.98.0-orange?logo=rust" alt="Rust version" />
  </a>
  <!-- TODO: add a Docker Hub pulls badge and a documentation-site badge once the image -->
  <!-- and the documentation site are published. -->
</p>

<p align="center">
  <a href="docs/development/Overview.md">Documentation</a>
  <span> · </span>
  <a href="https://github.com/Mnemorium/mnemorium/issues/new">Report Bug</a>
  <span> · </span>
  <a href="https://github.com/Mnemorium/mnemorium/issues/new">Request Feature</a>
</p>

## Table of Contents

- [About the Project](#about-the-project)
- [Features](#features)
- [Tech Stack](#tech-stack)
- [Getting Started](#getting-started)
  - [Prerequisites](#prerequisites)
  - [Run with Docker](#run-with-docker)
  - [Run Locally](#run-locally)
  - [Configuration](#configuration)
  - [First-Time Admin](#first-time-admin)
- [Usage](#usage)
- [Development](#development)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [License](#license)
- [Acknowledgments](#acknowledgments)

## About the Project

TODO: describe Mnemorium: what it is, who it is for, and the problem it solves.

<!-- TODO: replace with a real screenshot or a short demo recording. -->
<p align="center">
  <em>Screenshot placeholder.</em>
</p>

Mnemorium is a self-hosted media server designed for home use. Two design goals shape the whole project:

- **One container:** a single Docker container ships the entire backend — no external services.
- **REST-first:** clients talk to the server exclusively over HTTP.

The backend is written in Rust, uses [axum] for HTTP and [sqlx] with [SQLite3] as the single datastore.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Features

TODO: list the features here.

- Placeholder feature.
- Placeholder feature.
- Placeholder feature.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Tech Stack

<details>
<summary>Language and runtime</summary>

- [Rust](https://www.rust-lang.org/) — server implementation (edition 2024).

</details>

<details>
<summary>Web and persistence</summary>

- [axum](https://github.com/tokio-rs/axum) — HTTP routing and request handling.
- [sqlx](https://github.com/launchbadge/sqlx) — async SQL toolkit with compile-time checked queries.
- [SQLite3](https://sqlite.org/) — the single datastore.

</details>

<details>
<summary>API documentation</summary>

- [utoipa](https://github.com/juhaku/utoipa) — compile-time generated OpenAPI specification.

</details>

<details>
<summary>Tooling and operations</summary>

- [Docker](https://www.docker.com/) — packaging and delivery.
- [devenv](https://devenv.sh/) — reproducible development environment.

</details>

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Getting Started

### Prerequisites

TODO: list the prerequisites (Docker for running; Nix and devenv for development).

### Run with Docker

The published image is `wpelletier/mnemorium`.

```sh
docker run --rm -p 4080:4080 wpelletier/mnemorium
```

The server listens on port `4080`. Check that it is up:

```sh
curl http://localhost:4080/health
```

### Run Locally

TODO: expand this section once the local setup steps are settled.

```sh
devenv shell
cargo run --bin server
```

### Configuration

Configuration is layered: the persisted singleton row is the base, the optional `config.yaml` file overrides it, and the
environment overrides both. Environment variables use the `mnemorium` prefix and the `__` separator.

TODO: document the configuration keys, the `config.yaml` shape, and the exact environment variable names.

### First-Time Admin

On the first run the system creates a Root Admin account and logs a generated default password to standard output. The
default password stops being revealed once it has been changed. See
[UC-003 – Initialize Root Admin](docs/development/UseCases.md#uc-003---initialize-root-admin) for the full flow.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Usage

TODO: add a realistic end-to-end usage example.

The API is served under `/api/v1`. The OpenAPI specification is generated to
[`docs/development/api/openapi.json`](docs/development/api/openapi.json), and the running server can expose an
interactive Swagger UI.

<!-- TODO: document the Swagger UI path once it is confirmed. -->

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Development

Enter the environment first: `devenv shell`.

| Task      | Command                                                                                  |
| --------- | ---------------------------------------------------------------------------------------- |
| Build     | `cargo build` (server: `cargo build --bin server`)                                       |
| Run       | `cargo run --bin server` (listens on `:4080`; health `GET /health`)                      |
| Test      | `cargo test` (unit + integration)                                                        |
| Coverage  | `cargo llvm-cov --lib` (gate ≥ 80% lines/regions/functions)                              |
| E2E       | `pytest test/e2e` (needs the Docker server named `mnemorium-e2e`)                        |
| Lint      | `cargo clippy --all-targets --all-features -- -D warnings`; `cargo fmt --all -- --check` |
| OpenAPI   | `cargo run --bin openapi_gen` (writes `docs/development/api/openapi.json`)               |
| Full gate | `devenv test` (all pre-commit hooks)                                                     |

The module and layer map lives in [Overview](docs/development/Overview.md). That document also defines the branch, PR
title, and commit conventions; see the [Technical Design Document](docs/development/TechnicalDesign.md) § 5 for the
testing strategy and § 1 for the Rust and SQL conventions.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Roadmap

- [ ] Placeholder roadmap item.
- [ ] Placeholder roadmap item.

See the [open issues](https://github.com/Mnemorium/mnemorium/issues) for a full list of proposed features and known
issues.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Contributing

TODO: describe how to contribute and point to the canonical documentation.

Contributions are welcome. Read [AGENTS.md](AGENTS.md) before opening a pull request; it summarizes the build, test, and
linting commands, the code style, and the architecture. Commit messages and PR titles follow
[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## License

Distributed under the MIT License. See [`LICENSE`](LICENSE) for more information.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Acknowledgments

TODO: credit the people, projects, and resources that helped.

- Placeholder acknowledgment.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- Reference-style links -->

[axum]: https://github.com/tokio-rs/axum
[sqlx]: https://github.com/launchbadge/sqlx
[SQLite3]: https://sqlite.org/
