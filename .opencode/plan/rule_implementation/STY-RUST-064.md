# Add check — STY-RUST-064

## Rule guard — STY-RUST-064

- **Rule:** "`LoadConfiguration` runs once at startup; the resulting `Configuration` is stored as `Arc<ArcSwap<Configuration>>` in `AppState`." (`docs/development/TechnicalDesign.md` line 174, Section `Configuration`, under `## Code Style Guidelines` — a registered section, so `validate_registry` accepts the ID.)
- **Check state:** missing — `script/rule_enforcer.sh uncovered` lists `STY-RUST-064`; it is absent from `RULES` and no `rule/sty-rust-064.yml` exists.
- **Mechanism:** ast-grep + bash — an ast-grep rule captures the compliant `AppState` field and the composition-root `LoadConfiguration` binding; the Bash check applies the presence/file/execute-count logic ast-grep cannot express (positive-rule inversion, path comparison, cross-statement binding→`execute()` relation). No `rule_enforcer.yaml` block: the check has no forward-extensible data.

## Files

### `rule/sty-rust-064.yml` (new)

```yaml
---
id: sty-rust-064
language: Rust
severity: hint
message: >-
  extract the AppState live-configuration field and the LoadConfiguration
  startup binding for STY-RUST-064
rule:
  any:
    # AppState stores the live configuration behind an ArcSwap.
    - all:
        - kind: field_declaration
        - has:
            field: type
            regex: '^Arc<ArcSwap<Configuration>>$'
        - inside:
            kind: field_declaration_list
            inside:
              kind: struct_item
              pattern: 'pub struct AppState { $$$FIELDS }'
              stopBy: end
    # The composition root binds the LoadConfiguration use case before running it.
    - all:
        - pattern: 'let $VAR = $INIT;'
        - has:
            pattern: 'LoadConfiguration::new($$$ARGS)'
            stopBy: end
```

`sgconfig.yaml` already declares `ruleDirs: [rule]`, so no `sgconfig.yaml` change is needed.

### `script/rule_enforcer.sh`

Two edits. First, add one registry line at the end of `RULES` (after `STY-RUST-024`):

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-064:check_sty_rust_064"
)
```

Second, add the function after `check_sty_rust_024()` and before the `# --- Runner ---` banner:

```bash
check_sty_rust_064() {
	local id="STY-RUST-064"
	local rule_id="sty-rust-064"
	local app_state="src/lib/infrastructure/inbound/rest/app_state.rs"
	local server="src/bin/server.rs"
	local matches app_matches file bindings binding_count binding executions result=0

	if [ ! -f "$app_state" ]; then
		printf 'error: %s: missing %s\n' "$id" "$app_state" >&2
		return 1
	fi

	# sty-rust-064 is a positive rule: it matches the compliant `AppState`
	# field and the composition root's `LoadConfiguration` binding. It scans
	# all of `src` so the file comparison below also catches an `AppState`
	# declared in the wrong file.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	# Clause A: AppState stores the live configuration as
	# `Arc<ArcSwap<Configuration>>`. The rule is positive, so a missing match
	# is the violation.
	app_matches="$(printf '%s\n' "$matches" |
		jq -c 'select(.metaVariables.single.VAR | not)')"
	if [ -z "$app_matches" ]; then
		printf 'error: %s: AppState does not store the configuration as Arc<ArcSwap<Configuration>>\n' \
			"$id" >&2
		return 1
	fi
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		if [ "$file" != "$app_state" ]; then
			printf 'error: %s: AppState must be declared in %s (found in %s)\n' \
				"$id" "$app_state" "$file" >&2
			result=1
		fi
	done < <(printf '%s\n' "$app_matches" | jq -r '.file')

	# Clause B: LoadConfiguration runs once at startup. The rule captures the
	# binding the composition root creates; the execute count is read here
	# because an ast-grep rule cannot relate the binding to a later call.
	bindings="$(printf '%s\n' "$matches" |
		jq -r --arg file "$server" \
			'select(.metaVariables.single.VAR and (.file == $file)) | .metaVariables.single.VAR.text')"
	binding_count="$(printf '%s\n' "$bindings" | grep -c .)"
	if [ "$binding_count" -ne 1 ]; then
		printf 'error: %s: %s must bind LoadConfiguration exactly once at startup (found %s)\n' \
			"$id" "$server" "$binding_count" >&2
		result=1
	fi
	if [ "$binding_count" -eq 1 ]; then
		binding="$(printf '%s\n' "$bindings" | head -n 1)"
		executions="$(grep -cF "${binding}.execute(" "$server")"
		if [ "$executions" -ne 1 ]; then
			printf 'error: %s: LoadConfiguration must run exactly once at startup (found %s execute call(s) on %s in %s)\n' \
				"$id" "$executions" "$binding" "$server" >&2
			result=1
		fi
	fi

	return "$result"
}
```

