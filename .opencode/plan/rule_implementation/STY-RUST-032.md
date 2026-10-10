# Add check — STY-RUST-032

## Rule guard — STY-RUST-032

- **Rule:** "`commit` and `rollback` consume the unit of work." (`docs/development/TechnicalDesign.md`, row `STY-RUST-032`, § 7.1 Lifecycle)
- **Check state:** missing — `list` shows no `STY-RUST-032`; `uncovered` lists it.
- **Mechanism:** ast-grep — the rule is a method-signature shape (`self` by value vs. `&self`/`&mut self`), which is exactly a syntactic pattern; wrap it in the standard thin Bash wrapper.

The § 7.1 trait contract is `fn commit(self)` / `fn rollback(self)`. A declaration that borrows (`&self` / `&mut self`) does not consume the unit of work and must be flagged. Both the trait signatures (`function_signature_item`, no body) and the impl methods (`function_item`, with body) declare the lifecycle method, so the rule matches both kinds.

## Files

### `rule/sty-rust-032.yml` (new)

New ast-grep rule. Registered automatically by `sgconfig.yaml` (`ruleDirs: [rule]`); no `sgconfig.yaml` change.

```yaml
---
id: sty-rust-032
language: Rust
severity: error
message: >-
  commit and rollback consume the unit of work: they take `self` by value, never
  `&self` or `&mut self`
files:
  - src/**/*.rs
rule:
  all:
    # A trait declaration without a body is a `function_signature_item`; an impl
    # method with a body is a `function_item`. Both declare the lifecycle method.
    - any:
        - kind: function_item
        - kind: function_signature_item
    - has:
        field: name
        regex: '^(commit|rollback)$'
    # `&self` / `&mut self` borrow the unit of work; only a by-value `self`
    # (or `mut self`) consumes it.
    - has:
        field: parameters
        has:
          kind: self_parameter
          regex: '^&'
```

### `script/rule_enforcer.sh`

Registers the check in `RULES`, appending after `"STY-RUST-024:check_sty_rust_024"` (line 71):

```bash
	"STY-RUST-032:check_sty_rust_032"
```

Declares the wrapper, placed with the other ast-grep wrappers after `check_sty_rust_024` (line 752), before `# --- Runner`:

```bash
check_sty_rust_032() {
	local id="STY-RUST-032"
	local rule_id="sty-rust-032"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}
```

No `rule_enforcer.yaml` block: the check carries no forward-extensible data.

## Verification

- Red (trait signatures) — `/tmp/opencode/sty032/proj/src/red.rs` (`fn commit(&mut self)` and `fn rollback(&self)` in `trait UnitOfWork`):
  `ast-grep scan --config sgconfig.yaml --filter sty-rust-032 --report-style=short --color=never src/red.rs`
  observed: 2 errors, `src/red.rs:2:5` and `src/red.rs:3:5`.
- Red (impl methods) — `/tmp/opencode/sty032/proj2/src/red2.rs` (`async fn commit(&mut self)` and `async fn rollback(&self)` in `impl UnitOfWork`):
  `ast-grep scan --config sgconfig.yaml --filter sty-rust-032 --report-style=short --color=never src/red2.rs`
  observed: 2 errors, `src/red2.rs:2:5` and `src/red2.rs:5:5`.
- Green (trait + impl, by-value receivers) — `/tmp/opencode/sty032/proj/src/green.rs` and `/tmp/opencode/sty032/proj2/src/green2.rs` (`fn commit(self)`, `fn rollback(self)`, `async fn rollback(mut self)`, plus an unrelated `fn commit(x: i32)` and `fn other(&self)`):
  `ast-grep scan --config sgconfig.yaml --filter sty-rust-032 --report-style=short --color=never <fixture>`
  observed: no finding (`rc=0`) in both.
- Repository — candidate rule run against the live tree (surrogate `--rule` + `--globs`, since the rule is not yet installed):
  `ast-grep scan --rule /tmp/opencode/sty032/sty-rust-032.yml --globs 'src/**/*.rs' --report-style=short --color=never src`
  observed: no finding (`rc=0`) — the three `commit`/`rollback` declarations in `src/lib/domain/port/unit_of_work.rs`, `src/lib/infrastructure/outbound/sqlx/unit_of_work.rs`, and `src/lib/lib.rs` all take `self` by value. After applying the plan, the check runs `ast-grep scan --config sgconfig.yaml --filter sty-rust-032 …` and returns the same clean result.

## Next

- Apply: `rule/sty-rust-032.yml` (new), `script/rule_enforcer.sh` (register in `RULES`, add `check_sty_rust_032`)
- Verify: `ast-grep scan --config sgconfig.yaml --filter sty-rust-032 --report-style=short --color=never src` (expect no findings on the current tree; red/green fixtures cited above)
