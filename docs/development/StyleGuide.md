# Style Guide

## Rust

### General

- Extracting functionality from a function into its own function should only be
  done when that functionality is used in at least 4 different places.

### REST handler layout

Handlers live in `src/lib/infrastructure/inbound/rest/handler`.

- Split the directory into subdirectories, one folder per bounded context.
- Each file contains exactly one endpoint.

#### File naming

- File name is `<method>_<context>.rs`, e.g. `post_note.rs`, `get_note.rs`.
- The handler function is named like the file name, e.g. `post_note`,
  `get_note`.

#### Item order in a file

Declare items in this order:

1. Request body object (only for methods that carry a body: `POST`, `PUT`,
   `PATCH`)
2. Query object
3. Response body object
4. Mapping from use-case error to API error
5. The endpoint handler declaration

#### Payload structs

- Query parameters: struct named `<Context>Query`, deriving `Serialize`,
  `Deserialize`, `IntoParams`.
- Request body: struct named `<Context>Request`, deriving `Serialize`,
  `Deserialize`, `ToSchema`.
- Response body: struct named `<Context>Response`, deriving `Serialize`,
  `Deserialize`, `ToSchema`.
- Even when a payload has a single attribute, always prefer a struct over a raw
  return/parameter/query value.

For the `#[utoipa::path(...)]` declaration contract, see
[OpenAPI documentation](api/Overview.md).

### Error handling

Errors follow a layered model: one error type per role, each translating to the
next as it crosses an architectural boundary.

- **`ApiError`** — declared in
  `src/lib/infrastructure/inbound/rest/api_error.rs`. Its variants map one to
  one to HTTP status codes (e.g. `Conflict`, `BadRequest`,
  `InternalServerError`). It is **not** derived with `thiserror`; it is an HTTP
  transport concern, not a domain error.
- **Domain error** — for failure when initialising or updating a domain model.
  Declared **before the model struct, in the same file** as the model, e.g. in
  `src/lib/domain/model/user.rs`.
- **Use Case error** — one enum per use case, declared in
  `src/lib/application/port`. It must have:
  - an `Unknown(_)` variant carrying the underlying error, and
  - an invalid-parameter variant (e.g. `InvalidEmail`) describing invalid input.
- **`thiserror`** is used for the **Use Case**, **Domain**, and **Port** error
  enums. It is **not** used for `ApiError`.
- **Port errors** (Repository, External Service) are declared in
  `src/lib/domain/port/error.rs`. They do **not** map directly to a use-case
  error; the use case translates them.
- **`NotFound` is not an error.** A missing entity is a valid outcome and is
  returned as `Option`/`None` (or a corresponding non-error type), never as an
  error variant.

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

- **Constructor**: `new` when infallible, `try_new` when it can fail; it returns
  `Result<Self, _>` and performs validation.
- **Getter**: `<field>(&self) -> <field type>`. Return a borrowed reference
  (`&str`, `Option<&str>`) or a `Copy` value type — never an owned clone.
- **Setter**: `set_<field>(&mut self, <value>)`. Return `Result<(), _>` when the
  field is validated, `()` otherwise.

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

Note: reject-invalid-then-assign. A setter validates the new value, assigns only
on success, and reports the cause through the domain error enum when it fails.

#### Use Case error

Declared in `src/lib/application/port`, it always exposes an `Unknown(_)`
variant and one or more invalid-parameter variants.

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

Each rest handler file declares the mapping from its use-case error to the
`ApiError`:

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

`ApiError` implements `IntoResponse`, converting to the corresponding HTTP
status code and the standard error body.

#### Error public payload

Every error response carries the same body:

```json
{
  "error": "An error message"
}
```

#### Port error

Port errors are translated into use-case errors by the use case, never consumed
directly by the HTTP adapter.

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

### SQL data models

#### Enum for a CHECK constraint

For a column backed by a SQL `CHECK (... IN (...))` constraint, declare the Rust
enum **before** the model struct, in the same file.

