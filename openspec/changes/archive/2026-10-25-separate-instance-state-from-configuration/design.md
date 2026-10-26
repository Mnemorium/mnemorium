# Design

## Context

See `proposal.md` — Why. The current shape that this design replaces:

- `LoadConfiguration` reads the `configuration` singleton row as a base layer, then layers `config.yaml` and the
  `MNEMORIUM_…` environment on top (`src/lib/application/use_case/load_configuration.rs`,
  `src/lib/infrastructure/outbound/config/configuration_source.rs`).
- The row holds the generated `pepper` and `jwt_secret`, so a missing row regenerates identity.
- `bootstrap_sqlite3` duplicates the file-and-environment layering because the datastore path is itself in the row.
- `AppState` holds `Arc<ArcSwap<Configuration>>`, but nothing ever writes the `ArcSwap`.
- `PatchCredential` writes `is_root_admin_password_logged` into the row, but that value is read only while creating the
  root admin, so the write can never affect behaviour.

Constraints that shape the approach: the hexagonal dependency direction (`ARCH-001`), one bounded context per model and
port (`ARCH-002`), absent-versus-malformed configuration semantics (`STY-RUST-078`), secret redaction (`OBS-004`,
`SEC-001`), and mutable migrations while the schema is unreleased (`PERS-014`).

## Goals / Non-Goals

**Goals:**

- Load identity before everything else and never silently replace it.
- Resolve configuration from defaults, the file and the environment without the datastore.
- Generate a complete default configuration file on first launch.
- Keep the container contract to one mutable directory (`/app`).
- Remove the datastore from the configuration path and the `configuration` table entirely.

**Non-Goals:**

- A Postgres adapter or an engine-neutral connection schema.
- Configuration hot reload or a settings API.
- Removing the `MNEMORIUM_*` configuration layer: the environment keeps its precedence over the file.
- Encryption of `state.yaml` at rest.
- Detecting a datastore/state pair restored from two different instances.

## Decisions

### D1 — Two artifacts with different lifecycles

`state.yaml` holds what cannot be derived: the site pepper, the token signing secret, and the instant identity was
established. `config.yaml` holds what can be derived: every setting, all of which have a default. Identity is
existential (absent means lost or new); configuration is total (absent means defaults).

Alternative considered: keep the singleton row and split it conceptually (rejected: the datastore path still has to be
known before the row is readable, so the chicken-and-egg survives, and a lost row can regenerate identity).

### D2 — State file format: serde + YAML, versioned, no checksum

The payload is a serde type serialized as YAML with `yaml_serde` (the maintained fork of the retired `serde_yaml`,
published by the YAML organization). The version is an explicit `version` field carrying the format number: YAML is
self-describing, so the decoder reads the field by name and can reject an unknown number before it builds the body. The
adapter owns the file struct and converts it to `InstanceState`, so the domain model carries no format version.

```text
version:    1
created_at: <unix seconds>
pepper:     "<64 hex chars>"
jwt_secret: "<64 hex chars>"
```

Constraints that follow and must be enforced in review:

- The version is checked first; a version this build does not know fails startup naming it (`UnsupportedVersion`).
- `#[serde(deny_unknown_fields)]` on the file struct makes a misspelled or duplicated key fail startup instead of being
  ignored.
- Both secrets are stored as the exact 64-character hexadecimal strings the hasher and the token provider consume, never
  as raw bytes, so a round trip cannot change the key material. A stored value that is not exactly 64 hex characters
  fails startup as corrupt.
- The file is capped at 4 KiB before parsing, so a truncated or oversized document is rejected before it is decoded.
- No checksum field: it would make the file un-editable, which is most of the reason for choosing YAML. A bit flip that
  stays inside the hex alphabet is undetected; the file is local, mode `0600`, and written only by the server.

Alternative considered: bincode (rejected: an opaque binary file for a three-field document the operator must be able to
inspect). A checksum over a canonical serialization (rejected: it defeats hand inspection and editing).

### D3 — State file path, ownership and concurrency

