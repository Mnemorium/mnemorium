# Add check — STY-RUST-076

## Rule guard — STY-RUST-076

- **Rule:** "A setting that cannot be read after the pool or the router is built is a **startup-only setting**: it is read once at startup, listed in the registry in [§ 10.1](#101-startup-only-settings), and never silently cached. The registry is extensible: adding a startup-only setting appends it there in the same change, together with how a change is surfaced to the operator." — `docs/development/TechnicalDesign.md`, row `STY-RUST-076` (line 186), section **Configuration**; elaborated in § 10.1 "Startup-only settings" (lines 962–981), whose table is the registry (`src/bin/server.rs:60` is the one live "restart to apply it" warning, `handler/asset.rs:140` the one `TODO(hot-reload)`).
- **Check state:** missing — `STY-RUST-076` has a rulebook row but no `RULES=( … )` entry and no `check_sty_rust_076` function; it is not in `script/rule_enforcer.sh list` (22 entries) and is not covered.
- **Mechanism:** ast-grep + bash — the syntactic half (the configuration paths the pre-pool bootstrap reads) is an ast-grep extraction scoped to `bootstrap.rs`; the Bash wrapper parses the § 10.1 registry table out of the rulebook and relates the two, a document comparison an ast-grep rule cannot make (the `sty-rust-006`/`sty-rust-066` idiom). No `rule_enforcer.yaml` block: the registry is the § 10.1 table itself, the rule's own single source of truth, so adding a duplicate block would create a second one. `sgconfig.yaml` already declares `ruleDirs: [rule]`.

**Scope (what makes the rule mechanical).** A check cannot derive "read before the router is built" from source without false positives — the composition root also reads `security.pepper` and `security.log_root_admin_password`, which are legitimately *not* in the registry. `bootstrap_sqlite3` is the one surface where "cannot be read after the pool is built" holds **by construction**: it runs before the pool exists and reads its settings by string path (`settings.get_string`/`get_int`). The check holds those bootstrap reads to the registry (each unregistered read = a startup-only setting added without its registry row), and holds every registry row to a recorded operator restart feedback, which is the second half of the rule's extensibility clause.

## Files

### `rule/sty-rust-076.yml` (new)

New ast-grep extraction rule (severity `hint`, like `sty-rust-006`/`sty-rust-013`/`sty-rust-023`; extraction rules carry no failure semantics). It captures each `get_string`/`get_int` argument in the bootstrap, skipping a `#[cfg(test)]` module (the `sty-rust-067` idiom). `sgconfig.yaml` registers it automatically; no `sgconfig.yaml` change.

```yaml
---
id: sty-rust-076
language: Rust
severity: hint
message: >-
  extract the configuration paths the pre-pool bootstrap reads, for
  STY-RUST-076
files:
  - src/lib/infrastructure/outbound/config/bootstrap.rs
rule:
  any:
    - pattern: $RECEIVER.get_string($PATH)
    - pattern: $RECEIVER.get_int($PATH)
  not:
    inside:
      kind: mod_item
      stopBy: end
      follows:
        kind: attribute_item
        regex: 'cfg\(test\)'
```

### `script/rule_enforcer.sh`

Two edits; the runner is untouched.

1. Append one registry line to `RULES=( … )`, after the existing `"STY-RUST-024:check_sty_rust_024"` entry (currently the last entry, line 71), before the closing `)` (line 72):

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-076:check_sty_rust_076"
)
```

2. Add the check function immediately after `check_sty_rust_024()` (ends line 752) and before the `# --- Runner ---` divider (line 754):

