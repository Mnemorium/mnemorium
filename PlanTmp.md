# PlanTmp.md — Unit of Work refactor (application transaction boundary)

## Goal

Move the transaction boundary into the application service. Use cases open a Unit of Work, run business logic across
repositories, and `commit`/`rollback`. Presentation handlers only call the use case. Repositories are obtained from the
UoW and share one transaction; sqlx types never leave infrastructure.

`register_user` is the first use case migrated and the reviewed template. Because repository methods become `&mut self`
and adapters become transaction-only, **all use cases migrate in this branch**.

## Locked decisions

| Topic        | Decision                                                                                                                                             |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| Scope        | All 7 use cases; `register_user` first/template                                                                                                      |
| DI shape     | Generic `F: UnitOfWorkFactory` (no `dyn`); associated `type Uow: UnitOfWork`, owned `'static`                                                        |
| Transaction  | `SqlxUnitOfWork { transaction: Option<Transaction<'static, Sqlite>> }` via `Pool::begin()` (`Transaction<'static>`); `commit`/`rollback` `take()` it |
| Drop net     | **Deferred.** No panic-on-drop guard this branch                                                                                                     |
| Repositories | Obtained from UoW; port methods become `&mut self`; adapters hold `&mut Transaction<'static, Sqlite>` (transaction-only)                             |
| Contexts     | Base `UnitOfWork` + per-context views (`IdentityUnitOfWork`, `UserUnitOfWork`, `ConfigurationUnitOfWork`) over **one** shared transaction            |
| Failure      | Rollback error logged via `error!`, original business error preserved; commit failure → use-case `Unknown`                                           |
| Hashing      | Inside the UoW, with a `// TODO:` on single-connection blocking; fix later by raising `max_connections`                                              |
| Source       | `ConfigurationSource::load(base: Configuration)` inversion; source drops its repository                                                              |
| Homes        | `src/lib/domain/port/unit_of_work.rs` + per-context files; errors in `domain/port/error.rs`                                                          |
| Test double  | Per-use-case fake implementing only the context traits needed; `#[cfg(test)]` blanket `&mut T` impls                                                 |

## Work breakdown

### 1. Domain ports

- `domain/port/unit_of_work.rs`
  - `UnitOfWork: Send` — `commit(self)`, `rollback(self)` → `impl Future<Output = Result<(), UnitOfWorkError>> + Send`.
  - `UnitOfWorkFactory: Send + Sync` — `type Uow: UnitOfWork;`
    `fn begin(&self) -> impl Future<Output = Result<Self::Uow, UnitOfWorkError>> + Send`.
- Per-context files (`identity_unit_of_work.rs`, `user_unit_of_work.rs`, `configuration_unit_of_work.rs`):
  - `trait IdentityUnitOfWork: UnitOfWork { fn credentials(&mut self) -> impl CredentialRepository + '_; }`
  - `trait UserUnitOfWork: UnitOfWork { fn users(&mut self) -> impl UserRepository + '_; }`
  - `trait ConfigurationUnitOfWork: UnitOfWork { fn configuration(&mut self) -> impl ConfigurationRepository + '_; }`
  - Register all in `domain/port.rs`.
- `domain/port/error.rs`
  - Add `UnitOfWorkError { Unavailable, OperationFailed, Unknown(anyhow::Error) }`.
  - Add `ConfigurationSourceError { InvalidConfiguration(anyhow::Error), OperationFailed, Unknown(anyhow::Error) }`.
- Change `UserRepository`, `CredentialRepository`, `ConfigurationRepository` methods from `&self` to `&mut self`.
- Add, under `#[cfg(test)]`, `impl<T: Repo + ?Sized> Repo for &mut T` delegating every method for each of the three
  ports.

### 2. Infrastructure (`outbound/sqlx`)

- Add `SqlxUnitOfWork` implementing `UnitOfWork` + all three context traits; accessors build short-lived views
  `SqlxXRepository::new(self.transaction.as_mut()...)`.
- Add `SqlxUnitOfWorkFactory { pool: SqlitePool }` implementing `UnitOfWorkFactory`; `begin` calls `pool.begin()`.
- Refactor `SqlxUserRepository`, `SqlxCredentialRepository`, `SqlxConfigurationRepository` to hold
  `&'c mut Transaction<'static, Sqlite>` and execute via `&mut **self.transaction` (or `&mut *self.transaction`); drop
  `new(pool)`.
