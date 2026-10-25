# Tasks

## 1. Instance state foundation

- [x] 1.1 Add `InstanceState` in `src/lib/domain/model/instance_state.rs` with `created_at`, `pepper` and `jwt_secret`,
      a `try_new` that validates each secret as 64 hexadecimal characters, accessors, and a `Debug` that renders both
      secrets as `**`; verify unit tests cover a valid value, each invalid-hex rejection, and redacted `Debug` output.
- [x] 1.2 Add `InstanceStateStore` and its error in `src/lib/domain/port/` with `load() -> Option<InstanceState>` and
      `save(&InstanceState)`; verify the port compiles and `mockall` generates the mock used by the use-case tests.
- [x] 1.3 Add `yaml_serde` to `Cargo.toml` and record it in the § 6.5.1 runtime table (`yaml_serde`, 0.10.7, MIT OR
      Apache-2.0); verify `cargo build` and `cargo deny check` pass. `yaml_serde` was already present in the working
      tree; the § 6.5.1 row and `THIRD_PARTY_NOTICES.txt` are updated. `cargo deny check` reports a pre-existing
      advisory on `ctutils 0.4.2` (RUSTSEC-2026-0385, via `argon2`), which is also present at `HEAD` and is unrelated to
      this change.
- [x] 1.4 Implement `FileInstanceStateStore` under `src/lib/infrastructure/outbound/file_system/` with an explicit
      `version` field, a serde body, `deny_unknown_fields`, strict 64-hex validation and a 4 KiB read cap; write
      atomically with mode `0600` and guard load-or-create with `std::fs::File::lock` on `state.yaml.lock`; verify
      adapter tests cover round-trip, unsupported version, unknown field, non-hex secret, truncation, permissions, and
      concurrent creation.
- [x] 1.5 Add `LoadState` in `src/lib/application/use_case/load_state.rs` returning
      `{ state, created, pepper_generated, jwt_secret_generated }`; verify tests cover fresh creation, existing load,
      missing identity with an existing datastore, a seed used/generated/matching/differing, generator failure, store
      failure, and that no secret appears in an error. It takes no command: it reads the seeds through the
      `StateSeedSource` port and probes `DATABASE_PATH` itself (3.9, `design.md` D5).
- [x] 1.6 Register the adapter, secret type and use case in `rule_enforcer.yaml` — `OBS-004` secret types and fields,
      `TEST-027` adapter surfaces; verify `script/rule_enforcer.sh` passes. `STY-RUST-002`/`083` bounded contexts and
      `TEST-041` exempt use cases stay unchanged: identity is a supporting capability of the Identity bounded context,
      so it has no REST handler folder and no catalogued use case (see 1.7). The applied entries are `InstanceState` in
      `OBS-004.secret_types`, `jwt_secret` in `OBS-004.secret_fields`, and
      `src/lib/infrastructure/outbound/file_system/instance_state_store.rs` in `TEST-027.adapter_surfaces`.
- [x] 1.7 Document the identity as a supporting capability of the Identity bounded context in
      `docs/development/TechnicalDesign.md` § 10.2, and the new modules in the map in `docs/development/Overview.md`;
      verify `lint:md` and `fmt:md` pass. It is not a bounded context: there is no `Instance` row in the Overview
      registry, no `## Instance` section, diagram or `UC-` entry in `docs/development/UseCases.md`, and no
      `test/e2e/instance/` folder. The `UC-012` identifier conflict recorded earlier is void because no catalogue entry
      is added; the identity model and store belong to the Identity context under `ARCH-002`.

## 2. Wire identity into the runtime

- [x] 2.1 In `src/bin/server.rs`, run `LoadState` before `LoadConfiguration` and before opening the pool; the use case
      probes `DATABASE_PATH` itself with `Path::try_exists`; verify a fresh run creates `state.yaml` before
      `mnemorium.db`, and a run with an existing database but no `state.yaml` exits with the missing-state error.
- [x] 2.2 Hold `Arc<InstanceState>` alongside the configuration in `AppState`, in `RuntimeIdentityUseCaseFactory` and in
      `FromRef<Arc<JwtTokenProvider>>`, and build the password hasher and tokens from the state secrets; update the
      shared fixtures in `src/lib/lib.rs`; verify `cargo build` and the app-state and factory tests pass.
- [x] 2.3 Emit `system_object("instance_state", "create")` and one `secret_initialized` per generated secret after
      `logging::setup`, only when `created` is true; verify a fresh run logs each event once and a reuse run logs none.
- [x] 2.4 Update the identity paragraphs of `docs/development/TechnicalDesign.md` § 10 and `STY-RUST-061`/`064`/`065`
      for the immutable state and the state-first boot; verify markdown lint and the rule enforcer pass. Rule text and §
      10/§ 10.2 are updated, and `rule/sty-rust-064.yml` now matches the `Arc<Configuration>` field in `AppState` and
      the single `LoadConfiguration` binding in `src/bin/server.rs`.

