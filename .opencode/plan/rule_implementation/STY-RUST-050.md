# Add check — STY-RUST-050

## Rule guard — STY-RUST-050

- **Rule:** "It maps `sqlx::Error` to `RepositoryError` in `error_mapping.rs`; sqlx types never cross into the domain or application layers." — `docs/development/TechnicalDesign.md` § Code Style Guidelines / Rust, section **Repository** (row `STY-RUST-050`, line 160).
- **Check state:** missing — `script/rule_enforcer.sh` registers 22 rules and `STY-RUST-050` appears in `uncovered`; there is no `check_sty_rust_050` and no `rule/sty-rust-050.yml`.
- **Mechanism:** ast-grep + bash — the confinement clause is a syntactic pattern bounded to two layers (matched with node kinds, so the `"debug,sqlx=warn"` string literals in `application` are not false positives); the mapping-location clause is a path/content assertion ast-grep cannot see from the rule alone.

## Files

### `rule/sty-rust-050.yml`

New ast-grep rule. `files:` scopes it to the two guarded layers, so a scan of `src` reports only a crossing; the string literal `"debug,sqlx=warn"` and doc-comment prose are not matched because the rule keys on Rust node kinds, not raw text.

```yaml
---
id: sty-rust-050
language: Rust
severity: error
message: >-
  sqlx types must not cross into the domain or application layers; the
  sqlx::Error-to-RepositoryError mapping lives in error_mapping.rs
files:
  - src/lib/domain/**/*.rs
  - src/lib/application/**/*.rs
rule:
  any:
    # `use sqlx::...` / `use sqlx::{...}` — an import of a sqlx type.
    - kind: use_declaration
      regex: '\bsqlx\b'
    # `#[sqlx(...)]` / `#[derive(sqlx::Type)]` — a sqlx attribute.
    - kind: attribute_item
      regex: '\bsqlx\b'
    # `sqlx::query(...)` and any other `sqlx::...` value path.
    - kind: scoped_identifier
      regex: '^sqlx::'
    # `sqlx::SqlitePool` and any other `sqlx::...` type path.
    - kind: scoped_type_identifier
      regex: '^sqlx::'
```

### `script/rule_enforcer.sh`

Two changes; the runner is untouched.

1. Register the check in the `RULES` array (append after the last line, `"STY-RUST-024:check_sty_rust_024"`):

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-050:check_sty_rust_050"
)
```

2. Add the check function in the `# --- Checks ---` section (after `check_sty_rust_024`, before `# --- Runner ---`):

```bash
check_sty_rust_050() {
	local id="STY-RUST-050"
	local rule_id="sty-rust-050"
	local mapping="src/lib/infrastructure/outbound/sqlx/error_mapping.rs"
	local output result=0

	if [ ! -f "$mapping" ]; then
		printf 'error: %s: missing %s\n' "$id" "$mapping" >&2
		return 1
	fi

	# Clause 1: sqlx types never cross into the domain or application layers.
	# The rule is scoped to those two trees and matches only Rust paths,
	# `use` declarations and attributes — never a string literal such as the
	# `debug,sqlx=warn` logging directive.
	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		:
	else
		printf '%s\n' "$output"
		result=1
	fi

	# Clause 2: sqlx::Error is mapped to RepositoryError in error_mapping.rs.
	if ! grep -qE '^use sqlx::Error;' "$mapping" ||
		! grep -qE '^impl From<Error> for RepositoryError\b' "$mapping"; then
		printf 'error: %s: %s must map sqlx::Error to RepositoryError\n' "$id" "$mapping" >&2
		result=1
	fi

	return "$result"
}
```

`rule_enforcer.yaml` is unchanged: the check reads no per-check data, so it adds no `rules.STY-RUST-050` block.

## Verification

Candidate check body was proven with `devenv shell -- bash /tmp/opencode/sty-rust-050/check.sh <repo>` (a scratch script defining `check_sty_rust_050` exactly as above and `cd`-ing into the fixture repo). The rule itself was proven with `devenv shell -- ast-grep scan --config /tmp/opencode/sty-rust-050/cfg/sgconfig.yaml --filter sty-rust-050 --report-style=short --color=never src` run from each fixture root.

- **Red (clause 1):** `/tmp/opencode/sty-rust-050/red-repo` — `src/lib/domain/model/bad.rs` (`use sqlx::Error;`, `#[sqlx(rename_all = "UPPERCASE")]`, `sqlx::query`, `-> sqlx::Error`) and `src/lib/application/use_case/bad.rs` (`&sqlx::SqlitePool`, `-> sqlx::Error`), with the compliant `error_mapping.rs` present. Command: `devenv shell -- bash /tmp/opencode/sty-rust-050/check.sh /tmp/opencode/sty-rust-050/red-repo` — observed: 7 findings (`src/lib/domain/model/bad.rs:1,7,12,13` and `src/lib/application/use_case/bad.rs:1`), `exit=1`.
- **Red (clause 2):** `/tmp/opencode/sty-rust-050/badmap-repo` — compliant layers, but `error_mapping.rs` maps `From<std::io::Error> for RepositoryError` (no `use sqlx::Error;`). Command: same as above with `badmap-repo` — observed: `error: STY-RUST-050: src/lib/infrastructure/outbound/sqlx/error_mapping.rs must map sqlx::Error to RepositoryError`, `exit=1`.
- **Green:** `/tmp/opencode/sty-rust-050/green-repo` — `src/lib/domain/model/ok.rs`, `src/lib/domain/ok.rs` and `src/lib/application/use_case/ok.rs` containing only `"debug,sqlx=warn"`/`"info,sqlx=trace"` string literals, a `sqlx::` doc-comment mention and a test string; plus the real `impl From<Error> for RepositoryError` in `error_mapping.rs`. Command: same as above with `green-repo` — observed: no output, `exit=0`.
- **Repository:** `devenv shell -- ast-grep scan --config /tmp/opencode/sty-rust-050/cfg/sgconfig.yaml --filter sty-rust-050 --report-style=short --color=never src` — observed: no findings (the `"debug,sqlx=warn"` literals in `src/lib/application/use_case/load_configuration.rs` and `patch_credential.rs` are not matched; scanning `src/lib/infrastructure/outbound/sqlx` also yields nothing). Clause 2 on the tree: `grep -qE '^use sqlx::Error;'` and `grep -qE '^impl From<Error> for RepositoryError\b'` both match `src/lib/infrastructure/outbound/sqlx/error_mapping.rs` — observed: mapping present.

## Next

- Apply: `rule/sty-rust-050.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
