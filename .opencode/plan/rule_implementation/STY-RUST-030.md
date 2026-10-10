# Add check — STY-RUST-030

## Rule guard — STY-RUST-030

- **Rule:** "Prefer data-driven tests with `rstest` when possible." (`STY-RUST-030`, § 5 Testing)
- **Check state:** missing — no `rule/sty-rust-030.yml` and no `check_sty_rust_030` in the `RULES` registry.
- **Mechanism:** ast-grep — the mechanically-decidable violation is a test that hand-rolls a case table: a `for` loop over an inline array of cases whose body asserts. One `rule/sty-rust-030.yml` plus a thin wrapper in `script/rule_enforcer.sh`. No config block: the rule carries no per-check data.

### Files

#### `rule/sty-rust-030.yml`

New ast-grep rule. It matches a `for_expression` that iterates an inline `array_expression` (bare or behind `&`) and whose body contains an `assert*` macro invocation, scoped to a function carrying a `#[test]`, `#[tokio::test]` or `#[rstest]` attribute. A loop over a runtime collection, a setup loop without an assertion, a plain non-table test, and the `#[rstest]`/`#[case]` form all fall outside it.

```yaml
---
id: sty-rust-030
language: Rust
severity: error
message: >-
  a test loops over an inline case list and asserts each case; express the cases
  as rstest #[case] rows instead
rule:
  all:
    - kind: for_expression
    - any:
        - has:
            field: value
            kind: array_expression
        - has:
            field: value
            has:
              kind: array_expression
              stopBy: end
    - has:
        field: body
        has:
          kind: macro_invocation
          regex: 'assert'
          stopBy: end
    - inside:
        kind: function_item
        follows:
          kind: attribute_item
          regex: '#\[([A-Za-z_][A-Za-z0-9_]*::)*(test|rstest)\]'
          stopBy: end
        stopBy: end
```

#### `script/rule_enforcer.sh`

Two additions. Add the registry line after `"STY-RUST-024:check_sty_rust_024"`:

```bash
	"STY-RUST-030:check_sty_rust_030"
```

Add the thin wrapper after `check_sty_rust_024()` (before `# --- Runner`), matching the `check_sty_rust_004` idiom:

```bash
check_sty_rust_030() {
	local id="STY-RUST-030"
	local rule_id="sty-rust-030"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}
```

### Verification

The candidate rule proven below is `/tmp/opencode/ruleguard030/final.yml`, byte-identical to the proposed `rule/sty-rust-030.yml`.

- Red: `/tmp/opencode/ruleguard030/red_loop_table.rs` (a `#[test]` iterating `[("user@example.com", true), ("invalid-email", false)]` with an `assert_eq!` in the loop) — `devenv shell -- ast-grep scan --rule /tmp/opencode/ruleguard030/final.yml --report-style=short --color=never /tmp/opencode/ruleguard030/red_loop_table.rs` — observed: `error[sty-rust-030]` at the `for` on line 5, exit 1. Same result for the `&[…]` form (`red_ref_table.rs`) and for `#[tokio::test]` (`red_tokio_table.rs`).
- Green: `/tmp/opencode/ruleguard030/green_rstest.rs` (the § 5 `#[rstest]`/`#[case]` example) — same command — observed: no finding, exit 0. Also clear: `green_plain_test.rs` (non-table `#[test]`), `green_setup_loop.rs` (loop without an assertion), `green_runtime_loop.rs` (loop over a runtime collection).
- Repository: `devenv shell -- ast-grep scan --config sgconfig.yaml --filter sty-rust-030 --report-style=short --color=never src` (simulated against a `/tmp` copy of the config to avoid writing the tracked rule) — observed: exit 0, no findings. `devenv shell -- script/rule_enforcer.sh uncovered` currently lists `STY-RUST-030 (Testing) — Prefer data-driven tests with `rstest` when possible.`

## Next

- Apply: `rule/sty-rust-030.yml` (new), `script/rule_enforcer.sh` (registry line + `check_sty_rust_030` wrapper)
- Verify: `devenv shell -- ast-grep scan --config sgconfig.yaml --filter sty-rust-030 --report-style=short --color=never src`, then `devenv shell -- script/rule_enforcer.sh`