## 3. Configuration without the datastore

- [x] 3.1 Remove `pepper`, `jwt.secret` and `persistence.sqlite3.path` from `domain/model/configuration.rs`; add
      `Default` to `Configuration`, `Logging`, `Persistence`, `Sqlite3` and `Security` and move the default constants
      into the domain; verify empty and partial documents resolve to a validating `Configuration::default()` and
      present-but-invalid values still fail. The serde target is the sparse `PartialConfiguration` (6.1).
- [x] 3.2 Declare `DATABASE_PATH = "mnemorium.db"` beside `STATE_PATH` in the load-state port, remove `Sqlite3::path`
      and `PathEmpty`, and use the constant for the probe and the pool; verify the build and the updated unit tests
      pass.
- [x] 3.3 Replace `FileConfigurationSource`'s `DbSqlite3Source` base layer with defaults → `config.yaml` → a typed
      environment table; verify tests cover defaults, file, precedence, an unknown name and a malformed value.
      Implemented on `yaml_serde` (`design.md` D7): the file parses into a `yaml_serde::Value`, each override descends
      it one key per segment and assigns the leaf, a parent key holding a non-mapping value is reported naming the
      variable instead of aborting, and `yaml_serde::from_value::<Configuration>` validates the result. **Refined by
      6.1:** the sources return a sparse `PartialConfiguration` and the use case owns the layer merge (`design.md` D6).
- [x] 3.4 Add `default_config.yaml` as an embedded resource and write it atomically when the file is absent, leaving an
      existing file untouched; verify the template-sync test against `Configuration::default()`, the not-rewritten test,
      and the unwritable-path failure. The template-sync test compares `yaml_serde::Value` documents on both sides, so
      no second YAML parser is involved (`design.md` D8).
- [x] 3.5 Simplify `LoadConfiguration` to a datastore-free load returning `{ configuration, file_created, applied }`;
      verify its tests cover the absence of a unit of work, the created flag, the applied settings and source and
      template failures.
- [x] 3.6 Delete `bootstrap.rs`, `dbsqlite3.rs` and `resolve_sqlite3`, delete the `STY-RUST-066`/`076` extractors, and
      update the `STY-RUST-001` allow-list; verify `cargo test` and `script/rule_enforcer.sh` pass. The three source
      files were already removed, `rule/sty-rust-066.yml` and `rule/sty-rust-076.yml` are deleted, and
      `STY-RUST-001.allowed_locations` no longer names the deleted
      `outbound/config/configuration_source.rs::deserialize`. `script/rule_enforcer.sh` reports 241 rulebook rules, 167
      registered, 0 failed; the full `cargo test` matrix is re-run under 6.5.
- [x] 3.7 Replace `ArcSwap<Configuration>` with `Arc<Configuration>` in `AppState`, the factories and `FromRef`, and
      remove the `arc-swap` dependency if nothing else uses it; verify `cargo build` and the app-state and factory tests
      pass. `AppState`, the factories and `FromRef` use `Arc<Configuration>`, nothing imports `arc-swap`, and the crate
      is gone from `Cargo.toml`, `Cargo.lock` and the § 6.5.1 runtime table; `cargo build` and the app-state tests pass.
- [x] 3.8 Rewrite `docs/development/TechnicalDesign.md` § 10 and § 10.1, `docs/development/UseCases.md` UC-004, the
      configuration section of the module map in `docs/development/Overview.md`, and the affected rulebook examples;
      verify markdown lint, the rule enforcer, and that `cargo run --bin openapi_gen` leaves `docs/openapi.json`
      unchanged. Markdown lint and the OpenAPI diff pass, and the `STY-RUST-064` extractor was retargeted at the
      `Arc<Configuration>` field (2.4).
- [x] 3.9 Read the identity seeds with `envy` instead of `env::var`: declare the `StateSeeds` capability trait and the
      `StateSeedSource` port with `PEPPER_VARIABLE`/`JWT_SECRET_VARIABLE` in `application/port/load_state.rs`, and
      `EnvStateSeeds`/`EnvStateSeedSource` in `outbound/envy/state_seeds.rs` through `envy::prefixed("MNEMORIUM_")` with
      a `read_from` test seam; verify `src/bin/server.rs` names no environment variable, and tests cover a present seed,
      absent seeds, an unprefixed variable and a configuration variable that is discarded rather than rejected
      (`design.md` D7, D14).
- [x] 3.10 Record the dependency swap: remove the `config` row from the § 6.5.1 runtime table in
      `docs/development/TechnicalDesign.md`, add `envy` 0.4.2, regenerate `THIRD_PARTY_NOTICES.txt`, and confirm
      `cargo deny check` reports only its pre-existing `ctutils` advisory. `THIRD_PARTY_NOTICES.txt` was regenerated
      with `script/generate_third_party_notices.sh`, which also drops `config`'s `toml`/`pathdiff` subtree;
      `cargo deny check` reports only RUSTSEC-2026-0385 on `ctutils 0.4.2` (via `argon2`), which is present at `HEAD`
      and unrelated to this change.

