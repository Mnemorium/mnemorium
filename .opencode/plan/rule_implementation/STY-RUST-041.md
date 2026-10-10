# Add check — STY-RUST-041

## Rule guard — STY-RUST-041

- **Rule:** `Each accessor borrows `&mut self`, so repositories are requested **sequentially**, never held two at a time.` (section `Unit of Work`; rulebook `docs/development/TechnicalDesign.md` line 151, cross-ref `§ 7.3`)
- **Check state:** missing — no `rule/sty-rust-041.yml`, no registry entry; `script/rule_enforcer.sh uncovered` lists `STY-RUST-041`.
- **Mechanism:** ast-grep — the rule is a syntactic property of the accessor signature (`&self` vs `&mut self`), matched as a `function_item`/`function_signature_item`; no cross-file or filesystem logic is involved.

The check keys on the accessor *shape* (`-> impl <…>Repository + '_`) rather than on the file: the SQLite adapter file also holds the factory's legitimate `async fn begin(&self)` (`src/lib/infrastructure/outbound/sqlx/unit_of_work.rs:114`), so a bare `&self` scan would false-positive. An accessor is exactly the method whose return type is `impl …Repository + '_` (§ 7.2 / `STY-RUST-036`); the `begin` factory method returns `Result<…>` and is excluded. No `rule_enforcer.yaml` block is needed — the check carries no forward-extensible data.

## Files

### `rule/sty-rust-041.yml` (new)

```yaml
---
id: sty-rust-041
language: Rust
severity: error
message: >-
  a unit-of-work accessor must borrow `&mut self` so repositories are requested
  sequentially, never held two at a time
rule:
  all:
    - any:
        - kind: function_item
        - kind: function_signature_item
    - has:
        field: return_type
        regex: "^impl .*Repository [+] '_$"
    - has:
        field: parameters
        has:
          kind: self_parameter
          regex: '^&self$'
```

`function_item` covers an accessor implementation; `function_signature_item` covers the body-less declaration in the context-view trait (e.g. `fn users(&mut self) -> impl UserRepository + '_;`). The `self_parameter` regex `^&self$` matches only a shared borrow — `&mut self` and by-value `self` do not match.

### `script/rule_enforcer.sh`

Two edits.

1. Register the check — append one line to `RULES=( … )` (after the `"STY-RUST-024:check_sty_rust_024"` line, before the closing `)`):

```bash
	"STY-RUST-041:check_sty_rust_041"
```

2. Add the check function — insert immediately before the `# --- Runner ---` comment (after `check_sty_rust_024`):

```bash
check_sty_rust_041() {
	local id="STY-RUST-041"
	local rule_id="sty-rust-041"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}
```

This is the thin-wrapper idiom of `check_sty_rust_004`/`007`/`016`: the runner owns the failure formatting, and the function only forwards `ast-grep`'s diagnostics on violation.

## Verification

- Red: `/tmp/opencode/sty-rust-041/red.rs` (a `UserUnitOfWork` trait and its `SqlxUnitOfWork` impl declaring `fn users(&self) -> impl UserRepository + '_`) — `devenv shell -- ast-grep scan --config /tmp/opencode/sty-rust-041/sgconfig.yaml --filter sty-rust-041 --report-style=short --color=never /tmp/opencode/sty-rust-041/red.rs` — observed: two findings, `red.rs:4:5` (trait signature) and `red.rs:10:5` (impl), `2 error(s) found in code`, exit 1.
- Green: `/tmp/opencode/sty-rust-041/green.rs` (the same trait/impl with `&mut self`, plus a compliant `async fn begin(&self) -> Result<Self::Uow, UnitOfWorkError>`) — same command against `green.rs` — observed: no finding, exit 0. The `begin(&self)` factory method is not flagged.
- Repository: `devenv shell -- ast-grep scan --config /tmp/opencode/sty-rust-041/sgconfig.yaml --filter sty-rust-041 --report-style=short --color=never src` — observed: no finding, exit 0 (all six accessor surfaces — the five `src/lib/domain/port/*_unit_of_work.rs` traits and their `src/lib/infrastructure/outbound/sqlx/unit_of_work.rs` impls — already borrow `&mut self`).
- End-to-end: on a faithful copy at `/tmp/opencode/sty-rust-041/repo2` (real `src` copy, all `rule/*.yml` plus `sty-rust-041.yml`, `rule_enforcer.sh` with both edits) — `devenv shell -- bash /tmp/opencode/sty-rust-041/repo2/script/rule_enforcer.sh check` — observed: `227 rulebook rule(s), 23 registered, 0 failed`; after injecting `fn users(&self)` into the copy's `user_unit_of_work.rs`, observed: `FAIL STY-RUST-041 (Unit of Work) — …` with `src/lib/domain/port/user_unit_of_work.rs:7:5: error[sty-rust-041]`, `1 failed`.
- Lint of the edited harness: `devenv shell -- shellcheck /tmp/opencode/sty-rust-041/rule_enforcer.scratch.sh` — observed: clean, exit 0; `devenv shell -- shfmt -d -ln auto -s /tmp/opencode/sty-rust-041/rule_enforcer.scratch.sh` — observed: no diff, exit 0.

## Next

- Apply: `rule/sty-rust-041.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
