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
[OpenAPI documentation](api/Openapi.md).

### SQL data models

#### Enum for a CHECK constraint

For a column backed by a SQL `CHECK (... IN (...))` constraint, declare the Rust
enum **before** the model struct, in the same file.

- Name the enum after the attribute it represents, in `UpperCamelCase`, e.g.
  `Role` for the `role` column of table `app_user` used in model `AppUser`.
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
pub struct AppUser {
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
pub struct AppUser {
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
- Never use SQL reserved words (e.g. `user`, `order`, `group`, `date`, `select`)
  as object or column names without an identifying prefix or suffix (e.g.
  `app_user`, `purchase_order`, `created_at`).
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
