# Add check — STY-RUST-043

## Rule guard — STY-RUST-043

- **Rule:** "Declare the `UnitOfWorkFactory` trait second in `domain/port/unit_of_work.rs`." (`docs/development/TechnicalDesign.md`, § 7.4 Declaration order in the unit-of-work port file)
- **Check state:** missing — the registry has no `STY-RUST-043` entry and `uncovered` lists it; no `rule/sty-rust-043.yml` exists.
- **Mechanism:** ast-grep + bash — the rule is a *relative position* of two declarations, which an ast-grep rule cannot express; the harness idiom (`check_sty_rust_013`, `check_sty_rust_015`) extracts the syntactic items with ast-grep and compares their order in Bash. No `rule_enforcer.yaml` block is needed.

## Files

### `rule/sty-rust-043.yml` (new)

Extracts every top-level `trait_item` in the target file and captures its name. `files:` scopes the rule to the one governed file; being a `hint` extractor it emits findings for the Bash layer, it is not itself the verdict.

```yaml
---
id: sty-rust-043
language: Rust
severity: hint
message: extract the trait declarations of the unit-of-work port file
files:
  - src/lib/domain/port/unit_of_work.rs
rule:
  kind: trait_item
  pattern: pub trait $NAME
```

### `script/rule_enforcer.sh` (edit)

Add the function to the Checks section (after `check_sty_rust_024()`, before `# --- Runner ---`):

```bash
check_sty_rust_043() {
	local id="STY-RUST-043"
	local rule_id="sty-rust-043"
	local target="src/lib/domain/port/unit_of_work.rs"
	local matches line name second="" count=0 result=0

	if [ ! -f "$target" ]; then
		printf 'error: %s: missing %s\n' "$id" "$target" >&2
		return 1
	fi

	# sty-rust-043 extracts every top-level trait declaration in the
	# unit-of-work port file; the "second" position is compared here because
	# an ast-grep rule cannot relate two declarations.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$target" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi

	while IFS=$'\t' read -r line name; do
		[ -n "$name" ] || continue
		count=$((count + 1))
		if [ "$count" -eq 2 ]; then
			second="$name"
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r '[.range.start.line, .metaVariables.single.NAME.text] | @tsv' | sort -n)

	if [ "$count" -lt 2 ] || [ "$second" != "UnitOfWorkFactory" ]; then
		printf 'error: %s: %s must declare UnitOfWorkFactory as the second trait\n' \
			"$id" "$target" >&2
		result=1
	fi

	return "$result"
}
```

Add one registry line inside `RULES=( ... )`, after `"STY-RUST-024:check_sty_rust_024"`:

```bash
	"STY-RUST-043:check_sty_rust_043"
```

## Verification

Scratch mirror used for both runs: `/tmp/opencode/sty-rust-043/repo` holds the candidate `sgconfig.yaml` (`ruleDirs: [rule]`), the candidate `rule/sty-rust-043.yml`, and `src/lib/domain/port/unit_of_work.rs`; `/tmp/opencode/sty-rust-043/check.sh` holds the candidate function verbatim and runs it with cwd at the mirror root.

- **Red:** `/tmp/opencode/sty-rust-043/bad_order.rs` — `UnitOfWorkFactory` declared **first**, `UnitOfWork` second — command `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-043/repo && bash /tmp/opencode/sty-rust-043/check.sh'` — observed: `exit=1`, `error: STY-RUST-043: src/lib/domain/port/unit_of_work.rs must declare UnitOfWorkFactory as the second trait`.
- **Red (additional cases):** `missing.rs` (only `UnitOfWork`) and `factory_third.rs` (`UnitOfWork`, `Other`, `UnitOfWorkFactory`) — same command — observed: `exit=1` each with the same diagnostic.
- **Green:** `/tmp/opencode/sty-rust-043/good.rs` (`UnitOfWork` then `UnitOfWorkFactory`) — same command — observed: `exit=0`, no output. `/tmp/opencode/sty-rust-043/doc_comment.rs` (the text `pub trait UnitOfWorkFactory` inside a `///` doc comment, real declarations in order) — observed: `exit=0`, no output (the doc comment is not a `trait_item`).
- **Repository:** `devenv shell -- ast-grep scan --rule /tmp/opencode/sty-rust-043/sty-rust-043.yml --json=stream src/lib/domain/port/unit_of_work.rs` — observed: `9	UnitOfWork` and `18	UnitOfWorkFactory`, i.e. the second declaration is `UnitOfWorkFactory`; the candidate check over a copy of that file returned `exit=0`.

## Next

- Apply: `rule/sty-rust-043.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
