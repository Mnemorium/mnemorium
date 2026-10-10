# Add check — STY-RUST-063

## Rule guard — STY-RUST-063

- **Rule:** "The factory only supplies the unit-of-work factory; the use case still opens, commits and rolls back its own unit of work (see [Unit of Work](#7-unit-of-work))." — `docs/development/TechnicalDesign.md`, line 173, section *Use cases*
- **Check state:** missing — `script/rule_enforcer.sh uncovered` lists `STY-RUST-063`; `script/rule_enforcer.sh list` does not (`grep -c` → `0`), and `ast-grep scan --config sgconfig.yaml --filter sty-rust-063` reports "Rule with id 'sty-rust-063' not found in project configuration."
- **Mechanism:** ast-grep — the rule resolves to one syntactic fact (a factory adapter file calls `begin`/`commit`/`rollback`), which a `rule/sty-rust-063.yml` pattern expresses directly; the Bash check only scopes and runs it.

## Files

### `rule/sty-rust-063.yml` (new)

The factory adapter (`src/lib/infrastructure/use_case_factory/<context>.rs`, per `STY-RUST-061`) wires the use case with `Arc::clone(&self.unit_of_work_factory)` and must never call the lifecycle methods itself. `files:` pins the rule to that directory so the pattern cannot match `pool.begin()` in a repository or a use case's own legitimate `begin`/`commit`/`rollback`.

```yaml
---
id: sty-rust-063
language: Rust
severity: error
message: >-
  the use-case factory only supplies the unit-of-work factory; the use case
  opens, commits and rolls back its own unit of work — the factory must not
  call begin/commit/rollback
files:
  - src/lib/infrastructure/use_case_factory/*.rs
rule:
  any:
    - pattern: $UOW.begin()
    - pattern: $UOW.commit()
    - pattern: $UOW.rollback()
```

### `script/rule_enforcer.sh`

One registry line. Add after the existing `"STY-RUST-024:check_sty_rust_024"` entry (currently line 71), so the block ends:

```bash
	"STY-RUST-067:check_sty_rust_067"
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-063:check_sty_rust_063"
)
```

One function, inserted after `check_sty_rust_024` (ends at line 752) and before the `# --- Runner ---` banner (line 754), following the `check_sty_rust_022` idiom (directory guard, `--filter`, short report, diagnostic on failure):

```bash
check_sty_rust_063() {
	local id="STY-RUST-063"
	local rule_id="sty-rust-063"
	local factory_dir="src/lib/infrastructure/use_case_factory"
	local output

	if [ ! -d "$factory_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$factory_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$factory_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}
```

No `rule_enforcer.yaml` block: the check holds no forward-extensible data.

## Verification

Scratch layout (all under `/tmp/opencode/STY-RUST-063/`): `candidate.yml` is byte-identical to the proposed `rule/sty-rust-063.yml`; `mirror/` holds `sgconfig.yaml`, `rule/sty-rust-063.yml` and the violating tree `src/lib/infrastructure/use_case_factory/red.rs`; `mirror/greenonly/` holds the same config plus only the compliant `src/lib/infrastructure/use_case_factory/green.rs`.

- **Red:** `/tmp/opencode/STY-RUST-063/mirror/src/lib/infrastructure/use_case_factory/red.rs` (contains `self.unit_of_work_factory.begin()`, `unit_of_work.commit()`, `unit_of_work.rollback()`) — `devenv shell -- bash -c 'cd /tmp/opencode/STY-RUST-063/mirror && ast-grep scan --rule candidate.yml --report-style=short --color=never src/lib/infrastructure/use_case_factory'` — observed: `error[sty-rust-063]` at `red.rs:11:32`, `12:9`, `13:9` (exit 1). The candidate Bash check run over the same tree also returned `rc=1`.
- **Green:** `/tmp/opencode/STY-RUST-063/mirror/greenonly/src/lib/infrastructure/use_case_factory/green.rs` (only `Arc::new(BeginUpload::new(Arc::clone(&self.unit_of_work_factory)))`; the `begin_upload` accessor name does not match the pattern) — `devenv shell -- bash -c 'cd /tmp/opencode/STY-RUST-063/mirror/greenonly && ast-grep scan --rule candidate.yml --report-style=short --color=never src/lib/infrastructure/use_case_factory'` — observed: no findings (exit 0). The candidate Bash check returned `rc=0`.
- **Repository:** `devenv shell -- ast-grep scan --config /tmp/opencode/STY-RUST-063/repoconfig.yaml --filter sty-rust-063 --report-style=short --color=never src/lib/infrastructure/use_case_factory` — observed: no findings (exit 0); the four current `use_case_factory/*.rs` files call none of `begin`/`commit`/`rollback`. A probe rule carrying the same `files:` glob matched real findings in those files, confirming the glob resolves against the repository root; the pre-change `--filter sty-rust-063` against the repository `sgconfig.yaml` reports the rule is absent (the expected state before this plan is applied).

## Next

- Apply: `rule/sty-rust-063.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
