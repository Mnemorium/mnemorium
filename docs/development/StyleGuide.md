# Style Guide

## Rust

### General

- Extracting functionality from a function into its own function should only be done when that functionality is used in
  at least 4 different places. This applies to production code; shared test fixtures are exempt.

### Composition root and application state

`src/bin/server.rs` is the composition root: it is the only place that instantiates concrete adapters, runs the startup
use cases (`LoadConfiguration`, `InitializeRootAdmin`) and builds the `AppState` the HTTP layer shares.

`AppState` lives in `src/lib/infrastructure/inbound/rest/app_state.rs` and holds:

- the live configuration (`Arc<ArcSwap<Configuration>>`),
- the per-context use-case factories (see [Use-case factory](#use-case-factory)),
- the token provider the auth middleware validates with.

`AppState` is the single router state; `FromRef` impls expose exactly what middleware extracts from it. Getters return
`Arc` clones, never borrows of the shared state. Nothing outside the composition root constructs a concrete adapter.

### REST handler layout

Handlers live in `src/lib/infrastructure/inbound/rest/handler`.

- Split the directory into subdirectories, one folder per bounded context.
- Each file contains exactly one endpoint.
- A handler extracts `State<AppState>` and resolves its use case through the bounded context's factory
  (`state.<context>_use_case_factory().<use_case>()`) before calling `execute`; it never receives a pre-built use case.

#### File naming

- File name is `<method>_<context>.rs`, e.g. `post_note.rs`, `get_note.rs`.
- The handler function is named like the file name, e.g. `post_note`, `get_note`.

#### Item order in a file

Declare items in this order:

1. Request body object (only for methods that carry a body: `POST`, `PUT`, `PATCH`)
2. Query object
3. Response body object
4. Mapping from use-case error to API error
5. The endpoint handler declaration

#### Payload structs

- Query parameters: struct named `<Context>Query`, deriving `Serialize`, `Deserialize`, `IntoParams`.
- Request body: struct named `<Context>Request`, deriving `Serialize`, `Deserialize`, `ToSchema`.
- Response body: struct named `<Context>Response`, deriving `Serialize`, `Deserialize`, `ToSchema`.
- Even when a payload has a single attribute, always prefer a struct over a raw return/parameter/query value.

For the `#[utoipa::path(...)]` declaration contract, see [OpenAPI documentation](api/Overview.md).

### Error handling

Errors follow a layered model: one error type per role, each translating to the next as it crosses an architectural
boundary.

- **`ApiError`** — declared in `src/lib/infrastructure/inbound/rest/api_error.rs`. Its variants map one to one to HTTP
  status codes (e.g. `Conflict`, `BadRequest`, `InternalServerError`). It is **not** derived with `thiserror`; it is an
  HTTP transport concern, not a domain error.
- **Domain error** — for failure when initialising or updating a domain model. Declared **before the model struct, in
  the same file** as the model, e.g. in `src/lib/domain/model/user.rs`.
- **Use Case error** — one enum per use case, declared in `src/lib/application/port`. It must have:
  - an `Unknown(_)` variant carrying the underlying error, and
  - an invalid-parameter variant (e.g. `InvalidEmail`) describing invalid input.
- **`thiserror`** is used for the **Use Case**, **Domain**, and **Port** error enums. It is **not** used for `ApiError`.
- **Port errors** (Repository, External Service) are declared in `src/lib/domain/port/error.rs`. They do **not** map
  directly to a use-case error; the use case translates them.
- **`NotFound` is not an error.** A missing entity is a valid outcome and is returned as `Option`/`None` (or a
  corresponding non-error type), never as an error variant.

#### Domain error

Declare the error enum before the model struct, in the same file.

```rust
#[derive(Debug, thiserror::Error)]
pub enum CreateUserError {
    #[error("email has an invalid format")]
    InvalidEmail,
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

pub struct User {
    // ...
}
```

#### Getter and setter

Accessors are named after the field they expose.

- **Constructor**: `new` when infallible, `try_new` when it can fail; it returns `Result<Self, _>` and performs
  validation.
- **Getter**: `<field>(&self) -> <field type>`. Return a borrowed reference (`&str`, `Option<&str>`) or a `Copy` value
  type — never an owned clone.
- **Setter**: `set_<field>(&mut self, <value>)`. Return `Result<(), _>` when the field is validated, `()` otherwise.

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

#### Use Case error

Declared in `src/lib/application/port`, it always exposes an `Unknown(_)` variant and one or more invalid-parameter
variants.

```rust
#[derive(Debug, thiserror::Error)]
pub enum CreateUserError {
    #[error("a user with this email already exists")]
    UserAlreadyExists,
    #[error("email has an invalid format")]
    InvalidEmail,
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}
```

#### Mapping use-case error to API error

Each rest handler file declares the mapping from its use-case error to the `ApiError`:

```rust
impl From<CreateUserError> for ApiError {
    fn from(err: CreateUserError) -> Self {
        match err {
            CreateUserError::UserAlreadyExists => ApiError::Conflict,
            CreateUserError::InvalidEmail => ApiError::BadRequest,
            CreateUserError::Unknown(_) => ApiError::InternalServerError,
        }
    }
}
```

#### ApiError to axum response

`ApiError` implements `IntoResponse`, converting to the corresponding HTTP status code and the standard error body.

#### Error public payload

Every error response carries the same body:

```json
{
  "error": "An error message"
}
```

#### Port error

Port errors are translated into use-case errors by the use case, never consumed directly by the HTTP adapter — the one
exception being inbound middleware (see below).

Besides the two shared families below (Repository, External Service), a port may declare its **own** error family in
`src/lib/domain/port/error.rs`. Every port error exposes at least an `Unknown(anyhow::Error)` variant.

##### Repository

| Error                  | Description                                                                                                               |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| AlreadyExist           | The entity already exists and cannot be created again. Typically caused by duplicate business keys or unique constraints. |
| Conflict               | The operation cannot be completed because the current state of the data conflicts with the requested action.              |
| ConcurrencyConflict    | The operation failed due to a concurrent modification of the same entity (for example, optimistic locking failure).       |
| DataIntegrityViolation | The operation would violate a data integrity rule or constraint.                                                          |
| ValidationFailed       | The provided data does not satisfy validation rules required by the repository.                                           |
| OperationFailed        | The repository could not complete the requested operation for a non-specific reason.                                      |
| Timeout                | The operation exceeded the allowed execution time.                                                                        |
| Unavailable            | The repository or underlying datastore is currently unavailable.                                                          |
| Unknown                | An unexpected or unmapped error occurred.                                                                                 |

##### External Service

| Error                  | Description                                                                                  |
| ---------------------- | -------------------------------------------------------------------------------------------- |
| Unauthorized           | Authentication is required or the provided credentials are invalid.                          |
| Forbidden              | The caller is authenticated but does not have permission to perform the requested operation. |
| RateLimited            | The external service rejected the request because a usage limit was exceeded.                |
| Timeout                | The external service did not respond within the expected time.                               |
| Unavailable            | The external service is temporarily unavailable or unreachable.                              |
| CommunicationFailure   | A network, protocol, or transport-level error occurred while communicating with the service. |
| SerializationFailure   | The request could not be properly serialized before being sent to the service.               |
| DeserializationFailure | The service response could not be parsed or converted into the expected format.              |
| InvalidRequest         | The request was rejected because it contains invalid or missing information.                 |
| DependencyFailure      | The service failed due to a problem with one of its own dependencies.                        |
| RetryableFailure       | A transient error occurred and the operation may succeed if retried.                         |
| Unknown                | An unexpected or unmapped error occurred.                                                    |

##### Inbound middleware

Inbound middleware may consume a port directly when that port's decision is the middleware's own responsibility — for
example, `authenticate` calls `TokenProvider` to decide whether to admit a request. Two rules apply:

- Map only the variants that mean _the caller is unauthenticated_ (for example `InvalidClaims`, `InvalidToken`,
  `TokenExpired`) to a client error. Every other variant — including `OperationFailed` and `Unknown` — maps to `500` and
  is logged at `error`.
- Match the port error exhaustively. Port errors are `#[non_exhaustive]`, so end the match with a catch-all arm that
  defaults to `500`; never let a wildcard arm collapse a server-side failure into a misleading `401`.

##### Unit of Work

| Error           | Description                                                    |
| --------------- | -------------------------------------------------------------- |
| OperationFailed | The unit of work could not complete for a non-specific reason. |
| Unavailable     | The datastore is currently unavailable.                        |
| Unknown         | An unexpected or unmapped error occurred.                      |

##### Configuration Source

| Error                | Description                                                           |
| -------------------- | --------------------------------------------------------------------- |
| InvalidConfiguration | The layered settings do not form a valid configuration.               |
| OperationFailed      | The configuration source could not be read for a non-specific reason. |
| Unknown              | An unexpected or unmapped error occurred.                             |

### SQL data models

#### Enum for a CHECK constraint

For a column backed by a SQL `CHECK (... IN (...))` constraint, declare the Rust enum **before** the model struct, in
the same file.

- Name the enum after the attribute it represents, in `UpperCamelCase`, e.g. `Role` for the `role` column of table
  `user` used in model `User`.
- Derive `sqlx::Type` with `#[sqlx(rename_all = "UPPERCASE")]` to match the uppercase constraint strings required by the
  SQL section.
- Name variants in `UpperCamelCase`, one per allowed constraint value.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "UPPERCASE")]
pub enum Role {
    Admin,
    Standard,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    #[sqlx(primary_key)]
    pub user_id: NumericID,
    pub role: Role,
    // ...
}
```

#### Alias type for numeric IDs

Use the `NumericID` alias from `domain/alias.rs` for all numeric table columns that are identifiers (primary keys,
foreign keys) rather than a raw integer type.

```rust
use crate::domain::alias::NumericID;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub user_id: NumericID,
    pub credential_id: NumericID,
    // ...
}
```

### Test conventions

- Name tests `<UnitOfWork>_<Scenario>_<ExpectedResult>`, e.g. `apply_discount_code_valid_code_reduces_total_price`.
- Follow the Arrange–Act–Assert (AAA) pattern:

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

- Do not over-abstract test setup into deeply nested helper functions or distant global state.
- Keep tests deterministic and linear: no control flow (`if`, `match`, loops) inside a test function.
- Prefer data-driven tests with `rstest` when possible:

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

- Use `#[fixture]` functions when a test case input is a struct.

