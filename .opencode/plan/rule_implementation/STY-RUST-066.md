# Add check — STY-RUST-066

## Rule guard — STY-RUST-066

- **Rule:** "Keep the bootstrap sources and their order identical to `ConfigConfigurationSource`." (`docs/development/TechnicalDesign.md` § Rule tables, Configuration section; detail § 10, lines 935–947.)
- **Check state:** missing — `STY-RUST-066` has a rulebook row (line 176, section `Configuration`) but no `RULES=( … )` entry, no `check_sty_rust_066` function, and appears in `script/rule_enforcer.sh uncovered`.
- **Mechanism:** ast-grep + bash — an ast-grep rule extracts, in document order, each `.add_source(...)` argument of the two production chains; a Bash check relates the two files (an ast-grep rule cannot order or compare nodes across files).

The rule constrains two chains. `ConfigConfigurationSource::load` (`src/lib/infrastructure/outbound/config/configuration_source.rs`, lines 43–52) adds `DbSqlite3Source::new(base)`, then `File::with_name(USER_CONFIG_PATH).required(false)`, then the `Environment::with_prefix("mnemorium")…`, in that order. `bootstrap_sqlite3` (`src/lib/infrastructure/outbound/config/bootstrap.rs`, lines 35–43) adds only the file and the environment — it runs before the pool exists, so the datastore base layer is the one source it cannot build (§ 10: "reads the file and the environment only … keep the source list and order identical"). The check compares the bootstrap's chain to the reference's chain with the `DbSqlite3Source` layer removed, requiring the same entries in the same order.

## Files

### `rule/sty-rust-066.yml` (new)

