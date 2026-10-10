# Add check — STY-RUST-078

## Rule guard — STY-RUST-078

- **Rule:** "A configuration value that is **present but malformed or out of range** fails startup: a reader falls back to a default only when the key is absent, and propagates every other read or validation error." (`docs/development/TechnicalDesign.md` line 188, section Configuration, § 10.)
- **Check state:** missing — `script/rule_enforcer.sh uncovered` lists `STY-RUST-078 (Configuration)`, and no `rule/sty-rust-078.yml` exists.
- **Mechanism:** ast-grep + registry — the violation is a syntactic shape (a `config::Config` getter whose error is collapsed into a default instead of being matched against `ConfigError::NotFound`); the rule matches it structurally and a thin Bash wrapper runs it over `src`, exactly like `check_sty_rust_003`/`check_sty_rust_024`. No `rule_enforcer.yaml` block is needed — the check carries no data.

## Files

### `rule/sty-rust-078.yml`

New ast-grep rule. It flags two shapes that fall back to a default on *every* error: a defaulting combinator (`unwrap_or`, `unwrap_or_default`, `unwrap_or_else`, `map_or`, `map_or_else`) consuming a `config` getter (`get_string`/`get_int`/`get_float`/`get_bool`/`get_table`/`get_array`), and a `match` on such a getter with a catch-all `_`/`Err(_)` arm that does not itself return an `Err`.

```yaml
---
id: sty-rust-078
language: Rust
severity: error
message: >-
  a present-but-malformed configuration value must fail startup: fall back to a
  default only when the key is absent (ConfigError::NotFound), never on every
  read error
rule:
  any:
    # A defaulting combinator applied directly to a config getter swallows the
    # distinction between "absent" (NotFound) and "present but malformed".
    - kind: call_expression
      all:
        - has:
            field: function
            kind: field_expression
            has:
              field: field
              regex: '^(unwrap_or|unwrap_or_default|unwrap_or_else|map_or|map_or_else)$'
        - has:
            field: function
            kind: field_expression
            has:
              field: value
              stopBy: end
              has:
                field: function
                kind: field_expression
                has:
                  field: field
                  regex: '^get_(string|int|float|bool|table|array)$'
    # A match on a config getter whose catch-all arm returns a default.
    - kind: match_expression
      all:
        - has:
            field: value
            kind: call_expression
            has:
              field: function
              kind: field_expression
              has:
                field: field
                regex: '^get_(string|int|float|bool|table|array)$'
        - has:
            kind: match_arm
            stopBy: end
            all:
              - has:
                  field: pattern
                  kind: match_pattern
                  regex: '^(Err\(_\)|_)$'
              - not:
                  has:
                    field: value
                    regex: 'Err'
```

### `script/rule_enforcer.sh`

1. Register the check in `RULES` (after the `STY-RUST-024` line):

```bash
	"STY-RUST-078:check_sty_rust_078"
```

2. Declare the function after `check_sty_rust_024` and before the `# --- Runner ---` banner:

```bash
check_sty_rust_078() {
	local id="STY-RUST-078"
	local rule_id="sty-rust-078"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}
```

The wrapper scans `src` (including `src/bin`) so a config reader added anywhere is covered; `config`-crate getter names (`get_string`, `get_int`, …) scope it to configuration reads. `env::var(...).unwrap_or*` (e.g. `RUST_LOG` in `logging.rs`) is a different, non-`config`-crate read and is out of scope for this rule.

## Verification

- **Red:** `/tmp/opencode/steer-078/red/{combinator.rs,unwrap_or.rs,ok_unwrap.rs,err_wildcard.rs,wildcard.rs}` — `devenv shell -- ast-grep scan --rule /tmp/opencode/steer-078/rule/sty-rust-078.yml --report-style=short --color=never /tmp/opencode/steer-078/red` — observed 5 `error[sty-rust-078]` findings (e.g. `settings.get_string("persistence.sqlite3.path").unwrap_or_else(|_| "mnemorium.db".to_owned())`, `settings.get_int("…").ok().unwrap_or_else(…)`, and `match settings.get_int("…") { Ok(v) => v as u32, Err(_) => 4 }` / `_ => 4`). The scratch wrapper (`check_sty_rust_078` body in `/tmp/opencode/steer-078/scratch-check.sh` over a fake `src/`) returned `rc=1` with the same 5 diagnostics.
- **Green:** `/tmp/opencode/steer-078/green/{not_found_match.rs,propagate.rs,err_arm_propagates.rs}` — same scan over `/tmp/opencode/steer-078/green` — observed no finding. The fixtures are the repo's compliant idioms: `Err(ConfigError::NotFound(_)) => Ok(default)` with `Err(error) => Err(error)`, `settings.get_int("…")?` propagation, and `Err(error) => Err(error)`. The scratch wrapper returned `rc=0`.
- **Repository:** `devenv shell -- ast-grep scan --rule /tmp/opencode/steer-078/rule/sty-rust-078.yml --report-style=short --color=never src` — observed no finding (`rc=0`). The shipped invocation form (`ast-grep scan --config sgconfig.yaml --filter sty-rust-078 … src`, rule registered under `rule/`) was proven equivalent by running `--filter sty-rust-078` from a temporary sgconfig over `src`: `rc=0`, zero findings, so the current `bootstrap.rs::resolve_sqlite3` and `configuration_source.rs::deserialize` satisfy the rule.

## Next

- Apply: `rule/sty-rust-078.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
