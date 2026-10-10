# Add check — STY-RUST-029

## Rule guard — STY-RUST-029

- **Rule:** "Keep tests deterministic and linear: no control flow (`if`, `match`, loops) inside a test function." (§ 5, Testing)
- **Check state:** missing — `uncovered` lists STY-RUST-029; there is no `rule/sty-rust-029.yml`, no registry entry, and no `check_sty_rust_029`. (Its testing siblings STY-RUST-026/027/028/030/031 are also uncovered, so there is no sibling testing-rule convention — the general `STY-RUST-*` ast-grep convention applies.)
- **Mechanism:** ast-grep — the rule is a syntactic containment (control-flow node inside a `#[test]`/`#[rstest]`/`#[tokio::test]` function), so it is one pattern matched over `src`, run by a thin wrapper. No `rule_enforcer.yaml` block (no per-check data) and no `sgconfig.yaml` change (`ruleDirs: [rule]` already covers it).

### Design note (attribute binding)

`#[test]`/`#[rstest]` is a **sibling** of the function, not a child, so the rule uses `follows` on the `function_item`. A naive `follows: {stopBy: end}` crosses the preceding `function_item` and inherits its neighbour's `#[test]` — a non-test helper placed after a test is then falsely flagged (proven below). `stopBy: {kind: function_item}` stops the backward search at the previous function, fixing it. Control flow is matched anywhere in the body, including inside nested closures; on the current tree that interpretation flags 6 tests (see Repository).

## Files

### `rule/sty-rust-029.yml` (new)

```yaml
---
id: sty-rust-029
language: Rust
severity: error
message: >-
  a test function keeps control flow out of its body: no `if`, `match`, or loops
files:
  - src/**/*.rs
rule:
  kind: function_item
  all:
    # A test attribute precedes the function as a sibling, so `follows` — not
    # `has` — reaches it. `stopBy` the previous function keeps a non-test item
    # from inheriting its neighbour's #[test]; `stopBy: end` would cross that
    # boundary.
    - follows:
        kind: attribute_item
        regex: '#\[(rstest|([a-z0-9_]+::)*test)\b'
        stopBy:
          kind: function_item
    # Control flow anywhere in the test body, including nested closures.
    - has:
        stopBy: end
        any:
          - kind: if_expression
          - kind: match_expression
          - kind: loop_expression
          - kind: while_expression
          - kind: for_expression
```

### `script/rule_enforcer.sh`

Add the check to the registry, after the `STY-RUST-024` line:

```bash
	"STY-RUST-067:check_sty_rust_067"
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-029:check_sty_rust_029"
)
```

Add the check function in the `# --- Checks ---` section, after `check_sty_rust_024` (same thin-wrapper idiom):

```bash
check_sty_rust_029() {
	local id="STY-RUST-029"
	local rule_id="sty-rust-029"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}
```

## Verification

Fixtures and the rule body live under `/tmp/opencode/sty-rust-029/` (scratch only).

- **Red:** `/tmp/opencode/sty-rust-029/red.rs` — one `#[tokio::test]` with `if`, one `#[rstest]` with `for`, one `#[test]` with `match` — `devenv shell -- ast-grep scan --rule /tmp/opencode/sty-rust-029/candidate2.yml --report-style=short --color=never /tmp/opencode/sty-rust-029/red.rs` — observed: 3 findings (`red.rs:5`, `:14`, `:21`), exit 1.
- **Green:** `/tmp/opencode/sty-rust-029/green.rs` — two linear tests plus a branching non-test helper after a test — same command — observed: no findings, exit 0.
- **Adversarial (guards the `stopBy` design):** `/tmp/opencode/sty-rust-029/sibling.rs` — a `#[test]` function followed by a helper containing `if` — same command — observed: no findings, exit 0. With the naive `stopBy: end` variant (`candidate.yml`) this helper is falsely flagged, which is why `stopBy: {kind: function_item}` is required.
- **Repository:** `devenv shell -- ast-grep scan --rule /tmp/opencode/sty-rust-029/rules/sty-rust-029.yml --report-style=short --color=never src` (final rule body, including `files:`) — observed: 6 findings, exit 1: `upload_repository.rs:439` and `:498`, `get_user.rs:292`, `update_user.rs:433`, `handler.rs:239`, `complete_upload.rs:1501`. Each is control flow literally inside a test body (the `if`/closure branches in mock `.returning(...)` closures).

Note for the developer: the rule reads "inside a test function" plainly, so control flow in closures nested in a test counts. If that is too strict, the `has` clause can be narrowed with `not: { inside: { kind: closure_expression, stopBy: end } }` — but that would also stop flagging the 6 current sites. As specified, the check is red against the repository until those tests are made linear.

## Next

- Apply: `rule/sty-rust-029.yml` (new); `script/rule_enforcer.sh` (registry entry + `check_sty_rust_029` wrapper)
- Verify: `devenv shell -- ast-grep scan --rule /tmp/opencode/sty-rust-029/rules/sty-rust-029.yml --report-style=short --color=never src` (plan's red/green scratch runs cover the violating and compliant cases)