The extraction rule: one match per `add_source` argument, scoped to the two production functions (which excludes both files' `#[cfg(test)]` in-memory `Config::builder()` helpers).

```yaml
---
id: sty-rust-066
language: Rust
severity: hint
message: >-
  extract the configuration sources added by ConfigConfigurationSource::load
  and bootstrap_sqlite3, for STY-RUST-066
files:
  - src/lib/infrastructure/outbound/config/configuration_source.rs
  - src/lib/infrastructure/outbound/config/bootstrap.rs
rule:
  all:
    - pattern: $RECEIVER.add_source($SOURCE)
    - any:
        - inside:
            kind: function_item
            pattern: pub fn bootstrap_sqlite3($$$A) -> $$$RET { $$$BODY }
            stopBy: end
        - inside:
            kind: function_item
            pattern: fn load($$$A) -> $$$RET { $$$BODY }
            stopBy: end
```

`sgconfig.yaml` already declares `ruleDirs: [rule]`, so no change is needed there.

### `script/rule_enforcer.sh`

Two edits: one registry line and one function. The ID already has its rulebook row, so `validate_registry` accepts it. No `rule_enforcer.yaml` block is added — the check reads no per-check data.

**Edit 1 — registry** (in `RULES=( … )`, insert after the `STY-RUST-067` line, lines 70–71):

```bash
	"STY-RUST-067:check_sty_rust_067"
	"STY-RUST-066:check_sty_rust_066"
	"STY-RUST-024:check_sty_rust_024"
```

**Edit 2 — check function** (insert immediately after the `check_sty_rust_067` function, before `check_sty_rust_024`):

```bash
check_sty_rust_066() {
	local id="STY-RUST-066"
	local rule_id="sty-rust-066"
	local reference="src/lib/infrastructure/outbound/config/configuration_source.rs"
	local bootstrap="src/lib/infrastructure/outbound/config/bootstrap.rs"
	local matches row file offset source result=0 i n
	local -a rows=() ref_sources=() boot_sources=() expected=()

	for file in "$reference" "$bootstrap"; do
		if [ ! -f "$file" ]; then
			printf 'error: %s: missing %s\n' "$id" "$file" >&2
			return 1
		fi
	done

	# sty-rust-066 extracts each `.add_source(...)` argument from the two
	# production chains; the text-and-order comparison happens here because an
	# ast-grep rule cannot relate two files.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$reference" "$bootstrap" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no configuration source\n' "$id" >&2
		return 1
	fi

	mapfile -t rows < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.SOURCE) |
			[.file,
			 (.metaVariables.single.SOURCE.range.byteOffset.start | tostring),
			 (.metaVariables.single.SOURCE.text | gsub("\\s+"; " ") | sub("^ "; "") | sub(" $"; ""))] | @tsv' |
		sort -t$'\t' -k1,1 -k2,2n)

	for row in "${rows[@]}"; do
		IFS=$'\t' read -r file offset source <<<"$row"
		[ -n "$source" ] || continue
		if [ "$file" = "$reference" ]; then
			ref_sources+=("$source")
		elif [ "$file" = "$bootstrap" ]; then
			boot_sources+=("$source")
		fi
	done

	if [ "${#ref_sources[@]}" -eq 0 ] || [ "${#boot_sources[@]}" -eq 0 ]; then
		printf 'error: %s: extracted no configuration source from %s or %s\n' \
			"$id" "$reference" "$bootstrap" >&2
		return 1
	fi

	# The datastore base layer is the one source the bootstrap cannot build:
	# it runs before the pool exists. Every other source must match, in order.
	for source in "${ref_sources[@]}"; do
		case "$source" in
		DbSqlite3Source*) ;;
		*) expected+=("$source") ;;
		esac
	done

	if [ "${#boot_sources[@]}" -ne "${#expected[@]}" ]; then
		printf 'error: %s: bootstrap adds %s file/environment source(s); ConfigConfigurationSource duplicates %s\n' \
			"$id" "${#boot_sources[@]}" "${#expected[@]}" >&2
		result=1
	fi

	n="${#expected[@]}"
	[ "${#boot_sources[@]}" -lt "$n" ] && n="${#boot_sources[@]}"
	for ((i = 0; i < n; i++)); do
		if [ "${boot_sources[$i]}" != "${expected[$i]}" ]; then
			printf 'error: %s: bootstrap source %s is %s; ConfigConfigurationSource has %s\n' \
				"$id" "$((i + 1))" "${boot_sources[$i]}" "${expected[$i]}" >&2
			result=1
		fi
	done

	return "$result"
}
```

## Verification

Proved with a scratch repo root at `/tmp/opencode/sty-rust-066/repo` holding the candidate rule at `rule/sty-rust-066.yml`, a one-line `sgconfig.yaml` (`ruleDirs:\n  - rule`), and fixture sources at the two production paths; the candidate function lives in `/tmp/opencode/sty-rust-066/check.sh`, run from the repository with the fixture root as its argument (`bash check.sh <root>`).

- Red: `/tmp/opencode/sty-rust-066/repo/src/lib/infrastructure/outbound/config/bootstrap.rs` with the bootstrap's `File` and `Environment` sources swapped (Environment added first, File second) — `devenv shell -- bash /tmp/opencode/sty-rust-066/check.sh /tmp/opencode/sty-rust-066/repo` — observed:
  ```
  error: STY-RUST-066: bootstrap source 1 is Environment::with_prefix("mnemorium") .separator("__") .try_parsing(true) .ignore_empty(true); ConfigConfigurationSource has File::with_name(USER_CONFIG_PATH).required(false)
  error: STY-RUST-066: bootstrap source 2 is File::with_name(USER_CONFIG_PATH).required(false); ConfigConfigurationSource has Environment::with_prefix("mnemorium") .separator("__") .try_parsing(true) .ignore_empty(true)
  exit: 1
  ```
  (A second red, changing the bootstrap prefix to `with_prefix("mnemo")`, likewise flagged source 2 and exited 1.)
- Green: the same `bootstrap.rs` with the sources in reference order (File first, Environment second) — `devenv shell -- bash /tmp/opencode/sty-rust-066/check.sh /tmp/opencode/sty-rust-066/repo` — observed: no diagnostic, `exit: 0`.
- Repository: `/tmp/opencode/sty-rust-066/real-repo` symlinks the live `configuration_source.rs` and `bootstrap.rs` at their real paths under a root carrying the candidate rule and `sgconfig.yaml` — `devenv shell -- bash /tmp/opencode/sty-rust-066/check.sh /tmp/opencode/sty-rust-066/real-repo` — observed: no diagnostic, `exit: 0`. Extraction from the live files yields `configuration_source.rs` = [`DbSqlite3Source::new(base)`, `File::with_name(USER_CONFIG_PATH).required(false)`, `Environment::with_prefix("mnemorium") .separator("__") .try_parsing(true) .ignore_empty(true)`] and `bootstrap.rs` = [`File::with_name(USER_CONFIG_PATH).required(false)`, `Environment::with_prefix("mnemorium") .separator("__") .try_parsing(true) .ignore_empty(true)`], so the rule holds today.

## Next

- Apply: `rule/sty-rust-066.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
