# Add check — STY-RUST-075

## Rule guard — STY-RUST-075

- **Rule:** "A use case reaches a third-party dependency only through an outbound port, never by calling it directly; the dependencies listed in [§ 6.6](#66-third-party-dependencies-sit-behind-a-port) are the only exceptions." (`docs/development/TechnicalDesign.md` line 185, section `Ports`; detail § 6.6 "Third-party dependencies sit behind a port", lines 704–726. The rule row exists and is registered, so `validate_registry` accepts the ID.)
- **Check state:** missing — `devenv shell -- script/rule_enforcer.sh uncovered` prints `STY-RUST-075 (Ports) — A use case reaches a third-party dependency only through an outbound port, never by calling it directly; the dependencies listed in … are the only exceptions.`; `list` has no `STY-RUST-075` entry; `rule/sty-rust-075.yml` does not exist; `rule_enforcer.yaml` has no `STY-RUST-075` block.
- **Mechanism:** ast-grep + config — `rule/sty-rust-075.yml` extracts every `use` declaration in `src/lib/application/use_case/` outside a `#[cfg(test)]` module; the Bash check parses each path's root identifier and compares it against the allowed set. The allowed third-party crates (§ 6.6: `anyhow`, `chrono`, `tracing`) live in `rule_enforcer.yaml` because § 6.6 says the list "is extended by adding a bullet — an ordinary documentation change"; the language roots (`std`, `core`, `alloc`, `crate`, `self`, `super`) are fixed and stay in the check. Derive/attribute macros (`thiserror::Error`, `serde::Serialize`, `tokio::test`) are attributes, not `use` declarations, so they are out of scope; test-code `use`s are excluded by the rule's `not: inside: mod_item … cfg(test)` clause. Roots are parsed in Bash because an ast-grep rule cannot read `rule_enforcer.yaml`.

## Files

### `rule/sty-rust-075.yml` (new)

Extraction helper, `severity: hint`, matching the other extraction rules (`sty-rust-019.yml`, `sty-rust-023.yml`). It scans `src/lib/application/use_case` directly (like `check_sty_rust_071` scans `$port_dir`), so no `files:` glob is needed. `sgconfig.yaml` already declares `ruleDirs: [rule]`, so no `sgconfig.yaml` change is needed.

```yaml
---
id: sty-rust-075
language: Rust
severity: hint
message: extract every non-test `use` in an application use case
rule:
  all:
    - kind: use_declaration
    - not:
        inside:
          kind: mod_item
          stopBy: end
          follows:
            kind: attribute_item
            regex: cfg\(test\)
```

### `script/rule_enforcer.sh`

Two edits, no runner change.

1. Register the check in `RULES=(…)` (append after the last entry, `"STY-RUST-024:check_sty_rust_024"` at line 71; entries are tab-indented):

```diff
 	"STY-RUST-024:check_sty_rust_024"
+	"STY-RUST-075:check_sty_rust_075"
 )
```

2. Add the function in the `# --- Checks ---` section, immediately after `check_sty_rust_024` and before the `# --- Runner ---` banner (line 754):

```bash
check_sty_rust_075() {
	local id="STY-RUST-075"
	local rule_id="sty-rust-075"
	local use_case_dir="src/lib/application/use_case"
	local config matches file text root result=0
	local -A allowed=()
	local -A seen=()
	local -a local_roots=(std core alloc crate self super)
	local use_root='^(pub[^[:space:]]*[[:space:]]+)?use[[:space:]]+(::)?([A-Za-z_][A-Za-z0-9_]*)'

	config="$(rule_config "$id")" || return 2

	# The crates a use case may reach directly (TechnicalDesign.md § 6.6) are
	# data: the list grows by adding a bullet there and an entry here.
	while IFS= read -r root; do
		[ -n "$root" ] || continue
		allowed["$root"]=1
	done < <(jq -r '.allowed_crates[]?' <<<"$config")

	if [ "${#allowed[@]}" -eq 0 ]; then
		printf 'error: %s: rule_enforcer.yaml declares no allowed_crates\n' "$id" >&2
		return 2
	fi
	for root in "${local_roots[@]}"; do
		allowed["$root"]=1
	done

	if [ ! -d "$use_case_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$use_case_dir" >&2
		return 1
	fi

	# sty-rust-075 extracts each non-test `use`; parsing the path root and
	# comparing it against the allowed set happens here because an ast-grep
	# rule cannot read rule_enforcer.yaml.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$use_case_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no use declaration from %s\n' "$id" "$use_case_dir" >&2
		return 1
	fi

	while IFS=$'\t' read -r file text; do
		[ -n "$text" ] || continue
		if [[ $text =~ $use_root ]]; then
			root="${BASH_REMATCH[3]}"
		else
			continue
		fi
		if [ -z "${allowed[$root]+set}" ] && [ -z "${seen[$file:$root]+set}" ]; then
			printf 'error: %s: %s: use case reaches third-party crate %s directly; go through an outbound port\n' \
				"$id" "$file" "$root" >&2
			seen["$file:$root"]=1
			result=1
		fi
	done < <(printf '%s\n' "$matches" | jq -r '[.file, .text] | @tsv')

	return "$result"
}
```