### Outbound port traits

Outbound ports are the interfaces infrastructure implements and the application consumes: repositories, the unit of
work, the configuration source, the password hasher, and so on.

#### Port traits are `Send + Sync`

Always declare outbound port traits as `Send + Sync`:

```rust
pub trait UserRepository: Send + Sync {
    // ...
}
```

- `Send`: the port and its futures may be moved between threads.
- `Sync`: the port may be shared behind an `Arc`; the unit-of-work factory is shared this way.

Repositories are short-lived views over a transaction (see [Repository](#repository)); the shared mutable state is the
unit of work, not the repository itself.

#### Dyn-safety

Only use-case traits are object-safe: their `execute` returns `Pin<Box<dyn Future<Output = ...> + Send + 'future>>`, so
a factory can return `Arc<dyn <UseCaseName>UseCase>`.

Outbound ports that return `impl Future` (RPITIT) are **not** dyn-safe. `AppState` and the use-case factories therefore
hold concrete adapters (`Arc<SqlxUnitOfWorkFactory>`, `Arc<JwtTokenProvider>`, ...) and `Arc<dyn <UseCaseName>UseCase>`
trait objects — never `Arc<dyn UnitOfWorkFactory>` or `Arc<dyn TokenProvider>`.

#### Async methods take `&mut self` and return `Send` futures

Repository methods mutate the shared transaction, so they take `&mut self`. Return an explicitly `Send` future instead
of relying on the default (which is not guaranteed to be `Send`):

```rust
use std::future::Future;

pub trait UserRepository: Send + Sync {
    fn create(&mut self, user: User) -> impl Future<Output = Result<User, RepositoryError>> + Send;
}
```

Web frameworks (axum) move futures across worker threads; without `Send`, `tokio::spawn(...)` and other runtime
operations fail to compile.

#### Port traits are `'static`; implementations may borrow

The trait bound is `'static` so ports can be injected into the application:

```rust
struct Application<F: UnitOfWorkFactory> {
    unit_of_work_factory: Arc<F>,
}
```

The _implementation_ need not be `'static`: a sqlx repository borrows the unit-of-work transaction
(`SqlxUserRepository<'transaction>`). Never store a repository beyond the use-case `execute` that obtained it.

#### Prefer `Arc<T>` over `Clone`

Do not add `Clone` just for frameworks — share long-lived adapters and the unit-of-work factory through `Arc<T>`.
Repositories obtained from a unit of work are borrowed views, not cloned.

### Unit of Work

The transaction boundary lives in the application service, not the presentation layer. A use case opens a unit of work,
runs business logic across the repositories it exposes, then commits on success or rolls back on failure. Handlers only
call the use case.

#### Lifecycle

The application service is responsible for opening the unit of work, running the business logic, and committing on
success or rolling back on failure:

```rust
let mut unit_of_work = factory.begin().await.map_err(...)?;

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
        if let Err(rollback_error) = unit_of_work.rollback().await {
            error!(error = ?rollback_error, "failed to roll back the unit of work");
        }
        Err(error)
    }
}
```

- `commit` and `rollback` consume the unit of work.
- A rollback failure is logged and the **original** business error is returned.
- A commit failure maps to the use-case `Unknown(_)`.
- Repository views borrow the unit of work mutably; drop them before `commit`/`rollback`.

The base trait and the factory are declared in `domain/port/unit_of_work.rs`:

```rust
pub trait UnitOfWork: Send {
    fn commit(self) -> impl Future<Output = Result<(), UnitOfWorkError>> + Send;
    fn rollback(self) -> impl Future<Output = Result<(), UnitOfWorkError>> + Send;
}

pub trait UnitOfWorkFactory: Send + Sync {
    type Uow: UnitOfWork;
    fn begin(&self) -> impl Future<Output = Result<Self::Uow, UnitOfWorkError>> + Send;
}
```

The factory is generic with an associated `Uow` type: no `dyn`, and the concrete unit of work is owned and `'static`
(one `Transaction<'static, Sqlite>` in the SQLite adapter).

#### Context views

Each bounded context (see [Bounded context](Overview.md#bounded-context)) has a view trait over the unit of work,
declared in its own `domain/port/<context>_unit_of_work.rs`:

```rust
pub trait UserUnitOfWork: UnitOfWork {
    fn users(&mut self) -> impl UserRepository + '_;
}
```

- Repository accessors return a short-lived view that borrows the unit of work.
- A context view exposes only its own context's repositories.
- Repositories are obtained **only** from a unit of work; concrete sqlx adapters never leave infrastructure.

#### Cross-context use cases

A use case that spans bounded contexts declares every context view it needs as an `execute` bound:

```rust
impl<F, P> RegisterUserUseCase for RegisterUser<F, P>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
    P: PasswordHasher,
{
    // ...
}
```

- Every repository comes from the **same** unit of work (one `begin()`); opening a second unit of work would be a second
  transaction and break atomicity.
- Cross-context writes stay atomic: Register User (Identity + User), Delete User Account (Identity + User + Library +
  Asset; see [UseCases.md](UseCases.md)).
- Each accessor borrows `&mut self`, so repositories are requested **sequentially**, never held two at a time.

#### Declaration order in `domain/port/unit_of_work.rs`

The file declares, in order:

1. The `UnitOfWork` trait
2. The `UnitOfWorkFactory` trait

Each context view lives in its own file, e.g. `user_unit_of_work.rs`.

Naming: `UnitOfWork`, `UnitOfWorkFactory`, `<Context>UnitOfWork`.

### Repository

A repository persists and queries one aggregate. It is obtained from a unit of work and shares that unit of work's
transaction.

#### Port

The port trait is named `<Aggregate>Repository` and declared in `domain/port/<aggregate>_repository.rs`. Methods take
`&mut self` and return `impl Future<...> + Send`:

```rust
#[cfg_attr(test, mockall::automock)]
pub trait UserRepository: Send + Sync {
    fn create(&mut self, user: User) -> impl Future<Output = Result<User, RepositoryError>> + Send;
    // ...
}
```

| Name                                   | Action                                      |
| -------------------------------------- | ------------------------------------------- |
| `create(domain: DomainSpecificObject)` | Insert (identity assigned by the datastore) |
| `save(domain: DomainSpecificObject)`   | Insert/Update (upsert by identity)          |
| `delete(id: DomainSpecificId)`         | Delete                                      |
| `search(filter: &SomeFilter)`          | Query                                       |

- `create` inserts a new aggregate and never binds identity columns: the datastore auto-increments them and `create`
  returns the aggregate with its final identifier.
- `save` upserts an existing aggregate targeted by its identifier.
- `delete` returns `Ok(false)` when no row matched; a missing entity is not an error.
- `search` returns an empty collection when nothing matches.
- Missing entities are never errors (see [Port error](#port-error)).

#### Adapter

The implementation is named `<Tech><Aggregate>Repository`, e.g. `SqlxUserRepository`, and lives in
`infrastructure/outbound/<tech>/`.

- It holds the transaction it operates on (`&'transaction mut Transaction<'static, Sqlite>` for SQLite) and is built by
  the unit-of-work accessor — never with a connection pool.
- It maps `sqlx::Error` to `RepositoryError` in `error_mapping.rs`; sqlx types never cross into the domain or
  application layers.

#### Declaration order in a port file

A repository port file declares, in order:

1. The filter struct, e.g. `UserFilter`
2. The repository trait

### UseCase Method

- A use case trait has one and only one method, named `execute`.
- Trait named `<UseCaseName>UseCase`, e.g. `CreateNoteUseCase`.
- Implementation named just `<UseCaseName>`, e.g. `CreateNote`.

#### Declaration order in `src/lib/application/port`

A use case trait file declares, in order:

1. The **Command** object
2. The **Response** object (only when non-empty)
3. The **Error** enum
4. The **UseCase** trait

### Use-case factory

Handlers depend on a factory rather than on pre-built use cases: each bounded context exposes one factory that builds
its use cases on demand.

- **Port** — `src/lib/application/port/<context>_use_case_factory.rs`. The trait is named `<Context>UseCaseFactory`, is
  `Send + Sync`, carries `#[cfg_attr(test, mockall::automock)]`, and exposes one method per use case — named after the
  use case — returning `Arc<dyn <UseCaseName>UseCase>`.
- **Adapter** — `src/lib/infrastructure/use_case_factory/<context>.rs`. The struct is named
  `Runtime<Context>UseCaseFactory` and holds what the context's use cases need: the live configuration
  (`Arc<ArcSwap<Configuration>>`) when they read it, and the unit-of-work factory (`Arc<SqlxUnitOfWorkFactory>`).
- **Lazy instantiation** — every accessor builds the use case on the spot and rebuilds its configuration-derived
  adapters (password hasher, token provider) from the live configuration; never cache those adapters.
- The factory only supplies the unit-of-work factory; the use case still opens, commits and rolls back its own unit of
  work (see [Unit of Work](#unit-of-work)).

#### Declaration order in a factory port file

A factory port file declares the trait only: no `Command`, `Response` or `Error`.

### Live configuration

- `LoadConfiguration` runs once at startup; the resulting `Configuration` is stored as `Arc<ArcSwap<Configuration>>` in
  `AppState`.
- Never cache a configuration-derived value (pepper, `JWT` secret, TTL) in a long-lived adapter built at startup; read
  it from the live configuration.
- **Bootstrap** — the datastore path is needed before the pool exists, but the configuration singleton row lives behind
  that pool. `bootstrap_sqlite3` (`src/lib/infrastructure/outbound/config/bootstrap.rs`) therefore reads the file and
  the environment only. That layering is intentionally duplicated with `ConfigConfigurationSource` (see the extraction
  rule in [General](#general)); keep the source list and order identical.
- Runtime write-guarding of the configuration is deferred; see the `TODO` in `src/bin/server.rs`.

## SQL

### General

- Use `snake_case` (all lowercase, words separated by underscores). Avoid mixed casing or quoted identifiers (e.g.
  `"UserId"`).
- Use clear, descriptive English words. Avoid obscure abbreviations (e.g. prefer `customer_number` over `cust_num`).
- Never use SQL reserved words (e.g. `order`, `group`, `date`, `select`) as object or column names without an
  identifying prefix or suffix (e.g. `purchase_order`, `created_at`). Exception: SQLite accepts a few ANSI SQL reserved
  words (e.g. `user`) as identifiers, so they are allowed.
- Use only standard ASCII alphanumeric characters (`a-z`, `0-9`) and underscores (`_`). No spaces, hyphens, or special
  symbols.
- Enum constraint strings must be in uppercase.

### Table names

- Singular (`user`, not `users`).
- Junction / mapping tables combine both entity names in order of primary hierarchy, e.g. `user_role`.

### Column names

- Primary keys: use `<table_name>_id`, e.g. `user_id`, for readability across joins.
- Foreign keys: use the exact primary key name of the referenced table (e.g. `customer_id` inside the `orders` table).
- Data type naming:
  - Boolean: prefix with `is_`, `has_`, or `can_`
  - Timestamps: suffix `_at`
  - Dates: suffix `_date`
  - Counts: suffix `_count`
  - Totals: prefix `_total`

### Keys, indexes and constraints

Name constraints `<constraint_type>_<table_name>_<column_name(s)>`:

| Prefix | Kind             |
| ------ | ---------------- |
| `pk_`  | Primary key      |
| `fk_`  | Foreign key      |
| `uq_`  | Unique key       |
| `idx_` | Non-unique index |
| `chk_` | Check constraint |

Important: all constraints must be declared at table level, i.e. at the end of the `CREATE TABLE` statement.

### Triggers and functions

- Triggers: `tg_`
- Functions: `fn_`