## 4. Remove configuration persistence

- [x] 4.1 Delete `ConfigurationRepository`, `ConfigurationUnitOfWork`, `SqlxConfigurationRepository`, its row model and
      the `configuration()` view from `SqlxUnitOfWork` and `TestUnitOfWork`; remove the `PatchCredential` configuration
      write and its extra bound; verify `cargo test` passes with no remaining reference.
- [x] 4.2 Delete the three `configuration` migration pairs and remove the table from the entity-relationship diagram;
      verify a fresh database migrates and the diagram matches the schema (`PERS-011`).
- [x] 4.3 Remove `PERS-009` and `PERS-010`, update `TEST-002`'s exempt traits, and refresh the `OBS-004` secret-type
      list to `InstanceState`/`Credential`; verify the rule enforcer and markdown lint pass. `PERS-009`/`PERS-010` are
      removed, `TEST-002.exempt_traits` no longer names `ConfigurationUnitOfWork`, `OBS-004.secret_types` now lists only
      `InstanceState` and `Credential`, the security-event catalog and § 10.2 name the instance identity, the
      `sty-rust-064` comment no longer describes the field as an `ArcSwap`, and `check_sty_rust_061` requires
      `Arc<Configuration>` and detects a read through `self.configuration.`, so the clause fires for the asset, identity
      and library factories. The enforcer passes: 241 rulebook rules, 167 registered, 0 failed.

## 5. End-to-end and gate

- [x] 5.1 Keep the E2E suite black-box over the REST API: UC-004 owns the boot lifecycle, has no REST surface and is
      exempt from its own E2E file (`TEST-041`), so the boot scenarios (a fresh boot creates `state.yaml`, `config.yaml`
      and `mnemorium.db`; a restart preserves identity; an empty `config.yaml` boots on defaults; a missing `state.yaml`
      with an existing database exits non-zero) were removed from `test/e2e/` and remain tracked by `test/TODO.md` for
      the planned system-test suite. `pytest -p no:cacheprovider test/e2e` stays green; 6.5 records the current matrix.
- [x] 5.2 Run the full gate — `cargo test`, coverage ≥ 80 %, `cargo clippy --all-targets --all-features -- -D warnings`,
      `cargo fmt --all -- --check`, markdown lint and format, `cargo deny`, and `devenv test` — and verify all pass. The
      matrix passes at this point in the change; 6.5 re-runs it after the configuration rework and records the result.

## 6. State and configuration cohere

- [x] 6.1 Make the configuration layers share one sparse model: `FileConfigurationSource` returns the settings
      `config.yaml` names under `deny_unknown_fields`, `EnvConfigurationSource` returns the settings the `MNEMORIUM_*`
      variables name, every leaf is an `Option` in `PartialConfiguration`, and `LoadConfiguration` merges defaults ←
      file ← environment through the domain constructors; verify tests cover an absent, empty, comments-only, partial,
      unknown-key and malformed document, an overriding variable, an unknown variable and a malformed variable
      (`design.md` D6, D7).
- [x] 6.2 Keep the two identity seeds out of the setting table: `StateSeeds`/`StateSeedSource` in
      `application/port/load_state.rs` and `EnvStateSeeds`/`EnvStateSeedSource` in `outbound/envy/state_seeds.rs`, read
      through `envy::prefixed("MNEMORIUM_")`; verify a present seed, absent seeds, an unprefixed variable and a
      configuration variable that is discarded rather than rejected (`design.md` D7, D14).
- [x] 6.3 Update § 10, § 10.1 and § 10.2 of `docs/development/TechnicalDesign.md`, the `Configuration` use-case entry in
      `docs/development/UseCases.md`, the README configuration section, the Overview boot list and the embedded template
      comment; verify markdown lint and `script/rule_enforcer.sh`.
- [x] 6.4 Keep the E2E harness on the environment override: `script/e2e.sh` raises the login rate limit with
      `-e MNEMORIUM_SECURITY_RATE_LIMIT_BURST_SIZE`, `test/e2e/configuration/uc004_load_configuration.py` is gone and
      the remaining boot expectations are tracked in `test/TODO.md`; verify `script/e2e.sh`.
- [ ] 6.5 Run the full gate after 6.1–6.4 — `cargo test`, coverage ≥ 80 %,
      `cargo clippy --all-targets --all-features     -- -D warnings`, `cargo fmt --all -- --check`, markdown lint and
      format, `cargo deny`, and `devenv test` — and record the measured matrix here. The previously recorded matrix
      predates the rework.

## Workflow follow-up

- Record the one-time development database reset in the pull request description.
- Archive the change after the review requirements are satisfied.
