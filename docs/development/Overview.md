# Overview

## Source code structure

```text
└── src
    ├── bin
    │   ├── openapi_gen.rs
    │   └── server.rs
    └── lib
        ├── application
        │   ├── port
        │   └── use_case
        ├── domain
        │   ├── alias.rs
        │   ├── model
        │   ├── port
        │   │   └── error.rs
        │   └── service
        └── infrastructure
            ├── configuration.rs
            ├── inbound
            │   └── rest
            │       ├── api_error.rs
            │       ├── bootstrap.rs
            │       └── handler
            ├── logging.rs
            └── outbound
                ├── client
                ├── moka
                │   └── bootstrap.rs
                └── sqlx
                    ├── bootstrap.rs
                    ├── model
                    └── sqlite3
```

| Entity                                             | Description                                                           |
| -------------------------------------------------- | --------------------------------------------------------------------- |
| `docs`                                             |                                                                       |
| `src`                                              |                                                                       |
| `src/bin`                                          |                                                                       |
| `src/lib`                                          |                                                                       |
| `src/lib/application`                              | App Layer                                                             |
| `src/lib/application/port`                         | Interface declaration for usecase (one by file)                       |
| `src/lib/application/use_case`                     | Implementation of the **UseCase**                                     |
| `src/lib/domain`                                   | Domain Layer                                                          |
| `src/lib/domain/alias.rs`                          | Type alias for the project (ex: which integer to use for IDs)         |
| `src/lib/domain/model`                             | Aggregate, Entity, Value object declaration                           |
| `src/lib/domain/port`                              | Port interface declaration                                            |
| `src/lib/domain/port/error.rs`                     | Repository, External service error declaration                        |
| `src/lib/domain/service`                           | Domain Service implementation; see Terms Glossary for more info on it |
| `src/lib/infrastructure/inbound/rest`              | HTTP adapter layer                                                    |
| `src/lib/infrastructure/inbound/rest/api_error.rs` | API Error declaration                                                 |
| `src/lib/infrastructure/inbound/rest/handler`      | HTTP endpoint handler                                                 |
| `src/lib/infrastructure/inbound/rest/bootstrap.rs` | Setup the routes with axum                                            |
| `src/lib/infrastructure/outbound`                  | Outbound Port adapter declaration                                     |
| `src/lib/infrastructure/configuration.rs`          | Configuration related bootstrapping                                   |
| `src/lib/infrastructure/logging.rs`                | Logging related bootstrapping                                         |

## Branch naming and PR title naming

All change types and scopes below match the values enforced by the semantic PR
gating job in `.github/workflows/ci.yml`
(`amannn/action-semantic-pull-request`).

### Branch naming

Format: `{type}({scope})/{description}`, kebab-case.

Examples:

- `feature(core)/serve-media`
- `hotfix(devops)/fix-dockerfile`
- `docs(agent)/spellcheck-dead`

### PR title naming

Format: `{type}({scope}): {description}` — enforced by CI.

Examples:

- `feature(core): serve media via streaming`
- `hotfix(devops): fix Dockerfile registry`

### Scopes

| Scope  | Description                            |
| ------ | -------------------------------------- |
| core   | Core media server functionality        |
| agent  | AI/assistant agent features            |
| devops | CI, builds, Docker, and infrastructure |

### Type prefixes

| Prefix   | Description                                   |
| -------- | --------------------------------------------- |
| feature  | New feature, based on the implementation plan |
| bugfix   | Non-critical bug fixes                        |
| refactor | Code cleanup/restructure                      |
| docs     | Documentation changes                         |
| chore    | Build and maintenance tasks                   |
| release  | Release version bumps, with the version       |
| test     | Tests and test improvements                   |
| hotfix   | Production fixes                              |