- Add `impl From<sqlx::Error> for UnitOfWorkError` (reuse the `error_mapping.rs` kind mapping).
- `outbound/config/configuration_source.rs`: drop `<R>` and the repository field; `ConfigConfigurationSource::new()`
  takes no arguments; `load(&self, base: Configuration)` layers `DbSqlite3Source(base)` + file + env, returning
  `ConfigurationSourceError`.

### 3. Application use cases

Common shape:

```rust
let mut uow = factory.begin().await.map_err(...)?;
let result = async { /* business logic using uow.users()/credentials()/... */ }.await;
match result {
    Ok(value) => { uow.commit().await.map_err(...)?; Ok(value) }
    Err(error) => { if let Err(e) = uow.rollback().await { error!(...); } Err(error) }
}
```

- `register_user.rs` — `RegisterUser<F, P>`; order: validate username/email + password policy (pure) → `begin` → caller
  auth → username/email uniqueness → hash (**TODO comment**) → create credential → create user → commit. **Remove the
  compensating `credential.delete` on failure**; rollback covers it.
- `initialize_root_admin.rs` — same compensation removal; use `IdentityUnitOfWork + UserUnitOfWork`.
- `login_user.rs`, `patch_credential.rs` — read/verify/write through the UoW.
- `get_current_user.rs`, `get_user.rs`, `update_user.rs` — `UserUnitOfWork`; read-only ones still `commit`.
- `load_configuration.rs` — `ConfigurationUnitOfWork`; `ensure_row` + read base row through the UoW, then
  `configuration_source.load(base)`; map
  `ConfigurationSourceError::InvalidConfiguration → LoadConfigurationError::InvalidConfiguration`, else `Unknown`.

### 4. Composition root (`src/bin/server.rs`)

- Build `SqlxUnitOfWorkFactory::new(pool.clone())` once, after `init_db`.
- Wire every use case with `Arc<factory>`; remove direct repository construction from `main`.
- `ConfigConfigurationSource::new()` (no args).
- `load_configuration` executes before the server starts, using the factory (pool already exists).

### 5. Tests

- Application unit tests: keep `mockall` repository mocks; add a per-use-case fake `UnitOfWork`/ `Factory` implementing
  only the needed context traits; expose `committed`/`rolled_back` `Arc<AtomicBool>` flags and assert them. Remove
  `expect_delete` compensation expectations.
- Repository integration tests: helper acquires a `Transaction<'static, Sqlite>` for the adapter; run raw
  seeding/verification SQL through that same transaction (the single-connection pool cannot serve pool queries while the
  transaction holds the connection).
- Handlers/OpenAPI: unchanged (handlers already only call the use case).

### 6. Verification

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --bin server
cargo test --lib
cargo run --bin openapi_gen   # must produce no diff
```

## Out of scope (follow-up)

- `UnitOfWork` `Drop` panic net.
- Docs: `StyleGuide.md`, `Overview.md`, `UseCases.md`, `Test.md`, `Persistence.md`.
  - In particular, `StyleGuide.md` must gain a section explaining **why the UoW is separated this way**, both
    technically and conceptually:
    - **Technical**: the transaction boundary lives in the application service; the factory is generic with an
      associated `Uow` type (one owned `Transaction<'static, Sqlite>`); repository ports are `&mut self` and handed out
      as short-lived views over that single transaction; commit/rollback consume the UoW; the concrete sqlx adapters are
      transaction-only and never leak sqlx types past infrastructure.
    - **Conceptual**: per-bounded-context UoW views (`IdentityUnitOfWork`, `UserUnitOfWork`, `ConfigurationUnitOfWork`,
      …) over one shared transaction keep a modular-monolith structure and DDD bounded-context boundaries explicit,
      while still guaranteeing cross-context atomicity for use cases that span contexts (e.g. Register User: Identity +
      User; Delete User Account: Identity + User + Library + Asset).
  - Also in `StyleGuide.md`: a section for **port errors that are neither a Repository nor an External Service** (e.g.
    `UnitOfWorkError`, `ConfigurationSourceError`). Such a port declares its own error type in
    `src/lib/domain/port/error.rs`, following the same layered model, and must expose at least an
    `Unknown(anyhow::Error)` variant.
  - add that in `server.rs` no func/struct other than `main` should be declared
- Raising `max_connections` / moving hashing off the connection.

## Risks / notes

- Borrow discipline: repository views borrow the UoW mutably, so they must be dropped (end of statement) before
  `commit`/`rollback` consumes it.
- Integration-test seeding must move inside the transaction (single-connection pool).
- `ConfigurationSource` error migration is a behavior-neutral type change; map it in `load_configuration`.