```bash
check_sty_rust_076() {
	local id="STY-RUST-076"
	local rule_id="sty-rust-076"
	local bootstrap="src/lib/infrastructure/outbound/config/bootstrap.rs"
	local matches path feedback result=0
	local -A registered=()

	if [ ! -f "$bootstrap" ]; then
		printf 'error: %s: missing %s\n' "$id" "$bootstrap" >&2
		return 1
	fi

	# The § 10.1 table is the registry of startup-only settings; parse its rows
	# (setting, operator restart feedback) straight from the rulebook, as
	# read_rulebook parses the rule tables. An empty parse means the registry
	# moved, not that the rule holds.
	while IFS=$'\t' read -r path feedback; do
		[ -n "$path" ] || continue
		registered["$path"]=1
		if [ -z "$feedback" ]; then
			printf 'error: %s: startup-only setting %s records no operator restart feedback\n' \
				"$id" "$path" >&2
			result=1
		fi
	done < <(awk '
		function trim(s) { gsub(/^[[:space:]]+|[[:space:]]+$/, "", s); return s }

		/^##### 10\.1 Startup-only settings/ { in_registry = 1; next }
		in_registry && /^#/ { in_registry = 0 }
		in_registry && /^---/ { in_registry = 0 }
		in_registry && /^\|/ {
			n = split($0, c, "|")
			if (n < 5) next
			setting = trim(c[2]); feedback = trim(c[4])
			gsub(/`/, "", setting)
			if (setting == "Setting" || setting ~ /^-+$/) next
			if (setting !~ /^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$/) next
			printf "%s\t%s\n", setting, feedback
		}
	' "$RULEBOOK")

	if [ "${#registered[@]}" -eq 0 ]; then
		printf 'error: %s: parsed no startup-only setting from %s § 10.1\n' \
			"$id" "$RULEBOOK" >&2
		return 1
	fi

	# sty-rust-076 extracts the configuration paths the pre-pool bootstrap
	# reads. The bootstrap runs before the pool exists, so by construction
	# these settings cannot be read after the pool is built: the rule requires
	# each to be listed in the § 10.1 registry. The document comparison
	# happens here because an ast-grep rule cannot read a Markdown table.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$bootstrap" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no configuration path read from %s\n' "$id" "$bootstrap" >&2
		return 1
	fi

	while IFS= read -r path; do
		[ -n "$path" ] || continue
		if [ -z "${registered[$path]+set}" ]; then
			printf 'error: %s: startup-only setting %s is read before the pool but not listed in %s § 10.1\n' \
				"$id" "$path" "$RULEBOOK" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.PATH.text | startswith("\"")) |
			.metaVariables.single.PATH.text | gsub("^\"|\"$"; "")')

	return "$result"
}
```

Notes: the function runs under the runner's `cd -- "$REPO_ROOT"`, so `$SGCONFIG` (`sgconfig.yaml`), `$RULEBOOK` (`docs/development/TechnicalDesign.md`) and `src` resolve; both are the existing `readonly` globals. It calls only `ast-grep`/`jq`/`grep`-free `awk`, all dev-shell tools, so it adds no install guard. The `awk` restricts to identifier-shaped dotted paths (`persistence.sqlite3.path` style) so the header/separator rows are skipped, and the `jq` filter keeps the comparison to string-literal paths, so a non-literal argument (e.g. `settings.get_string(SOME_CONST)`) is not falsely keyed.

## Verification

Proved with a scratch repo at `/tmp/opencode/STY-RUST-076/` holding the candidate rule (`rule/sty-rust-076.yml`), a `sgconfig.yaml` (`ruleDirs:\n  - rule`), a registry document at `docs/development/TechnicalDesign.md` (the real § 10.1 rows), and fixture sources at the production path. The candidate function is the literal block above in `/tmp/opencode/STY-RUST-076/check.sh`, whose driver `cd`s into the given root, sets `RULEBOOK`/`SGCONFIG`, runs `check_sty_rust_076`, and prints `rc`. All runs were invoked from the repository root via `devenv shell --`.

- **Red (unregistered startup read):** `/tmp/opencode/STY-RUST-076/red` — a copy of the real `bootstrap.rs` with `let _journal_mode = match settings.get_string("persistence.sqlite3.journal_mode") { … }` injected into `resolve_sqlite3`, a path absent from the registry. Command: `devenv shell -- bash /tmp/opencode/STY-RUST-076/check.sh /tmp/opencode/STY-RUST-076/red` — observed:
  ```
  error: STY-RUST-076: startup-only setting persistence.sqlite3.journal_mode is read before the pool but not listed in docs/development/TechnicalDesign.md § 10.1
  rc=1
  ```
- **Red (registry row without operator feedback):** `/tmp/opencode/STY-RUST-076/red_feedback` — the green tree with the `asset.storage.root` row's restart-feedback cell blanked. Same command with `red_feedback` — observed:
  ```
  error: STY-RUST-076: startup-only setting asset.storage.root records no operator restart feedback
  rc=1
  ```
- **Green:** `/tmp/opencode/STY-RUST-076/green` — a byte copy of the real `bootstrap.rs` (reads only `persistence.sqlite3.path` and `persistence.sqlite3.max_connections`) against the unmodified registry. Same command with `green` — observed: no diagnostic, `rc=0`. Direct engine check: `devenv shell -- ast-grep scan --config /tmp/opencode/STY-RUST-076/sgconfig.yaml --filter sty-rust-076 --json=stream <bootstrap>` yields exactly `"persistence.sqlite3.path"` and `"persistence.sqlite3.max_connections"`.
- **Repository:** `SGCONFIG=/tmp/opencode/STY-RUST-076/sgconfig.yaml devenv shell -- bash /tmp/opencode/STY-RUST-076/check.sh /home/will/project/mine/Mnemorium/mnemorium` — observed: no diagnostic, `rc=0`. Both bootstrap reads (`persistence.sqlite3.path`, `persistence.sqlite3.max_connections`) are registry rows, and all nine § 10.1 rows carry a non-empty restart-feedback cell; `rc=0` confirms the candidate rule was loaded from the tmp config (the repository `sgconfig.yaml` has no `sty-rust-076` rule, which would otherwise error). Baseline: `devenv shell -- script/rule_enforcer.sh list | wc -l` → `22`; after applying the two edits the rule is registered and runs in place of its absent entry.

## Next

- Apply: `rule/sty-rust-076.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