- Name the enum after the attribute it represents, in `UpperCamelCase`, e.g.
  `Role` for the `role` column of table `user` used in model `User`.
- Derive `sqlx::Type` with `#[sqlx(rename_all = "UPPERCASE")]` to match the
  uppercase constraint strings required by the SQL section.
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

Use the `NumericID` alias from `domain/alias.rs` for all numeric table columns
that are identifiers (primary keys, foreign keys) rather than a raw integer
type.

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

- Name tests `<UnitOfWork>_<Scenario>_<ExpectedResult>`, e.g.
  `apply_discount_code_valid_code_reduces_total_price`.
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

- Do not over-abstract test setup into deeply nested helper functions or distant
  global state.
- Keep tests deterministic and linear: no control flow (`if`, `match`, loops)
  inside a test function.
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

### Trait declarations

#### Repository traits are `Send + Sync`

Always declare repository traits as `Send + Sync`:

```rust
pub trait UserRepository: Send + Sync {
    // ...
}
```

- `Send`: the repository may be moved between threads.
- `Sync`: the repository may be shared concurrently through
  `Arc<UserRepository>`.

Most database pools already satisfy both, e.g. `sqlx::Pool<Postgres>` and
`sqlx::Pool<Sqlite>`.

#### Async methods return `Send` futures

Return an explicitly `Send` future instead of relying on the default (which is
not guaranteed to be `Send`):

```rust
use std::future::Future;

pub trait UserRepository: Send + Sync {
    fn find(&self, id: UserId) -> impl Future<Output = Result<User, FindUserError>> + Send;
}
```

Web frameworks (axum) move futures across worker threads; without `Send`,
`tokio::spawn(...)` and other runtime operations fail to compile.

#### Traits are `'static`

Repositories usually live for the whole application lifetime and frameworks
require injected state to be `'static`:

```rust
struct AppState {
    user_repo: Arc<Repository>,
}
```

#### Prefer `Arc<T>` over `Clone`

Do not add `Clone` just for frameworks — prefer sharing through `Arc<R>`.

### Repository Method

| Name                                 | Action        |
| ------------------------------------ | ------------- |
| `save(domain: DomainSpecificObject)` | Insert/Update |
| `delete(id: DomainSpecificId)`       | Delete        |
| `search(filter: &SomeFilter)`        | Query         |

- Trait named `<Aggregate>Repository`, e.g. `NoteRepository`.
- Implementation named `<Tech><Aggregate>Repository`, e.g. `SqlxNoteRepository`.

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

## SQL

### General

- Use `snake_case` (all lowercase, words separated by underscores). Avoid mixed
  casing or quoted identifiers (e.g. `"UserId"`).
- Use clear, descriptive English words. Avoid obscure abbreviations (e.g. prefer
  `customer_number` over `cust_num`).
- Never use SQL reserved words (e.g. `order`, `group`, `date`, `select`) as
  object or column names without an identifying prefix or suffix (e.g.
  `purchase_order`, `created_at`). Exception: SQLite accepts a few ANSI SQL
  reserved words (e.g. `user`) as identifiers, so they are allowed.
- Use only standard ASCII alphanumeric characters (`a-z`, `0-9`) and underscores
  (`_`). No spaces, hyphens, or special symbols.
- Enum constraint strings must be in uppercase.

### Table names

- Singular (`user`, not `users`).
- Junction / mapping tables combine both entity names in order of primary
  hierarchy, e.g. `user_role`.

### Column names

- Primary keys: use `<table_name>_id`, e.g. `user_id`, for readability across
  joins.
- Foreign keys: use the exact primary key name of the referenced table (e.g.
  `customer_id` inside the `orders` table).
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

Important: all constraints must be declared at table level, i.e. at the end of
the `CREATE TABLE` statement.

### Triggers and functions

- Triggers: `tg_`
- Functions: `fn_`
