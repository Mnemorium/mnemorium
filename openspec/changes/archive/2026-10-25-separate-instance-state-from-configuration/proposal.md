# Proposal

## Why

The instance's non-derivable identity and its derivable settings are fused in one `configuration` singleton row.
Generated secrets (`pepper`, `jwt_secret`) are stored, merged, and shadowed like ordinary settings, so a missing or
reordered layer can silently replace the key material that every password hash and every token depends on. The same row
also holds the datastore path, which the process must know _before_ it can read the row, forcing `bootstrap_sqlite3` to
duplicate the file-and-environment layering and turning configuration loading into a chicken-and-egg problem.

Separating the two makes identity survive independently of the datastore, makes configuration fully derivable, and
removes the datastore from the configuration path entirely. Nothing has been released yet, so the split can land without
a compatibility or adoption path.

## What Changes

- Introduce `state.yaml`, a YAML instance-identity file holding the generated secrets. It is loaded before anything
  else; it is created only when the instance is genuinely new; and a missing file for an existing datastore fails
  startup instead of silently regenerating credentials.
- Make configuration derivable: a total set of defaults, overlaid by `config.yaml` and then by the `MNEMORIUM_*`
  environment, keeping the precedence the server already has. The environment's two identity seeds are read separately
  because they are creation input, not settings.
- Write a default `config.yaml` from an embedded template when the file is absent. An empty or partial file is valid
  because every key has a default.
- Fix the datastore path to `mnemorium.db`; remove the configurable datastore path.
- **BREAKING** Remove `security.jwt.secret`, `security.pepper` and `persistence.sqlite3.path` from the configuration
  file schema; secrets now live in `state.yaml` and the datastore path is no longer configurable.
- Split the domain models: `Configuration` loses the secrets, `InstanceState` owns them, and `Jwt` becomes the token
  lifetime alone.
- Delete the `configuration` table, its repository, unit-of-work view and SQLx adapter, along with `DbSqlite3Source` and
  `bootstrap_sqlite3`.
- Stop `PatchCredential` from writing configuration; `log_root_admin_password` remains a first-boot configuration
  setting rather than a durable flag.
- Install the logging subscriber from the resolved configuration before the datastore is opened, so the first-boot
  lifecycle events reach a sink; the `created` flag survives only to carry them past `logging::setup` and no longer
  travels through a datastore transaction.
- Replace the live `ArcSwap<Configuration>` with an immutable `Arc<Configuration>`; nothing writes it any more.
- Out of scope: a Postgres adapter, configuration hot reload, and a settings API.

## Capabilities

### New Capabilities

- `instance/state`: the instance identity lifecycle — loading identity before anything else, creating it only for a
  genuinely new instance, seeding its secrets from the environment at creation, persisting it atomically, and failing
  startup when it is missing for an existing datastore.
- `configuration/loading`: resolving a total configuration from defaults, the configuration file and the environment;
  generating the default file on first launch; rejecting unknown or malformed file content and malformed environment
  values; and doing all of it without touching the datastore.

### Modified Capabilities

None — no existing specification covers configuration or instance identity. The project's current specs are
`library/music-albums`, `library/music-playlists` and `user/account-creation`, and none of their requirements change.

## Impact

- **Domain**: `domain/model/configuration.rs` (secrets removed, defaults added, constants moved inward), new
  `domain/model/instance_state.rs`, new `domain/port/instance_state_store.rs`; removed
  `domain/port/configuration_repository.rs` and `domain/port/configuration_unit_of_work.rs`.
- **Application**: `application/use_case/load_configuration.rs` loses the datastore merge and secret generation and owns
  the layer merge over a sparse `PartialConfiguration`; new `application/use_case/load_state.rs` and its port
  `application/port/load_state.rs`; `application/use_case/patch_credential.rs` loses its configuration write and the
  extra unit-of-work bound.
- **Infrastructure**: `outbound/file_system/configuration_source.rs` loses `DbSqlite3Source` and keeps the file layer,
  `outbound/envy/configuration.rs` keeps the environment layer, and `outbound/envy/state_seeds.rs` reads the creation
  seeds; removed `outbound/config/bootstrap.rs`, `outbound/config/dbsqlite3.rs`,
  `outbound/sqlx/configuration_repository.rs` and its row model; new `outbound/file_system/instance_state_store.rs`;
  `app_state.rs` and the per-context use-case factories carry both the configuration and the instance state.
- **Composition root**: `src/bin/server.rs` takes the new load order and installs logging earlier.
- **Persistence**: a migration drops the `configuration` table; `DATABASE_PATH = "mnemorium.db"` is declared beside
  `STATE_PATH` in `application/port/load_state.rs`, which the state use case probes and the composition root uses to
  open the pool.
- **Dependencies**: add `yaml_serde` and `envy`; remove `config`. `yaml_serde` reads both `state.yaml` and
  `config.yaml`; `envy` maps the flat identity-seed fields to their environment names, so a new seed is a struct field
  rather than an `env::var` in the composition root; `config` no longer earns its place once the datastore base layer is
  gone.
- **API**: no HTTP endpoint changes, so `docs/openapi.json` is unchanged.
- **Tests**: unit and integration tests across the domain, application and adapter layers; the E2E harness keeps the
  black-box suite on the environment override (`script/e2e.sh`), and UC-004's boot scenarios stay tracked in
  `test/TODO.md` for the planned system-test suite.
- **Documentation**: `docs/development/UseCases.md` UC-004, `docs/development/TechnicalDesign.md` § 10, § 10.1 and §
  10.2 (the identity is a supporting capability, so it adds no use case), the module map in
  `docs/development/Overview.md`, and the configuration section of `README.md`.