The path is fixed at `state.yaml` relative to the working directory, which resolves to `/app/state.yaml` in the
container and to the repository root in development. The adapter takes the path as a constructor argument, so tests
inject a temporary path without a production environment override.

- Written atomically: temp file in the same directory, `sync_all`, rename, then sync the directory.
- Mode `0600`; the container user already owns `/app`.
- A load-or-create critical section is guarded by an exclusive `std::fs::File::lock` on a sibling `state.yaml.lock`
  (stable since Rust 1.89, which the toolchain exceeds). The OS releases it on process exit, so a crash leaves no stale
  lock.
- A file that is present but cannot be decoded, fails validation, or declares an unknown version fails startup. It is
  never treated as absent.

Alternative considered: `create_new` sentinel locking (rejected: a crash leaves a stale sentinel). A per-process temp
name plus rename (rejected: last-writer-wins can leave a process serving an unpersisted secret).

### D4 — New, lost or existing

The datastore path is fixed, so the probe needs no configuration and runs immediately after the identity read attempt:

```text
state.yaml | mnemorium.db | outcome
-----------+--------------+-------------------------------------------------
absent    | absent       | create identity, then create the datastore
absent    | present      | FAIL: identity lost for an existing datastore
present   | absent       | reuse identity, create the datastore
present   | present      | reuse identity, use the datastore
```

The probe samples `Path::try_exists(DATABASE_PATH)` before `init_db` creates the file, so an unreadable datastore fails
startup (`DatastoreUnreadable`) instead of reading as a new instance. A zero-length or half-initialised file counts as
present; failing is the safe direction.

Alternative considered: a database marker row or an instance identifier on both sides (deferred; it would detect a
database and state file restored from different instances, which this change accepts as a residual risk).

### D5 — Boot order

```text
1. state   = LoadState().execute()            // reads EnvStateSeeds; probes DATABASE_PATH itself
2. config  = LoadConfiguration({ file, env }) // defaults <- config.yaml <- MNEMORIUM_*
3. logging::setup(config.logging)
4. state.created -> system_object("instance_state", "create") + one secret_initialized per generated secret
5. config.file_created -> warn naming the resolved storage root and datastore path
6. config.applied() -> info naming the settings the environment overrode
7. init_db(DATABASE_PATH, config.max_connections)
8. initialize_root_admin (hasher built from state.pepper)
9. compose AppState { config: Arc<Configuration>, state: Arc<InstanceState>, factories }
```

`LoadState` takes no command: it reads the seeds through its port and probes the fixed datastore path itself, so the
composition root injects no presence flag. Identity creation happens before configuration, as required by the identity
spec, and its lifecycle events are emitted once logging is installed. The `created` flag therefore survives: logging
settings are themselves configuration, so the subscriber cannot exist before step 2. The improvement over the current
code is that the flag no longer travels through a datastore transaction, and the subscriber is installed before
`init_db` rather than after it.