### `rule_enforcer.yaml`

Append a `STY-RUST-075` block under `rules:` (after the existing `STY-RUST-002` block, the file's last line). The block is the rule's data: the crates § 6.6 allows directly.

```yaml
  STY-RUST-075:
    allowed_crates:
      - anyhow
      - chrono
      - tracing
```

## Verification

All runs used the candidate rule file verbatim and the exact function above (scratch harness `/tmp/opencode/sty075/harness/check.sh`, which sets `SGCONFIG`/`CONFIG`, defines `rule_config` via `yq`, cds into the fixture root, and runs the function; candidate rule at `/tmp/opencode/sty075/harness/rule/sty-rust-075.yml`, config at `/tmp/opencode/sty075/harness/rule_enforcer.yaml`; fixtures under `/tmp/opencode/sty075/proj/`).

- Red: `/tmp/opencode/sty075/proj/red/src/lib/application/use_case/bad.rs` — imports `use reqwest::Client;` and `use sqlx::SqlitePool;` beside allowed `std`/`chrono`/`crate::` imports, plus a `#[cfg(test)] mod tests` that imports and calls `reqwest`/`tokio` — `devenv shell -- bash /tmp/opencode/sty075/harness/check.sh /tmp/opencode/sty075/proj/red` — observed: `error: STY-RUST-075: src/lib/application/use_case/bad.rs: use case reaches third-party crate reqwest directly; go through an outbound port` and `error: STY-RUST-075: src/lib/application/use_case/bad.rs: use case reaches third-party crate sqlx directly; go through an outbound port`, rc=1. `std`, `chrono` and `crate::` are not flagged, and the test-module `reqwest::Client`/`tokio::test` are not flagged (the `#[cfg(test)]` exclusion works).
- Green: `/tmp/opencode/sty075/proj/green/src/lib/application/use_case/good.rs` — imports only `use std::sync::Arc;`, `use anyhow::Error;`, `use chrono::Utc;`, `use tracing::warn;` and `use crate::domain::port::file_storage::FileStorage;`, with a `#[cfg(test)] mod tests` importing `tracing_subscriber` — `devenv shell -- bash /tmp/opencode/sty075/harness/check.sh /tmp/opencode/sty075/proj/green` — observed: no finding, rc=0. The `anyhow`/`chrono`/`tracing` cases prove the § 6.6 allow-list passes, and the `tracing_subscriber` test import proves test code is exempt.
- Repository: the candidate rule scanned over the real tree across the real `rule_enforcer.yaml` via `/tmp/opencode/sty075/repo_check.sh` (scratch `sgconfig.yaml` with `ruleDirs: [/tmp/opencode/sty075/harness/rule]`) — `devenv shell -- bash /tmp/opencode/sty075/repo_check.sh` — observed: rc=0. A raw extraction over the target (`devenv shell -- ast-grep scan --rule /tmp/opencode/sty075/harness/rule/sty-rust-075.yml --json=stream src/lib/application/use_case`) returns 430 non-test `use` declarations; their roots are `crate` (339), `std` (78), `chrono` (13) — all allowed, so the repository is green. After applying, confirm with `devenv shell -- script/rule_enforcer.sh check -v`.

## Next

- Apply: `rule/sty-rust-075.yml` (new), `script/rule_enforcer.sh`, `rule_enforcer.yaml`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
