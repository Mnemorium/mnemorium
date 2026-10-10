# Add check — STY-RUST-039

## Rule guard — STY-RUST-039

- **Rule:** "Every repository comes from the **same** unit of work (one `begin()`); opening a second unit of work would be a second transaction and break atomicity." — `docs/development/TechnicalDesign.md`, rulebook row `STY-RUST-039`, section **Unit of Work** (More info: [§ 7.3](#73-cross-context-use-cases)).
- **Check state:** missing — `STY-RUST-039` has no `RULES` entry, no `rule/sty-rust-039.yml`, and `script/rule_enforcer.sh uncovered` prints `STY-RUST-039 (Unit of Work) — …`.
- **Mechanism:** ast-grep extraction rule + bash — the violation is a per-function cardinality/order property over `begin()` call sites that ast-grep cannot express on its own: `rule/sty-rust-039.yml` extracts each `begin()` with the function that encloses it, and the Bash check groups, orders and compares them. No `rule_enforcer_config` block (the check reads no per-rule data).

Interpretation, from the rule's own rationale ("a second transaction … would break atomicity") and the code it constrains: a use case must never hold **two live units of work at once**. A second `begin()` in the same function is a violation unless the previous unit of work was consumed first (`commit`/`rollback`, or a helper wrapping them such as `discard`/`drop`). This is exactly the invariant `complete_upload.rs::execute` establishes and tests — its phase-1 unit of work is consumed before phase 2 opens (`complete_upload_hashes_after_phase_one_closes`, "phase 1 must be closed before the hash"; "phase 2 must begin after the hash") — so the check is green on the current tree, while a same-function second `begin()` with the first still live is red.

## Files

### `rule/sty-rust-039.yml` (new)

New ast-grep rule; registered automatically by `sgconfig.yaml` (`ruleDirs: [rule]`). It is an extraction rule (`severity: hint`), matching every unit-of-work `begin()` call and capturing the enclosing `function_item` as `$ENCLOSING` (its start line is the grouping key) and its name as `$FUNC` (the diagnostic). It carries no `files:` block; the check scopes the scan to `src/lib/application/use_case`, matching the `sty-rust-006` idiom.

```yaml
---
id: sty-rust-039
language: Rust
severity: hint
message: extract unit-of-work begin() call sites for STY-RUST-039
rule:
  pattern: $RECV.begin()
  inside:
    kind: function_item
    pattern: $ENCLOSING
    has:
      field: name
      pattern: $FUNC
    stopBy: end
```

### `script/rule_enforcer.sh`

Two edits.

1. Register the check in `RULES`, appending after `"STY-RUST-024:check_sty_rust_024"` (line 71):

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-039:check_sty_rust_039"
)
```

2. Declare the wrapper in the `# --- Checks ---` section, after `check_sty_rust_024()` (line 752) and before `# --- Runner ---` (line 754):

```bash
check_sty_rust_039() {
	local id="STY-RUST-039"
	local rule_id="sty-rust-039"
	local target="src/lib/application/use_case"
	local matches file func enc key line a b i text result=0
	local -a lines=()
	local -A begins=()
	local -A function_of=()

	if [ ! -d "$target" ]; then
		printf 'error: %s: missing %s\n' "$id" "$target" >&2
		return 1
	fi

	# sty-rust-039 extracts every unit-of-work `begin()` call with the function
	# that encloses it; counting and ordering happen here because an ast-grep
	# rule cannot compare two matches.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$target" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	[ -n "$matches" ] || return 0

	while IFS=$'\t' read -r file enc func line; do
		[ -n "$file" ] || continue
		key="${file}:${enc}"
		begins["$key"]+="${line}"$'\n'
		function_of["$key"]="$func"
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, ((.metaVariables.single.ENCLOSING.range.start.line) | tostring),
			.metaVariables.single.FUNC.text, ((.range.start.line + 1) | tostring)] | @tsv')

	# A function may open more than one unit of work only when every earlier one
	# is consumed before the next opens (commit/rollback, or a helper such as
	# discard/drop that wraps them): two live units of work are two concurrent
	# transactions and break atomicity.
	for key in "${!begins[@]}"; do
		mapfile -t lines < <(printf '%s' "${begins[$key]}" | sort -n)
		if [ "${#lines[@]}" -lt 2 ]; then
			continue
		fi
		file="${key%:*}"
		for ((i = 1; i < ${#lines[@]}; i++)); do
			a="${lines[$((i - 1))]}"
			b="${lines[$i]}"
			text="$(sed -n "$((a + 1)),$((b - 1))p" "$file")"
			if ! printf '%s\n' "$text" |
				grep -Eq '\.(commit|rollback)[[:space:]]*\(\)|(^|[^[:alnum:]_])(discard|drop)[[:space:]]*\('; then
				printf 'error: %s: %s: %s() opens a second unit of work before the previous one is consumed\n' \
					"$id" "$file" "${function_of[$key]}" >&2
				result=1
			fi
		done
	done

	return "$result"
}
```

No `rule_enforcer.yaml` block: the check carries no forward-extensible data.

## Verification

The candidate rule proven below is `/tmp/opencode/sty-rust-039/sg/rules/sty-rust-039.yml` (byte-identical to the proposed `rule/sty-rust-039.yml`); the candidate function is `/tmp/opencode/sty-rust-039/final_function.sh` (byte-identical to the `check_sty_rust_039` above). Runs use a scratch `sgconfig.yaml` (`ruleDirs: [rules]`) at `/tmp/opencode/sty-rust-039/sg/sgconfig.yaml`, so nothing tracked is touched.

- **Red:** `/tmp/opencode/sty-rust-039/red_tree/src/lib/application/use_case/red.rs` — two `begin()` calls in `execute` with nothing between them (the first unit of work is still live):
  `devenv shell -- bash -c "cd /tmp/opencode/sty-rust-039/red_tree && SGCONFIG=/tmp/opencode/sty-rust-039/sg/sgconfig.yaml; source /tmp/opencode/sty-rust-039/final_function.sh; check_sty_rust_039"` — observed: rc=1, `error: STY-RUST-039: src/lib/application/use_case/red.rs: execute() opens a second unit of work before the previous one is consumed`.
- **Red (nested):** `/tmp/opencode/sty-rust-039/red_nested_tree/src/lib/application/use_case/nested.rs` — a second `begin()` in an inner block while the outer unit of work is still live (`red_nested_tree` in the command above) — observed: rc=1, `error: STY-RUST-039: src/lib/application/use_case/nested.rs: execute() opens a second unit of work before the previous one is consumed`.
- **Green:** `/tmp/opencode/sty-rust-039/green_tree/src/lib/application/use_case/green.rs` — the `complete_upload.rs` two-phase shape (phase 1 consumed by `discard(...)` before the phase-2 `begin`), a single-`begin` function, and a function with no unit of work (`green_tree` in the command above) — observed: rc=0, no output.
- **Repository:** `devenv shell -- bash -c "SGCONFIG=/tmp/opencode/sty-rust-039/sg/sgconfig.yaml; source /tmp/opencode/sty-rust-039/final_function.sh; check_sty_rust_039"` (run from the repository root) — observed: rc=0, no finding. The extraction finds 21 `begin()` sites across the 20 use-case functions under `src/lib/application/use_case`; only `complete_upload.rs::execute` has two (lines 188 and 268), and `discard(unit_of_work)`/`unit_of_work.commit()` between them consumes the phase-one unit of work. After applying the two edits, the registered check runs `ast-grep scan --config sgconfig.yaml --filter sty-rust-039 --json=stream src/lib/application/use_case` and returns the same clean result.

## Next

- Apply: `rule/sty-rust-039.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