Alternative considered: defer identity creation until after logging (rejected: it violates "identity before
configuration" and the pair decision already needs no configuration).

### D6 — Configuration resolution is total, datastore-free and layered

`LoadConfiguration` no longer opens a unit of work. It resolves the three layers it already had, in the precedence the
server keeps: the built-in defaults (`Configuration::default()`) are the base, `FileConfigurationSource` loads the
settings `config.yaml` names, and `EnvConfigurationSource` loads the settings the `MNEMORIUM_*` variables name. Both
sources return the same sparse `PartialConfiguration`: every leaf is an `Option` under
`#[serde(default, deny_unknown_fields)]`, a `None` keeps the value of the layer below, and the use case merges each
layer itself, rebuilding the resolved value through the domain constructors so a merged value can never bypass
validation. No layer reads the datastore.

That shape keeps the three properties the change needs:

- an absent, empty or comments-only document yields a complete configuration;
- a file key the schema does not know fails startup, while an environment name that matches no setting is discarded
  (D7);
- a present but invalid value fails startup (`STY-RUST-078`).

`Default` implementations are added for `Configuration`, `Logging`, `Persistence`, `Sqlite3` and `Security`. The
constants that currently live in `application/use_case/load_configuration.rs` (`DEFAULT_LOG_*`, `DEFAULT_JWT_TTL`,
`DEFAULT_SQLITE3_MAX_CONNECTIONS`) move to `domain/model/configuration.rs`, because the domain cannot import the
application layer. `persistence.sqlite3.max_connections` defaults to a fixed `8`; the `available_parallelism` clamp is
removed so `Default` is deterministic and the domain performs no OS probe.

Alternative considered: keep `Option<T>` fields on the resolved `Configuration` and overlay them at runtime (rejected:
the effective configuration would stop being total and every consumer would handle absence).

### D7 — The environment keeps its layer

`config.yaml` and the `MNEMORIUM_*` variables remain the two operator-facing ways to change a setting, with the
environment taking precedence over the file as it does today (`docs/development/TechnicalDesign.md` § 10). This change
does not remove that layer: an operator's existing `-e MNEMORIUM_LOGGING_LEVEL=debug` keeps working, and `script/e2e.sh`
keeps using one to raise the login rate limit.

What the change does change is what the environment can reach. The two identity seeds are not settings: they are
creation input, consumed once before the configuration file is read, and they live on `EnvStateSeeds` (D14) rather than
in the setting table. A prefixed variable that names no setting and is not a seed is discarded silently — the server
does not reject variables it does not know — while a variable that is present but does not hold a value its setting
accepts fails startup naming the variable and never its value (`STY-RUST-078`, `OBS-004`). The setting paths the
environment applied are logged once, after the subscriber is installed, so an operator can see which settings it took
over.

`EnvConfigurationSource` therefore keeps one flat struct whose field names are the setting paths, and
`LoadConfiguration` merges its `PartialConfiguration` over the file layer (D6). No layer reads the datastore.

Alternative considered: remove the environment layer so `config.yaml` is the only source (rejected: it is an unrelated
breaking change for container operators, and it does not remove a source of truth — the file and the environment name
the same schema through the same `PartialConfiguration`).

### D8 — Default file generated from an embedded template

The default file is a checked-in YAML resource included with `include_str!` and written atomically (mode `0644`) when
`config.yaml` is absent. It carries the built-in defaults only — never secrets. An existing file, including an empty
one, is never rewritten.

A test parses the embedded template and `yaml_serde::to_value(Configuration::default())` with the same crate and
compares the two documents. That catches a missing, renamed or stale key without a second YAML parser, and it keeps the
template free to carry explanatory comments.

Alternative considered: serialize `Configuration::default()` with a YAML emitter (rejected: a new dependency, no
comments, and every internal rename rewrites the operator's file). Hand-maintaining the template without a test
(rejected: guaranteed drift).

### D9 — Split models

```text
Configuration { asset, logging, persistence, security }     // total, no secrets, Serialize allowed
Security { jwt: Jwt { ttl }, log_root_admin_password, rate_limit }
Persistence { sqlite3: Sqlite3 { max_connections } }         // path removed
InstanceState { created_at, pepper, jwt_secret }             // redacted Debug, not Serialize
```

`Jwt` keeps its name and drops `secret`, so `security().jwt().ttl()` and the token provider keep their shape. `Sqlite3`
drops `path` and its `PathEmpty` validation. `log_root_admin_password` stays a first-boot configuration setting; the
`PatchCredential` write and the `ConfigurationUnitOfWork` bound it required are deleted. The durable flag cannot affect
behaviour because it is read only while the root admin is being created.

### D10 — Fixed datastore path

`pub const DATABASE_PATH: &str = "mnemorium.db"` is declared beside `STATE_PATH` in `application/port/load_state.rs`.
`LoadState` uses it for the presence probe and the composition root uses it to open the pool; the domain does not know a
file path. The container deploy mounts `/app`, which already holds the datastore, the media root and the configuration
file.

Alternative considered: keep the path configurable and record the expected path in `state.yaml` to detect relocation
(rejected: it reintroduces a configuration value inside identity and makes moving the datastore a state migration).

### D11 — Remove configuration persistence

Delete the three `configuration` migration pairs, the `configuration` table from the entity-relationship diagram
(`PERS-011`), `ConfigurationRepository`, `ConfigurationUnitOfWork`, `SqlxConfigurationRepository`, its row model,
`DbSqlite3Source`, `bootstrap.rs`, and the `configuration()` view from `SqlxUnitOfWork` and `TestUnitOfWork`. Remove
`PERS-009` and `PERS-010`, which describe the deleted singleton and flag. `PERS-014` makes this a deletion in place
rather than a drop migration; developers reset the disposable database as that rule already documents.

### D12 — Immutable runtime configuration

`AppState` holds `Arc<Configuration>` and `Arc<InstanceState>` instead of `Arc<ArcSwap<Configuration>>`. The Identity
factory reads the JWT TTL from configuration and the pepper and signing secret from state; other factories keep reading
configuration. `FromRef<Arc<JwtTokenProvider>>` reads both. Nothing writes configuration, so the `ArcSwap` and the
lazy-reload justification for it disappear.

`STY-RUST-065` ("never cache a configuration-derived value") is restated: the pepper is no longer configuration and is
immutable, so building the password hasher from `state.pepper()` once is correct; TTL and other configuration-derived
values are still read live.

### D13 — Fail-fast errors and secret handling

`LoadStateError` distinguishes `DatastoreUnreadable`, `StateMissing`, `StateCorrupt`, `UnsupportedVersion`,
`InvalidSeed`, `SeedMismatch`, `SeedsUnreadable`, `StoreFailed` and `Unknown`. Messages name `state.yaml` and the
recovery ("restore the file; deleting it does not regenerate credentials"). `InvalidSeed` and `SeedMismatch` name the
environment variable, never the secret's value. `InstanceState` renders both secrets as `**` in `Debug`; no log line,
error chain or lifecycle event carries a secret value.

### D14 — Identity seeds are read with `envy`

The seed variables are not configuration, so they do not go through the setting table. They are read from a flat struct
whose field names are the variable names below the shared prefix, which is what `envy` maps:

```rust
#[derive(Default, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]                 // no deny_unknown_fields: the prefix carries other variables
pub struct EnvStateSeeds {
    pepper: Option<String>,       // MNEMORIUM_PEPPER
    jwt_secret: Option<String>,   // MNEMORIUM_JWT_SECRET
}
```

`envy::prefixed("MNEMORIUM_")` strips the prefix, lowercases the remainder and matches it to a field, so adding a seed
is adding a field — no `env::var` call and no growth in the composition root. `EnvStateSeedSource::read_from` is the
test seam, so adapter tests never mutate the process environment. The struct and its source live in the adapter
(`outbound/envy/state_seeds.rs`), and the application layer sees only the `StateSeeds` capability trait and the
`StateSeedSource` port in `application/port/load_state.rs`, so no adapter type is named inward (`ARCH-001`).
`EnvStateSeeds` implements `Debug` by hand, rendering a supplied seed as `**`.

Three properties constrain the shape:

- **No unknown-field denial in serde.** `envy` feeds every matching variable into serde, so `deny_unknown_fields` would
  report a field name rather than the variable. A prefixed name no field matches is discarded, never rejected (D7).
- **The seed fields stay strings.** `envy` renders a parse failure as
  `"<error> while parsing value '<value>' provided by <NAME>"`; a non-string seed field would put the value into the
  error chain, which `OBS-004` forbids. Both fields are `Option<String>`, and their shape is checked by the use case,
  whose errors name the variable and never the value.
- **The names live once.** `PEPPER_VARIABLE` and `JWT_SECRET_VARIABLE` sit beside the port, which the use case uses in
  `InvalidSeed`/`SeedMismatch`; the adapter's prefix is the only other literal naming the environment.

Alternative considered: `env::var` per seed in the composition root (rejected: two declarations plus a third in the
resolver, and every new seed edits the entrypoint). Giving identity its own `MNEMORIUM_STATE_` prefix (rejected: it
renames operator-facing variables for no behavioural gain).

## Risks / Trade-offs

- **Backup set grows and must stay together.** `mnemorium.db`, `state.yaml` and `config.yaml` are one unit. Losing
  `state.yaml` while keeping the database fails startup (safe); losing both looks like a fresh install (data loss).
  Document the backup set; the failure message names the missing file.
- **A mismatched pair is undetectable.** A database restored from instance A with the state file from instance B passes
  the presence check and boots with unusable password hashes. Accepted; an instance identifier is the deferred fix.
- **Fixed datastore path blocks relocation.** An operator cannot move the database with a setting; the whole `/app`
  volume moves. Accepted in exchange for removing the pre-configuration resolution and the relocation hazard.
- **Generated defaults freeze at generation time.** A later change to a default does not reach an existing file. This is
  intentional: the operator owns the file, and new keys fall back to their defaults.
- **Losing `config.yaml` reverts settings.** `asset.storage.root` falls back to `data`, so the library looks empty even
  though the files are intact. Mitigation: a startup warning names the resolved storage root when the file is generated;
  the datastore path can no longer move.
- **The environment silently outranks the file.** A variable left behind in a deployment overrides `config.yaml` without
  changing the file, which is why the applied setting paths are logged once at startup. A malformed variable fails
  startup rather than being ignored, so a typo stops the process instead of quietly doing nothing.
- **`state.yaml` is text an operator can edit.** A hand edit that keeps the shape but changes a secret boots with a
  pepper that no longer matches the existing hashes. Mitigations: `deny_unknown_fields`, exact 64-hex validation, mode
  `0600`, and documentation that the file is restore-only. A checksum field was rejected because it would defeat hand
  editing, which is most of the reason for choosing YAML.
- **Dependency swap.** `yaml_serde` 0.10.7 (`MIT OR Apache-2.0`), already added for the state file, and `envy` 0.4.2
  (`MIT`, depends only on `serde`) are the direct additions; `config` 0.15.25 is removed, taking its `yaml-rust2` parser
  with it, so the tree carries one YAML parser instead of two (§ 6.5.1). Both additions are inside the licence
  allow-list and the recorded toolchain.
- **Dev databases reset once.** Deleting the applied migration pairs makes `sqlx::migrate!` refuse the stale database;
  the documented recovery (delete `mnemorium.db` and its sidecars) is required, and `state.yaml` must be removed with
  it.
- **Secrets are not encrypted at rest.** `state.yaml` relies on `0600`. The database already held the same secrets in
  plaintext, so this is not a regression; encryption would need a key source and is out of scope.
- **First boot emits its event after logging is installed, not at creation.** The deferred flag remains for that one
  event; the trade-off is accepted because logging settings are configuration.

## Migration Plan

Nothing is released under `PERS-014`, so there is no data migration and no adoption path:

1. Delete the `configuration` table migrations (`20260904021531_create_configuration_table`,
   `20260904021532_add_configuration_logging`, `20261004000000_add_configuration_rate_limit`) and the table from the
   entity-relationship diagram.
2. Ship the identity artifact first if the change is split into two pull requests: state file, model split, boot order.
   Then ship the configuration artifact: defaults, file, template, fixed path, table removal. Each stage boots on its
   own; state must precede configuration because the configuration model cannot lose its secrets before another artifact
   owns them.
3. On deployment, `/app` must be the mounted volume. First start with an empty `/app` creates `state.yaml`,
   `config.yaml` and `mnemorium.db`. A start with an existing `mnemorium.db` but no `state.yaml` fails by design.
4. Rollback: restore `state.yaml` and `mnemorium.db` from the same backup. Downgrading to a build that predates this
   change regenerates secrets once the state file is ignored, invalidating every password hash and token; that rollback
   is not supported. Reverting only the configuration artifact to the identity-artifact build is safe because identity
   stays in `state.yaml`.

## Open Questions

- Should identity carry an instance identifier so a database and state file restored from different instances can be
  rejected? Deferred; it does not change this change's specs, approach or tasks.
- Should `state.yaml` be encrypted at rest? Deferred; it needs an external key source, which is its own design.
- When a Postgres adapter lands, does the fixed `mnemorium.db` path become an engine-specific default alongside a
  connection setting? Deferred to that change.
