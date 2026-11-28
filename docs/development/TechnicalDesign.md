# Technical Design Document

## Purpose

The audience is everyone who changes the project: maintainers, review agents, and implementation agents. Rules here are
written to be cited and checked.

---

## How this document is extended

The rulebook is a set of numbered sections. The two sections before the first numbered one — `Purpose` and this one —
are process material: they carry no number and no rule. A section's number follows its order in this document; numbers
are append-only and never reused.

---

### Section anatomy

Each section states:

1. **Purpose** — one short paragraph: what the section governs.
2. **Rules** — the normative statements, one per row of the [rule table](#rule-model).
3. **Details** — the numbered sections after the table that hold code, examples and reference material.
4. **References** — links to adjacent documents or external sources.

---

### Rule model

A section presents its rules as a **rule table**:

| ID             | Section       | Rule                                                                                                                             | More info | Enforced by          |
| -------------- | ------------- | -------------------------------------------------------------------------------------------------------------------------------- | --------- | -------------------- |
| `STY-RUST-001` | REST handlers | Split the handler directory (`src/lib/infrastructure/inbound/rest/handler`) into subdirectories, one folder per bounded context. |           | `check_sty_rust_001` |

- **ID** — a stable identifier (`STY-RUST-001`, `STY-SQL-001`, ...) that names one rule; its prefix is unique to the
  section that owns the rule. A merged or removed rule is closed by an explicit renumbering change that shifts the
  following IDs down and updates every citation in the same commit; the sequence is always gap-free.
- **Section** — the topic the rule belongs to.
- **Rule** — one line, imperative and testable.
- **More info** — a link to the numbered detail section below the table that holds the rule's code, rationale or
  reference material; empty when the rule is self-contained.
- **Enforced by** — the mechanical mechanism that enforces the rule: a `check_*` function in `script/rule_enforcer.sh`,
  or a linter such as `sqlfluff`; empty when no such mechanism exists, and those rules stay listed by
  `script/rule_enforcer.sh uncovered`.

The numbered detail sections that follow the table carry everything non-normative: code, examples, tables and
explanation. A numbered detail sits one heading level below the heading that owns it: `#### 1.`, `#### 2.` under a `###`
topic heading, or `### 1.`, `### 2.` directly under the section's `##` heading. Nest `##### 2.1`, and so on; numbering
restarts at 1 within each `###` group — or within the section when it has none; detail numbers are append-only.

Only normative statements carry IDs; every section present in this document uses the table form. A newly added section
follows the same shape.

Cite a rule as `TechnicalDesign.md § 1 (Code Style Guidelines), STY-RUST-001`, or a detail section by its number, e.g.
`§ 2.6`.

---

### Adding or migrating a section

1. Append the section after the last numbered section; numbers follow document order and are never reused.
2. Write the section following the [section anatomy](#section-anatomy). A detail it leaves out may stay in an adjacent
   document the section links to, as Architecture links to [Overview.md](Overview.md). Add the section to the
   `AGENTS.md` router, or refresh its existing entry, in the same change.
3. To migrate: move the content, re-level its headings under the section, keep every existing anchor working, then
   delete the source document and update every reference — including `AGENTS.md` and `mkdocs.yml` — in the same change.
4. Give the section's rules a prefix no other section uses (see [Rule model](#rule-model)).
5. Run the Markdown gates: the `lint:md` and `fmt:md` devenv tasks.

---

## Code Style Guidelines

---

### Rust

| ID             | Section                      | Rule                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | More info                                                        | Enforced by                                                                                            |
| -------------- | ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| `STY-RUST-001` | REST handlers                | Split the handler directory (`src/lib/infrastructure/inbound/rest/handler`) into subdirectories, one folder per bounded context.                                                                                                                                                                                                                                                                                                                                                                                                       |                                                                  | `check_sty_rust_001`                                                                                   |
| `STY-RUST-002` | REST handlers                | Each endpoint lives in its own file; a bounded-context folder holds only endpoint files.                                                                                                                                                                                                                                                                                                                                                                                                                                               |                                                                  | `check_sty_rust_002`                                                                                   |
| `STY-RUST-003` | REST handlers                | A handler extracts `State<AppState>` and resolves its use case through the bounded context's factory (`state.<context>_use_case_factory().<use_case>()`) before calling `execute`; it never receives a pre-built use case.                                                                                                                                                                                                                                                                                                             |                                                                  | `check_sty_rust_003`; `State<AppState>` by construction (factories are reachable only from `AppState`) |
| `STY-RUST-004` | REST handlers                | File name is `<method>_<resource>.rs`, e.g. `post_note.rs`, `get_note.rs`.                                                                                                                                                                                                                                                                                                                                                                                                                                                             |                                                                  | `check_sty_rust_004`                                                                                   |
| `STY-RUST-005` | REST handlers                | The handler function is named like the file name, e.g. `post_note`, `get_note`.                                                                                                                                                                                                                                                                                                                                                                                                                                                        |                                                                  | `check_sty_rust_005`                                                                                   |
| `STY-RUST-006` | REST handlers                | Declare, in order: 1. the request body object (only for methods that carry a body: `POST`, `PUT`, `PATCH`), 2. the query object, 3. the response body object, 4. the use-case-error-to-`ApiError` mapping, and 5. last the endpoint handler.                                                                                                                                                                                                                                                                                           |                                                                  | `check_sty_rust_006`                                                                                   |
| `STY-RUST-007` | REST handlers                | Query parameters: struct named `<Context>Query`, deriving at least `Deserialize` and `IntoParams`.                                                                                                                                                                                                                                                                                                                                                                                                                                     |                                                                  | `check_sty_rust_007`                                                                                   |
| `STY-RUST-008` | REST handlers                | Request body: struct named `<Context>Request`, deriving at least `Deserialize` and `ToSchema`.                                                                                                                                                                                                                                                                                                                                                                                                                                         |                                                                  | `check_sty_rust_008`                                                                                   |
| `STY-RUST-009` | REST handlers                | Response body: struct named `<Context>Response`, deriving at least `Serialize` and `ToSchema`.                                                                                                                                                                                                                                                                                                                                                                                                                                         |                                                                  | `check_sty_rust_009`                                                                                   |
| `STY-RUST-010` | REST handlers                | Even when a payload has a single attribute, always prefer a struct over a raw return/parameter/query value.                                                                                                                                                                                                                                                                                                                                                                                                                            |                                                                  | `check_sty_rust_010`                                                                                   |
| `STY-RUST-011` | Error handling               | **`ApiError`** — declared in `src/lib/infrastructure/inbound/rest/api_error.rs`. Its variants map one to one to HTTP status codes (e.g. `Conflict`, `BadRequest`, `InternalServerError`). It is **not** derived with `thiserror`; it is an HTTP transport concern, not a domain error.                                                                                                                                                                                                                                                 | [§ 2.4](#24-apierror-to-axum-response)                           | `check_sty_rust_011`                                                                                   |
| `STY-RUST-012` | Error handling               | **Domain error** — for failure when initialising or updating a domain model. Declared **before the model struct, in the same file** as the model, e.g. in `src/lib/domain/model/user.rs`.                                                                                                                                                                                                                                                                                                                                              | [§ 2.1](#21-domain-error)                                        | `check_sty_rust_012`                                                                                   |
| `STY-RUST-013` | Error handling               | **Use Case error** — one enum per use case, declared in `src/lib/application/port`, with an `Unknown(_)` variant and an invalid-parameter variant for each client-supplied parameter the use case validates. A use-case error may model a missing entity (e.g. `NoSuchUser`) for the inbound boundary to map to `404`; absence is a value only at an outbound port.                                                                                                                                                                    | [§ 2.2](#22-use-case-error)                                      | `check_sty_rust_013`                                                                                   |
| `STY-RUST-014` | Error handling               | **Port errors** (Repository, External Service) are declared in `src/lib/domain/port/error.rs`. They do **not** map directly to a use-case error; the use case translates them. A port error type is not one-per-port — it can serve multiple outbound ports of the same family, so every repository shares `RepositoryError` for example.                                                                                                                                                                                              | [§ 2.6](#26-port-errors)                                         | `check_sty_rust_014`                                                                                   |
| `STY-RUST-015` | Error handling               | **A missing entity is not an outbound-port error.** At an outbound port (Repository, External Service, File Storage, and any other outbound-port family), a missing entity is a valid outcome: return `Option`/`None` (or a corresponding non-error type) — never a port error variant. A repository expresses absence as an empty `search` result or `Ok(false)` from `delete`; a path-addressed port expresses a missing target as `Ok(false)` or `Ok(None)`; see [Repository](#8-repository) and [File Storage](#265-file-storage). | [§ 2.6](#26-port-errors)                                         | `check_sty_rust_015`                                                                                   |
| `STY-RUST-016` | Domain models                | **Constructor**: `new` when infallible, `try_new` when it can fail; it returns `Result<Self, _>` and performs validation.                                                                                                                                                                                                                                                                                                                                                                                                              | [§ 3.1](#31-getter-and-setter)                                   | `check_sty_rust_016`                                                                                   |
| `STY-RUST-017` | Domain models                | **Getter**: `<field>(&self) -> <field type>`. Return a borrowed reference (`&str`, `Option<&str>`) or a `Copy` value type — never an owned clone.                                                                                                                                                                                                                                                                                                                                                                                      | [§ 3.1](#31-getter-and-setter)                                   | `check_sty_rust_017`                                                                                   |
| `STY-RUST-018` | Domain models                | **Setter**: `set_<field>(&mut self, <value>)`. Return `Result<(), _>` when the field is validated, `()` otherwise.                                                                                                                                                                                                                                                                                                                                                                                                                     | [§ 3.1](#31-getter-and-setter)                                   | `check_sty_rust_018`                                                                                   |
| `STY-RUST-019` | Error handling               | Only the variants inbound middleware explicitly recognizes as an unauthenticated caller (for example `InvalidClaims`, `InvalidToken`, `TokenExpired`) become a client error. Every other variant — including `OperationFailed` and `Unknown` — is an uncaught error: the error handler, or the project's equivalent path, serves `500`, never a `401`.                                                                                                                                                                                 | [§ 2.7](#27-inbound-middleware)                                  |                                                                                                        |
| `STY-RUST-020` | Error handling               | Match the port error exhaustively. Port errors are `#[non_exhaustive]`, so end the match with a catch-all arm that defaults to `500`; never let a wildcard arm collapse a server-side failure into a misleading `401`.                                                                                                                                                                                                                                                                                                                 | [§ 2.7](#27-inbound-middleware)                                  | `check_sty_rust_020`                                                                                   |
| `STY-RUST-021` | SQL data models              | Name the enum after the attribute it represents, in `UpperCamelCase`, e.g. `Role` for the `role` column of table `user` used in model `User`.                                                                                                                                                                                                                                                                                                                                                                                          | [§ 4.1](#41-enum-for-a-check-constraint)                         | `check_sty_rust_021`                                                                                   |
| `STY-RUST-022` | SQL data models              | Derive `sqlx::Type` with `#[sqlx(rename_all = "UPPERCASE")]` to match the uppercase constraint strings required by the SQL section.                                                                                                                                                                                                                                                                                                                                                                                                    | [§ 4.1](#41-enum-for-a-check-constraint)                         | `check_sty_rust_022`                                                                                   |
| `STY-RUST-023` | SQL data models              | Name variants in `UpperCamelCase`, one per allowed constraint value.                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | [§ 4.1](#41-enum-for-a-check-constraint)                         | `check_sty_rust_023`                                                                                   |
| `STY-RUST-024` | Testing                      | Name tests `<UnitOfWork>_<Scenario>_<ExpectedResult>`, e.g. `apply_discount_code_valid_code_reduces_total_price`.                                                                                                                                                                                                                                                                                                                                                                                                                      | [§ 5](#5-testing)                                                | `check_sty_rust_024`                                                                                   |
| `STY-RUST-025` | Testing                      | Follow the Arrange–Act–Assert (AAA) pattern.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | [§ 5](#5-testing)                                                | `check_sty_rust_025`                                                                                   |
| `STY-RUST-026` | Testing                      | Do not over-abstract test setup into deeply nested helper functions or distant global state.                                                                                                                                                                                                                                                                                                                                                                                                                                           | [§ 5](#5-testing)                                                | `check_sty_rust_026`                                                                                   |
| `STY-RUST-027` | Testing                      | Keep tests deterministic and linear: no control flow (`if`, `match`, loops) in a test body, except inside a closure the test passes (a mock's `returning`, an iterator adapter).                                                                                                                                                                                                                                                                                                                                                       | [§ 5](#5-testing)                                                | `check_sty_rust_027`                                                                                   |
| `STY-RUST-028` | Testing                      | Prefer data-driven tests with `rstest` when possible.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | [§ 5](#5-testing)                                                | `check_sty_rust_028`                                                                                   |
| `STY-RUST-029` | Testing                      | A helper that builds a value from no test input is an rstest `#[fixture]` function the test injects; a helper that takes a runtime parameter is a constructor (rstest cannot inject it), and a helper that returns nothing is setup or an action written inline, except the datastore seeds of `STY-RUST-088`.                                                                                                                                                                                                                         | [§ 5](#5-testing)                                                | `check_sty_rust_029`                                                                                   |
| `STY-RUST-030` | Unit of Work                 | `commit` and `rollback` consume the unit of work.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      | [§ 7.1](#71-lifecycle)                                           | `check_sty_rust_030`                                                                                   |
| `STY-RUST-031` | Unit of Work                 | A rollback failure returns the **original** business error.                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | [§ 7.1](#71-lifecycle)                                           | `check_sty_rust_031`                                                                                   |
| `STY-RUST-032` | Unit of Work                 | A commit failure maps to the use-case `Unknown(_)`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | [§ 7.1](#71-lifecycle)                                           | `check_sty_rust_032`                                                                                   |
| `STY-RUST-033` | Unit of Work                 | Repository views borrow the unit of work mutably; drop them before `commit`/`rollback`.                                                                                                                                                                                                                                                                                                                                                                                                                                                | [§ 7.1](#71-lifecycle)                                           | `check_sty_rust_033`                                                                                   |
| `STY-RUST-034` | Unit of Work                 | Repository accessors return a short-lived view that borrows the unit of work.                                                                                                                                                                                                                                                                                                                                                                                                                                                          | [§ 7.2](#72-context-views)                                       | `check_sty_rust_034`                                                                                   |
| `STY-RUST-035` | Unit of Work                 | A context view exposes only its own bounded context's repositories.                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | [§ 7.2](#72-context-views)                                       |                                                                                                        |
| `STY-RUST-036` | Unit of Work                 | Repositories are obtained **only** from a unit of work; concrete sqlx adapters never leave infrastructure.                                                                                                                                                                                                                                                                                                                                                                                                                             | [§ 7.2](#72-context-views)                                       | `check_sty_rust_036`                                                                                   |
| `STY-RUST-037` | Unit of Work                 | Every repository comes from the **same** unit of work (one `begin`/`begin_write` call); opening a second unit of work would be a second transaction and break atomicity.                                                                                                                                                                                                                                                                                                                                                               | [§ 7.3](#73-cross-context-use-cases)                             | `check_sty_rust_037`                                                                                   |
| `STY-RUST-038` | Unit of Work                 | Cross bounded context writes stay atomic.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | [§ 7.3](#73-cross-context-use-cases)                             |                                                                                                        |
| `STY-RUST-039` | Unit of Work                 | Each accessor borrows `&mut self`, so repositories are requested **sequentially**, never held two at a time.                                                                                                                                                                                                                                                                                                                                                                                                                           | [§ 7.3](#73-cross-context-use-cases)                             | `check_sty_rust_039`                                                                                   |
| `STY-RUST-040` | Unit of Work                 | Declare the `UnitOfWork` trait first in `domain/port/unit_of_work.rs`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | [§ 7.4](#74-declaration-order-in-the-unit-of-work-port-file)     | `check_sty_rust_040`                                                                                   |
| `STY-RUST-041` | Unit of Work                 | Declare the `UnitOfWorkFactory` trait second in `domain/port/unit_of_work.rs`.                                                                                                                                                                                                                                                                                                                                                                                                                                                         | [§ 7.4](#74-declaration-order-in-the-unit-of-work-port-file)     | `check_sty_rust_041`                                                                                   |
| `STY-RUST-042` | Repository                   | `create` inserts a new aggregate and never binds identity columns: the datastore auto-increments them and `create` returns the aggregate with its final identifier.                                                                                                                                                                                                                                                                                                                                                                    | [§ 8.1](#81-repository-port)                                     | `check_sty_rust_042`                                                                                   |
| `STY-RUST-043` | Repository                   | `save` upserts an existing aggregate targeted by its identifier, and writes the child collections the aggregate carries.                                                                                                                                                                                                                                                                                                                                                                                                               | [§ 8.1](#81-repository-port)                                     | `check_sty_rust_043`                                                                                   |
| `STY-RUST-044` | Repository                   | `delete` returns `Ok(false)` when no row matched; a missing entity is not an error.                                                                                                                                                                                                                                                                                                                                                                                                                                                    | [§ 8.1](#81-repository-port)                                     | `check_sty_rust_044`                                                                                   |
| `STY-RUST-045` | Repository                   | `search` returns an empty collection when nothing matches.                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | [§ 8.1](#81-repository-port)                                     | `check_sty_rust_045`                                                                                   |
| `STY-RUST-046` | Repository                   | Missing entities are never errors (see [Port error](#26-port-errors)).                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | [§ 8.1](#81-repository-port)                                     | `check_sty_rust_046`                                                                                   |
| `STY-RUST-047` | Repository                   | It holds the transaction it operates on (`&'transaction mut Transaction<'static, Sqlite>` for SQLite) and is built by the unit-of-work accessor — never with a connection pool.                                                                                                                                                                                                                                                                                                                                                        | [§ 8.2](#82-repository-adapter)                                  | `check_sty_rust_047`                                                                                   |
| `STY-RUST-048` | Repository                   | It maps `sqlx::Error` to `RepositoryError` in `error_mapping.rs`; sqlx types never cross into the domain or application layers.                                                                                                                                                                                                                                                                                                                                                                                                        | [§ 8.2](#82-repository-adapter)                                  | `check_sty_rust_048`                                                                                   |
| `STY-RUST-049` | Repository                   | Declare the filter struct (e.g. `UserFilter`) first in a repository port file.                                                                                                                                                                                                                                                                                                                                                                                                                                                         | [§ 8.3](#83-declaration-order-in-a-repository-port-file)         | `check_sty_rust_049`                                                                                   |
| `STY-RUST-050` | Repository                   | Declare the repository trait second in a repository port file.                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | [§ 8.3](#83-declaration-order-in-a-repository-port-file)         | `check_sty_rust_050`                                                                                   |
| `STY-RUST-051` | Use cases                    | A use case trait has one and only one method, named `execute`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |                                                                  | `check_sty_rust_051`                                                                                   |
| `STY-RUST-052` | Use cases                    | Trait named `<UseCaseName>UseCase`, e.g. `CreateNoteUseCase`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |                                                                  | `check_sty_rust_052`                                                                                   |
| `STY-RUST-053` | Use cases                    | Implementation named just `<UseCaseName>`, e.g. `CreateNote`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |                                                                  | `check_sty_rust_053`                                                                                   |
| `STY-RUST-054` | Use cases                    | Declare the command object first in a use-case port file.                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | [§ 9.1](#91-declaration-order-in-a-use-case-port-file)           | `check_sty_rust_054`                                                                                   |
| `STY-RUST-055` | Use cases                    | Declare the response object second in a use-case port file (only when non-empty).                                                                                                                                                                                                                                                                                                                                                                                                                                                      | [§ 9.1](#91-declaration-order-in-a-use-case-port-file)           | `check_sty_rust_055`                                                                                   |
| `STY-RUST-056` | Use cases                    | Declare the error enum third in a use-case port file.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | [§ 9.1](#91-declaration-order-in-a-use-case-port-file)           | `check_sty_rust_056`                                                                                   |
| `STY-RUST-057` | Use cases                    | Declare the use-case trait last in a use-case port file.                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | [§ 9.1](#91-declaration-order-in-a-use-case-port-file)           | `check_sty_rust_057`                                                                                   |
| `STY-RUST-058` | Use cases                    | **Port** — `src/lib/application/port/<context>_use_case_factory.rs`. The trait is named `<Context>UseCaseFactory`, is `Send + Sync`, carries `#[cfg_attr(test, mockall::automock)]`, and exposes one method per use case — named after the use case — returning `Arc<dyn <UseCaseName>UseCase>`.                                                                                                                                                                                                                                       | [§ 9.2](#92-use-case-factory)                                    | `check_sty_rust_058`                                                                                   |
| `STY-RUST-059` | Use cases                    | **Adapter** — `src/lib/infrastructure/use_case_factory/<context>.rs`. The struct is named `Runtime<Context>UseCaseFactory` and holds what the context's use cases need: the immutable configuration (`Arc<Configuration>`), the instance identity (`Arc<InstanceState>`) when they use its secrets, and the unit-of-work factory (`Arc<SqlxUnitOfWorkFactory>`).                                                                                                                                                                       | [§ 9.2](#92-use-case-factory)                                    | `check_sty_rust_059`                                                                                   |
| `STY-RUST-060` | Use cases                    | **Lazy instantiation** — every accessor builds the use case on the spot. It rebuilds its configuration-derived adapters (e.g. password hasher, token provider) from the immutable configuration and identity and never caches them; it builds its stateless adapters at the point of use and never stores them in a long-lived struct (see [§ 9.2](#92-use-case-factory)). The startup-only settings listed in `STY-RUST-073` are exempt.                                                                                              | [§ 9.2](#92-use-case-factory)                                    | `check_sty_rust_060`                                                                                   |
| `STY-RUST-061` | Use cases                    | The factory only supplies the unit-of-work factory; the use case still opens, commits and rolls back its own unit of work (see [Unit of Work](#7-unit-of-work)).                                                                                                                                                                                                                                                                                                                                                                       | [§ 9.2](#92-use-case-factory)                                    | `check_sty_rust_061`                                                                                   |
| `STY-RUST-062` | Configuration                | `LoadConfiguration` runs once at startup; the resulting `Configuration` is immutable and stored as `Arc<Configuration>` in `AppState`, beside the `Arc<InstanceState>` identity.                                                                                                                                                                                                                                                                                                                                                       | [§ 10](#10-configuration)                                        | `check_sty_rust_062`                                                                                   |
| `STY-RUST-063` | Configuration                | Never cache a configuration-derived value (for example the `JWT` TTL) in a long-lived adapter built at startup; read it when the adapter is built for a use case. The pepper and the signing secret are not configuration: they are the immutable instance identity, so building the password hasher from the pepper once is correct. The startup-only settings listed in `STY-RUST-073` are the only exception.                                                                                                                       | [§ 10](#10-configuration)                                        | `check_sty_rust_063`                                                                                   |
| `STY-RUST-064` | Application state            | `src/lib/app.rs` is the composition root: the only place that builds `AppState` and constructs a concrete adapter that lives for the process lifetime. The binary `src/bin/server.rs` only launches it. A factory accessor (see [§ 9.2](#92-use-case-factory)) or an adapter constructs the per-use adapters it needs.                                                                                                                                                                                                                 | [§ 1](#1-application-state)                                      | `check_sty_rust_064`                                                                                   |
| `STY-RUST-065` | Application state            | `AppState` is the single router state; `FromRef` impls expose exactly what middleware extracts, and getters return `Arc` clones, never borrows.                                                                                                                                                                                                                                                                                                                                                                                        | [§ 1](#1-application-state)                                      | `check_sty_rust_065`                                                                                   |
| `STY-RUST-066` | Ports                        | Always declare outbound port traits as `Send + Sync`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | [§ 6.1](#61-port-traits-are-send-and-sync)                       | `check_sty_rust_066`                                                                                   |
| `STY-RUST-067` | Ports                        | Outbound ports that return `impl Future` are not dyn-safe: hold concrete adapters and `Arc<dyn <UseCaseName>UseCase>` trait objects, never `Arc<dyn UnitOfWorkFactory>` or `Arc<dyn TokenProvider>`.                                                                                                                                                                                                                                                                                                                                   | [§ 6.2](#62-dyn-safety)                                          | `check_sty_rust_067`                                                                                   |
| `STY-RUST-068` | Ports                        | Repository methods mutate the shared transaction: take `&mut self` and return an explicitly `Send` future.                                                                                                                                                                                                                                                                                                                                                                                                                             | [§ 6.3](#63-async-methods-take-mut-self-and-return-send-futures) | `check_sty_rust_068`                                                                                   |
| `STY-RUST-069` | Ports                        | Outbound port traits are `'static`; implementations may borrow, but never store a repository beyond the use-case `execute` that obtained it.                                                                                                                                                                                                                                                                                                                                                                                           | [§ 6.4](#64-port-traits-are-static-implementations-may-borrow)   | `check_sty_rust_069`                                                                                   |
| `STY-RUST-070` | Ports                        | Prefer sharing long-lived adapters through `Arc<T>` over adding `Clone`.                                                                                                                                                                                                                                                                                                                                                                                                                                                               | [§ 6.5](#65-prefer-arc-over-clone)                               | `check_sty_rust_070`                                                                                   |
| `STY-RUST-071` | Configuration                | The `logging` section drives the runtime logs; install the subscriber once, after `LoadConfiguration` and before the datastore is opened, so first-boot lifecycle events reach a sink.                                                                                                                                                                                                                                                                                                                                                 | [§ 10](#10-configuration)                                        | `check_sty_rust_071`                                                                                   |
| `STY-RUST-072` | Ports                        | A use case reaches a third-party dependency only through an outbound port, never by calling it directly; the dependencies listed in [§ 6.6](#66-third-party-dependencies-sit-behind-a-port) are the only exceptions.                                                                                                                                                                                                                                                                                                                   | [§ 6.6](#66-third-party-dependencies-sit-behind-a-port)          | `check_sty_rust_072`                                                                                   |
| `STY-RUST-073` | Configuration                | A setting that cannot be applied after the component consuming it is built is a **startup-only setting**: it is read once at startup, listed in the registry in [§ 10.1](#101-startup-only-settings), and never silently cached. The registry is extensible: adding a startup-only setting appends it there in the same change, together with how a change is surfaced to the operator.                                                                                                                                                | [§ 10.1](#101-startup-only-settings)                             | `check_sty_rust_073`                                                                                   |
| `STY-RUST-074` | Domain models                | **Validated deserialization**: a domain model that validates in `try_new` must not derive a `Deserialize` that bypasses it. Route serde through the validating constructor (`#[serde(try_from = "…")]` plus `TryFrom`, or a custom `Deserialize`) so every construction path enforces the same invariant.                                                                                                                                                                                                                              | [§ 3.2](#32-validated-deserialization)                           | `check_sty_rust_074`                                                                                   |
| `STY-RUST-075` | Configuration                | A configuration value that is **present but malformed or out of range** fails startup: a reader falls back to a default only when the key is absent, and propagates every other read or validation error.                                                                                                                                                                                                                                                                                                                              | [§ 10](#10-configuration)                                        | `check_sty_rust_075`                                                                                   |
| `STY-RUST-076` | Repository                   | A repository persists and queries exactly one aggregate; a port that spans two aggregates is split per aggregate.                                                                                                                                                                                                                                                                                                                                                                                                                      | [§ 8](#8-repository)                                             | `check_sty_rust_076`                                                                                   |
| `STY-RUST-077` | Repository                   | A filter field is named after the aggregate field it constrains, e.g. `UserFilter.id` for `User::id()`. A join constraint on a child collection is the documented exception (`MusicAlbumFilter.audio_id`).                                                                                                                                                                                                                                                                                                                             |                                                                  | `check_sty_rust_077`                                                                                   |
| `STY-RUST-078` | Repository                   | The classic `search` returns the aggregate, never a summary or a detail the port declares; the application assembles the representation.                                                                                                                                                                                                                                                                                                                                                                                               |                                                                  | `check_sty_rust_078`                                                                                   |
| `STY-RUST-079` | REST handlers                | Shared representations, links and helpers live in the bounded context module file (`<context>.rs`); a context folder holds only endpoint files, and an endpoint file imports no `<context>::<operation>` submodule — it takes shared items from the `<context>.rs` module file and `hal`.                                                                                                                                                                                                                                              |                                                                  | `check_sty_rust_079`                                                                                   |
| `STY-RUST-080` | REST handlers                | `rest/` holds only the known REST infrastructure (`api_error.rs`, `app_state.rs`, `hal.rs`, `handler.rs`, `handler`, `middleware.rs`, `middleware`); `handler/` holds only `get_health.rs` plus each bounded context's `<context>.rs` module file and `<context>/` endpoint folder.                                                                                                                                                                                                                                                    |                                                                  | `check_sty_rust_080`                                                                                   |
| `STY-RUST-081` | Constants                    | **Site** — a production `const` is declared at file scope in `src/lib/domain/`, `src/lib/application/` or `src/lib/infrastructure/`, or privately in an endpoint file `handler/<context>/<operation>.rs`; `src/bin/`, production `src/lib/lib.rs`, a block inside a `fn`, an `impl`, a `trait` and any production `static` declare none. Its visibility follows `STY-RUST-084`.                                                                                                                                                        | [§ 11](#11-visibility)                                           | `check_sty_rust_081`                                                                                   |
| `STY-RUST-082` | Constants                    | A name has one owner: a constant is declared once, in the layer that owns the fact it expresses, with a technology-free name and a unit-bearing suffix (`_LENGTH` characters, `_BYTES`, `_SECONDS`, `_MILLISECONDS`) and a type that states its width; a name declared in two layers is imported from the innermost, and a name declared in three files of one layer is hoisted to their lowest common ancestor module.                                                                                                                |                                                                  | `check_sty_rust_082`                                                                                   |
| `STY-RUST-083` | General                      | Do not gate code on the host platform: the server targets Linux only, so a `#[cfg(unix)]`, `#[cfg(windows)]`, `#[cfg(target_os = …)]`, `#[cfg(target_family = …)]` or `cfg!(…)` gate is a constant. Write the code unconditionally.                                                                                                                                                                                                                                                                                                    |                                                                  | `check_sty_rust_083`                                                                                   |
| `STY-RUST-084` | Visibility                   | A declaration is as narrow as its use: private by default, `pub(crate)` when another module names it, and `pub` only at the crate entry point — `pub mod app;` in `src/lib/lib.rs` and `pub async fn run` in `src/lib/app.rs`; `pub(super)` and `pub(in …)` never. Struct fields are out of scope.                                                                                                                                                                                                                                     | [§ 11](#11-visibility)                                           | `check_sty_rust_084`                                                                                   |
| `STY-RUST-085` | General                      | Imports from one module are one declaration: two adjacent `use` declarations in the same scope never share a parent module — merge them into one brace group (`use a::b::{C, D};`), keeping `as _` trait imports inside the group.                                                                                                                                                                                                                                                                                                     |                                                                  | `check_sty_rust_085`                                                                                   |
| `STY-RUST-086` | Ports                        | A module with child modules is the import surface for them: a leaf child module is declared `mod <name>;`, only a child that is itself a module root is `pub(crate) mod <name>;`, and callers import `crate::…::<root>::<Item>`.                                                                                                                                                                                                                                                                                                       | [§ 6.7](#67-module-facades)                                      | `check_sty_rust_086`                                                                                   |
| `STY-RUST-087` | Testing                      | A test module declares no assertion helper: an assertion is written inline in the test that owns it, so no `#[cfg(test)]` function is named `assert_*`.                                                                                                                                                                                                                                                                                                                                                                                | [§ Test helpers](#test-helpers)                                  | `check_sty_rust_087`                                                                                   |
| `STY-RUST-088` | Testing                      | A datastore seed is declared once, in the `mod test_helpers` of `src/lib/infrastructure/outbound/sqlx.rs`; no other test module declares a datastore seed helper or a helper that inserts a row.                                                                                                                                                                                                                                                                                                                                       | [§ Test helpers](#test-helpers)                                  | `check_sty_rust_088`                                                                                   |
| `STY-RUST-089` | Testing                      | A file declares at most one `#[cfg(test)] mod tests`, and a module root's `mod tests` tests only the code in that file: `src/lib/infrastructure/outbound/sqlx.rs` keeps the `init_db` tests, and a repository or schema test lives beside the adapter it exercises.                                                                                                                                                                                                                                                                    | [§ Test helpers](#test-helpers)                                  | `check_sty_rust_089`                                                                                   |
| `STY-RUST-090` | Testing                      | A test configures its mocks in its own body: no `expect_*` setup runs in a helper or a `#[fixture]`, only inline in the test that owns it.                                                                                                                                                                                                                                                                                                                                                                                             | [§ Test helpers](#test-helpers)                                  | `check_sty_rust_090`                                                                                   |
| `STY-RUST-091` | OpenAPI generation           | Annotate each handler in `src/lib/infrastructure/inbound/rest/handler` with `#[utoipa::path(...)]`, so its path item is generated from the declaration that also registers the route.                                                                                                                                                                                                                                                                                                                                                  | [HttpApi.md § OpenAPI generation](HttpApi.md#openapi-generation) | `check_sty_rust_091`                                                                                   |
| `STY-RUST-092` | OpenAPI generation           | Aggregate the document-scoped members in one struct deriving `utoipa::OpenApi`, and let `utoipa-axum` collect each registered route's path item and schemas.                                                                                                                                                                                                                                                                                                                                                                           | [HttpApi.md § OpenAPI generation](HttpApi.md#openapi-generation) | `check_sty_rust_092`                                                                                   |
| `STY-RUST-093` | Endpoint handler declaration | Declare `operation_id`, a unique operation identifier.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | [§ 12](#12-endpoint-handler-declaration)                         | `check_sty_rust_093`                                                                                   |
| `STY-RUST-094` | Endpoint handler declaration | Declare the HTTP method (`get`/`post`/`put`/`patch`/`delete`).                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | [§ 12](#12-endpoint-handler-declaration)                         | `check_sty_rust_094`                                                                                   |
| `STY-RUST-095` | Endpoint handler declaration | Declare `path`, the route of the endpoint.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | [§ 12](#12-endpoint-handler-declaration)                         | `check_sty_rust_095`                                                                                   |
| `STY-RUST-096` | Endpoint handler declaration | Declare `tag`, the bounded context the endpoint belongs to, or the reserved tag `system` for a system-level endpoint such as `get_health` that is bound to no bounded context.                                                                                                                                                                                                                                                                                                                                                         | [§ 12](#12-endpoint-handler-declaration)                         | `check_sty_rust_096`                                                                                   |
| `STY-RUST-097` | Endpoint handler declaration | Declare `request_body` for an endpoint that carries a body.                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | [§ 16](#16-request-body-declaration)                             | `check_sty_rust_097`                                                                                   |
| `STY-RUST-098` | Endpoint handler declaration | Declare `params` for path, query, header and cookie parameters.                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | [§ 13](#13-parameter-declarations)                               | `check_sty_rust_098`                                                                                   |
| `STY-RUST-099` | Endpoint handler declaration | Declare `responses` with all the possible responses, at most one row per status code.                                                                                                                                                                                                                                                                                                                                                                                                                                                  | [§ 17](#17-response-declaration)                                 | `check_sty_rust_099`                                                                                   |
| `STY-RUST-100` | Endpoint handler declaration | Declare `security` on every operation — the scheme(s) protecting it (`("bearer_auth" = [])`), or an explicitly empty requirement (`security(())`) when the endpoint is public. A public operation is not wrapped by the authentication middleware; only an operation declaring `("bearer_auth" = [])` is guarded by `authenticate`.                                                                                                                                                                                                    | [§ 12](#12-endpoint-handler-declaration)                         | `check_sty_rust_100`                                                                                   |
| `STY-RUST-101` | Endpoint handler declaration | Declare `summary`, a one-line human-readable summary.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | [§ 12](#12-endpoint-handler-declaration)                         | `check_sty_rust_101`                                                                                   |
| `STY-RUST-102` | Parameters                   | Declare `name`, the parameter name as it appears in the URL, header or cookie.                                                                                                                                                                                                                                                                                                                                                                                                                                                         | [§ 13](#13-parameter-declarations)                               | `check_sty_rust_102`                                                                                   |
| `STY-RUST-103` | Parameters                   | Declare `in`, the location: `Path`, `Query`, `Header` or `Cookie`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | [§ 13](#13-parameter-declarations)                               | `check_sty_rust_103`                                                                                   |
| `STY-RUST-104` | Parameters                   | Declare `parameter_type` (optional), written `= Type` after the name to bind a specific Rust type.                                                                                                                                                                                                                                                                                                                                                                                                                                     | [§ 13](#13-parameter-declarations)                               | `check_sty_rust_104`                                                                                   |
| `STY-RUST-105` | Parameters                   | Declare `description`, a human-readable description.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | [§ 13](#13-parameter-declarations)                               | `check_sty_rust_105`                                                                                   |
| `STY-RUST-106` | Parameters                   | Use the constraint attributes to refine a parameter value, and enforce the declared bound in the handler or use case (`REQ-API-024`).                                                                                                                                                                                                                                                                                                                                                                                                  | [§ 14](#14-constraint-attributes)                                |                                                                                                        |
| `STY-RUST-107` | Parameters                   | Prefer a reused `IntoParams` struct over inline tuples when several handlers share the query.                                                                                                                                                                                                                                                                                                                                                                                                                                          | [§ 13](#13-parameter-declarations)                               |                                                                                                        |
| `STY-RUST-108` | Examples                     | Provide an `example` for a schema, parameter or response, inline or named (`name`, `summary`, `value`).                                                                                                                                                                                                                                                                                                                                                                                                                                | [§ 15](#15-example-values)                                       |                                                                                                        |
| `STY-RUST-109` | Request body                 | Declare `content_type`, the media type of the body (for example `application/json`).                                                                                                                                                                                                                                                                                                                                                                                                                                                   | [§ 16](#16-request-body-declaration)                             | `check_sty_rust_109`                                                                                   |
| `STY-RUST-110` | Request body                 | Declare `body`, the schema of the body (`content` in OpenAPI terms).                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | [§ 16](#16-request-body-declaration)                             | `check_sty_rust_110`                                                                                   |
| `STY-RUST-111` | Responses                    | Declare `status`, the HTTP status code.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | [§ 17](#17-response-declaration)                                 | `check_sty_rust_111`                                                                                   |
| `STY-RUST-112` | Responses                    | Declare `description`, a human-readable description.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | [§ 17](#17-response-declaration)                                 | `check_sty_rust_112`                                                                                   |
| `STY-RUST-113` | Responses                    | Declare `body`, the response payload schema, when there is one.                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | [§ 17](#17-response-declaration)                                 | `check_sty_rust_113`                                                                                   |
| `STY-RUST-114` | Responses                    | Declare `content_type`, the payload media type, when there is one.                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | [§ 17](#17-response-declaration)                                 | `check_sty_rust_114`                                                                                   |
| `STY-RUST-115` | Responses                    | Declare `headers`, when there are any.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | [§ 17](#17-response-declaration)                                 | `check_sty_rust_115`                                                                                   |
| `STY-RUST-116` | Responses                    | Declare `link` to another operation through its `operation_id` (optional).                                                                                                                                                                                                                                                                                                                                                                                                                                                             | [§ 17](#17-response-declaration)                                 |                                                                                                        |
| `STY-RUST-117` | Responses                    | Declare every possible response, the success case as well as every error.                                                                                                                                                                                                                                                                                                                                                                                                                                                              | [§ 17](#17-response-declaration)                                 | `check_sty_rust_117`                                                                                   |
| `STY-RUST-118` | HAL payload                  | A HAL payload struct declares no wrapper member at its root: the business data sits at the root, and `_embedded` is the only nesting key (`STY-RUST-120`).                                                                                                                                                                                                                                                                                                                                                                             | [§ 18](#18-hal-payload-structs)                                  | `check_sty_rust_118`                                                                                   |
| `STY-RUST-119` | HAL payload                  | A `_links` field's type holds only `Link` members, keyed by the relation name; `_links` never carries business data.                                                                                                                                                                                                                                                                                                                                                                                                                   | [§ 18](#18-hal-payload-structs)                                  | `check_sty_rust_119`                                                                                   |
| `STY-RUST-120` | HAL payload                  | A payload struct uses `_embedded` only at a recorded location. The four catalog list response structs are the recorded need: their partial page items spare one request per item. Every other `_embedded` is reported.                                                                                                                                                                                                                                                                                                                 | [§ 18](#18-hal-payload-structs)                                  | `check_sty_rust_120`                                                                                   |
| `STY-RUST-121` | Error handling               | An `ApiError` message argument is a string literal, optionally wrapped by `.to_owned()`/`.to_string()`; never a rejection value, a field of one, or its `Display`/`Debug`.                                                                                                                                                                                                                                                                                                                                                             | [§ 19](#19-error-message-arguments)                              | `check_sty_rust_121`                                                                                   |
| `STY-RUST-122` | Parameters                   | Every endpoint handler extracts the query: a handler that takes parameters declares its own `<Context>Query` deriving `Deserialize` and `IntoParams` with `serde(deny_unknown_fields)`, and a handler that takes none extracts `Query<NoParametersQuery>`. An undeclared query parameter answers `400` and applies nothing.                                                                                                                                                                                                            | [§ 20](#20-the-query-extractor)                                  | `check_sty_rust_122`                                                                                   |
| `STY-RUST-123` | Lint policy                  | An `#[expect]`/`#![expect]` attribute, including one nested in `cfg_attr`, names only `clippy::print_stdout`, and only in `app.rs` and `seed.rs`, where the operator must read a credential the file sinks may not carry; every other lint finding is fixed in the code rather than expected, and `#[allow]` is not an alternative.                                                                                                                                                                                                    | [§ 21](#21-lint-expectations)                                    | `check_sty_rust_123`                                                                                   |

---

#### 1. Application state

`src/lib/app.rs` is the composition root: it is the only place that builds the process-lifetime adapters, runs the
startup use cases (`LoadConfiguration`, `InitializeRootAdmin`, `EnsureStorageDirectories`) and builds the `AppState` the
HTTP layer shares. The binary `src/bin/server.rs` only launches it (`pub async fn run`, `STY-RUST-064`).

`AppState` lives in `src/lib/infrastructure/inbound/rest/app_state.rs` and holds:

- the immutable configuration (`Arc<Configuration>`),
- the immutable instance identity (`Arc<InstanceState>`),
- the per-context use-case factories (see [Use cases](#9-use-cases)).

`AppState` is the single router state; `FromRef` impls expose exactly what middleware extracts from it. The auth
middleware does not get a cached token provider: `FromRef<AppState> for Arc<JwtTokenProvider>` builds one from the
instance identity and the configured TTL, so it validates with the same secret the Identity use cases issue with.
Getters return `Arc` clones, never borrows of the shared state. Nothing outside the composition root builds an adapter
that lives for the process lifetime; the per-use adapters a factory accessor or an adapter needs are built where they
are used (see [§ 9.2](#92-use-case-factory)).

---

#### 2. Error handling

Errors follow a layered model: one error type per role, each translating to the next as it crosses an architectural
boundary.

Where each error is logged, at what severity, and what may never reach a log sink is a separate concern, governed by
[§ 7 (Logging & Observability)](#logging--observability).

---

##### 2.1 Domain error

Declare the error enum before the model struct, in the same file. It reports failures when initialising or updating a
domain model.

Drawn from `src/lib/domain/model/user.rs`:

```rust
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum UserError {
    #[error("email has an invalid format")]
    InvalidEmail(#[from] email_address::Error),
    // [...]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    // [...]
}
```

---

##### 2.2 Use Case error

Declared in `src/lib/application/port`, it exposes an `Unknown(_)` variant and one invalid-parameter variant per
client-supplied parameter the use case validates (for example a raw integrity hash or a chunk body). Malformed input
that never reaches the use case — a path or JSON field rejected at the inbound boundary — needs no variant, so a command
whose fields are already-validated types (such as `NumericID`) may expose only `Unknown(_)` and its business variants.

Drawn from `src/lib/application/port/register_user.rs`:

```rust
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RegisterUserError {
    #[error("the caller is not allowed to register the requested role")]
    Forbidden,
    #[error("email has an invalid format")]
    InvalidEmail,
    // [...]
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
    #[error("a user with this username or email already exists")]
    UserAlreadyExists,
}
```

A use-case error may also carry a not-found variant (e.g. `NoSuchUser`, `UnknownCredential`) that the REST handler maps
to `ApiError::NotFound`; only outbound ports must express absence as a value, not an error (see
[Port errors](#26-port-errors)).

---

##### 2.3 Mapping use-case error to API error

Each rest handler file declares the mapping from its use-case error to the `ApiError`.

Drawn from `src/lib/infrastructure/inbound/rest/handler/identity/post_register.rs`:

```rust
impl From<RegisterUserError> for ApiError {
    fn from(err: RegisterUserError) -> Self {
        match err {
            RegisterUserError::Forbidden => Self::Forbidden(err.to_string()),
            RegisterUserError::InvalidEmail
            | RegisterUserError::InvalidPassword
            | RegisterUserError::InvalidUsername => Self::UnprocessableEntity(err.to_string()),
            RegisterUserError::Unknown(_) => Self::InternalServerError,
            RegisterUserError::UserAlreadyExists => Self::Conflict(err.to_string()),
        }
    }
}
```

---

##### 2.4 ApiError to axum response

`ApiError` implements `IntoResponse`, converting to the corresponding HTTP status code and attaching its message to the
response. It cannot build the error envelope itself — `IntoResponse` has no access to the request path, so it cannot
fill `_links.self` (`REQ-API-008`), and it never sees framework-generated errors. The root `hal_errors` middleware is
the single envelope builder: it reads the attached message and serves the one `ErrorBody` for every `4xx`/`5xx`
(`REQ-API-008`).

---

##### 2.6 Port errors

Port errors are translated into use-case errors by the use case, never consumed directly by the HTTP adapter — the one
exception being inbound middleware (see [Inbound middleware](#27-inbound-middleware)).

Besides the two shared families below (Repository, External Service), a port may declare its **own** error family in
`src/lib/domain/port/error.rs`. A port error type is not one-per-port: it can back several outbound ports of the same
family, so every repository shares `RepositoryError` and a new port reuses its family's type rather than declaring a
duplicate. Only a port error family that can surface a failure it cannot classify declares an `Unknown(anyhow::Error)`
variant; a family whose failures are all enumerable names each of them and declares no catch-all. The Repository,
External Service, Unit of Work, Configuration Source, File Storage, Content Hasher, Password Hasher, Media Probe,
Content Type Sniffer and Media Source families declare `Unknown`; `PasswordGeneratorError`, `TokenProviderError`,
`InstanceStateStoreError`, `StateSeedSourceError` and `DatastorePresenceError` name every failure, and
`SecretGeneratorError` names none because pepper generation has no failure the caller can act on.

###### 2.6.1 Repository

| Error                  | Description                                                                                                               |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| AlreadyExist           | The entity already exists and cannot be created again. Typically caused by duplicate business keys or unique constraints. |
| Conflict               | The operation cannot be completed because the current state of the data conflicts with the requested action.              |
| ConcurrencyConflict    | The operation failed due to a concurrent modification of the same entity (for example, optimistic locking failure).       |
| DataIntegrityViolation | The operation would violate a data integrity rule or constraint.                                                          |
| OperationFailed        | The repository could not complete the requested operation for a non-specific reason.                                      |
| Timeout                | The operation exceeded the allowed execution time.                                                                        |
| Unavailable            | The repository or underlying datastore is currently unavailable.                                                          |
| Unknown                | An unexpected or unmapped error occurred.                                                                                 |

###### 2.6.2 External Service

| Error                  | Description                                                                                  |
| ---------------------- | -------------------------------------------------------------------------------------------- |
| Unauthorized           | Authentication is required or the provided credentials are invalid.                          |
| Forbidden              | The caller is authenticated but does not have permission to perform the requested operation. |
| RateLimited            | The external service rejected the request because a usage limit was exceeded.                |
| Timeout                | The external service did not respond within the expected time.                               |
| Unavailable            | The external service is temporarily unavailable or unreachable.                              |
| CommunicationFailure   | A network, protocol, or transport-level error occurred while communicating with the service. |
| DeserializationFailure | The service response could not be parsed or converted into the expected format.              |
| InvalidRequest         | The request was rejected because it contains invalid or missing information.                 |
| DependencyFailure      | The service failed due to a problem with one of its own dependencies.                        |
| Unknown                | An unexpected or unmapped error occurred.                                                    |

###### 2.6.3 Unit of Work

| Error           | Description                                                    |
| --------------- | -------------------------------------------------------------- |
| OperationFailed | The unit of work could not complete for a non-specific reason. |
| Unavailable     | The datastore is currently unavailable.                        |
| Unknown         | An unexpected or unmapped error occurred.                      |

###### 2.6.4 Configuration Source

| Error                | Description                                                 |
| -------------------- | ----------------------------------------------------------- |
| FileWriteFailed      | The configuration file was absent and could not be written. |
| InvalidConfiguration | The layered settings do not form a valid configuration.     |
| Unknown              | An unexpected or unmapped error occurred.                   |

###### 2.6.5 File Storage

A missing file is a valid outcome, not a port error ([`STY-RUST-015`](#26-port-errors)). The path-addressed methods
report absence as a value: `add_chunk` and `promote` return `Ok(false)`, `integrity_hash` returns `Ok(None)`, `restore`
treats a missing file as success, `directory_exist` returns `Ok(false)`, and `delete_upload_file` — which removes an
upload's staging file or a file promoted into the media library — treats a missing file as success too.

| Error           | Description                                                                  |
| --------------- | ---------------------------------------------------------------------------- |
| OperationFailed | The file storage could not complete the operation for a non-specific reason. |
| Unavailable     | The file storage is temporarily unavailable or unreachable.                  |
| Unknown         | An unexpected or unmapped error occurred.                                    |

---

##### 2.7 Inbound middleware

Inbound middleware may consume a port directly when that port's decision is the middleware's own responsibility — for
example, `authenticate` calls `TokenProvider` to decide whether to admit a request. Two rules apply when it maps that
port's error:

- **A client error only for an unauthenticated caller** (`STY-RUST-019`). Only the variants inbound middleware
  explicitly recognizes as an unauthenticated caller — for example `InvalidClaims`, `InvalidToken` and `TokenExpired` —
  become a client error. Every other variant, including `OperationFailed`, is an uncaught error: the handler, or the
  project's equivalent path, serves `500`, never a `401`.
- **An exhaustive match that ends in a `500` catch-all** (`STY-RUST-020`). Port errors are `#[non_exhaustive]`, so the
  match ends with a catch-all arm that defaults to `500`; a wildcard arm must never collapse a server-side failure into
  a misleading `401`.

Drawn from `src/lib/infrastructure/inbound/rest/middleware/auth.rs`:

```rust
let user_id = match token_provider.validate(token).await {
    Ok(user_id) => user_id,
    Err(
        TokenProviderError::InvalidClaims
        | TokenProviderError::InvalidToken
        | TokenProviderError::TokenExpired,
    ) => {
        // [...]
        return ApiError::Unauthorized("invalid or expired token".to_owned()).into_response();
    }
    Err(error) => {
        let reason = match error {
            TokenProviderError::InvalidClaims => "invalid_claims",
            TokenProviderError::InvalidToken => "invalid_token",
            TokenProviderError::OperationFailed => "operation_failed",
            TokenProviderError::TokenExpired => "token_expired",
        };
        // [...]
        return ApiError::InternalServerError.into_response();
    }
};
```

---

#### 3. Domain models

---

##### 3.1 Getter and setter

Accessors are named after the field they expose.

```rust
impl User {
    pub fn try_new(username: String) -> Result<Self, UserError> {
        let username = Self::validate_username(username)?;
        Ok(Self { username })
    }

    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn set_username(&mut self, username: String) -> Result<(), UserError> {
        self.username = Self::validate_username(username)?;
        Ok(())
    }
}
```

Note: reject-invalid-then-assign. A setter validates the new value, assigns only on success, and reports the cause
through the domain error enum when it fails.

---

##### 3.2 Validated deserialization

A domain model that validates in `try_new` must enforce the same invariant on every deserialization path. A derived
`Deserialize` constructs the fields directly and bypasses the constructor, so a layered configuration (file,
environment) can produce a value the model would reject. Route serde through the validating constructor instead: declare
a private unchecked mirror struct and deserialize through it with `#[serde(try_from = "…")]` and `TryFrom`.

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "UserConfig")]
pub struct User {
    username: String,
}

#[derive(serde::Deserialize)]
struct UserConfig {
    username: String,
}

impl TryFrom<UserConfig> for User {
    type Error = UserError;

    fn try_from(config: UserConfig) -> Result<Self, Self::Error> {
        Self::try_new(config.username)
    }
}
```

A `#[serde(default = "…")]` attribute on a field moves to the mirror struct with the field itself. Validation errors
surface through the existing domain error enum, so the caller maps them the same way as a `try_new` failure.

---

#### 4. SQL data models

---

##### 4.1 Enum for a CHECK constraint

For a column backed by a SQL `CHECK (... IN (...))` constraint, declare the Rust enum **before** the model struct, in
the same file.

Drawn from `src/lib/infrastructure/outbound/sqlx/model/user.rs`:

```rust
use crate::domain::alias::NumericID;

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "UPPERCASE")]
#[non_exhaustive]
pub enum Role {
    Admin,
    Standard,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    // [...]
    pub role: Role,
    #[sqlx(primary_key)]
    pub user_id: NumericID,
    // [...]
}
```

---

##### 4.2 Alias type for numeric IDs

Use the `NumericID` alias from `domain/alias.rs` for all numeric table columns that are identifiers (primary keys,
foreign keys) rather than a raw integer type.

Drawn from `src/lib/infrastructure/outbound/sqlx/model/user.rs`:

```rust
use crate::domain::alias::NumericID;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub credential_id: NumericID,
    // [...]
    #[sqlx(primary_key)]
    pub user_id: NumericID,
    // [...]
}
```

---

#### 5. Testing

Arrange–Act–Assert example:

```rust
#[test]
fn apply_discount_code_valid_code_reduces_total_price() {
    // Arrange
    let mut cart = ShoppingCart::new();
    cart.add_item(CartItem {
        name: "Rust Book".to_string(),
        price: 1000, // $10.00
        quantity: 2,
    });

    // Act
    let result = cart.apply_discount_code("SAVE10");

    // Assert
    assert!(result.is_ok());
    assert_eq!(cart.total_price(), 1800);
}
```

Data-driven test with `rstest`:

```rust
#[rstest]
#[case::valid_standard_format("user@example.com", true)]
#[case::missing_at_symbol("invalid-email", false)]
#[case::empty_string("", false)]
fn validate_email_scenario_returns_expected(
    #[case] email: &str,
    #[case] expected: bool,
) {
    // Act & Assert
    assert_eq!(validate_email(email), expected);
}
```

Scenario labels:

A test states which declared scenario it proves. A system test stacks one `@scenario` decorator per scenario; the
identifier names the capability and the requirement, so neither is repeated at the call site and neither can disagree
with the specification. The optional second argument is the kind of proof, from the closed set `Positive`, `Negative`,
`Boundary`, `Security`, `Error handling`, `Recovery`, `Alternate flow` and `Business rule`; it is also a pytest marker,
so `pytest -m security` selects the security proofs. From `test/system/user.py`:

```python
@scenario("SCN-USER-004-A")
@scenario("SCN-USER-004-B")
def test_an_omitted_or_null_email_is_stored_as_none(server: Sut, root_token: str) -> None:
    """A creation with no email value or an explicit null stores no email address."""
```

A case whose scenarios all prove one kind declares that kind once, with the bare decorator `@positive`, `@negative`,
`@boundary`, `@security`, `@error_handling`, `@recovery`, `@alternate_flow` or `@business_rule`, and every scenario it
declares inherits it. A scenario that names its own kind keeps it, so a case that mixes kinds types the scenarios and
carries no case-level decorator; the two forms may sit in either order. A case-level decorator also marks the case, so
`pytest -m positive` selects it alongside the scenario-declared proofs. Declaring the kind is optional, and a case that
declares none counts as `Unspecified` in the matrix until one does. What each of the eight types means is in
[Test.md § Scenario types](Test.md#scenario-types).

A Rust test declares the scenarios it proves in a `// proves:` comment above its first attribute, and names the kind of
proof after an em dash when it has one: `// proves: SCN-AUTHENTICATION-001-C — Recovery` from
`src/lib/application/use_case/initialize_root_admin.rs`. The generator associates the comment with the next `fn`, and a
rule check verifies that both the identifier and the function exist. From
`src/lib/infrastructure/outbound/random/password_generator.rs`:

```rust
// proves: SCN-AUTHENTICATION-002-A
#[tokio::test]
async fn random_password_generator_samples_follow_declared_shape() -> Result<(), Box<dyn Error>> {
    // [...]
}
```

A scenario the fixture fuzz run proves has no pytest case to decorate: it is declared in `test/api/scenarios.toml` with
the kind of proof it carries, and the matrix records `API suite` as its evidence.

Neither form produces a run report: the labels are a static map from a test to the scenario it proves.
`docs/development/artifacts/RequirementsTraceabilityCoverage.html` is generated from them and from the specification
tree by `script/investigation.sh` ([`TEST-054`](#system-test)).

---

#### 6. Ports

Outbound ports are the interfaces infrastructure implements and the application consumes: repositories, the unit of
work, the configuration source, the password hasher, and so on.

---

##### 6.1 Port traits are `Send` and `Sync`

Always declare outbound port traits as `Send + Sync`.

Drawn from `src/lib/domain/port/user_repository.rs`:

```rust
#[cfg_attr(test, mockall::automock)]
pub trait UserRepository: Send + Sync {
    // [...]
}
```

- `Send`: the port and its futures may be moved between threads.
- `Sync`: the port may be shared behind an `Arc`; the unit-of-work factory is shared this way.

Repositories are short-lived views over a transaction (see [Repository](#8-repository)); the shared mutable state is the
unit of work, not the repository itself.

---

##### 6.2 Dyn-safety

Only use-case traits are object-safe: their `execute` returns `Pin<Box<dyn Future<Output = ...> + Send + 'future>>`, so
a factory can return `Arc<dyn <UseCaseName>UseCase>`.

Outbound ports that return `impl Future` (RPITIT) are **not** dyn-safe. `AppState` and the use-case factories therefore
hold concrete adapters (`Arc<SqlxUnitOfWorkFactory>`, ...) and build concrete per-use adapters
(`Arc<JwtTokenProvider>`), while the use cases they hand back are `Arc<dyn <UseCaseName>UseCase>` trait objects — never
`Arc<dyn UnitOfWorkFactory>` or `Arc<dyn TokenProvider>`.

---

##### 6.3 Async methods take `&mut self` and return `Send` futures

Repository methods mutate the shared transaction, so they take `&mut self`. Return an explicitly `Send` future instead
of relying on the default (which is not guaranteed to be `Send`):

Drawn from `src/lib/domain/port/user_repository.rs`:

```rust
use std::future::Future;

pub trait UserRepository: Send + Sync {
    fn create(&mut self, user: User) -> impl Future<Output = Result<User, RepositoryError>> + Send;
    // [...]
}
```

Web frameworks (axum) move futures across worker threads; without `Send`, `tokio::spawn(...)` and other runtime
operations fail to compile.

---

##### 6.4 Port traits are `'static`; implementations may borrow

The trait bound is `'static` so ports can be injected into the application:

```rust
struct Application<F: UnitOfWorkFactory> {
    unit_of_work_factory: Arc<F>,
}
```

The _implementation_ need not be `'static`: a sqlx repository borrows the unit-of-work transaction
(`SqlxUserRepository<'transaction>`). Never store a repository beyond the use-case `execute` that obtained it.

---

##### 6.5 Prefer `Arc` over `Clone`

Do not add `Clone` just for frameworks — share long-lived adapters and the unit-of-work factory through `Arc<T>`.
Repositories obtained from a unit of work are borrowed views, not cloned.

---

##### 6.6 Third-party dependencies sit behind a port

A third-party dependency is any crate outside the standard library and this workspace. A use case in the application
layer (`src/lib/application/use_case/**`) reaches one only through an outbound port declared in `src/lib/domain/port/`
and implemented by an infrastructure adapter; it never calls the crate itself. The rule covers both calling into a crate
and naming one of its types in a use-case body, command, response or error enum.

Two things are exempt:

- **Derive and attribute macros** — they run at compile time and are not a runtime dependency the port boundary protects
  (`thiserror::Error`, `serde::Serialize`, `tokio::test`).
- **Test code** — `#[cfg(test)]` modules inside the application layer, like the shared fixtures in the crate's
  `test_helpers` module.

These crates are allowed directly; the list is deliberately short and is extended by adding a row — an ordinary
documentation change:

| Crate     | Role           | Why it is allowed                                                                                                                                    |
| --------- | -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `anyhow`  | Error plumbing | `Unknown(#[source] anyhow::Error)` is the mandated use-case error shape ([§ 2.2](#22-use-case-error), [§ 2.6](#26-port-errors)).                     |
| `chrono`  | Date and time  | Use cases read the current time and carry date-time types.                                                                                           |
| `tracing` | Logging        | Use cases and infrastructure adapters emit the events required by [§ 7 (Logging & Observability)](#logging--observability), `OBS-002` and `OBS-006`. |

---

##### 6.7 Module facades

A **module root** is a file `x.rs` with a sibling directory `x/`; the files under `x/` are its **child modules**. A root
with child modules is the import surface for them: callers import `crate::…::<root>::<Item>`, never
`crate::…::<root>::<child>::<Item>`.

A root declares its children first, each with its `///` doc, then re-exports what its callers use. Drawn from
`src/lib/domain/port.rs`:

```rust
/// The audio repository port.
mod audio_repository;
/// The configuration source port.
mod configuration_source;
// [...]

pub(crate) use self::audio_repository::{AudioFilter, AudioRepository};
#[cfg(test)]
pub(crate) use self::audio_repository::MockAudioRepository;
pub(crate) use self::configuration_source::{ConfigurationSource, EnvironmentConfiguration};
```

- A **leaf child** — one with no directory of its own — is `mod <name>;`. A child that is itself a module root stays
  `pub(crate) mod <name>;`, and callers reach its items through its own facade
  (`crate::…::<root>::<nested_root>::<Item>`).
- Entries are **demand-driven**: a re-export exists exactly when another module imports the item. Adding a port, model
  or use case adds its entry in the same change; removing the last consumer removes the entry. The compiler keeps it
  honest — a missing entry is an unresolved import, and a surplus one is `unused_imports` under `-D warnings`.
- `pub(crate)` is the only legal width. The re-exported item is `pub(crate)` at most, so `pub use` would be `E0364`, and
  `pub(super)` / `pub(in …)` are banned by [§ 11](#11-visibility).
- A `Mock*` type generated by `#[cfg_attr(test, mockall::automock)]` exists only in the test build, so its entry carries
  `#[cfg(test)]`. An item only test code imports carries the same gate, and the entries sharing a child module form one
  brace group (`STY-RUST-085`).
- A module callers reach as a **namespace** — the module itself rather than the items it re-exports — is itself a root.
  Promotion is a file move, never a widening: the module stays `pub(crate) mod <name>;`, its implementation moves under
  it, and its items are re-exported so the callers' `<name>::…` paths keep resolving.

The compiler enforces the import path: with a leaf private, any `crate::…::<root>::<child>::<Item>` outside its root is
`E0603`. `STY-RUST-086` enforces the visibility the compiler cannot check — a `pub(crate) mod` whose name has no
directory is a leaf and must be private (`check_sty_rust_086`). No import-path rule duplicates the compiler: it would
need a generated alternation of every root, and `E0603` already covers the whole tree.

---

#### 7. Unit of Work

The transaction boundary lives in the application service, not the presentation layer. A use case opens a unit of work,
runs business logic across the repositories it exposes, then commits on success or rolls back on failure. Handlers only
call the use case.

---

##### 7.1 Lifecycle

The application service is responsible for opening the unit of work, running the business logic, and committing on
success or rolling back on failure:

```rust
let mut unit_of_work = factory.begin_write().await.map_err(...)?;

let result = async {
    // business logic using unit_of_work.users() / credentials() / ...
    Ok(value)
}
.await;

match result {
    Ok(value) => {
        unit_of_work.commit().await.map_err(...)?;
        Ok(value)
    }
    Err(error) => {
        // The unit-of-work adapter logs a rollback failure (OBS-002); the use
        // case returns the original business error (STY-RUST-031).
        let _ = unit_of_work.rollback().await;
        Err(error)
    }
}
```

A use case opens the unit of work with the intent it has. `begin` opens a read-only unit of work and `begin_write` opens
one that writes; a use case that writes must use `begin_write` (`PERS-022`). The `SQLx` adapter runs a deferred
transaction for `begin` and `BEGIN IMMEDIATE` for `begin_write`, so a writer takes the write lock at the start instead
of upgrading a snapshot: an upgrade over a snapshot another writer committed meanwhile fails with `SQLITE_BUSY_SNAPSHOT`
immediately, and the busy timeout is never consulted. Read-only use cases keep `begin`, so concurrent reads stay
concurrent instead of serializing behind the single writer.

Because a write-intent unit of work holds the single writer lock, it stays short: hashing a whole staged file, probing
media or calling an external service happens between two units of work — as `complete_upload` does — never inside one.
Work bounded to one row or one chunk may stay.

The base trait and the factory are declared in `domain/port/unit_of_work.rs`:

```rust
pub trait UnitOfWork: Send + Sync {
    fn commit(self) -> impl Future<Output = Result<(), UnitOfWorkError>> + Send;
    fn rollback(self) -> impl Future<Output = Result<(), UnitOfWorkError>> + Send;
}

pub trait UnitOfWorkFactory: Send + Sync {
    type Uow: UnitOfWork;
    fn begin(&self) -> impl Future<Output = Result<Self::Uow, UnitOfWorkError>> + Send;
    fn begin_write(&self) -> impl Future<Output = Result<Self::Uow, UnitOfWorkError>> + Send;
}
```

The factory is generic with an associated `Uow` type: no `dyn`, and the concrete unit of work is owned and `'static`
(one `Transaction<'static, Sqlite>` in the SQLite adapter).

---

##### 7.2 Context views

Each bounded context (see [Bounded context](Overview.md#bounded-context)) has a view trait over the unit of work,
declared in its own `domain/port/<context>_unit_of_work.rs`:

Drawn from `src/lib/domain/port/user_unit_of_work.rs`:

```rust
pub trait UserUnitOfWork: UnitOfWork {
    fn users(&mut self) -> impl UserRepository + '_;
}
```

---

##### 7.3 Cross-context use cases

A use case that spans bounded contexts declares every context view it needs as an `execute` bound (`ARCH-003`). It opens
one unit of work, so every write it makes across those contexts commits or rolls back together (`STY-RUST-038`).

Drawn from `src/lib/application/use_case/register_user.rs`:

```rust
impl<F, P> RegisterUserUseCase for RegisterUser<F, P>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
    P: PasswordHasher,
{
    // [...]
}
```

---

##### 7.4 Declaration order in the unit-of-work port file

`domain/port/unit_of_work.rs` declares, in order:

1. `UnitOfWork` — the transaction boundary.
2. `UnitOfWorkFactory` — the opener of a `UnitOfWork`.

Each context view lives in its own file, e.g. `user_unit_of_work.rs`.

Naming: `UnitOfWork`, `UnitOfWorkFactory`, `<Context>UnitOfWork`.

---

#### 8. Repository

A repository persists and queries one aggregate (`STY-RUST-076`). It is obtained from a unit of work and shares that
unit of work's transaction.

---

##### 8.1 Repository port

The port trait is named `<Aggregate>Repository` and declared in `domain/port/<aggregate>_repository.rs`. Methods take
`&mut self` and return `impl Future<...> + Send`.

Drawn from `src/lib/domain/port/user_repository.rs`:

```rust
#[cfg_attr(test, mockall::automock)]
pub trait UserRepository: Send + Sync {
    fn create(&mut self, user: User) -> impl Future<Output = Result<User, RepositoryError>> + Send;
    // [...]
}
```

| Name                                   | Action                                               |
| -------------------------------------- | ---------------------------------------------------- |
| `create(domain: DomainSpecificObject)` | Insert (identity assigned by the datastore)          |
| `save(domain: DomainSpecificObject)`   | Insert/Update (upsert by identity), writing children |
| `delete(filter: &SomeFilter)`          | Delete every aggregate matching the filter           |
| `search(filter: &SomeFilter)`          | Query the aggregates matching the filter             |

`save` is the default mutation when an aggregate is stored as one row, and it persists the child collections the
aggregate carries (a playlist's tracks, an album's mediums and placements, a show's seasons and episodes). An aggregate
may instead expose narrowly-scoped child mutators named `<verb>_<child>` — for example `SeriesRepository::save_episode`,
`MusicAlbumRepository::save_medium` and `GalleryRepository::delete_item` — when a whole-row write would be wrong.

`delete` takes a filter rather than an identity: an all-`None` filter deletes every aggregate the repository owns, and a
filter naming one or more fields deletes the matches. It returns `Ok(false)` when nothing matched (`STY-RUST-044`).
There is no per-identity `get`, `find` or `exists`: `search` with the identity in the filter is the read, and an empty
result is the not-found answer.

The classic `search` is the aggregate query: it returns the aggregate (`STY-RUST-078`). The port declares no read model
for it — a summary or a detail the representation needs is assembled by the application from the aggregate, never
returned by the port and never stitched from per-aggregate lookups. A query that reads an aggregate's child collection
is named for the child (`GalleryRepository::search_items`, `SeriesRepository::search_episodes`,
`MusicAlbumRepository::search_recordings`), which is what distinguishes it from the classic query.

---

##### 8.2 Repository adapter

The implementation is named `<Tech><Aggregate>Repository`, e.g. `SqlxUserRepository`, and lives in
`infrastructure/outbound/<tech>/`.

A trigger abort (`RAISE(ABORT)`) maps to `RepositoryError::Conflict`; see `PERS-017`.

---

##### 8.3 Declaration order in a repository port file

A repository port file declares, in order:

1. The filter struct(s) — one or more `*Filter` structs.
2. The `<Aggregate>Repository` trait.

Anything else the file needs — the `#[cfg(test)]` `&mut T` impl — follows the trait. A port declares no read model for a
query (`STY-RUST-078`).

The `STY-RUST-049` and `STY-RUST-050` checks in `script/rule_enforcer.sh` enforce this order.

---

#### 9. Use cases

An **application use case** — the `<Name>UseCase` trait and its `<Name>` implementation — is the unit this section
governs. It is not the same as a **business use case**: the `UC-###` entries in [UseCases.md](UseCases.md) are
business-level, and each may be implemented by several application use cases.

---

##### 9.1 Declaration order in a use-case port file

A use case trait file declares, in order:

1. The command object (`STY-RUST-054`).
2. The response object, when non-empty (`STY-RUST-055`).
3. The error enum (`STY-RUST-056`).
4. The use-case trait (`STY-RUST-057`).

---

##### 9.2 Use-case factory

Handlers depend on a factory rather than on pre-built use cases: each bounded context exposes one factory that builds
its use cases on demand.

A factory accessor constructs the adapters its use case needs, classified as:

- **Configuration-derived adapter** — its behaviour depends on a configuration value or on the instance identity
  (password hasher, token provider). The accessor rebuilds it from the immutable configuration and identity on every
  call and never caches it (`STY-RUST-060`).
- **Stateless adapter** — it has no runtime state and its behaviour is independent of the configuration (for example
  `Sha2ContentHasher`, `RandomPasswordGenerator`). The accessor builds it at the point of use and never stores it in a
  struct reachable from `AppState` across requests. An adapter may likewise build a stateless adapter it owns at the
  point of use rather than storing it (`STY-RUST-060`). The file-storage adapter is stateless: the accessor builds it
  with no configuration, and every path is composed from the fixed storage root owned by the upload-layout module
  (`src/lib/application/use_case/upload_layout.rs`).
- **Resource-owning adapter** — it owns a resource created once at startup (for example the media-probe permit pool).
  The composition root captures the startup value and the accessor builds the adapter afresh per use from it; the
  adapter is never a long-lived field of the factory.

Nothing outside the composition root builds a process-lifetime adapter (`STY-RUST-064`); building a per-use adapter
inside an accessor is not a violation.

---

##### 9.3 Declaration order in a factory port file

A factory port file declares the trait only: no `Command`, `Response` or `Error`.

---

#### 10. Configuration

`LoadConfiguration` runs once at startup; the resulting `Configuration` is immutable for the process and stored as
`Arc<Configuration>` in `AppState`. Nothing writes it, so there is no hot-reload path: a change to `config.yaml` takes
effect only after a restart. The configuration-derived token lifetime is read when a token provider is built, so the key
used to issue a token is always the key used to validate it (`STY-RUST-063`).

Configuration is total and datastore-free. `LoadConfiguration` resolves it in three layers: the built-in defaults
(`Configuration::default()`) are the base, `FileConfigurationSource` loads the settings `config.yaml` names, and
`EnvConfigurationSource` loads the settings the `MNEMORIUM_*` variables name. Both loaders return the same sparse
`PartialConfiguration` — every leaf is an `Option`, and a `None` keeps the value of the layer below — and the use case
merges each layer itself, rebuilding the resolved value through the domain constructors, so a merged value can never
bypass validation and no adapter owns a merge.

The environment names a setting as `MNEMORIUM_` plus the setting path uppercased and joined with `_`, so `logging.level`
is `MNEMORIUM_LOGGING_LEVEL` and `security.rate_limit.burst_size` is `MNEMORIUM_SECURITY_RATE_LIMIT_BURST_SIZE`. A
prefixed name that matches no setting is discarded silently: the server does not reject variables it does not know. A
variable that is present but malformed fails startup naming the variable and never its value (`STY-RUST-075`,
`OBS-004`). The setting paths the environment applied are logged once, after the subscriber is installed, so an operator
can see which settings the environment took over.

The file document is sparse by construction: an absent, empty or partial document is valid because a key the document
omits is `None` in the partial model, while a key it names is applied even when it repeats the built-in default. An
unknown key fails startup; a value that is present but of the wrong type also fails startup (`STY-RUST-075`). When
`config.yaml` is absent the source writes an embedded default template atomically at mode `0644`; a file that already
exists — including an empty or partial one — is never rewritten. A test compares the template with
`Configuration::default()`, so the two cannot drift.

Instance secrets are not configuration: the site pepper and the token signing secret live in the instance state
(`state.yaml`, [§ 10.2](#102-instance-identity)). A video metadata provider's `api_key` is the one secret-bearing
configuration value (`library.metadata.video.providers`): it is config-file-only, the environment layer names none of
the provider settings, it is never written back, and its `Debug` output is redacted (`OBS-004`). Two paths are fixed
facts rather than settings, each a constant owned by the module that composes it: the datastore path is declared once as
`DATABASE_PATH` in the load-state port (`src/lib/application/port/load_state.rs`), beside `STATE_PATH`; and the library
root is `MEDIA_ROOT` in the upload-layout module (`src/lib/application/use_case/upload_layout.rs`), which owns every
path composed from it, beside the server-owned staging root. Neither can be set through the file or the environment. The
library root is the operator's mount and is never created by the server; the staging root is server-owned and recreated
when absent. The two seed variables in § 10.2 are the only environment inputs that reach the state, and they apply only
at creation.

The configuration also carries the logging settings (`logging.level`, `logging.rotation`, `logging.max_files`,
`logging.ansi`). The `tracing` subscriber is installed from the resolved configuration after `LoadConfiguration` and
before the datastore is opened, so first-boot lifecycle events reach a sink. `logging::setup`
(`src/lib/infrastructure/logging.rs`) fails fast on invalid filter directives and returns the `WorkerGuard` the
composition root holds for the lifetime of the process. Security events emit on the reserved `security` target at their
catalogued level, and `logging.level` cannot suppress them (see
[§ 7 (Logging & Observability)](#logging--observability), `OBS-007`); `Logging::try_new` rejects `off`/`none` as a
whole-level value and any filter directive that targets `security`, and `logging::setup` appends the security floor.

##### 10.1 Startup-only settings

A few settings are read once while the process starts — because the component that consumes them is built once — and a
change to their value has no effect until the server restarts. They are the **startup-only settings**; this table is
their registry.

| Setting                                   | Consumed at startup by                                                                                                                                                                                                | Restart feedback                                |
| ----------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------- |
| `persistence.sqlite3.max_connections`     | The pool sizing in `init_db`, called from the composition root.                                                                                                                                                       | None.                                           |
| `asset.upload.chunk_size_bytes`           | The axum body limit fixed when the router is built (`src/lib/infrastructure/inbound/rest/handler/asset.rs`), and the value the Asset use-case factory snapshots (`src/lib/infrastructure/use_case_factory/asset.rs`). | A new value applies after a restart.            |
| `asset.upload.reap_interval_seconds`      | The upload reaper's tick interval, fixed when the runner is built in the composition root (`src/lib/app.rs`).                                                                                                         | A new value applies after a restart.            |
| `library.startup.media_root_wait_seconds` | The bounded wait for an absent library root in the composition root (`src/lib/app.rs`) and the ingest runner.                                                                                                         | A new value applies after a restart.            |
| `library.startup.fail_without_media_root` | The startup decision in the composition root (`src/lib/app.rs`): refuse startup, or start and reconcile the library as empty.                                                                                         | A new value applies after a restart.            |
| `logging.level`                           | `logging::setup` (`src/lib/infrastructure/logging.rs`) when the `tracing` subscriber is installed.                                                                                                                    | None.                                           |
| `security.root_admin_setup.reveal`        | The Root Admin bootstrap in the composition root (`src/lib/app.rs`): the guard refusing a start that would issue a token it may not reveal, and the one-time reveal itself.                                           | A new value applies after a restart.            |
| `security.root_admin_setup.ttl_seconds`   | **Live, not startup-only:** read whenever the bootstrap issues a token, so the registry names it beside its startup-only sibling.                                                                                     | None; the next issued token uses the new value. |
| `security.rate_limit.trusted_proxies`     | The per-client login limiter built in the composition root (`src/lib/app.rs`) when the router is built.                                                                                                               | None.                                           |
| `security.rate_limit.client_ip_header`    | The per-client login limiter built in the composition root (`src/lib/app.rs`) when the router is built.                                                                                                               | None.                                           |
| `security.rate_limit.burst_size`          | The two login limiters — per client and per account — built in the composition root (`src/lib/app.rs`).                                                                                                               | None.                                           |
| `security.rate_limit.period_seconds`      | The two login limiters — per client and per account — built in the composition root (`src/lib/app.rs`).                                                                                                               | None.                                           |

The registry is **extensible**: a setting becomes startup-only by being added here. Adding one records, in the same
change, how a change to its value is surfaced to the operator (a startup warning, or an explicit `None`).

##### 10.2 Instance identity

The state is specified by `openspec/specs/state/spec.md` (`REQ-STATE-001`–`REQ-STATE-008`). It is the non-derivable part
of the instance: the site pepper, the token signing secret and the instant the state was established. It is not
configuration — configuration is derivable and total, the state is existential.

The Rust vocabulary stays in the Identity bounded context: `InstanceState` and `InstanceStateStore` are the
authentication root the password hasher and the token provider consume, so they belong there under `ARCH-002`. The
capability has no catalogue entry and no REST surface of its own.

The file is a versioned YAML document (`version`, `created_at`, `pepper`, `jwt_secret`) holding both secrets as the
exact 64-character hexadecimal strings the password hasher and the token provider consume (`REQ-STATE-001`,
`REQ-STATE-002`, `REQ-STATE-003`). It is capped at 4 KiB, decoded with `deny_unknown_fields`, and written atomically at
mode `0600` under an exclusive lock on `state.yaml.lock`, so two processes starting against a fresh instance cannot each
persist a different identity (`REQ-STATE-007`). An unknown version, an unknown field, a non-hex secret or a truncated
document fails startup and is never treated as absent.

The state is loaded before the datastore is opened; a failure to read the state stops startup before any datastore file
is opened (`REQ-STATE-005`). The ordering against configuration is a startup design decision, not a state requirement:
`LoadState` runs first because the logging settings are themselves configuration.

`LoadState` mirrors `LoadConfiguration`: it takes no command and no injected presence flag. It reads the persisted state
from `InstanceStateStore`, the creation seeds from `StateSeedSource`, and the datastore's presence from
`DatastorePresence`. The file adapter maps a missing path to `false` and every other `try_exists` failure to an error,
so an unreadable datastore fails startup rather than reading as a new instance and writing a second identity beside it
(`REQ-STATE-006`). The seed port carries the seeds through the `StateSeeds` capability trait, so the seed struct itself
lives in the envy adapter and the application layer never names an adapter type (`ARCH-001`).

State is created only for a genuinely new instance. `LoadState` checks the datastore first: when the datastore exists
and no state does, startup fails and no replacement is written; when neither exists, the state is created first and the
datastore afterwards. The two environment seeds are applied only at creation, and a supplied seed that differs from the
persisted value fails startup naming the variable, never its value (`REQ-STATE-004`, `OBS-004`). A prefixed variable
that is neither a seed nor a configuration override is discarded, never rejected. Creation emits `system_object` and one
`secret_initialized` per generated secret, once the subscriber is installed; reuse emits neither.

A persisted state is private to the server account (`REQ-STATE-008`): the opened descriptor must be a regular file owned
by the account that owns the working directory, and a wider mode is tightened to `0600` with `File::set_permissions`
before the file is read. A file owned by another account, or one whose mode cannot be repaired, fails startup and is
left unchanged; the error names the `chmod`/`chown` remedy and never a secret. The repair runs on the descriptor, so a
symlink or a file swap between the check and the read cannot bypass it.

---

#### 11. Visibility

A declaration is as narrow as its use, and the compiler is the arbiter of every step but the last.

**The entry point.** The crate advertises exactly two declarations: `pub mod app;` in `src/lib/lib.rs` and
`pub async fn run() -> anyhow::Result<()>` in `src/lib/app.rs`. `run` is the composition root (`STY-RUST-064`), and
`src/bin/server.rs` does nothing but call it. `pub` anywhere else declares a reach no outside crate has, which
`unreachable_pub` reports.

**The ladder.** One test orders every other declaration:

1. **private** — the default, when no other module names the item;
2. **`pub(crate)`** — when another module of this crate names the item;
3. **`pub`** — only the two entry-point declarations above.

`pub(super)` and `pub(in …)` are never used. They encode the shape of the module tree in the declaration, so moving a
module silently changes who may see the item; `pub(crate)` states the fact the declaration actually rests on.

**Re-exports.** A `pub(crate) use` in a module facade is a re-export, not a widening: the item keeps the visibility it
declares, and the facade becomes the other module the ladder counts. The facade is therefore the one file allowed to
name its children's items ([§ 6.7](#67-module-facades)), and its own entries are `pub(crate)` exactly for that reason. A
child module with no child modules of its own is private — the ladder's narrowest step — so nothing outside its root can
name it (`STY-RUST-086`).

**The signature lower bound.** A declaration is at least as wide as the widest signature that names it: a type named by
a `pub(crate)` signature cannot be private, and a name the crate entry point exposes cannot be narrower than the entry
point. rustc enforces the bound through its private-interface diagnostics, so the ladder narrows a declaration only as
far as its callers allow: a `pub(crate)` item whose every caller lives in its own file is a finding, while a private
item another module names is a compile error.

**Item coverage.** Every item that can carry a visibility modifier follows the ladder: modules, functions, methods of
inherent `impl` blocks, structs, enums, unions, traits, type aliases, constants and statics. Struct fields are out of
scope: `clippy::field_scoped_visibility_modifiers` and `clippy::partial_pub_fields` already pin their form, and choosing
`pub` fields over private fields plus accessors is an encapsulation decision per struct kind. Trait items and enum
variants carry no modifier of their own — their reach follows the trait or the enum.

**Tests.** Test code is not exempt. `unreachable_pub` reports a `pub` item inside a private `#[cfg(test)]` module in the
`--test` build, so an exemption would be prose the build contradicts. The shared fixtures follow the ladder too: a
`test_helpers` item is `pub(crate)` because the test modules of other files name it, and one only its own file names is
private.

**Constants.** A constant's visibility follows this rule; its site is `STY-RUST-081`'s, and its name and unit are
`STY-RUST-082`'s.

**Enforcement.** Two mechanisms, split at the step each can see:

- `[lints.rust] unreachable_pub = "deny"` in `Cargo.toml` covers `pub` → `pub(crate)`, in the library build and in the
  `--all-targets` test build.
- `check_sty_rust_084` in `script/rule_enforcer.sh` covers `pub(crate)` → private, the step rustc cannot see: a
  `pub(crate)` item used by one file is still "used", so neither a lint nor `dead_code` reports its width. The check
  reads "another module names it" as an identifier occurrence in another file, comments excluded. The approximation
  over-exposes rather than under-exposes — a name mentioned in another module's doc comment reads as a use — and the
  compiler catches the opposite direction.

A `#[cfg_attr(test, mockall::automock)]` attribute generates a `pub` `Mock*` type. `unreachable_pub` does not report it
— the type comes from an external macro expansion — so no allow is needed; should a future generator emit a reachable
`pub` item, the lint reports it like any other.

**The lint set the narrow surface activates.** While every item was `pub`, two lints had nothing to say; a crate-private
surface turns them on, and each finding is settled here rather than by widening a declaration back:

- `clippy::single_call_fn` is not enabled. `STY-RUST-002` gives every endpoint its own file, so the router names it
  exactly once, and `STY-RUST-016`–`STY-RUST-018` mandate the constructors; the lint cannot separate those shapes from
  an accidental one. A single-use helper is a review matter, not a mechanical gate.
- `clippy::pattern_type_mismatch` is not enabled. It rejects the match-ergonomics arms the crate uses
  (`match &candidate { Candidate::Album(group) => … }`), and every rewrite that satisfies it trips `ref_patterns` or
  `needless_borrowed_reference`; enabling it would trade one suppression for three.
- `clippy::missing_docs_in_private_items` is satisfied, not allowed: every module declaration carries a `///` doc and
  every private field and variant carries one.

**Before and after.** The `after` lines of both examples are the source today, drawn from
`src/lib/domain/model/movie.rs` and `src/lib/infrastructure/inbound/rest/handler/library/get_movie.rs`:

```rust
// Before: `pub` on an item only this crate names.
pub struct Movie {
    // [...]
}

// After: `pub(crate)`, because other modules of this crate name it.
pub(crate) struct Movie {
    // [...]
}
```

```rust
// Before: `pub` on an endpoint function only the router names.
pub async fn get_movie(
    // [...]
) -> Response {
    // [...]
}

// After: `pub(crate)`, because `handler/library.rs` mounts it.
pub(crate) async fn get_movie(
    // [...]
) -> Response {
    // [...]
}
```

---

#### 12. Endpoint handler declaration

A complete declaration:

```rust
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// Document-scoped members of the Notes HTTP API.
#[derive(utoipa::OpenApi)]
#[openapi(
    tags(
        (name = "notes", description = "Note bounded context")
    )
)]
struct ApiDoc;

/// Register the context's routes. Each handler carries its path and its path
/// item, so `utoipa-axum` derives the axum route and the specification from one
/// declaration.
fn note_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(create_note, get_note, list_notes))
}

/// Create a note.
///
/// Returns the created note, `400` if the payload is not valid
/// and `401` if the caller is unauthenticated.
#[utoipa::path(
    post,
    operation_id = "create_note",
    path = "/notes",
    tag = "notes",
    request_body = CreateNoteRequest,
    responses(
        (status = CREATED, body = CreateNoteResponse, description = "Note created"),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid payload"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = PAYLOAD_TOO_LARGE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body exceeds the maximum allowed size"
        ),
        (
            status = UNSUPPORTED_MEDIA_TYPE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body is not application/json"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body does not match the expected schema"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Create a new note"
)]
pub async fn create_note() -> axum::response::Json<CreateNoteResponse> {
    unimplemented!()
}
```

An endpoint that is deliberately public declares an empty security requirement instead of a scheme:

Drawn from `src/lib/infrastructure/inbound/rest/handler/get_health.rs`:

```rust
/// Check the service health.
///
/// Returns `200` with the current health status when the service is running.
#[utoipa::path(
    get,
    operation_id = "get_health",
    path = "/health",
    tag = "system",
    servers(
        (url = "http://0.0.0.0:4080", description = "Root of the service, outside /api/v1")
    ),
    responses(
        (status = OK, body = HealthResponse, description = "Service is healthy"),
    ),
    security(()),
    summary = "Check service health"
)]
pub async fn get_health() -> axum::response::Json<HealthResponse> {
    unimplemented!()
}
```

The empty requirement mirrors the router: a public operation is mounted outside the authentication middleware, so it
admits anonymous callers, while a protected operation sits in a route group wrapped by `authenticate` (`STY-RUST-100`).
The declaration is the source of the served path item, so a route cannot drift from the document (`REQ-API-014`).

---

#### 13. Parameter declarations

Parameters declared as inline tuples inside `params(...)`:

```rust
#[utoipa::path(
    get,
    operation_id = "get_note",
    path = "/notes/{id}",
    tag = "notes",
    params(
        ("id" = NumericID, Path, description = "Note id"),
        (
            "X-Request-Id" = String,
            Header,
            description = "Correlation id",
        ),
    ),
    responses(
        (status = OK, body = GetNoteResponse, description = "Note found"),
        (status = NOT_FOUND, body = ErrorBody, description = "Unknown note"),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Fetch a single note"
)]
pub async fn get_note() -> axum::response::Json<GetNoteResponse> {
    unimplemented!()
}
```

A dedicated `IntoParams` struct reused by several handlers:

```rust
use serde::Deserialize;
use utoipa::IntoParams;

/// List notes, paginated.
#[derive(Debug, Deserialize, IntoParams)]
pub struct ListNotesQuery {
    /// Maximum number of notes to return.
    #[param(maximum = 100, minimum = 1, default = 20)]
    pub limit: u32,

    /// Offset of the first note to return.
    #[param(minimum = 0, default = 0)]
    pub offset: u32,

    /// Only return notes matching this title.
    #[param(min_length = 1, max_length = 128)]
    pub title: Option<String>,
}
```

Referenced from the macro as `params(ListNotesQuery)`:

```rust
#[utoipa::path(
    get,
    operation_id = "list_notes",
    path = "/notes",
    tag = "notes",
    params(ListNotesQuery),
    responses(
        (status = OK, body = [ListNotesResponse], description = "Notes matching the query"),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "List notes"
)]
pub async fn list_notes() -> axum::response::Json<Vec<ListNotesResponse>> {
    unimplemented!()
}
```

---

#### 14. Constraint attributes

The following attributes can be applied to any parameter (and, through `#[schema(...)]`, to any property of a `ToSchema`
model):

| Attribute           | Type                         | Meaning                                                                                          |
| ------------------- | ---------------------------- | ------------------------------------------------------------------------------------------------ |
| `format`            | `KnownFormat` or open string | The data type format. See below.                                                                 |
| `write_only`        | flag                         | Property is used only in write operations (`POST`, `PUT`, `PATCH`), never in `GET`.              |
| `read_only`         | flag                         | Property is used only in read operations (`GET`), never in `POST`, `PUT`, `PATCH`.               |
| `nullable`          | flag                         | Property is nullable (note: different from non-required).                                        |
| `multiple_of`       | number                       | Value must be a multiple — the division must yield an integer. Value must be strictly above `0`. |
| `maximum`           | number                       | Inclusive upper bound for a number value.                                                        |
| `minimum`           | number                       | Inclusive lower bound for a number value.                                                        |
| `exclusive_maximum` | number                       | Exclusive upper bound for a number value.                                                        |
| `exclusive_minimum` | number                       | Exclusive lower bound for a number value.                                                        |
| `max_length`        | number                       | Maximum length for string types.                                                                 |
| `min_length`        | number                       | Minimum length for string types.                                                                 |
| `pattern`           | string                       | A valid regular expression in the ECMA-262 dialect the value must match.                         |
| `max_items`         | number                       | Maximum items allowed for array fields. Value must be a non-negative integer.                    |
| `min_items`         | number                       | Minimum items allowed for array fields. Value must be a non-negative integer.                    |

> `format` may either be a variant of the `KnownFormat` enum, or otherwise an open value as a string. By default the
> format is derived from the type of the property according to the OpenAPI specification.

These attributes are declarative: utoipa emits them into the OpenAPI schema, but nothing enforces them during
deserialization. A declared constraint is a promise the handler or use case keeps, or the attribute is not declared
(`REQ-API-024`, `STY-RUST-106`): a value that violates a bound answers `422 Unprocessable Content` and applies nothing.

A closed vocabulary follows the same rule (`REQ-API-018`). It is published as an enum schema on the property or
parameter that carries it, and the handler still parses the raw wire value: a value outside the vocabulary answers `422`
through `ApiError::reject_enum`, so the rejection is the catalogued `enum_validation_failed` event (`OBS-006`) rather
than a framework decode failure. A published enum is the set the server accepts, never the domain's whole vocabulary: a
value the server rejects — a non-grantable role such as `OWNER`, or an `ADMIN` child kind — is absent from it.

The query object is a closed set as well: an operation declares every parameter it accepts, and a request carrying one
it does not declare answers `400 Bad Request` and applies nothing (`REQ-API-022`). A query DTO therefore denies unknown
fields, so a parameter the document does not publish never changes a response silently.

A JSON request body is a JSON object: the shared `JsonObject<T>` extractor in
`src/lib/infrastructure/inbound/rest/handler.rs` rejects a sequence with `422`, because the OpenAPI document describes
every body DTO as an object and serde would otherwise accept a positional sequence. The extractor also normalises a
whole-number float (`1.0`) to an integer before deserialization, because JSON Schema treats it as an integer while serde
does not.

These attributes apply identically on request/response body models through the `#[schema(...)]` attribute of the
`ToSchema` derive:

```rust
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct NoteResponse {
    /// Unique note identifier.
    #[schema(read_only, format = "uuid", example = json!("2lJT"))]
    pub id: String,

    /// Note title.
    #[schema(min_length = 1, max_length = 128, example = json!("Habit tracking"))]
    pub title: String,

    /// Note content.
    #[schema(min_length = 1, max_length = 65536)]
    pub body: String,
}
```

---

#### 15. Example values

An `example` documents a concrete sample value for a schema, parameter or response. It can be declared inline or as a
named example with a `name`, a `summary` and a `value`.

Inline:

```rust
#[schema(example = json!({"title": "Habit tracking", "body": "Log daily streaks."}))]
```

Named:

```rust
use utoipa::openapi::Example;

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = Example(
    name = "minimal",
    summary = "A note with the smallest valid payload",
    value = json!({"title": "Todo", "body": "x"}),
))]
pub struct CreateNoteRequest {
    /// Note title.
    #[schema(min_length = 1, max_length = 128)]
    pub title: String,

    /// Note content.
    #[schema(min_length = 1, max_length = 65536)]
    pub body: String,
}
```

---

#### 16. Request body declaration

```rust
#[utoipa::path(
    post,
    operation_id = "create_note",
    path = "/notes",
    tag = "notes",
    request_body(
        content_type = "application/json",
        content = CreateNoteRequest,
        example = json!({"title": "Habit tracking", "body": "Log daily streaks."}),
    ),
    responses(
        (status = CREATED, body = CreateNoteResponse, description = "Note created"),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid payload"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = PAYLOAD_TOO_LARGE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body exceeds the maximum allowed size"
        ),
        (
            status = UNSUPPORTED_MEDIA_TYPE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body is not application/json"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body does not match the expected schema"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Create a new note"
)]
pub async fn create_note() -> axum::response::Json<CreateNoteResponse> {
    unimplemented!()
}
```

A raw secret supplied _in_ a request body — a login password, a Root Admin setup token — is an input, not a
secret-bearing struct. It is governed by the endpoint's own authentication requirement and by the log-content rules
(`OBS-003`, `OBS-004`), and its debug representation still redacts the value.

---

#### 17. Response declaration

```rust
use utoipa::openapi::{
    content::Content,
    header::Header,
    response::{Link, Response},
};

#[utoipa::path(
    post,
    operation_id = "create_note",
    path = "/notes",
    tag = "notes",
    request_body = CreateNoteRequest,
    responses(
        (status = CREATED, body = CreateNoteResponse, description = "Note created"),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid payload"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the conflicting note"),
            ),
            example = json!({
                "_links": {"self": {"href": "/api/v1/notes"}},
                "error": "A note with this title already exists"
            }),
            links(
                ("conflict" = Link("get_note") = ("id")),
            ),
            description = "A note with the same title already exists"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = PAYLOAD_TOO_LARGE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body exceeds the maximum allowed size"
        ),
        (
            status = UNSUPPORTED_MEDIA_TYPE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body is not application/json"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body does not match the expected schema"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Create a new note"
)]
pub async fn create_note() -> axum::response::Json<CreateNoteResponse> {
    unimplemented!()
}
```

`links` reference another operation declared in the same `OpenApi` derive by its `operation_id`, and express how a field
of this response maps to a parameter of the linked operation. In the example above, on a `409 Conflict`, `create_note`
points to `get_note` using the `id` field of the response body.

Every error response declares `content_type = "application/hal+json"` and carries the body defined in
[HttpApi.md § Error public payload](HttpApi.md#error-public-payload).

> `write_only`, `read_only` and all the [constraint attributes](#14-constraint-attributes) apply on request/response
> body models through the `#[schema(...)]` attribute of the `ToSchema` derive, exactly as they do on parameters.

---

#### 18. HAL payload structs

A payload struct is the source shape of a HAL representation. Its business fields sit at the root as members of the
struct, and its links are a `_links` field whose type lists one `Link` per relation (`STY-RUST-118`, `STY-RUST-119`).
The struct declares no wrapper field (`data`, `payload`, `result`, and the like) around the representation, and `_links`
carries navigation and related resources only, never business data.

`_embedded` is the one nesting key a payload struct may declare, and only where a proven performance need is recorded
(`STY-RUST-120`). The four catalog list response structs are the recorded need: each embeds its page items' partial
representations, so a catalog page needs no request per item (`REQ-API-016`). Every other collection keeps its items at
the root under `items`.

Drawn from `src/lib/infrastructure/inbound/rest/handler/library.rs`:

```rust
/// The partial film representations embedded in a catalog page.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct MovieEmbedded {
    /// The films of the returned page.
    pub items: Vec<MovieSummary>,
}

/// Films matching the requested filters.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct MovieListResponse {
    /// The partial films of the returned page.
    #[serde(rename = "_embedded")]
    pub embedded: MovieEmbedded,
    /// Navigation links of the collection.
    #[serde(rename = "_links")]
    pub links: MovieListLinks,
}
```

---

#### 19. Error message arguments

An `ApiError` variant carries the client-visible message its handler authors (`STY-RUST-121`). Constructing it with a
string literal — optionally wrapped by `.to_owned()` or `.to_string()` — keeps that message stable and free of request
content. A formatted value, a rejection field, or a `Display`/`Debug` rendering would echo a client-supplied value, an
extractor's wording, or an internal type name; rejection detail is logged instead (`REQ-API-010`, `OBS-003`).

Drawn from `src/lib/infrastructure/inbound/rest/api_error.rs`:

```rust
Self::BadRequest("the request query string is invalid".to_owned())
```

The envelope the message becomes is built at the response boundary from the request path, so the message never needs to
carry the path or the query (`REQ-API-008`).

---

#### 20. The query extractor

Every endpoint handler's signature extracts the query, which is what makes the query string a closed set (`REQ-API-022`,
`STY-RUST-122`). A handler that takes query parameters declares its own `<Context>Query` — a `Deserialize` +
`IntoParams` struct whose fields are the parameters the document publishes — and extracts it as `Query<ContextQuery>`.
The struct declares `#[serde(deny_unknown_fields)]`, so a parameter it does not name is a `QueryRejection`: the shared
mapping answers `400 Bad Request`, logs one `input_validation_failed` event for the query, and applies nothing.

A handler that takes no query parameters extracts the shared `NoParametersQuery` from `handler.rs` instead. That empty,
deny-unknown struct rejects every query parameter the same way, so a client's typo is never silently ignored on an
operation whose document publishes no query contract. The struct is not referenced from `params(...)`, because it
declares nothing to publish; it exists only to reject.

Drawn from `src/lib/infrastructure/inbound/rest/handler.rs`:

```rust
/// The query a business operation with no query parameters accepts.
#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
#[serde(deny_unknown_fields)]
pub(crate) struct NoParametersQuery {
    /// No parameter is declared. The skipped marker keeps the type a struct an
    /// empty query still deserializes into, and is never published.
    #[serde(skip)]
    #[param(ignore)]
    _private: (),
}
```

#### 21. Lint expectations

`#[expect]` records that a lint is _expected_ to fire, and the compiler warns when the expectation goes unfulfilled, so
the `-D warnings` gate already removes a stale suppression. What the compiler cannot judge is whether silencing was the
right call. The crate-level lint set in `Cargo.toml` (`[lints.clippy]`, `[lints.rust]`) is the standard: a finding is
fixed in the code, or the lint set itself changes after the trade-off is written down here.

The one standing expectation is `clippy::print_stdout`, in the two files whose operator must read a credential the file
sinks may not carry: the root-admin one-time password (`src/lib/app.rs`) and the seeded demo passwords
(`src/lib/infrastructure/seed.rs`). Each carries the reason that makes it safe (reviewed, not checked:
`check_sty_rust_123` reads the lint names and the declaring file, never the item or the `reason`). No other lint is
expected anywhere, test code included: `pattern_type_mismatch`, `wildcard_enum_match_arm`, `cast_possible_truncation`,
`expect_used`, `enum_variant_names`, `field_scoped_visibility_modifiers` and the rest each have an explicit fix in the
codebase's own idiom, and the rulebook records the choice.

The rule covers every spelling of an expectation — the outer `#[expect(...)]`, the inner `#![expect(...)]`, and an
`expect(...)` nested in `#[cfg_attr(..., ...)]` — and only the lint name matters, never the item it sits on. `#[allow]`
is not a way around it: `clippy::allow_attributes` turns an outer `allow` into the `expect` this rule governs.

`check_sty_rust_123` fails on an expectation that names another lint, and on a `clippy::print_stdout` expectation
outside the two files above. It reads parsed attributes, so a `.expect(...)`/`.unwrap()` call is never an expectation.

### SQL

Rules are grouped in the table below; the numbered detail sections that follow hold their reference material.
`STY-SQL-001` and `STY-SQL-003` are enforced by `sqlfluff` with the repository's `.sqlfluff` configuration, through the
`sqlfluff` pre-commit hook and the `Mnemorium SQL` workflow; the remaining mechanical checks run in
`script/rule_enforcer.sh`.

| ID            | Section                       | Rule                                                                                                                                                                                                                                                                                                   | More info                              | Enforced by               |
| ------------- | ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------- | ------------------------- |
| `STY-SQL-001` | General                       | Use `snake_case` (all lowercase, words separated by underscores). Avoid mixed casing or quoted identifiers (e.g. `"UserId"`).                                                                                                                                                                          |                                        | `sqlfluff` `CP02`, `RF06` |
| `STY-SQL-002` | General                       | Use clear, descriptive English words. Avoid obscure abbreviations (e.g. prefer `customer_number` over `cust_num`).                                                                                                                                                                                     |                                        |                           |
| `STY-SQL-003` | General                       | Never use SQL reserved words (e.g. `order`, `group`, `date`, `select`) as object or column names without an identifying prefix or suffix (e.g. `purchase_order`, `created_at`). Exception: SQLite accepts a few ANSI SQL reserved words (e.g. `user`) as identifiers, so they are allowed.             |                                        | `sqlfluff` `RF04`         |
| `STY-SQL-004` | General                       | Use only standard ASCII alphanumeric characters (`a-z`, `0-9`) and underscores (`_`). No spaces, hyphens, or special symbols.                                                                                                                                                                          |                                        | `check_sty_sql_004`       |
| `STY-SQL-005` | General                       | Enum constraint strings must be in uppercase.                                                                                                                                                                                                                                                          |                                        | `check_sty_sql_005`       |
| `STY-SQL-006` | Table names                   | Singular (`user`, not `users`).                                                                                                                                                                                                                                                                        |                                        | `check_sty_sql_006`       |
| `STY-SQL-007` | Table names                   | Junction / mapping tables combine both entity names in order of primary hierarchy, e.g. `user_role`.                                                                                                                                                                                                   |                                        |                           |
| `STY-SQL-008` | Column names                  | Primary keys: use `<table_name>_id`, e.g. `user_id`, for readability across joins.                                                                                                                                                                                                                     |                                        | `check_sty_sql_008`       |
| `STY-SQL-009` | Column names                  | Foreign keys: use the exact primary key name of the referenced table (e.g. `customer_id` inside the `orders` table). A foreign key named for its meaning instead is permitted only when `rule_enforcer.yaml` records `<table>::<column>-><referenced table>(<primary key>)` in `allowed_foreign_keys`. |                                        | `check_sty_sql_009`       |
| `STY-SQL-010` | Column names                  | Name columns by data type: boolean `is_`/`has_`/`can_`, timestamps `_at`, dates `_date`, counts `_count`, totals `_total`.                                                                                                                                                                             |                                        | `check_sty_sql_010`       |
| `STY-SQL-013` | Keys, indexes and constraints | Name constraints `<constraint_type>_<table_name>_<column_name(s)>`.                                                                                                                                                                                                                                    | [§ 1](#1-keys-indexes-and-constraints) |                           |
| `STY-SQL-014` | Keys, indexes and constraints | Declare all constraints at table level, at the end of the `CREATE TABLE` statement.                                                                                                                                                                                                                    |                                        |                           |
| `STY-SQL-011` | Triggers and functions        | Triggers: `tg_`                                                                                                                                                                                                                                                                                        |                                        | `check_sty_sql_011`       |
| `STY-SQL-012` | Triggers and functions        | Functions: `fn_`                                                                                                                                                                                                                                                                                       |                                        | `check_sty_sql_012`       |

---

#### 1. Keys, indexes and constraints

The type prefix of a constraint name:

| Prefix | Kind             |
| ------ | ---------------- |
| `pk_`  | Primary key      |
| `fk_`  | Foreign key      |
| `uq_`  | Unique key       |
| `idx_` | Non-unique index |
| `chk_` | Check constraint |

---

## Architecture

This section states the invariants the source layout must hold: which way module dependencies point, how bounded
contexts own their models and ports, how work that spans contexts is orchestrated, and how far a cascade delete may
reach. The detail — the source layout, the layer map, the bounded contexts, the composition root and the server
lifecycle — lives in [Overview.md](Overview.md), which these rules constrain.

| ID         | Section         | Rule                                                                                                                                                                                                                                                    | More info                            | Enforced by      |
| ---------- | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ | ---------------- |
| `ARCH-001` | Layer direction | Dependencies point inward only: `domain` ← `application` ← `infrastructure`; no module imports outward (see `AGENTS.md`, "Special Rules").                                                                                                              | [Overview.md](Overview.md)           | `check_arch_001` |
| `ARCH-002` | Bounded context | Each domain model and port belongs to exactly one bounded context; a context view exposes only its own bounded context's repositories.                                                                                                                  | [§ 7.2](#72-context-views)           |                  |
| `ARCH-003` | Cross-context   | Work that spans bounded contexts is orchestrated in the application layer, never by a domain model or an outbound adapter.                                                                                                                              | [§ 7.3](#73-cross-context-use-cases) |                  |
| `ARCH-004` | Cascade scope   | `ON DELETE CASCADE` is used only between tables of one bounded context; a foreign key that crosses a bounded-context boundary declares no cascade, and the owning use case orchestrates the delete (`PERS-020`).                                        | [§ 1](#1-cascade-scope)              |                  |
| `ARCH-005` | Layer direction | A domain service is stateless and invoked by the application layer: it receives the domain models or values it operates on as parameters, holds no I/O, and is a concrete type rather than an interface. A domain model never invokes a domain service. | [§ 2](#2-domain-services)            | `check_arch_005` |

---

### 1. Cascade scope

`ON DELETE CASCADE` is used only within one bounded context: `gallery` → `gallery_item` cascades because both tables are
owned by the Library context. A foreign key that crosses a context boundary does not cascade — `image.file_id` and
`video.file_id` into `file` (Asset-owned) — because deleting a `file` row would reach into Library-owned rows. The
application orchestrates that deletion instead; [`PERS-020`](#persistence) is the persistence restatement of this rule.

---

### 2. Domain services

A domain service is a concrete type under `src/lib/domain/service/` holding a business rule shared by two or more use
cases. It is declared as a concrete type rather than an interface, so the application layer calls it directly with no
trait between; it is stateless, keeping nothing between calls, and performs no I/O; and it receives the models or values
it operates on as parameters. Prefer a domain model's own invariant first: introduce a service only when the same rule
spans use cases. A domain model never invokes a domain service: the rule runs in the use case that constructs or updates
the model, because the use case must see the typed error to answer `422` (`REQ-API-024`, `REQ-API-011`). `ARCH-005`
states the rule, and `check_arch_005` in `script/rule_enforcer.sh` enforces the model-to-service direction.

---

## Persistence

Mnemorium uses a single datastore: **SQLite3**, accessed through `sqlx`. The schema is versioned by migrations in
`migrations/` (one `.up.sql`/`.down.sql` pair per change) and applied at startup by `sqlx::migrate!` in
`src/lib/infrastructure/outbound/sqlx.rs`.

Migrations run at boot inside `init_db`; besides the table schema they seed reference data and install triggers that
enforce invariants.

| ID         | Section                     | Rule                                                                                                                                                                                                                                                                                                                | More info                                                    | Enforced by      |
| ---------- | --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ | ---------------- |
| `PERS-001` | Datastore & migrations      | Use a single datastore: **SQLite3**, accessed through `sqlx`.                                                                                                                                                                                                                                                       |                                                              |                  |
| `PERS-002` | Datastore & migrations      | Version the schema with migrations in `migrations/`, one `.up.sql`/`.down.sql` pair per change.                                                                                                                                                                                                                     |                                                              |                  |
| `PERS-003` | Datastore & migrations      | Apply the migrations at startup with `sqlx::migrate!` in `src/lib/infrastructure/outbound/sqlx.rs`.                                                                                                                                                                                                                 |                                                              |                  |
| `PERS-004` | Datastore & migrations      | Run the migrations at boot inside `init_db`.                                                                                                                                                                                                                                                                        |                                                              |                  |
| `PERS-005` | Seeds                       | Seed the reference data (`color`, `gallery`, `language`, `mime_type`) and the music bucket (`genre('UNKNOWN')`, `music_group` 0, `music_album` 0 and its `DIGITAL` medium) from the migrations.                                                                                                                     |                                                              |                  |
| `PERS-006` | Invariants                  | A `user` row with `user_id = 0` (the Root Admin) cannot be deleted or modified.                                                                                                                                                                                                                                     |                                                              |                  |
| `PERS-007` | Invariants                  | A `gallery` row with `gallery_id = 0` (the default gallery) cannot be deleted or modified.                                                                                                                                                                                                                          |                                                              |                  |
| `PERS-008` | Invariants                  | Normalise `codec`, `genre_id`, `movie.country_of_origin` and `series.country_of_origin` to uppercase on insert/update.                                                                                                                                                                                              |                                                              |                  |
| `PERS-011` | Entity-relationship diagram | Keep the entity-relationship diagram in sync with `migrations/` in the same change that changes a table, column, constraint, trigger or seed.                                                                                                                                                                       | [EntityRelationshipDiagram.md](EntityRelationshipDiagram.md) |                  |
| `PERS-012` | Entity-relationship diagram | Use only the markers `PK`, `FK`, `NN`, `UN`, `CC`, `DF`.                                                                                                                                                                                                                                                            | [§ Legend](EntityRelationshipDiagram.md#legend)              |                  |
| `PERS-013` | Entity-relationship diagram | Put the FK marker on the child table and draw relationships from the "one" side to the "many" side.                                                                                                                                                                                                                 | [EntityRelationshipDiagram.md](EntityRelationshipDiagram.md) |                  |
| `PERS-014` | Datastore & migrations      | While the schema is unreleased, treat migrations as mutable: edit a migration in place, change its SQL, or delete its `.up.sql`/`.down.sql` pair. This exception ends when the schema is released and an Expand/Contract workflow replaces it.                                                                      |                                                              |                  |
| `PERS-015` | Invariants                  | Delete an upload's staging file on every path that does not commit the upload row; a failed begin-upload never leaves a staging file behind.                                                                                                                                                                        |                                                              |                  |
| `PERS-016` | Invariants                  | Treat a staging file that already exists for a newly created upload identifier as a leftover from an interrupted begin and reclaim it, instead of failing the begin.                                                                                                                                                |                                                              |                  |
| `PERS-017` | Datastore & migrations      | Map SQLite's `SQLITE_CONSTRAINT_TRIGGER` extended result code (`1811`, a trigger `RAISE(ABORT)`) to `RepositoryError::Conflict` in `error_mapping.rs`; repositories must not test the extended result code themselves.                                                                                              | [§ 8.2](#82-repository-adapter)                              |                  |
| `PERS-018` | Invariants                  | A `gallery_item` references exactly one media row: either `image_id` or `video_id` is set, never both and never neither (`chk_gallery_item_one_media`).                                                                                                                                                             |                                                              |                  |
| `PERS-019` | Invariants                  | A media row belongs to at most one gallery: the partial unique indexes `uq_gallery_item_image_id` and `uq_gallery_item_video_id` enforce one `gallery_item` per `image`/`video`.                                                                                                                                    |                                                              |                  |
| `PERS-020` | Datastore & migrations      | Cascade deletes only within one bounded context (`ARCH-004`); a cross-context foreign key (for example `image.file_id` and `video.file_id` into `file`) does not use `ON DELETE CASCADE`, so the application orchestrates the deletion.                                                                             |                                                              |                  |
| `PERS-021` | Invariants                  | Two galleries owned by the same user cannot share a name (`uq_gallery_user_id_name`). SQLite treats `NULL` owner identifiers as distinct, so the ownerless default gallery (identifier `0`) stays outside the constraint.                                                                                           |                                                              |                  |
| `PERS-022` | Datastore & migrations      | The `SQLx` unit of work opens a write-intent transaction for a use case that writes: `begin_write` issues `BEGIN IMMEDIATE`, while `begin` keeps the deferred transaction a read-only use case needs. A deferred transaction that reads before it writes fails on a stale snapshot instead of waiting for the lock. | [§ 7.1](#71-lifecycle)                                       | `check_pers_022` |

Editing an already-applied migration (`PERS-014`) changes its checksum, and deleting an applied pair removes its
version. `sqlx::migrate!` then fails at startup — `VersionMismatch` for the changed checksum, `VersionMissing` for the
deleted pair (this change deletes the `20260904012859_create_gallery_video` pair and the three `configuration` pairs) —
instead of leaving a stale-but-running database. Reset by deleting the SQLite file (default `mnemorium.db`) together
with its `-wal` and `-shm` sidecars and restarting the server; the development database is disposable and is rebuilt
from `migrations/`.

---

## Testing

This section defines the testing strategy per layer — unit, integration and system — and the rules every test must
satisfy. The naming and structure of Rust tests are governed by [§ 1 (Code Style Guidelines)](#code-style-guidelines);
the flows the system suite covers come from the critical exception paths in [UseCases.md](UseCases.md).

| ID         | Section            | Rule                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | More info                                    | Enforced by      |
| ---------- | ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------- | ---------------- |
| `TEST-001` | Unit test          | The test module lives in the same file as the unit under test.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | [§ Unit test](#unit-test)                    | `check_test_001` |
| `TEST-002` | Unit test          | Mock an external dependency with `mockall`'s `#[automock]` attribute.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | [§ Unit test](#unit-test)                    | `check_test_002` |
| `TEST-003` | Unit test          | Unit tests cover every layer except the outbound port adapters (SQLite, configuration source), which are exercised in the [Integration test](#integration-test) section, and the REST HTTP handler layer, which is exercised by the [system test](#system-test) suite and by Schemathesis against the served OpenAPI document.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | [§ Unit test](#unit-test)                    |                  |
| `TEST-004` | Unit test          | Do not test the payload DTOs (`<Context>Request`, `<Context>Query`, `<Context>Response`) in isolation; the [system test](#system-test) suite and Schemathesis cover them through the endpoints that consume them.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | [§ Unit test](#unit-test)                    | `check_test_004` |
| `TEST-005` | Unit test          | Repository query filters must be tested for strictness: the filter backs the SQLite repository and a rejected filter must surface its error unchanged.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | [§ Unit test](#unit-test)                    |                  |
| `TEST-006` | Unit test          | Generic tests (those whose subject is not password behavior) use the shared `SECRET_PASSWORD` from `crate::test_helpers` for any password they need.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | [§ Unit test](#unit-test)                    |                  |
| `TEST-007` | Unit test          | Tests whose subject is password behavior (policy validation, wrong-password rejection, change-password, hashing, verification) keep their own explicit passwords and must not use the shared secret.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | [§ Unit test](#unit-test)                    | `check_test_007` |
| `TEST-017` | Application        | Application tests are `async`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | [§ Application](#application-strategy)       | `check_test_017` |
| `TEST-018` | Application        | Have one or more happy-path tests.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | [§ Application](#application-strategy)       |                  |
| `TEST-019` | Application        | Test the domain models here — **they have no dedicated test of their own** (domain services _do_; see [§ Domain service](#domain-service-strategy)).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | [§ Application](#application-strategy)       | `check_test_019` |
| `TEST-020` | Application        | Cover business-rule violations.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | [§ Application](#application-strategy)       |                  |
| `TEST-021` | Application        | Cover validation errors.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | [§ Application](#application-strategy)       |                  |
| `TEST-022` | Application        | Cover dependency failures.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | [§ Application](#application-strategy)       | `check_test_022` |
| `TEST-023` | Application        | Any test that falls outside the scope above (rare situation, cheap coverage) must be justified in the documentation.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | [§ Application](#application-strategy)       |                  |
| `TEST-024` | Domain service     | Domain services (in `src/lib/domain/service/`) have a dedicated unit test module of their own, living in the same file as the service under test.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | [§ Domain service](#domain-service-strategy) | `check_test_024` |
| `TEST-025` | Domain service     | Cover the business rules each service enforces.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | [§ Domain service](#domain-service-strategy) | `check_test_025` |
| `TEST-026` | Domain service     | Domain services are pure — no external dependency, so no mocking.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | [§ Domain service](#domain-service-strategy) | `check_test_026` |
| `TEST-027` | Integration test   | Integration tests exercise the outbound port adapters: the repository (SQLite), the configuration source, and the external services.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | [§ Integration test](#integration-test)      | `check_test_027` |
| `TEST-028` | Integration test   | The test module lives in the same file as the code under test.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | [§ Integration test](#integration-test)      | `check_test_028` |
| `TEST-029` | Repository         | Have one happy-path test.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | [§ Repository](#repository)                  |                  |
| `TEST-030` | Repository         | Verify every constraint — declared in the db schema and reflected in the rest of the project.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | [§ Repository](#repository)                  |                  |
| `TEST-031` | Repository         | Test every trigger defined in the schema.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | [§ Repository](#repository)                  |                  |
| `TEST-032` | Repository         | When one test covers two rules at once it is acceptable, but it must be documented. Any test that falls outside the scope above (rare situation, cheap coverage) must be justified in the documentation.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | [§ Repository](#repository)                  |                  |
| `TEST-033` | SQLite3            | Use an in-memory database — one `:memory:` connection per test, so each test runs against a fresh database.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | [§ SQLite3](#sqlite3)                        | `check_test_033` |
| `TEST-034` | SQLite3            | Configure sqlx with the SQLite option that caps the pool at one connection.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | [§ SQLite3](#sqlite3)                        | `check_test_034` |
| `TEST-035` | External service   | Stub the service with `wiremock`; canned responses come from golden files.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | [§ External service](#external-service)      | `check_test_035` |
| `TEST-036` | External service   | Test every possible server response — including failures such as `Timeout`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | [§ External service](#external-service)      |                  |
| `TEST-037` | External service   | The test module lives in the same file as the client under test. Reformat the file when needed so the test module stays clear.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | [§ External service](#external-service)      | `check_test_037` |
| `TEST-044` | Test helpers       | A helper a test module shares lives in an inline, `#[cfg(test)]`-gated `mod test_helpers`, declared once per root file: crate-wide helpers in `src/lib/lib.rs`, helpers shared only by the descendants of one module in that module's own root (for example `src/lib/infrastructure/outbound/sqlx.rs`); never a separate helper file or module.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | [§ Test helpers](#test-helpers)              | `check_test_044` |
| `TEST-045` | Test helpers       | Adding a new shared test helper, or changing the existing helper surface, requires the developer's explicit approval (their thumbs-up on the change).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | [§ Test helpers](#test-helpers)              |                  |
| `TEST-046` | Factory            | Production factory structs need no dedicated test; they only wire use cases, adapters and transactions, so test the assembled parts, not the wiring.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | [§ Factory](#factory-strategy)               | `check_test_046` |
| `TEST-048` | Shell script       | Shell scripts in the `script/` folder are tooling and CI glue and are exempt from the unit, integration and black-box test requirements; they are validated by their own gates — `shellcheck` and `shfmt` plus their pre-commit and CI execution — not by a test module.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |                                              |                  |
| `TEST-049` | CI / prompt assets | GitHub workflow files (`.github/workflows/*`) and the opencode agent, command and skill prompts (`.opencode/agents/*`, `.opencode/commands/*`, `.opencode/skills/*/SKILL.md`, `.agents/skills/**`) are tooling and agent configuration, not application code; they are exempt from the unit, integration and black-box test requirements. A workflow's YAML is validated by `yamllint`. The triage workflow's `gh`-driven inline prompt, its `OPENCODE_CONFIG_CONTENT` shell permission allowlist, and its effect on issues are exercised only when the workflow runs; they have no automated behavioural gate and no dry-run rehearsal — an explicit residual, validated by review rather than asserted by a test.                                                                                                                                                                                                                   |                                              |                  |
| `TEST-052` | Integration test   | An outbound adapter that drives a native binary is covered by an integration test in the same file; the child process is replaced by a stub executable through the adapter's test-only program seam, every child outcome is asserted, and the real binary is exercised by the container system suite.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | [§ Native binary](#native-binary)            | `check_test_052` |
| `TEST-054` | System test        | Every system test declares the scenarios it proves with `@scenario("SCN-…", TestType.…)`, one decorator per scenario and its type from the closed eight-value set, or with one case-level type decorator (`@positive`, `@security`, …) that every scenario of the case inherits. A Rust test declares the scenarios it proves in a `// proves:` comment above its attributes, naming the kind of proof after an em dash when it has one; the generator rejects a kind outside the closed set. A scenario only the fixture fuzz run proves is declared in `test/api/scenarios.toml` with its kind instead, because that run has no pytest case to decorate. `docs/development/artifacts/RequirementsTraceabilityCoverage.html` is the generated view of declared requirements and scenarios and their proofs; it is regenerated by `script/investigation.sh`, and the authored annotations in its JSON sidecar are never hand-emptied. | [§ System test](#system-test)                |                  |
| `TEST-056` | System test        | A test sleeps at most 3 s on purpose. A longer-horizon observation is reached by pre-seeding the directory the component reads at startup, never by waiting for it.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | [§ System test](#system-test)                |                  |
| `TEST-057` | System test        | An operator message is matched through the suite's single constants module, never inline. A structured `event=` / `component=` field is matched where the server emits one.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | [§ System test](#system-test)                |                  |
| `TEST-058` | System test        | A test runs against its worker's shared instance — booted once, one writable working directory, a random loopback port, the suite's fixed identity and a raised login quota — or, when marked `@pytest.mark.own_instance(reason=...)` with a reason from the closed set (`first_boot`, `restart`, `global_filesystem`, `log_sink`, `exact_collection`, `process_state`), against its own container with its own working directory. A shared-instance test never asserts on global state another test can extend, and never starts, stops or restarts a container; no test touches another test's container or the repository working tree.                                                                                                                                                                                                                                                                                            | [§ System test](#system-test)                | `check_test_058` |
| `TEST-059` | System test        | A refusal case asserts the refusal effect: the persisted file is byte-identical, no replacement identity is written, no `config.yaml` or datastore is created, and the secret value never reaches stdout or a log file.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | [§ System test](#system-test)                |                  |
| `TEST-060` | Logging test       | A test that asserts on emitted log output is annotated `#[traced_test]` from `tracing-test` and matches through `logs_contain`/`logs_assert`; it installs no subscriber of its own — no `tracing::subscriber::set_default`, no `MakeWriter`/capture writer and no `capture_logs`/`captured_logs` helper.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | [§ Logging test](#logging-test)              | `check_test_060` |
| `TEST-061` | Test tooling       | No Allure usage remains: no `allure` import, no `allure.*` decorator, no `#[allure_test]`, no `allure-cargotest` or `allure-pytest` dependency and no `ALLURE_RESULTS_DIR` toggle.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | [§ System test](#system-test)                | `check_test_061` |
| `TEST-062` | System test        | Every `test/system/*.py` case carries at least one `@scenario("SCN-…")` with a well-formed identifier; a declared type is a member of the closed eight-value set, and at most one case-level type decorator names a member of that set beside it.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | [§ System test](#system-test)                | `check_test_062` |

---

### Unit test

Unit tests cover every layer above the outbound ports except the REST HTTP handler layer; the adapters that implement
those ports are covered by the [Integration test](#integration-test) layer instead.

---

#### HTTP handler strategy

REST handlers have no Rust unit-test module. Their routing, extraction, handler and HAL representation are exercised end
to end by the [system test](#system-test) suite (`script/system.sh`) and by Schemathesis (`script/api_fuzz.sh`) against
the served OpenAPI document. The middleware the project owns (`auth`, `hal_errors`, `rate_limit`, `trace`) keeps its own
middleware-level unit tests; the served document itself has no Rust test module, because the fuzz run is the proof of
its contract.

---

#### Application strategy

Application tests exercise a use case with its ports replaced by mocks.

Domain models (`src/lib/domain/model/`) have no dedicated test of their own ([TEST-019](#application-strategy)); they
are covered here. The `TEST-019` check in `script/rule_enforcer.sh` enforces this in pre-commit and CI.

---

#### Domain service strategy

Domain services are pure, so their tests need no doubles.

---

#### Factory strategy

Production factory structs assemble use cases, adapters and transactions; they hold no business rule and no branching
worth isolating, so they have no dedicated test module ([TEST-046](#factory-strategy)). This covers the per-context
use-case factories (`RuntimeUserUseCaseFactory`, `RuntimeIdentityUseCaseFactory` in
`src/lib/infrastructure/use_case_factory/`) and the unit-of-work factory (`SqlxUnitOfWorkFactory`); test the behavior
where it lives instead — the use cases, and, for `SqlxUnitOfWorkFactory`, the repository integration tests.

The test doubles in `crate::test_helpers` (`TestFactory`, `TestUnitOfWorkFactory`, ...) are scaffolding for other tests
and are not covered by this rule.

---

### Integration test

Integration tests exercise the outbound port adapters against their real backing technology rather than mocking it.

---

#### Repository

Repository tests run against the datastore and assert the constraints and triggers the schema declares.

---

##### SQLite3

The SQLite repository is exercised against an in-memory database.

---

#### External service

External service clients are exercised against a stubbed HTTP server ([`TEST-035`](#integration-test)); the strategy is
scoped to an HTTP client. An adapter that instead drives a native binary follows [Native binary](#native-binary).

---

#### Native binary

An outbound adapter that drives a native binary — `LocalMediaProbe` in `src/lib/infrastructure/outbound/media_probe.rs`
runs `ffprobe` — is an outbound-port adapter, so [`TEST-027`](#integration-test) puts its integration test in the same
file. The child process is replaced through the adapter's test-only program seam (`LocalMediaProbe::with_program`), so
the test drives every child outcome without the binary: a success, a non-zero exit, non-JSON output, and a timeout
([`TEST-052`](#native-binary)). The real binary is a container concern, exercised by the system suite, which supplies it
on the image.

Handing the staged, caller-supplied bytes to `ffprobe` is a security event governed by
[`OBS-008`](#logging--observability); the test seam does not replace that obligation.

---

### API fuzz test

The `schemathesis` job in `.github/workflows/api.yml` starts a disposable server through `script/api_fuzz.sh --start`,
which builds it with the `api-docs` and `test-seed` features, seeds the demo library, completes one upload, and
publishes the schema URL, the generated configuration and the Root Admin credentials to the following steps. The
[Schemathesis action](https://github.com/schemathesis/action) then fuzzes the served OpenAPI document against that
server, and `script/api_fuzz.sh --stop` tears it down. It triggers on Rust sources, SQL, the fuzz configuration and
hooks, the harness, the development environment, and the workflow itself. Its JUnit report is uploaded as the
`schemathesis-results` artifact (`junit.xml`) whether the run passes or fails; the report is an untrusted review input,
while the job result gates the pull request.

A configuration entry or a hook is a **fuzz carve-out**: an exception that drops or tolerates a case the served document
should model. A carve-out is allowed only when its comment names one of two sanctioned categories: a concrete server
behaviour the document cannot express (a dependency between two query parameters, a resource a POST-created container
does not hold yet, a `422` where a body value names a missing resource under `REQ-API-013`), or a tool limitation in the
generator or the transport (the `auth.auto-signup` detector crash, the invalid-header-name filter that works around a
`requests` restriction). A carve-out that fits neither category is deleted, and the gap it compensated for is fixed in
the document instead.

Fixture values belong on the operation's own `[[operations]]` entry. A second entry for the same operation does not
augment a `parameters` table: the first non-empty table wins, so a later entry setting only the body silently keeps the
earlier path pin alone. The fuzzing phase honours a body pin. The examples and coverage phases seed the request from the
schema's examples instead, so an operation whose required body value must name an existing entity disables those phases
and proves its pinned positive case in fuzzing. A one-shot destructive operation runs exactly one positive fuzzing case:
the coverage phase would repeat the pinned identifier, and every request after the first is a `404` by design. A patch
body whose optional value names a resource keeps coverage and narrows its displayed warnings to missing test data,
dropping the mismatch warning, because the missing resource is a `422` the document cannot express. The chunk write is
deselected: the generator cannot make a body, `Content-Range` and `Content-Digest` agree, and the system suite drives
that protocol with a coherent request.

The served document declares one response no handler owns: `414`, which the HTTP/1 parser returns before the router runs
when a request line outgrows its 64 KiB buffer. A document-scoped addon adds it to every operation, and both data checks
accept it as a refusal, because no handler can produce it and the response has no body.

Five custom checks extend what a schema-conformance check can see; their bodies are pure functions in
`test/api/checks.py`, and `test/api/hooks.py` registers the thin wrappers the fuzz run uses. `valid_hal_links`
(`REQ-API-003`, `REQ-API-004`, `REQ-API-008`, `REQ-API-021`) proves that a HAL representation carries `_links.self`,
that every link is a non-empty relative target or a marked URI template, and that a `4xx` envelope's `self` is the
request path with its query ignored. `valid_collection_links` (`REQ-API-006`, `REQ-API-007`) proves that a collection's
`search` link is templated and names the collection, that its filters are not payload fields, that `self` and `next`
preserve the request filters, that `next` advances by `limit`, that `prev` never appears at offset `0`, and that a page
without `limit` holds at most 20 items. `valid_embedded_payloads` (`REQ-API-005`, `REQ-API-016`) proves that only the
four catalog listings embed their page items, each a partial representation. `valid_success_responses` (`REQ-API-012`)
proves that a `201` carries `Location` and a `204` no body. `valid_declared_headers` (`REQ-API-009`, `REQ-API-019`)
proves that every emitted application header is declared on its status, that a `401` carries the bearer challenge, that
a `405` keeps `Allow` and that a `429` keeps `Retry-After`.

Three probes reach what Schemathesis' own generation cannot. `request_body_contract_check` drives every operation with a
JSON body once through its `415`, `400` and `422` refusals. `undeclared_query_check` appends a query parameter the
operation does not declare and expects `400` (`REQ-API-022`). `protected_operation_check` sends every operation that
declares `bearer_auth` once without credentials and expects the `401` envelope with its challenge (`REQ-API-023`).
`hypermedia_body_check` replays an accepted JSON body with `_links`/`_embedded` added and expects the business data to
decide it (`REQ-API-002`). Each runs once per operation with the same credential guard the body probe uses.
`devenv tasks run test:api` runs the fuzz.

---

### System test

System tests prove a capability's specification scenarios black box. They drive the containerised image as a black box:
every case of a pytest worker shares the one instance that worker booted — one writable working directory, a random
loopback port, the suite's fixed identity and a raised login quota — and a case that cannot share marks itself
`@pytest.mark.own_instance(reason=...)` and starts its own container instead (`TEST-058`). The reason comes from the
closed set `first_boot`, `restart`, `global_filesystem`, `log_sink`, `exact_collection` and `process_state`; a
collection hook keeps the marker and the `sut` fixture in agreement, and `script/rule_enforcer.sh check TEST-058` audits
every module. The shared instance is booted lazily by the first case that requests `server`, is probed once before each
such case (an unhealthy instance fails that case with its evidence attached and stops the session), and hands out a
cached token for the Root Admin and one for the system profile's seeded identity, each re-obtained after a `401`. A
shared case never asserts on global state another case can extend and never starts, stops or restarts a container. The
image is named by `MNEMORIUM_IMAGE`, default `mnemorium:latest`; a test never builds it. One module per feature area
sits directly under `test/system/` — `api.py`, `audio.py`, `authentication.py`, `authorization.py`, `configuration.py`,
`cover_art.py`, `download.py`, `file.py`, `gallery.py`, `image.py`, `logging_events.py`, `media_ingest.py`,
`metadata_enrichment.py`, `movies.py`, `music.py`, `series.py`, `state.py`, `upload.py`, `user.py` and `video.py`.
`test/system/conftest.py` collects those unprefixed modules because the global `python_files` patterns match none of
them. Each case asserts only what an operator can observe: the HTTP response, standard output, the log sink and the
working directory. The worker's shared instance boots the `system` seed profile, so one browsable catalogue serves every
case of that worker; an own container stays unseeded unless the case names a profile. A log-sink assertion treats the
sink as a collection: it snapshots what the sink holds, acts, then filters the added records on a value the case owns.
No system case reads the datastore: a stored record is read over HTTP at its exact path, a typed row through the item's
own link, and staging and content bytes through the working directory. `authorization.py` is the primary proof of the
authorization engine — grant algebra, decision order, visibility, explanation, the permission API and denial semantics —
and each capability suite proves its own bindings: the resource parent, the default creation grant and the action its
operations require. Every case runs on the shared instance, provisioning fresh accounts it owns and restoring any shared
deny before it ends.

Configuration has no suite of its own: resolution is proved here on cheap probes rather than on every setting, so
`configuration.py` proves the layer order and the file's lifecycle, and `upload.py` proves the `asset.upload.*` effects,
because each one is an upload scenario rather than a configuration-layer property. Input parsing and input rejection are
not system-test subjects: a wrong type, an unknown key, a malformed environment value and a domain-invalid value all
fail startup in [§ 10 (Configuration)](#10-configuration); those cases are covered by the Rust application and
integration tests that own the loader and the domain constructors. The suite asserts the effect of a **valid** value.

`state.py` starts, stops or restarts its own container in every case, so it marks the whole module `own_instance` with
the `global_filesystem` reason and uses the `sut` fixture directly rather than the shared `server` fixture.

`test/util/investigation.py` generates `docs/development/artifacts/RequirementsTraceabilityCoverage.html`, the coverage
report over the specification tree. `script/investigation.sh` runs it without pytest and without a container: it parses
`openspec/specs/**/spec.md`, the `@scenario` decorators of `test/system/*.py` — and the case-level type decorator beside
them, when a case declares its kind once — the scenario manifest `test/api/scenarios.toml`, and the `// proves:`
comments under `src/`, and rewrites the report in place. The page carries summary cards, charts, a search box and
filters; each requirement card carries the requirement's declared description and one row per scenario with its steps,
types and evidence, and any authored annotation directly below it. **Covered** means a committed system case, an
API-suite entry or a Rust `// proves:` claim. Each scenario is counted once by its strongest source — system, then API,
then Rust, then a Cause (Skipped), then no proof — and the requirement card shows that distribution in one bar. A
scenario is covered, skipped (no proof and a Cause) or uncovered (neither). `Tested` means every declared scenario of a
requirement is covered, not that a particular run passed. The derived views are generated; `Cause`, `Why not black box`,
`Note` and `N/A` are authored in `docs/development/artifacts/RequirementsTraceabilityCoverage.json`, which the generator
reads and never writes. A malformed document or an unknown key is an error, an `N/A` contradicted by a real proof is a
warning, and an identifier that leaves the specification is reported as stale rather than dropped.
`script/investigation.sh --check` renders the report without writing and fails when it is stale
([`TEST-054`](#system-test)).

The suite drives a declared operation through the typed client `openapi-python-client` generates from the served
document. The client is never committed: `script/openapi_client.sh` boots a throwaway, unseeded container from the image
under test — `script/system.sh` builds that image with `api-docs test-seed`, the feature pair the performance image
already uses — reads `/doc/openapi.json`, and writes `test/generated/openapi.json` and
`test/generated/mnemorium_client/`, both ignored. `script/system.sh` regenerates before pytest, so no case runs against
a client the handlers have outgrown, while a bare `pytest test/system` stops with the command that produces it.
`Sut.client(token)` hands a case its cached client on the instance's `/api/v1` base, and `payload(response)` reads the
raw body.

Two requests stay hand-built through `Sut.request`: a request the document cannot declare, such as a `HEAD` on a content
route or the operator-configured `security.rate_limit` client-address header; and `/health`, which sits outside the
`/api/v1` base the client is bound to and therefore stays on `requests`. The API index is still read through `Sut.index`
when a case proves a published relation — the index and item routes come from the document, so a case builds neither.

The document is the client's contract, so a request the suite cannot express is a missing declaration: a path, query or
header parameter, a request body, a response body or a status belongs in the handler's `#[utoipa::path(...)]` or
`ToSchema` and is then covered by the [API fuzz test](#api-fuzz-test). The single generator-side alias is
`test/openapi_client.yaml`, which maps the content responses' `*/*` — the stored file's own media type — onto
`application/octet-stream`; `script/openapi_client.sh` fails on any generator warning, so no operation or response is
dropped silently. A declared field is read from the model, whose `None`/`UNSET` distinction tells a cleared value from
an omitted one, while the field set itself — a value a representation must omit, a relation it must not publish — is
read from `payload(response)`, because a model drops what the document does not declare.

A scenario's proof is a system test whenever the container can observe the outcome. A Rust test follows only for what
the black box cannot reach — a failure injected into a port, a redaction mechanism, a sampling distribution or response
timing. A declared status, content type, body shape or response header belongs to the [API fuzz test](#api-fuzz-test).

When one black-box suite takes a scenario over from another, the replaced case is deleted in the same change, so one
failure never reports twice.

Every test declares the scenarios it proves, and `docs/development/artifacts/RequirementsTraceabilityCoverage.html` is
the resulting report of declared requirements and scenarios to their proofs. A system test stacks
`@scenario("SCN-…", TestType.…)` per scenario, or declares the kind once for the case with a bare `@positive`-style
decorator; a Rust test writes `// proves: SCN-…` above its first attribute and may name the kind after an em dash. Both
forms are described in [§ 5 (Scenario labels)](#5-testing) and required by [`TEST-054`](#system-test) and
[`TEST-062`](#system-test).

The authorization capability splits its proof by level. The decision engine in `src/lib/domain/service/authorization.rs`
is a pure domain service, so its grant algebra, traversal bound, relation lend, owner/root/upload special cases and
explanation live in its own test module. The resource, principal, grant, action and role models have no dedicated test
and are covered by **application tests** through the use cases that construct and read them (`TEST-019`). The
resource-and-grant read adapter is covered by **repository integration tests** (`TEST-027`–`TEST-030`). The published
scenarios are proven black box by the **system suite** in `test/system/authorization.py`; a Rust test runs only for a
mechanism the container cannot observe.

---

### Seed data

`test/api/seed` holds the demo fixture, `test/performance/seed` the volume library, and `test/system/seed` the browsable
catalogue the system suite addresses; each tree carries its `sql/` scripts and its `media/` bytes. The `test-seed` Cargo
feature is off by default and is the switch that compiles the seed module; which fixture it applies is
`MNEMORIUM_SEED_PROFILE`. A boot that names no profile applies nothing, and an unknown profile fails startup naming
`seed.profile`. A build without the feature neither compiles the seed module nor reads the trees, so the shipped image
is unaffected (`ARG CARGO_FEATURES=""` in the `Dockerfile`).

The fixtures are **data, not part of the binary**: a seeded boot resolves them from its working directory
(`test/<fixture>/seed`), so a launcher mounts or links the checkout's `test/` tree beside it and adding a row or a media
file is a data change rather than a rebuild. The scripts are applied once through a `sqlx` migrator with a history table
of its own (`_sqlx_seeds_api`, `_sqlx_seeds_perf`, `_sqlx_seeds_system`); they are one-way — no `.down.sql` — because
seeded data is disposable. Changing a script that already ran changes its checksum and is refused like a migration;
reset by deleting `mnemorium.db` (with its `-wal`/`-shm` sidecars) and the profile's media directory (`media/seed` or
`media/system`), then restart.

The scripts declare every row, the `file` rows included, with the size and modification instant left as placeholders
because SQL cannot know them. Two steps stay in Rust: an Argon2 hash bound to the live pepper cannot be written by SQL,
so the feature creates the demo `credential`/`user` rows; and each fixture's `file` rows are then _grounded_ — the byte
at the row's path is materialised from the fixture's `media/` directory when the launcher mounted none, and the two
placeholder columns take the values the filesystem reports, so the startup scan reports every file `Untouched`.
`script/system.sh` and `script/performance.sh` mount the tree read-only; for the system profile the media is mounted at
`media/system`, the directory its rows name, so nothing is copied. The three host-binary launchers
(`script/api_fuzz.sh`, `script/zap_scan.sh` and the typed-client step of `typescript.yml`) link the tree into their
scratch directory instead. The demo accounts are `alice` and `bobby`; their shared password is fixed test data and is
printed on the first seeded run. The system profile's identity is `coverart`, the owner of its seeded playlist and
gallery. The feature creates `media/` when it is absent, because the run owns the directory it populates; a build
without the feature still never creates the operator's mount. A run from the repository root needs no launcher at all,
because the working directory already holds `test/`:

```sh
MNEMORIUM_SEED_PROFILE=api cargo run --features test-seed --bin server
```

The feature is exercised by every seeded suite run and by `cargo llvm-cov --features test-seed`; the standard test image
stays unseeded because the existing suites assert seed-free counts. The disposable server `script/api_fuzz.sh` starts is
built with both `api-docs` and `test-seed`, so Schemathesis fuzzes a populated library. The run takes its fixture
identifiers from `schemathesis.toml`: reads address the fixed seed rows, and the provider-backed `apply` operations are
deselected because the disposable server configures no metadata provider. One upload is completed before fuzzing, so the
completion endpoint addresses a finished session. The run drops one generator artifact the transport cannot express — a
malformed header name — through `test/api/hooks.py`; the served document describes the date, username, name and
season/episode vocabulary precisely enough that the other filters the configuration once carried are gone. It accepts a
`422` where a request body names a missing resource (`REQ-API-013`).

---

### Test helpers

Shared Rust test helpers live in an inline, `#[cfg(test)]`-gated `mod test_helpers`, declared once per root file, at one
of two scopes. The crate-wide module lives in `src/lib/lib.rs` and is the sanctioned exception to [TEST-001](#unit-test)
(a test module lives with the code under test). A module-scoped helper module lives in the root file of the module whose
descendants share the helper — for example `src/lib/infrastructure/outbound/sqlx.rs` for the SQLite adapters — and every
user must sit inside that module's subtree; a helper that escapes it moves to `src/lib/lib.rs`. The module-scoped roots
are listed in `rule_enforcer.yaml` under `TEST-044.module_scoped_helper_roots`, so the set cannot grow or rot silently.
Both scopes declare the module inline (never `mod test_helpers;`) and gate it with `#[cfg(test)]`.

The crate-wide module holds the fixtures the in-crate tests share: the secrets `SECRET_PASSWORD` and `TEST_JWT_SECRET`;
the unit-of-work double `TestUnitOfWork`/`TestUow` and its factory double `TestUnitOfWorkFactory`/`TestFactory`, wrapped
by `AssetFactoryHarness::new` and `GalleryFactoryHarness::new`; `TEST_PEPPER` and `instance_state`; the app-state
builder `app_state`; the album release date `album_release_date`; the video date `video_date`; and the response helpers
`error_message_of` and `into_status_and_body`.

`TestUnitOfWork` implements `Default`, and a test names only the repositories it stubs —
`TestUnitOfWork { galleries, ..TestUnitOfWork::default() }` — so adding a repository to a unit of work changes this
module, the context's `UnitOfWork` implementation, and the tests of the use cases that reach it, never the test modules
of the other use cases.

Helpers are never scattered into per-module files — for example a `test_helper.rs` beside the code under test, or a
second helper module in the same file. A new shared helper is added to one of these modules, and only with the
developer's explicit approval.

**Fixtures.** A fixture is an rstest `#[fixture]` function (`STY-RUST-029`): it builds one piece of test input — a
domain model, a command, a fixed value — from no test input, and a test takes it as a parameter. A fixture that needs an
argument receives it through `#[with(...)]`. A zero-argument helper that builds a value is therefore a fixture:
`fn album()`, `fn command()`, `fn user()`, `fn timestamp()` and `fn first()` are findings, whether or not a test calls
them. A helper that takes a runtime parameter is a constructor, not a fixture: rstest resolves a fixture parameter as
another fixture, never as a caller-computed value, so such a helper stays a function. A helper that returns nothing is
setup or an action and belongs inline in the test.

**Assertions.** There is no assertion helper (`STY-RUST-087`). `assert_eq!`, `assert!` and `matches!` run inline in the
test that owns the assertion, so a failure names that test and its line; a helper that takes the response and asserts on
it hides both.

**Mock expectations.** A test configures its mocks in its own body (`STY-RUST-090`). The `.expect_*()` chain that stubs
a repository is written where the test arranges its input; no `expect_*` function exists, and no `#[fixture]` sets an
expectation. A shared mock returning a fixed value is still arranged by each test, because the expectation is part of
that test's arrangement.

The test double's own constructors are part of the double, not a test's setup helper: `TestUnitOfWork::default()` and
`TestUnitOfWork::with_any_caller()` live in `mod test_helpers`, and a test starts from one with a struct update,
`TestUnitOfWork { files, ..TestUnitOfWork::with_any_caller() }`.

**Datastore seeds.** A helper that seeds datastore rows lives only in the module-scoped `test_helpers` of
`src/lib/infrastructure/outbound/sqlx.rs` (`STY-RUST-088`). A repository test imports its `seed_*` helper; it declares
neither a seed helper nor a differently named helper whose body inserts a row.

**Test module placement.** A file declares at most one `#[cfg(test)] mod tests`, and that module covers only the code in
its own file (`STY-RUST-089`). The sanctioned `mod test_helpers` is not a `mod tests` and may sit beside it. The module
root `src/lib/infrastructure/outbound/sqlx.rs` keeps only the `init_db` tests; a constraint, trigger or cascade test
lives in the test module of the repository adapter whose code it exercises, never in the root's module. The check counts
the `mod tests` blocks of a file and holds that root to its `init_db_*` prefix; for any other root, whether a test
exercises a descendant is a review matter, because a root's own test may legitimately name its children's types.

The module-scoped root of the SQLite adapters holds `begin_transaction`, the migrated in-memory transaction their
repository tests run in ([TEST-033](#sqlite3), [TEST-034](#sqlite3)), and every datastore row seed (`STY-RUST-088`).

---

### Logging test

A test that asserts on emitted log output uses `#[traced_test]` from `tracing-test` and matches through the injected
`logs_contain` and `logs_assert` helpers ([TEST-060](#logging-test)). It installs no subscriber of its own: no
`tracing::subscriber::set_default`, no `MakeWriter` capture writer and no `capture_logs`/`captured_logs` helper.

`#[traced_test]` installs one global subscriber for the whole test binary and opens a span named after the test, so the
capture does not depend on a thread-local dispatcher. A scoped dispatcher is what made the capture tests flaky: the
`tracing` callsite cache is process-wide, and installing or dropping a scoped subscriber on one test thread while
another test emits an event could cache the event as disabled, silently losing it. The global subscriber removes both
the install churn and the per-thread limitation.

The dependency enables `tracing-test`'s `no-env-filter` feature because the server emits its security events on the
reserved `security` target (`OBS-007`), which the crate's default `<crate>=trace` filter would drop. The consequence is
that every dependency's lines are captured too, so:

- match the event or message token in `logs_contain` (`event="limit_exceeded"`, `kind="watch_registration_failed"`),
  never the plain level; a level-only count also matches a dependency's line;
- use `logs_assert` for an exact count or an absence check, and restrict an absence check to the test's own event lines
  — `line.contains("security:")` — because a dependency legitimately logs the raw error (for example sqlx logs the SQL
  and the `RAISE(ABORT)` message a security event must redact);
- do not use a needle that also appears in the test's own name: the line carries the test span, so
  `from_bytes_rejection_oversized_body_does_not_log_buffer_failed` matches `buffer_failed` in its own prefix — assert on
  `event="buffer_failed"` instead;
- a shared assertion helper takes `&[&str]` and returns `Result<(), String>`, and the test calls it from
  `logs_assert(|lines| helper(lines, …))`; the injected functions exist only in the annotated function body.

`logs_contain` and `logs_assert` see only the lines tagged with the test's span, so the asserted event must be emitted
on the test thread. `#[tokio::test]` runs on a current-thread runtime and the awaited call is polled on that thread; do
not assert logs emitted from `spawn_blocking`, a worker thread or the `notify` watcher thread.

`#[traced_test]` follows the test attribute (`#[tokio::test]`, then `#[traced_test]`). It does not compose with
`#[rstest]`'s parameterised cases, so a case matrix is written as one test per case over a shared helper.

---

## Dependencies & Dev Environment

The direct dependencies — the Rust crates, development tooling, container images and runtime system binaries — are
listed with their versions and licences in [DependencyManifest.md](DependencyManifest.md); this section governs the CI
rules that keep them resolvable and trustworthy.

| ID         | Section               | Rule                                                                                                                                                                                                                                                                                                                                                                               | More info                       | Enforced by |
| ---------- | --------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------- | ----------- |
| `DEPS-001` | GitHub Actions        | Pin every third-party GitHub Action to a full-length commit SHA and record the release in a same-line comment, for example `# v6` or `# v1.2.3`.                                                                                                                                                                                                                                   | [§ 1](#1-ci-supply-chain)       |             |
| `DEPS-002` | CI trust boundary     | A CI/CD job that holds a repository secret, a write-scoped token, or a registry credential must load the configuration it executes from a trusted revision, never from the pull-request revision.                                                                                                                                                                                  | [§ 2](#2-ci-trust-boundary)     |             |
| `DEPS-003` | CI runtime resolution | Reference every package or executable a CI job resolves at run time by an exact version, never a floating range; a transitive dependency tree remains a documented residual where no lockfile exists.                                                                                                                                                                              | [§ 3](#3-ci-runtime-resolution) |             |
| `DEPS-004` | Dev/CI tool parity    | A tool a CI job resolves through `devenv.lock` resolves from the lockfile's pinned revision — never restated in the workflow — and the job's change filter matches `devenv.lock`, `devenv.yaml`, and the workflow file that defines the job, so a lockfile or job change re-runs the gate; hand-restated pins and other non-`devenv.lock` tool installs are a documented residual. | [§ 4](#4-devci-tool-parity)     |             |

---

### 1. CI supply chain

A GitHub Action referenced by a tag or branch is mutable: whoever controls the action repository can move the ref. Pin
every third-party action to a full-length commit SHA and record the release beside it, so a human can read it and
Dependabot can bump it:

```yaml
- uses: actions/checkout@d23441a48e516b6c34aea4fa41551a30e30af803 # v6
```

Dependabot reads `.github/dependabot.yml` and opens a pull request when a pinned SHA falls behind; it only updates the
release comment when that comment sits on the same line as the SHA.

Pinning a SHA strips the ref of the meaning the action gave it, so configuration carried by the ref moves into an input.
`dtolnay/rust-toolchain` selects its toolchain from the ref: it is pinned to a SHA with the release tag in the comment,
and the toolchain is supplied through `toolchain:`. The CI table records the toolchain it installs (`1.98.0`), not the
action's own release tag.

---

### 2. CI trust boundary

A CI/CD job that holds a repository secret, a write-scoped token, or a registry credential must load the configuration
it executes from a trusted revision, never from the pull request under review. Pull-request content is data: pass it to
a tool that interprets it as input, never as the agent definitions, skills, project configuration, or scripts that run
with the credential.

The write scopes a job needs are a property of the job, not a violation. The rule bounds _what executes with the
credential_, not which scopes the job may hold. Output a job produces from untrusted content stays untrusted and must be
treated as data by any later step that consumes it.

---

### 3. CI runtime resolution

Reference every package or executable a CI job resolves at run time by an exact version; never a floating range.
`npx --yes`, `pip install`, and `nix run nixpkgs#…` resolve their dependency tree fresh on each run, so an exact direct
version still leaves every transitive dependency unpinned. Where a job cannot use the pinned `devenv.lock` toolchain or
a digest-pinned image, this residue is a documented residual risk: the direct reference must still carry an exact
version, and the residual must be recorded rather than implied pinned.

---

### 4. Dev/CI tool parity

A tool that both the dev environment and a CI job run must resolve to the same revision; otherwise the CI gate silently
validates against a different formatter, linter, or generator than the developer uses. Restating `devenv.lock`'s
revision in a workflow by hand creates a second source of truth that no automated bump maintains — derive it from the
lockfile instead, and fail rather than run when the lock does not pin a full commit, so pull-request content cannot
float the fetched revision.

A job whose validity depends on the lockfile must re-run when the inputs move, so its file filter matches `devenv.lock`
(which pins the revision), `devenv.yaml` (which declares the inputs), and the workflow file that defines the job — the
last so the gate is exercised when its own definition changes.

The rule binds only the tools a job resolves through `devenv.lock`. Jobs that install tools from another source — the
`supply-chain` job's hand-restated `cargo-deny`/`cargo-about` pins and the `taiki-e/install-action` tool installs — are
a documented residual of this parity, recorded rather than implied pinned, exactly as `DEPS-003` records its own.

---

## Documentation

This section governs the examples, diagrams, citations, routes, excerpts, business-use-case entries and use-case
diagrams the documentation publishes. It keeps every published example traceable to the code it restates, so a reader
can verify it and a reviewer can catch it going stale.

| ID        | Section               | Rule                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | More info                          | Enforced by     |
| --------- | --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------- | --------------- |
| `DOC-001` | Code examples         | An example that restates production code names the source path it is drawn from, is updated in the same change as that code, and may elide omitted regions with a `// [...]` marker.                                                                                                                                                                                                                                                                                                                                                                                            | [§ 1](#1-code-examples)            |                 |
| `DOC-002` | Domain model diagrams | A domain model diagram that the documentation publishes represents the domain's real-world concepts — conceptual classes, typed attributes, and named associations with multiplicity — and never software methods, database tables or other implementation detail.                                                                                                                                                                                                                                                                                                              | [§ 2](#2-domain-model-diagrams)    |                 |
| `DOC-003` | Citations             | Documentation (`docs/`, the root Markdown) and production source (`src/`) cite only paths the repository tracks; they never name the local, untracked `.artifacts/` scratch directory, which a reader cannot resolve.                                                                                                                                                                                                                                                                                                                                                           | [§ 3](#3-citations)                | `check_doc_003` |
| `DOC-004` | Published route       | A route the documentation publishes names a path the handler declarations publish in the specification the server serves.                                                                                                                                                                                                                                                                                                                                                                                                                                                       | [§ 4](#4-published-routes)         |                 |
| `DOC-005` | Business use cases    | Every business use case entry in [UseCases.md](UseCases.md) is a `UC-<seq> - <Name>` heading — `###`, or `####` inside a sub-package — filed under the package section that owns it, carries the § 5 field headings in the documented order one heading level below the entry, carries none of the removed headings (`Application use case(s)`, `Business rules`, `Secondary actor (optional)`, `Bounded context(s)`), names one capability without joining alternatives with `and` or `or`, and the catalog holds each of `UC-001` … `UC-0NN` exactly once.                    | [§ 5](#5-business-use-case-format) | `check_doc_005` |
| `DOC-006` | Code examples         | An example that illustrates the endpoint-handler declaration (`STY-RUST-091`–`STY-RUST-122`) or the HTTP reference in [HttpApi.md](HttpApi.md) rather than restating a cited source file has no source to cite, but is kept in step with the rule range and the reference: when a rule it illustrates changes, the example changes with it.                                                                                                                                                                                                                                     | [§ 1](#1-code-examples)            |                 |
| `DOC-007` | Business use cases    | A business use case entry states intent and outcome in business terms: in every field — `Trigger` included — it names no HTTP method, route, status code, representation media type, representation mechanic, request or response field, query parameter, event payload field, storage or transaction mechanism, concurrency race, or implementation-layer term; the wire contract belongs to the `api` specification (`openspec/specs/api/spec.md`) and the generated specification.                                                                                           | [§ 5](#5-business-use-case-format) | `check_doc_007` |
| `DOC-008` | Use case diagrams     | A use-case diagram the documentation publishes stays inside [UseCases.md](UseCases.md): the preamble carries one master diagram that declares the package registry — the bounded contexts, with their sub-packages nested — and no use case; every package that holds use cases carries exactly one diagram of its own package, and a package that only groups sub-packages carries none; each catalogued use case appears exactly once, labelled `UC-<seq> - <name>`, in the diagram of the section that holds it; and the only actors are the [Glossary](Glossary.md) actors. | [§ 6](#6-use-case-diagrams)        | `check_doc_008` |

---

### 1. Code examples

A code example drawn from production code has two duties and one allowance:

- **Cite the source.** Name the path it is drawn from, in an inline code span in the paragraph immediately before the
  fenced block.
- **Track the source.** When the named file changes, update the example in the same change. Never let the example and
  the code disagree.
- **Elide, don't paraphrase.** Mark each run of omitted lines with `// [...]`, keep the lines that remain in the
  source's order, and keep their attributes. Doc comments may be dropped without a marker.

An example that illustrates the endpoint-handler declaration (`STY-RUST-091`–`STY-RUST-122`) or the HTTP reference in
[HttpApi.md](HttpApi.md) rather than restating a named source file has no source to cite; it is kept in step with those
rules and that reference instead, so a change carries the example with it (`DOC-006`).

For example, drawn from `src/lib/domain/model/user.rs`:

```rust
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum UserError {
    #[error("email has an invalid format")]
    InvalidEmail(#[from] email_address::Error),
    // [...]
}
```

---

### 2. Domain model diagrams

A **Domain Model Diagram (MDD)** is a conceptual model: it represents the real-world concepts of the domain rather than
the software classes that implement them. It records what exists in the domain, what information must be remembered, and
how the concepts relate.

An MDD is not the [entity-relationship diagram](EntityRelationshipDiagram.md). The ERD is the physical schema and tracks
`migrations/` (`PERS-011`–`PERS-013`, [§ Persistence](#persistence)); an MDD is product vocabulary and tracks the
concepts documented in [`Overview.md`](Overview.md#domain-model). Neither replaces the other.

**Conceptual classes.** Model real-world concepts, never implementation classes or database tables, and give a class no
methods. A class name is a singular noun in `UpperCamelCase` (`MusicAlbum`, not `MusicAlbums`). Identify classes from
business transactions and their items, products and services, people and their roles, organizations, physical objects,
important events, descriptions and catalogs, and external systems.

**Attributes.** An attribute carries information a use case must remember and always has a type (`username : String`,
`release_date : Date`); visibility markers (`+`, `-`) add nothing to a domain model. A concept with identity of its own
is a class, not an attribute: `MusicRecording` references a `MusicGroup` class rather than holding a `group : String`
field.

**Associations.** An association is a relationship the domain must remember. Name it with a domain verb read from the
owning side, and state its multiplicity:

```text
User           1 ---- 1      Credential     : Authenticates with
User           1 ---- 0..*   File           : Uploads
MusicAlbum     1 ---- 1..*   MusicMedium    : Packages
MusicMedium    1 ---- 0..*   MusicRecording : Sequences
MusicRecording 0..* ---- 0..* MusicGroup    : Made by
```

One album packages one or more mediums; each medium belongs to one album. A medium sequences zero or more recordings,
and a recording is made by zero or more music groups. Avoid verbs that carry no domain meaning — `Has`, `Uses`,
`Contains`, `Owns`.

**Attributes versus associations.** A relationship between two concepts is an association, not a field: model `User` —
`Uploads` — `File`, never a `files : File` attribute on `User`. Associations document domain knowledge and read better
than a field would.

**Derived attributes.** Prefix an attribute the model computes from other information with a slash
(`/occupiedCountries : Integer`). A derived attribute follows from an association and is not stored.

**Common association patterns.**

| Pattern               | Example                          |
| --------------------- | -------------------------------- |
| Transaction ↔ Item    | `Order` ↔ `OrderLine`            |
| Transaction ↔ Actor   | `Customer` ↔ `Payment`           |
| Description ↔ Object  | `ProductDescription` ↔ `Product` |
| Part ↔ Whole          | `GalleryItem` ↔ `Gallery`        |
| Container ↔ Content   | `MusicMedium` ↔ `MusicRecording` |
| Organization ↔ Member | `MusicGroup` ↔ `Person`          |
| Neighboring objects   | `City` ↔ `City`                  |

**What not to model.** No software methods (`calculatePrice()`, `save()`, `getName()`), no database tables, and no
programming-language or infrastructure detail (`Vec<T>`, foreign keys, column types). Model only the relationships the
domain must remember, not every possible one: the diagram describes business reality, not the software design.

The examples above use the project's concepts for familiarity but are conceptual illustrations — they do not restate
production code and are not bound by [`DOC-001`](#1-code-examples).

**Reference** — Craig Larman, _Applying UML and Patterns_, conceptual modeling, the source of the conventions above.

---

### 3. Citations

A citation is a promise to the reader: the named path can be opened at the revision being read, and it supports what the
text says. Cite a path the repository tracks — a source file, `docs/development/UseCases.md`, a migration — and prefer
the source that owns the statement, as [`DOC-001`](#1-code-examples) does for an example's origin.

`.artifacts/` is machine-local scratch: git-ignored, written per change by the execution and review agents and by CI,
and absent from a fresh clone. A citation into it leaves the reader nothing to open, so documentation and production
source never name it. The tooling that produces or consumes the directory — `script/`, `devenv.nix`, `.github/`, the
ignore files and the agent harness under `.opencode/` — names it as a destination, not as a source, and is out of scope.

When a citation's target has been removed, replace it with the artifact the code now implements — a use case in
`docs/development/UseCases.md`, a rule in this document — or state the obligation in the comment itself.

---

### 4. Published routes

The specification is assembled from the handler annotations when the server starts
([`STY-RUST-091`](HttpApi.md#openapi-generation)), so it is the machine-checked contract of the HTTP surface. A route
the documentation publishes — the response-code examples and the `_links.self` examples in
[HttpApi.md § Error public payload](HttpApi.md#error-public-payload) — is written as `METHOD /api/v1/...` and names a
path the specification declares, with the `/api/v1` base removed: `GET /api/v1/user/{id}` corresponds to the spec path
`/user/{id}`, and `/health` is declared as itself.

No check asserts this correspondence yet ([`DOC-004`](#4-published-routes)); the rule is validated by review. A route
example is updated in the same change that moves the endpoint.

---

### 5. Business use case format

A business use case is one entry in [UseCases.md](UseCases.md), filed under the package section that owns the capability
it describes. The entry states what an actor achieves and what the business guarantees afterwards; the wire, the
datastore and the code stay out of it (`DOC-007`).

The entry is abstract with respect to the interface that carries it. A use case never assumes an entry point: one use
case may be served by several entry points, and one entry point may serve several use cases. The implementation
reconciles the two and carries the detail the entry leaves out.

#### 5.1 The entry

An entry is one `UC-<seq> - <Name>` heading carrying the field headings below, in this order, each written one heading
level below the entry: a `###` entry — the use cases a bounded context owns directly — carries `####` fields, and a
`####` entry, inside a sub-package, carries `#####` fields. Only `Alternative flow` may be omitted; every other field is
required.

| Field               | Content                                                                                                                                                                                                                         |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Description`       | One or two sentences stating the business intent of the use case.                                                                                                                                                               |
| `Primary actor`     | The actor who initiates the use case, named with a [Glossary](Glossary.md) actor term (`Standard User` or `Admin`); a use case only the Root Admin may perform says so in `Pre condition(s)`.                                   |
| `Parameters`        | One bullet per input the triggering event carries, in business vocabulary (§ 5.4), or `None — the event carries no data.` when it carries none.                                                                                 |
| `Pre condition(s)`  | One condition per bullet; every condition holds before the use case can run.                                                                                                                                                    |
| `Trigger`           | One bullet per triggering event, each a single external event (§ 5.3).                                                                                                                                                          |
| `Happy path`        | Numbered steps of the main success scenario.                                                                                                                                                                                    |
| `Alternative flow`  | Numbered branches, each starting from a happy-path step (e.g. `3a.`); every flow is a `Critical exception path` (see [Glossary](Glossary.md)), covered following [§ 4 (Testing)](#testing); list a flow only when it is needed. |
| `Post condition(s)` | One condition per bullet, guaranteed after the use case succeeds.                                                                                                                                                               |

The name states the business capability, not an application use-case file name. The name states one capability and joins
no alternatives with “and” or “or”: a title that needs the conjunction is either two use cases — `Add an Image…` and
`Add a Video…` — or one name for the shared capability, such as `Add a Gallery Item`. `check_doc_005` rejects a title
that carries either word. Identifiers are zero-padded and gap-free: the catalog holds each of `UC-001` … `UC-0NN`
exactly once, in any order across its sections, so grouping the catalog by package never renumbers an entry. A new use
case takes the next free identifier and is appended inside the section that owns it; a withdrawn capability is removed
by a change that also renumbers the entries after it.

An entry never carries the `Application use case(s)`, `Business rules`, `Secondary actor (optional)` or
`Bounded context(s)` fields. The catalog describes capabilities, not the code that implements them, so a business use
case is implemented by one or more application use cases, governed by [§ 9 (Use cases)](#9-use-cases); the specification
owns the validation criteria (§ 5.6); and the section an entry sits in is its package.

#### 5.2 Actors

The actor who initiates the use case is the only actor the entry declares. A participant that does not initiate — an
external system, a service the use case calls — is named in the flow where it matters; the entry has no field for it.

The entry lists the roles the use case is for, and does not split one role in two when the split changes nothing.
`Admin` covers every administrator, the Root Admin included: a use case only the Root Admin may perform says so in
`Pre condition(s)`, never by naming a separate actor. An override the Root Admin holds over a resource it does not own
is stated in the flow that grants it, never in the actor list.

`System` is never an actor. The system's work is the use case's body (§ 5.5), and a trigger only the system can produce
would break the external-origin rule (§ 5.3). A use case started by the service coming up names the person who starts
the service: “the Root Admin starts the service”, not “the system starts”.

#### 5.3 Triggering events

The trigger is the event that starts the use case. It is the field authors get wrong most often, because an endpoint
list, a goal and a precondition all read like a trigger and none of them is one. Four properties decide it:

- **External to the boundary.** It originates with an actor, an external system or a timer, never with the system under
  design.
- **A state change, not a state.** It is a discrete event that alters the conditions enough to start the normal course;
  “the user has not signed in” is a state and belongs in `Pre condition(s)`.
- **Separated from the preconditions.** What must already be true goes in `Pre condition(s)`; what starts the work goes
  here, and never both.
- **Atomic.** It says only what starts the process — never what happens next, never its outcome, never a step of the
  flow.

| Rejected                                                            | Why                            | Accepted                                                       |
| ------------------------------------------------------------------- | ------------------------------ | -------------------------------------------------------------- |
| “The customer wants to buy a book.”                                 | a desire, not an event         | “The customer submits the checkout request.”                   |
| “When it is time to create the invoices.”                           | an unmeasurable threshold      | “An order is shipped.” or “The billing run reaches 00:00 UTC.” |
| “The customer enters the card, clicks buy and waits for the email.” | the flow folded into the event | “The customer submits the payment details.”                    |
| “The user is logged in.”                                            | a precondition                 | — it belongs in `Pre condition(s)`                             |

A trigger may be temporal or system-generated, and then it states the exact threshold or time mechanism: “the Root Admin
starts the service”, “the export job receives the daily archive”. An entry carries one trigger; it carries more only
when every bullet starts the same goal through a different entry point, and each bullet is still one event. A bullet
that starts a different goal is a different use case.

The trigger names the event, never the entry point that carries it: “the caller asks to browse the galleries it may
see”, with no method and no path. Which entry point delivers an event is the implementation's concern (§ 5), because one
use case may be served by several entry points and one entry point may serve several use cases. No field of an entry
names a route, `Trigger` included; a trigger may describe its event in business terms — “the system receives a shipment
notice” — but it never names the payload's fields, and `DOC-007` rejects a method-and-path pair or a bare route path
wherever it appears.

#### 5.4 Parameters

`Parameters` lists what the triggering event carries: one bullet per input, in business vocabulary, with its optionality
stated.

- `username — the account name of the new user; required.`
- `email — an optional contact address.`

Never a field name, a type, a media type or a schema fragment. The happy path does not repeat the parameters: its first
step is “the Admin submits a user-creation request”, not “carrying a username, an optional email, a password and the
role to grant”. A use case whose event carries no data says `None — the event carries no data.`

#### 5.5 Happy path, alternative flow and post conditions

The happy path is the main success scenario and only that: numbered business steps, one action per step, no branch and
no technical mechanism. The alternative flow branches from a happy-path step — `3a.`, `3b.` — and every branch ends in
an outcome; a rejected request is written “the system rejects the request and applies nothing”, never as a status code.
The post conditions state what the business guarantees once the use case succeeds, never what the wire returns.

#### 5.6 Where the validation criteria live

The entry states intent and outcome; its acceptance criteria are not part of it. A rule about what the input must
satisfy, what may never be deleted or when a name is unique belongs to the specification of the capability — the
OpenSpec requirement that owns it. Do not add a `Business rules` field, and do not restate a specification rule in a
flow: when a rule changes the outcome, change the flow and the specification together.

#### 5.7 Business vocabulary (`DOC-007`)

A use case stays in business vocabulary. Every field, `Trigger` included, states intent and outcome in the language of
the domain, never in the language of the wire or the implementation.

The vocabulary a narrative field excludes (`DOC-007`):

| Excluded                                                                     | Lives in                                                           |
| ---------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| HTTP method, route, status code, header                                      | `api` specification (`openspec/specs/api/spec.md`) (`DOC-004`)     |
| Representation media type (`application/hal+json`), mechanics, links         | `api` specification (`openspec/specs/api/spec.md`)                 |
| Request or response field, query parameter, payload, request body, JSON      | `api` specification (`openspec/specs/api/spec.md`)                 |
| Datastore, table, row, column, schema, cascade, transaction, migration, byte | [§ 3 (Persistence)](#persistence)                                  |
| Server, client, endpoint, handler, adapter, repository, cache, thread, race  | [§ 2 (Architecture)](#architecture), [§ 1](#code-style-guidelines) |

The [Glossary](Glossary.md) defines the vocabulary a narrative field may use: `Triggering event`, `Parameters`,
`Caller`, `Owner`, `Visibility`, `Position`, `Last-modified time`, `System` and the actors, alongside the domain
concepts of the [Overview](Overview.md) — “playlist”, “recording”, “track”, “medium”, “gallery”, “item”. Write those; do
not write `last_modified_at`, `track_index`, `recording_id`, `is_public`, `add_track`, `limit`, `offset`, `total`,
`0..n-1` or `MAX(track_index) + 1`. The business outcome replaces the status code:

| Technical form                | Business form                                       |
| ----------------------------- | --------------------------------------------------- |
| `404 Not Found`               | reported as not found, without revealing which case |
| `422 Unprocessable Content`   | rejected, and nothing is applied                    |
| `last_modified_at` advances   | the last-modified time advances                     |
| `MAX(track_index) + 1`        | appended after the last track                       |
| `schema cascade`              | removed together with its track placements          |
| `add_track: { recording_id }` | a request to add a recording                        |

An entry names no route in any field, `Trigger` included; routes live in the `api` specification
(`openspec/specs/api/spec.md`) and the specification the server serves (`DOC-004`). `script/rule_enforcer.sh` enforces
the narrative vocabulary over every field of every entry (`check_doc_007`): it reports an inline code span, a denylisted
technical term, a method-and-path pair or a bare route path.

#### 5.8 Definition of done

A use case is done when all of the following hold:

- the entry sits in the section of the bounded context that owns the capability;
- the identifier is the next free one and appears exactly once in the catalog;
- the trigger is one external, state-changing, atomic event;
- the parameters list the event's data in business terms;
- no removed field appears, and no field — `Trigger` included — names the wire, the storage or the implementation;
- the use case appears in its section's diagram (§ 6);
- `script/rule_enforcer.sh check` passes.

#### 5.9 Worked example

The example below restates the entry for UC-013; when that entry changes, change the example with it.

```markdown
### UC-013 - Create a Music Playlist

#### Description

Allow an authenticated user to create a playlist they own to collect and order music recordings.

#### Primary actor

- Standard User
- Admin

#### Parameters

- name — the playlist name; required.
- visibility — whether other users may read the playlist; optional, private by default.

#### Pre condition(s)

- The caller is authenticated.

#### Trigger

- The caller submits a playlist-creation request.

#### Happy path

1. The caller submits a playlist-creation request.
2. The system verifies the caller's account still exists.
3. The system trims and validates the name.
4. The system verifies the caller does not already own a playlist with the same name.
5. The system creates the playlist owned by the caller, private unless requested otherwise.
6. The system returns the created playlist.

#### Alternative flow

- 3a. The name cannot be used: it is empty, only whitespace, or too long; the system rejects the request and creates
  nothing.
- 4a. The caller already owns a playlist with the same name; the system rejects the request and creates nothing.

#### Post condition(s)

- A playlist owned by the caller exists and holds no track.
```

---

### 6. Use case diagrams

[UseCases.md](UseCases.md) publishes the package view of the catalog: one PlantUML use-case diagram per package that
holds use cases, plus a master diagram in its preamble. A reader sees which capabilities each context exposes before
reading any entry.

- **Packages.** A package is a bounded context listed in the [Overview](Overview.md#bounded-context), or a sub-package
  inside one. The master diagram declares the registry — each context, with its sub-packages nested — and no use case. A
  package that holds use cases carries the diagram of its own package; a package that only groups sub-packages carries
  none, because the master already shows the grouping.
- **Placement.** The master diagram sits in the preamble. A `##` section carries a context's diagram and the entries it
  owns directly; a context that divides into areas nests `###` sub-package sections (Library's `Gallery` and `Music`),
  each with its own diagram and its own entries. An entry sits one heading level below its package: `###` in a context
  section, `####` in a sub-package, with its fields one level below it.
- **Membership.** A use case appears in exactly one diagram: the one of the section that holds its entry. A capability
  spanning several contexts is filed where it is owned, and [§ 7.3](#73-cross-context-use-cases) governs how its writes
  stay atomic.
- **Actors.** Only the [Glossary](Glossary.md) actors — `Standard User` and `Admin` — declared outside the package.
  `Admin` covers every administrator, the Root Admin included; a use case only the Root Admin may perform states it in
  its pre-conditions, never as a distinct actor (§ 5.2).
- **Nodes.** `usecase "UC-013 - Create a music playlist" as UC013`: the identifier and the business name, joined so
  `check_doc_008` can verify the correspondence.
- **No invented structure.** No `<<include>>`, no `<<extend>>`, no package the code does not have. The diagram mirrors
  the catalog and the bounded contexts, and nothing else.
- **Rendering.** The MkDocs PlantUML plugin renders the `puml` fences in the published site; `check_doc_008` validates
  structure, never layout.

---

<!-- GitHub slugs this heading to `logging--observability`, mkdocs to `logging-observability`. This explicit anchor
keeps the GitHub-style links to this section resolvable in the published mkdocs site. -->

<a id="logging--observability"></a>

## Logging & Observability

This section holds the `OBS-*` rules the rest of the rulebook cites. The logging contract itself is normative in the
`logging` capability (`openspec/specs/logging/spec.md`): its requirements state the two sinks and their rotation, the
verbosity control and its non-suppressible floor, the emitting layers, the once-only rule, log content and redaction,
the five severities, and the closed security-event catalog. Each rule below states the obligation the build enforces and
names the requirement it cites instead of restating it. A logging obligation lives only in that capability — a rule in
another section cites an `OBS-*` rule, it never restates one. The section also owns the trust-boundary obligation for an
adapter that hands untrusted content to an external native parser, since such a parser is a security event source
(`OBS-008`).

| ID        | Section         | Rule                                                                                                                                                                                                                                                                                                                                 | More info                                     | Enforced by     |
| --------- | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------- | --------------- |
| `OBS-001` | Layers          | Emit log events only from the application and infrastructure layers; the domain returns errors as values and emits none, using no logging facade (`tracing`, `log`), standard-output macro (`println!`, `eprintln!`, `dbg!`) or instrumentation attribute, and the composition root is infrastructure — `logging` `REQ-LOGGING-005`. | [§ 1](#1-logging-layers)                      | `check_obs_001` |
| `OBS-002` | Ownership       | Record each error or security event exactly once, at the layer that owns the decision that produced it or that still holds its detail; every layer above maps it to a coarser form without recording it again — `logging` `REQ-LOGGING-006`.                                                                                         | [§ 2](#2-log-once-at-the-owning-layer)        |                 |
| `OBS-003` | Content         | Carry a stable classification — an event code or a `kind()` — and structured identifiers, never the rendered message of a framework, extractor, deserializer, database or filesystem error, and never a raw client-supplied query string, file name or request-body value — `logging` `REQ-LOGGING-007`.                             | [§ 3](#3-log-content)                         | `check_obs_003` |
| `OBS-004` | Redaction       | Never record application source, session identifiers, access tokens, passwords or password material, database connection strings, the pepper, the signing secret or personal data; a type holding one must not expose it through `Debug` — `logging` `REQ-LOGGING-008`.                                                              | [§ 4](#4-redaction)                           | `check_obs_004` |
| `OBS-005` | Severity        | Use only `error`, `warn`, `info`, `debug` and `trace` with the meanings the capability declares; an expected business rejection is never `error` — `logging` `REQ-LOGGING-009`.                                                                                                                                                      | [§ 5](#5-severity)                            |                 |
| `OBS-006` | Catalog         | Emit only events the security-event catalog in [§ 6](#6-security-event-catalog) declares, at the declared severity and with exactly the declared fields, and emit every declared event — `logging` `REQ-LOGGING-010`.                                                                                                                | [§ 6](#6-security-event-catalog)              | `check_obs_006` |
| `OBS-007` | Suppression     | Emit every catalogued event on the reserved `security` target under a floor no configured verbosity can undercut; `Logging::try_new` rejects `off`/`none` and any directive that targets `security` — `logging` `REQ-LOGGING-004`.                                                                                                   | [§ 7](#7-non-suppressible-security-events)    | `check_obs_007` |
| `OBS-008` | Untrusted input | An adapter that hands caller-supplied content to an external native parser restricts the protocols it may open to an explicit allow-list, bounds the child's wall clock and output, and records the residual isolation gap rather than implying the parser is sandboxed.                                                             | [§ 8](#8-external-parsers-on-untrusted-input) | `check_obs_008` |

---

### 1. Logging layers

The **application** (`src/lib/application/**`) and **infrastructure** (`src/lib/infrastructure/**`) layers emit log
events. The **domain** (`src/lib/domain/**`) does not: it returns errors as values and must not depend on a logging
facade, call a standard-output macro or carry an instrumentation attribute. The composition root (`src/lib/app.rs`) is
part of the infrastructure layer and logs lifecycle events.

`tracing` is the single logging facade; it is a direct dependency of the application layer under the exemption in
[§ 6.6](#66-third-party-dependencies-sit-behind-a-port).

---

### 2. Log-once at the owning layer

Each error or security event is logged exactly once, by the layer that owns the decision that produced it — or, for an
error received from below, the last layer that still holds its detail. Every layer above maps the error to a coarser,
more abstract form and never logs it again.

| Error or event                                                                                      | Owning layer          | Level                                                                                                       | Logged                             | Upper layers                                                   |
| --------------------------------------------------------------------------------------------------- | --------------------- | ----------------------------------------------------------------------------------------------------------- | ---------------------------------- | -------------------------------------------------------------- |
| Port failure (repository, file storage, external service; unit-of-work `begin`/`commit`/`rollback`) | outbound adapter      | `error` (`port_fault`) for a dependency fault, or `warn` (`constraint_rejected`) for an expected constraint | the failure's `kind` and operation | application maps it and logs nothing; the handler logs nothing |
| `Unknown` a use case originates (no port produced it)                                               | application use case  | `error`                                                                                                     | its `kind`, never a source chain   | the handler logs nothing                                       |
| Security decision (authentication failure, authorization denial, out-of-order action)               | application use case  | `warn`                                                                                                      | the actor and a reason code        | the handler maps it to `401`/`403` and logs nothing            |
| Token-boundary rejection                                                                            | inbound middleware    | `warn`, or `error` for a provider fault                                                                     | the rejection's kind               | —                                                              |
| Domain rule violation                                                                               | — (returned as value) | —                                                                                                           | nothing                            | the use case that invoked it logs it once                      |

The owning layer is the only one that can log something useful: the raw cause of a port failure exists only inside the
adapter, and the actor and the reason of an authentication decision exist only inside the use case. By the time the
error reaches the handler it is an opaque `Unknown`, so logging it there would emit a content-free duplicate. Logging an
error in more than one layer, or logging a chain already classified below, violates this rule.

An **expected constraint** is a constraint outcome a use case handles as a business result — `AlreadyExist` or
`Conflict`. The adapter logs it at `warn` as `constraint_rejected` (`OBS-005`, `OBS-006`); any other port failure is a
dependency fault logged at `error` as `port_fault`.

---

### 3. Log content

An event logs a stable classification — an event code, or a `kind()` that names a variant — and structured identifiers.
It never logs the `Display` or `Debug` of an extractor, deserializer, database, filesystem or framework error, because
those strings embed the offending input value and internal detail. It never logs a raw client-supplied value either: not
a query string, not a file name, not a request-body field. Where a client value must be correlated across events, log a
generated request identifier instead.

For example, the trace middleware logs the request method and path; it never logs the full `Uri`, whose query string is
client-controlled and may carry personal data or a token. A file-storage failure logs its classification; it never logs
the user-supplied file name.

---

### 4. Redaction

Never log:

- application source code;
- session identifiers, access tokens or JWT tokens;
- passwords or any password material;
- database connection strings;
- the pepper or the JWT signing secret;
- personal data.

A type that holds a secret or personal data must not derive a `Debug` that exposes it. `Security`, `Jwt`,
`Configuration` and `Credential` (`src/lib/domain/model/`) carry a pepper, a signing secret, a configuration dump and a
password hash; `User` (`src/lib/domain/model/`) carries an email address and a username. Each must implement `Debug` by
redaction, or hold its value in a type whose `Debug` prints a placeholder. The clause binds a secret-bearing type
wherever it lives, `domain/model/` or not — `IssuedToken` (`src/lib/domain/port/`) holds an access token and redacts it
the same way.

---

### 5. Severity

| Level   | Use                                                                   | Examples                                                                                                                                                                                                |
| ------- | --------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `error` | An operator-actionable fault the caller cannot fix.                   | datastore or filesystem unavailable, a commit or rollback failure, a token-provider operational failure, an inbound extractor misconfiguration, an unhandled `500`.                                     |
| `warn`  | A reviewable security event; not necessarily a fault.                 | input or output validation failure, discrete-list rejection, authentication or authorization failure, an invalid or expired token, a deserialization failure, a limit exceeded, an out-of-order action. |
| `info`  | A security success or a lifecycle event.                              | startup, shutdown, logging initialization, authentication success, a user-administration or credential change, an accepted upload, a generated secret.                                                  |
| `debug` | Low-volume diagnostics, including the lowest-severity catalog events. | request tracing, internal state, a personal-data read (`sensitive_data_access`).                                                                                                                        |
| `trace` | Driver or framework detail.                                           | —                                                                                                                                                                                                       |

`error` is reserved for faults an operator must act on; an expected business-rule violation is not an `error`.

---

### 6. Security-event catalog

The security-event catalog is normative in the `logging` capability. `REQ-LOGGING-010` states the closed set, the
reserved `security` target and the declared severity and fields; the table it introduces, `The security-event catalog:`,
declares each event's severity, fields and recorded-when condition, including `upload` and `upload_reap` from the
`upload` capability. `OBS-006` cites that requirement, and `check_obs_006` parses the table from
`openspec/specs/logging/spec.md` and verifies every emission site against it, so a `security`-target event the catalog
does not declare fails the build.

---

### 7. Non-suppressible security events

Catalog events emit on the reserved `security` target at whatever level the catalog declares for them (`error`, `warn`,
`info` or `debug`; `OBS-006`). `logging::setup` (`src/lib/infrastructure/logging.rs`) builds the filter so
`logging.level` can never suppress a catalog event: `Logging::try_new` rejects `off`/`none` as a whole-level value and
rejects any directive that targets `security`, and `logging::setup` appends the security floor (`security=debug`).
`logging.level` therefore tunes operational verbosity only; it is startup-only (`STY-RUST-073`; see
[§ 10.1](#101-startup-only-settings)).

---

### 8. External parsers on untrusted input

An outbound adapter that hands caller-supplied bytes to an external native parser — a memory-unsafe C parser in
particular — extends the trust boundary to that process. Before the content is handed over, the adapter restricts the
protocols the parser may open to an explicit allow-list, so a crafted container cannot request a nested fetch, bounds
the child's wall clock and its standard output, and records the residual isolation gap rather than implying the parser
is sandboxed. User isolation, namespaces and a syscall filter are the required end state for a memory-unsafe parser and
are tracked separately.

The media probe is the current instance: it runs `ffprobe` on the staged file with an explicit
`-protocol_whitelist file`, a wall-clock timeout, a stdout cap and a concurrency permit; a fuller sandbox is tracked by
[#242](https://github.com/Mnemorium/mnemorium/issues/242).

---

**Reference** — [OWASP Logging Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html),
filtered to the events this server can emit.

---

## Security

This section defines the **secret-bearing struct** the logging rules cite. The access control that protects a secret at
the HTTP boundary is stated by the capability requirements — `REQ-API-025` closes the public surface, and
`REQ-AUTHENTICATION-020`/`-021` guard the setup exchange — rather than by a prose rule here. Keeping secrets out of log
sinks is governed by [§ 7 (Logging & Observability)](#logging--observability); this section cites an `OBS-*` rule for it
and never restates one.

A **secret-bearing struct** is a type that declares a field holding a password or password hash, a signing or encryption
key, or an access token. The canonical set is the one `OBS-004` names — `InstanceState` and `Credential`, both in
`src/lib/domain/model/` — and every new type that adds such a field joins it.

Seed data is test data, not a secret: the fixture trees live under `test/` and never ship in the image, their fixed demo
password is printed only on an explicitly seeded run, and a seeded boot's pepper and signing secret come from the
instance state exactly as an unseeded boot's do.
