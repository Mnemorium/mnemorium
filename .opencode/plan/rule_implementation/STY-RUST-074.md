# Add check — STY-RUST-074

## Rule guard — STY-RUST-074

- **Rule:** "The `logging` section drives the runtime logs; install the subscriber once, after `LoadConfiguration`, because the configuration lives behind the datastore." (`docs/development/TechnicalDesign.md` line 184, Section `Configuration` — a registered section, so `validate_registry` accepts the ID; detail at § 10, lines 949–957.)
- **Check state:** missing — `STY-RUST-074` has a rulebook row (line 184) and appears in `script/rule_enforcer.sh uncovered`, but it is absent from `RULES=( … )` and no `rule/sty-rust-074.yml` exists.
- **Mechanism:** ast-grep + bash — an ast-grep rule extracts the `logging::setup(...)` install call and the composition root's `LoadConfiguration` binding; the Bash check applies the count, file and cross-statement ordering logic an ast-grep rule cannot see (how many call sites exist, which file each is in, and whether one statement precedes another). No `rule_enforcer.yaml` block: the check has no forward-extensible data.

## Files

### `rule/sty-rust-074.yml` (new)

```yaml
---
id: sty-rust-074
language: Rust
severity: hint
message: >-
  extract the logging::setup install call and the LoadConfiguration startup
  binding for STY-RUST-074
rule:
  any:
    # The single subscriber install site: logging::setup(...).
    - pattern: 'logging::setup($$$ARGS)'
    # The composition root binds the LoadConfiguration use case before running it.
    - all:
        - pattern: 'let $VAR = $INIT;'
        - has:
            pattern: 'LoadConfiguration::new($$$ARGS)'
            stopBy: end
```

`sgconfig.yaml` already declares `ruleDirs: [rule]`, so no `sgconfig.yaml` change is needed. This is an extraction rule (`severity: hint`): the check reads its matches, so the rule itself reports nothing on its own.

### `script/rule_enforcer.sh`

Two edits. First, add one registry line at the end of `RULES` (after the `STY-RUST-024` line, current lines 70–72):

```bash
	"STY-RUST-067:check_sty_rust_067"
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-074:check_sty_rust_074"
)
```

Second, add the function after `check_sty_rust_024()` (current lines 741–752) and before the `# --- Runner ---` banner (line 754):

```bash
check_sty_rust_074() {
	local id="STY-RUST-074"
	local rule_id="sty-rust-074"
	local server="src/bin/server.rs"
	local matches installs install_count install_file var
	local install_line execute_line result=0

	if [ ! -f "$server" ]; then
		printf 'error: %s: missing %s\n' "$id" "$server" >&2
		return 1
	fi

	# sty-rust-074 extracts the `logging::setup` install call and the
	# composition root's `LoadConfiguration` binding; the count, path and
	# ordering logic happens here because an ast-grep rule cannot see how many
	# call sites exist, which file they are in, or the order of two statements.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi

	# Clause A: the subscriber is installed exactly once, by `logging::setup`,
	# from the composition root. A match carrying no VAR is an install call.
	installs="$(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.VAR | not) | [.file, (.range.start.line | tostring)] | @tsv')"
	install_count="$(printf '%s\n' "$installs" | grep -c .)"
	if [ "$install_count" -ne 1 ]; then
		printf 'error: %s: the subscriber must be installed exactly once via logging::setup (found %s)\n' \
			"$id" "$install_count" >&2
		return 1
	fi
	IFS=$'\t' read -r install_file _ <<<"$installs"
	if [ "$install_file" != "$server" ]; then
		printf 'error: %s: the subscriber must be installed from %s (found in %s)\n' \
			"$id" "$server" "$install_file" >&2
		return 1
	fi

	# Clause B: the install runs after LoadConfiguration. The binding variable
	# is read here so the check does not hard-code the local name.
	var="$(printf '%s\n' "$matches" |
		jq -r --arg file "$server" \
			'select(.metaVariables.single.VAR and (.file == $file)) | .metaVariables.single.VAR.text' |
		head -n 1)"
	if [ -z "$var" ]; then
		printf 'error: %s: %s binds no LoadConfiguration use case\n' "$id" "$server" >&2
		return 1
	fi
	execute_line="$(grep -nF "${var}.execute(" "$server" | head -n 1 | cut -d: -f1)"
	install_line="$(grep -nF "logging::setup(" "$server" | head -n 1 | cut -d: -f1)"
	if [ -z "$execute_line" ]; then
		printf 'error: %s: %s never runs %s.execute()\n' "$id" "$server" "$var" >&2
		return 1
	fi
	if [ -z "$install_line" ] || [ "$install_line" -le "$execute_line" ]; then
		printf 'error: %s: logging::setup must run after LoadConfiguration (%s.execute at line %s)\n' \
			"$id" "$var" "$execute_line" >&2
		result=1
	fi

	return "$result"
}
```

## Verification

All runs used the candidate rule and the exact check function above. Scratch harness at `/tmp/opencode/sty-rust-074/check_scratch.sh` (sets `SGCONFIG="sgconfig.yaml"`, `cd`s into its root argument, runs `check_sty_rust_074`); each fixture root carries `sgconfig.yaml` (`ruleDirs:\n  - rule`), `rule/sty-rust-074.yml`, and `src/`. Invoked from the repository root as `devenv shell -- bash /tmp/opencode/sty-rust-074/check_scratch.sh <root>`.

- Red (install twice): `/tmp/opencode/sty-rust-074/red_a` — `src/bin/server.rs` calls `logging::setup(...)` twice after `load_configuration.execute()` — observed: `error: STY-RUST-074: the subscriber must be installed exactly once via logging::setup (found 2)` / `FAIL (rc=1)`.
- Red (install before load): `/tmp/opencode/sty-rust-074/red_b` — one `logging::setup(...)` placed before `load_configuration.execute()` — observed: `error: STY-RUST-074: logging::setup must run after LoadConfiguration (load_configuration.execute at line 9)` / `FAIL (rc=1)`.
- Red (install in the wrong file): `/tmp/opencode/sty-rust-074/red_c` — the sole `logging::setup(...)` lives in `src/lib/infrastructure/other.rs` — observed: `error: STY-RUST-074: the subscriber must be installed from src/bin/server.rs (found in src/lib/infrastructure/other.rs)` / `FAIL (rc=1)`.
- Green: `/tmp/opencode/sty-rust-074/green` — one `load_configuration.execute()` then one `logging::setup(...)` in `src/bin/server.rs` — observed: `PASS: /tmp/opencode/sty-rust-074/green` / exit 0.
- Repository: `/tmp/opencode/sty-rust-074/real-repo` symlinks the live `src` at its real path under a root carrying the candidate rule and `sgconfig.yaml`; `devenv shell -- bash /tmp/opencode/sty-rust-074/check_scratch.sh /tmp/opencode/sty-rust-074/real-repo` — observed: `PASS`. The check's own scan, `ast-grep scan --config sgconfig.yaml --filter sty-rust-074 --json=stream src`, returns exactly two matches from the live tree: `src/bin/server.rs:47` binding `load_configuration` (`LoadConfiguration::new`) and `src/bin/server.rs:55` install `logging::setup`; no other `logging::setup` call exists in `src/`.

## Next

- Apply: `rule/sty-rust-074.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
