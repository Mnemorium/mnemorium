# Add check — STY-RUST-044

## Rule guard — STY-RUST-044

- **Rule:** "`create` inserts a new aggregate and never binds identity columns: the datastore auto-increments them and `create` returns the aggregate with its final identifier." — `docs/development/TechnicalDesign.md` line 154, rulebook section **Repository** (More info: [§ 8.1](#81-repository-port)). § 8.1 states the port contract in its method table: `create(domain: DomainSpecificObject)` → **Insert (identity assigned by the datastore)**, and the port doc comments in `src/lib/domain/port/*_repository.rs` restate it ("The identifier of `<aggregate>` is ignored: the repository assigns a fresh identity").
- **Check state:** missing — `STY-RUST-044` has no `RULES` entry, there is no `check_sty_rust_044`, no `rule/sty-rust-044.yml` exists, and `devenv shell -- script/rule_enforcer.sh uncovered` prints `STY-RUST-044 (Repository) — …`.
- **Mechanism:** ast-grep extraction rule + Bash — the violation is a correlation between a SQL string literal and the table it names, which ast-grep cannot express (it cannot read inside a string literal, nor derive `<table>` → `<table>_id`). `rule/sty-rust-044.yml` extracts the `INSERT INTO` string literal of every repository-adapter `create`, and the Bash check parses the table and its column list and rejects a bound identity column. No `rule_enforcer.yaml` block (the check reads no per-rule data).

### Interpretation (why the check is shaped this way)

The enforceable action is the clause before the colon — *never binds identity columns*. The colon clause is the rationale and the outcome. In each adapter the identity column of the table named by `INSERT INTO <table>` is `<table>_id` (`user_id`, `credential_id`, `file_id`, `image_id`, `video_id`, `gallery_id`, `upload_id`): `create` must **not** list it in the insert columns, and must list it in `RETURNING` so the persisted aggregate carries its final identifier. The companion `save` methods *do* bind `<table>_id` (upsert by identity), so the extraction rule's `^create` name filter is what separates them.

`configuration_repository.rs` is out of scope: the `configuration` row is a singleton pinned to `configuration_id = 0` (`PERS-009`), so the datastore does not assign its identity and its adapter must bind it (`.bind(0i64)`). The rule says *a new aggregate*, and the configuration port explicitly omits the "identifier is ignored" clause. The wrapper excludes that one adapter with a `--globs` negation (proven load-bearing below).

## Files

### `rule/sty-rust-044.yml` (new)

New ast-grep extraction rule; registered automatically by `sgconfig.yaml` (`ruleDirs: [rule]`), so no `sgconfig.yaml` change. `severity: hint` keeps the scan at exit 0 on a match, so the wrapper consumes the `--json=stream` extraction (the `sty-rust-006`/`sty-rust-015`/`sty-rust-023` idiom). It matches the `INSERT INTO` string literal inside a function whose name begins with `create`, binding `$FUNC` (the enclosing function's name, for the diagnostic). No `files:` block: the wrapper's `--globs` scope it and `--rule` runs it directly on a fixture.

```yaml
---
id: sty-rust-044
language: Rust
severity: hint
message: >-
  extract the insert statement of a repository adapter `create`, for
  STY-RUST-044
rule:
  all:
    - kind: string_literal
    - regex: 'INSERT INTO'
    - inside:
        kind: function_item
        pattern: $ENCLOSING
        has:
          field: name
          pattern: $FUNC
          regex: '^create'
        stopBy: end
```

### `script/rule_enforcer.sh`

Two edits.

1. Register the check in `RULES`, appending after `"STY-RUST-024:check_sty_rust_024"` (line 71, before the closing `)` on line 72):

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-044:check_sty_rust_044"
)
```

2. Declare the wrapper in the `# --- Checks ---` section, after `check_sty_rust_024()` (ends line 752) and before `# --- Runner ---` (line 754):

```bash
check_sty_rust_044() {
	local id="STY-RUST-044"
	local rule_id="sty-rust-044"
	local dir="src/lib/infrastructure/outbound/sqlx"
	local matches file func sql table_name cols identity returning result=0

	if [ ! -d "$dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$dir" >&2
		return 1
	fi

	# sty-rust-044 extracts every `INSERT INTO` of a repository adapter `create`
	# together with the enclosing function; the identity-column check happens
	# here because an ast-grep rule cannot read inside the SQL string. The
	# `configuration` singleton (PERS-009, `configuration_id = 0`) is not a new
	# aggregate, so the datastore does not assign its identity and its adapter is
	# out of scope.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs 'src/lib/infrastructure/outbound/sqlx/*_repository.rs' \
		--globs '!src/lib/infrastructure/outbound/sqlx/configuration_repository.rs' \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no repository create insert\n' "$id" >&2
		return 1
	fi

	while IFS=$'\t' read -r file func sql; do
		[ -n "$sql" ] || continue
		# The identity column of the table named by `INSERT INTO <table>` is
		# `<table>_id`: the datastore assigns it, so `create` must neither bind
		# it nor omit it from `RETURNING`.
		table_name="$(printf '%s' "$sql" |
			sed -n 's/.*INSERT INTO[[:space:]]*\([A-Za-z_][A-Za-z0-9_]*\)[[:space:]]*(.*/\1/p')"
		cols="$(printf '%s' "$sql" |
			sed -n 's/.*INSERT INTO[[:space:]]*[A-Za-z_][A-Za-z0-9_]*[[:space:]]*(\([^)]*\)).*/\1/p')"
		identity="${table_name}_id"
		if printf '%s' "$cols" | tr ',' '\n' | grep -Eq "^[[:space:]]*${identity}[[:space:]]*$"; then
			printf 'error: %s: %s: %s must not bind the identity column %s\n' \
				"$id" "$file" "$func" "$identity" >&2
			result=1
		fi
		returning="${sql#*RETURNING}"
		if ! printf '%s' "$returning" | grep -Eq "(^|[ ,])${identity}([ ,\"]|$)"; then
			printf 'error: %s: %s: %s must return the identity column %s\n' \
				"$id" "$file" "$func" "$identity" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, .metaVariables.single.FUNC.text, (.text | gsub("\\s+"; " "))] | @tsv')

	return "$result"
}
```

### `rule_enforcer.yaml`

No change. The check reads no per-rule configuration.

## Verification

The candidate rule proven below is `/tmp/opencode/sty-rust-044/sg/rules/sty-rust-044.yml` (byte-identical to the proposed `rule/sty-rust-044.yml`); the candidate function is `/tmp/opencode/sty-rust-044/check044.sh` (byte-identical to `check_sty_rust_044` above, plus one trailing `check_sty_rust_044` call). Runs use a scratch `sgconfig.yaml` (`ruleDirs: [rules]`) at `/tmp/opencode/sty-rust-044/sg/sgconfig.yaml`, so nothing tracked is touched. `$SGCONFIG` is set to that scratch config and the `--globs` are the final ones.

- **Red (identity bound):** `/tmp/opencode/sty-rust-044/red_tree/src/lib/infrastructure/outbound/sqlx/user_repository.rs` — a `create` copied from `save`: `INSERT INTO user (user_id, credential_id, email, role, username) … .bind(user.id())`.
  `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-044/red_tree && SGCONFIG=/tmp/opencode/sty-rust-044/sg/sgconfig.yaml bash /tmp/opencode/sty-rust-044/check044.sh'` — observed: rc=1, `error: STY-RUST-044: src/lib/infrastructure/outbound/sqlx/user_repository.rs: create must not bind the identity column user_id`.
- **Red (identity not returned):** `/tmp/opencode/sty-rust-044/red_returning_tree/src/lib/infrastructure/outbound/sqlx/file_repository.rs` — `INSERT INTO file (path, user_id, is_public, mime_type_id, upload_at, integrity_hash) … RETURNING path, user_id, is_public, mime_type_id, upload_at, integrity_hash` (no `file_id`, so the final identifier is lost) — same command with `cd …/red_returning_tree` — observed: rc=1, `error: STY-RUST-044: src/lib/infrastructure/outbound/sqlx/file_repository.rs: create must return the identity column file_id`.
- **Direct rule extraction (both fixtures):** `devenv shell -- ast-grep scan --rule /tmp/opencode/sty-rust-044/sg/rules/sty-rust-044.yml --report-style=short --color=never <fixture>` — observed: the extraction rule (`severity: hint`) exits 0 and reports the extracted literal at `red_tree/…/user_repository.rs:14:13` and `green_tree/…/user_repository.rs:16:13`; the discriminating assertion is the Bash check above.
- **Green:** `/tmp/opencode/sty-rust-044/green_tree/src/lib/infrastructure/outbound/sqlx/user_repository.rs` — a compliant `create` (`user_id` absent from the insert, present in `RETURNING`), a `save` that binds `user_id`, and a non-`create` `seed_user` helper that inserts `user_id`; plus `configuration_repository.rs` whose `create` binds `configuration_id`. Same command with `cd …/green_tree` — observed: rc=0, no finding (the `^create` filter skips `save`/`seed_user`; the `--globs` negation skips the configuration singleton).
- **Exclusion is load-bearing:** the same check with the `--globs '!…/configuration_repository.rs'` line deleted (`/tmp/opencode/sty-rust-044/check044_noexcl.sh`) run on `green_tree` and on the repository root — observed: rc=1, `error: STY-RUST-044: …/configuration_repository.rs: create must not bind the identity column configuration_id`, confirming both that the parser reads the column list and that the singleton would otherwise be a false positive.
- **Repository:** `devenv shell -- bash -c 'cd /home/will/project/mine/Mnemorium/mnemorium && SGCONFIG=/tmp/opencode/sty-rust-044/sg/sgconfig.yaml bash /tmp/opencode/sty-rust-044/check044.sh'` — observed: rc=0, no finding. The extraction finds exactly seven in-scope creates (`user_repository.rs::create`, `credential_repository.rs::create`, `file_repository.rs::create`, `upload_repository.rs::create`, `media_repository.rs::create_image`, `media_repository.rs::create_video`, `gallery_repository.rs::create`); none lists its `<table>_id` and each returns it, and `configuration_repository.rs::create` is excluded by the glob. After applying the two edits above, `devenv shell -- script/rule_enforcer.sh` registers and runs `STY-RUST-044` in place of its current `uncovered` listing.

## Next

- Apply: `rule/sty-rust-044.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