## Verification

All runs used the candidate rule file verbatim and the exact check function above (scratch harness at `/tmp/opencode/sty-rust-064/check_scratch.sh`, which sets `SGCONFIG` and cds into the fixture root; fixtures under `/tmp/opencode/sty-rust-064/proj/`).

- Red (clause A): `/tmp/opencode/sty-rust-064/proj/red_a` — an `AppState { configuration: Arc<Configuration> }` and a single `LoadConfiguration` binding — `devenv shell -- bash /tmp/opencode/sty-rust-064/check_scratch.sh /tmp/opencode/sty-rust-064/proj/red_a` — observed: `error: STY-RUST-064: AppState does not store the configuration as Arc<ArcSwap<Configuration>>` / `FAIL`.
- Red (clause B): `/tmp/opencode/sty-rust-064/proj/red_b` — correct `Arc<ArcSwap<Configuration>>` field but `load_configuration.execute()` twice — same command with `red_b` — observed: `error: STY-RUST-064: LoadConfiguration must run exactly once at startup (found 2 execute call(s) on load_configuration in src/bin/server.rs)` / `FAIL`.
- Red (wrong file): `/tmp/opencode/sty-rust-064/proj/red_c` — compliant field declared in `src/bin/server.rs` — same command with `red_c` — observed: `error: STY-RUST-064: AppState must be declared in src/lib/infrastructure/inbound/rest/app_state.rs (found in src/bin/server.rs)` / `FAIL`.
- Red (two bindings): `/tmp/opencode/sty-rust-064/proj/red_d` — two `LoadConfiguration` bindings in `src/bin/server.rs` — same command with `red_d` — observed: `error: STY-RUST-064: src/bin/server.rs must bind LoadConfiguration exactly once at startup (found 2)` / `FAIL`.
- Green: `/tmp/opencode/sty-rust-064/proj/green` — `AppState { configuration: Arc<ArcSwap<Configuration>> }` and one `load_configuration.execute()` — same command with `green` — observed: `PASS: /tmp/opencode/sty-rust-064/proj/green`.
- Repository: `SGCONFIG=/tmp/opencode/sty-rust-064/ruleonly/sgconfig.yaml devenv shell -- bash /tmp/opencode/sty-rust-064/check_scratch.sh` (run from the repository root; `ruleonly/sgconfig.yaml` is `ruleDirs: [rule]` over the candidate rule) — observed: `PASS: repo`. Raw scan of the candidate rule over `src` returns exactly two matches: `src/bin/server.rs:47` binding `load_configuration` and `src/lib/infrastructure/inbound/rest/app_state.rs:25` field `configuration`; `devenv shell -- ast-grep scan --config /tmp/opencode/sty-rust-064/ruleonly/sgconfig.yaml --filter sty-rust-064 --json=stream src` reports no violation.

## Next

- Apply: `rule/sty-rust-064.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
