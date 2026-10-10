# Add check — STY-RUST-033

## Rule guard — STY-RUST-033

- **Rule:** "A rollback failure returns the **original** business error." (`docs/development/TechnicalDesign.md`, row `STY-RUST-033`, § 7.1 Lifecycle, category **Unit of Work**).
- **Check state:** missing — `STY-RUST-033` has no registry line and no `check_sty_rust_033`; `script/rule_enforcer.sh uncovered` lists it (`STY-RUST-033 (Unit of Work) — A rollback failure returns the **original** business error.`).
- **Mechanism:** ast-grep (+ thin Bash wrapper) — the violation is syntactic: the `?` operator applied to an expression whose subtree contains a `rollback()` method call, i.e. the `UnitOfWorkError` propagating instead of the bound business error. The lifecycle shape (`if … .is_err()`, `let _ =`, `if let Err(_e) =`) contains no `?`, so it is not flagged. The rule file is self-contained; no `rule_enforcer.yaml` block is needed.

The § 7.1 canonical illustration returns the original error on the failure path — `let _ = unit_of_work.rollback().await; Err(error)` — and the unit-of-work adapter logs the rollback failure (`OBS-002`). A `rollback()` result propagated with `?` surfaces the `UnitOfWorkError` instead of the business error and must be flagged.

## Files

### `rule/sty-rust-033.yml` (new)

New ast-grep rule. Registered automatically by `sgconfig.yaml` (`ruleDirs: [rule]`); no `sgconfig.yaml` change.

```yaml
---
id: sty-rust-033
language: Rust
severity: error
message: >-
  a rollback failure must return the original business error; discard the
  rollback result (OBS-002 logs it), never propagate it with `?`
rule:
  kind: try_expression
  has:
    kind: field_identifier
    regex: '^rollback$'
    stopBy: end
```

`has` matches descendants only with `stopBy: end`; the method name is nested (`try_expression → await_expression|call_expression → … → field_identifier`). This covers both `unit_of_work.rollback().await?` and `unit_of_work.rollback().await.map_err(...)?`. Scoping is applied by the wrapper's `--globs`, so this rule is directly runnable with `--rule` on a fixture (a `files:` block under `--rule` does not resolve against the scan path).

### `script/rule_enforcer.sh`

Registers the check in `RULES`, appending after `"STY-RUST-024:check_sty_rust_024"`:

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-033:check_sty_rust_033"
)
```

Declares the wrapper, placed with the other ast-grep wrappers after `check_sty_rust_024`, before `# --- Runner ---`:

```bash
check_sty_rust_033() {
	local id="STY-RUST-033"
	local rule_id="sty-rust-033"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs 'src/lib/application/use_case/**/*.rs' \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}
```

No `rule_enforcer.yaml` change: the check reads no per-rule data.

## Verification

- **Red:** `/tmp/opencode/sg033/probe.rs` (two violating functions: `uow.rollback().await?` at line 2, `uow.rollback().await.map_err(|e| …)?` at line 7) — `devenv shell -- ast-grep scan --rule /tmp/opencode/sg033/sty-rust-033.yml --report-style=short --color=never /tmp/opencode/sg033/probe.rs` — observed: two `error[sty-rust-033]` findings at `probe.rs:2:5` and `probe.rs:7:5`.
- **Green:** `/tmp/opencode/sg033/repo/src/lib/application/use_case/green.rs` (compliant `if … .is_err()`, `if let Err(_e) = …`, `let _ = …`) — same command on that fixture — observed: no finding.
- **Exact form (red/green):** scratch `sgconfig.yaml` + `--filter` + `--globs`, function verbatim in `/tmp/opencode/sg033/check033.sh` — red mirror printed `src/lib/application/use_case/red.rs:2:5: error[sty-rust-033]` and `rc=1`; green mirror printed `rc=0`.
- **Repository:** `devenv shell -- ast-grep scan --config /tmp/opencode/sg033/sgconfig.yaml --filter sty-rust-033 --globs 'src/lib/application/use_case/**/*.rs' --report-style=short --color=never src` — observed: `rc=0`, 0 bytes stdout (all 23 current use cases discard the rollback result; the sole `rollback` mention inside a test module, `begin_upload.rs:656`, is a comment, not a call).

Note on boundary (not a separate rule): the check flags the rollback error **returned/propagated** via `?`; it does not flag `.unwrap()`/`.expect()` (panic, not a return), which no current rule covers.

## Next

- Apply: `rule/sty-rust-033.yml` (new), `script/rule_enforcer.sh` (registry entry + `check_sty_rust_033()`)
- Verify: `devenv shell -- script/rule_enforcer.sh check STY-RUST-033` — red on the two violating fixtures, green on the compliant fixture and the current `src` tree
