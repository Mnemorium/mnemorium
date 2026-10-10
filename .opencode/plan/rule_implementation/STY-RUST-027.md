# Add check — STY-RUST-027

## Rule guard — STY-RUST-027

- **Rule:** `| STY-RUST-027 | Testing | Follow the Arrange–Act–Assert (AAA) pattern. | [§ 5](#5-testing) |` (docs/development/TechnicalDesign.md § 5, Testing)
- **Check state:** missing — `script/rule_enforcer.sh uncovered` lists STY-RUST-027, and there is no registry line, no `check_sty_rust_027` function, and no `rule/sty-rust-027.yml`.
- **Mechanism:** ast-grep — the rule constrains each test function's body, which is a syntactic shape; the ordered `// Act` … `// Assert` markers shown in § 5 are expressible with `has` + `follows`, so a thin wrapper (like `check_sty_rust_003`) suffices. No `rule_enforcer.yaml` block: the check carries no per-check data.

The § 5 example annotates the sections `// Arrange`, `// Act`, `// Assert`; the § 5 `rstest` example uses `// Act & Assert` and omits `// Arrange`, so `// Arrange` cannot be required. The mechanical invariant is therefore: a test function's body carries an `// Act` marker and an `// Assert` marker in that order (a single `// Act & Assert` marker satisfies both).

## Files

### `rule/sty-rust-027.yml` (new)

```yaml
---
id: sty-rust-027
language: Rust
severity: error
message: >-
  a test function must follow Arrange-Act-Assert (AAA): annotate its body with
  `// Act` and `// Assert` comments in that order (`// Act & Assert` satisfies
  both)
rule:
  # A test function: a function_item whose own attribute list carries
  # `#[test]`, `#[rstest]` or `#[tokio::test]`. `stopBy: function_item` stops the
  # backward search at the previous function, so a helper that merely follows a
  # test in the module is not mistaken for one; the search still sees past the
  # intervening attributes (e.g. `#[case::...]`) to the test attribute.
  kind: function_item
  follows:
    kind: attribute_item
    regex: '#\[(test|rstest|tokio::test)\b'
    stopBy:
      kind: function_item
  # Compliant when an `// Assert` comment follows an `// Act` comment, or a
  # single comment carries both markers (`// Act & Assert`, as in the § 5 rstest
  # example). `// Arrange` is optional — the § 5 rstest example omits it.
  not:
    any:
      - has:
          kind: line_comment
          regex: '//[^\n]*\bAssert\b'
          stopBy: end
          follows:
            kind: line_comment
            regex: '//[^\n]*\bAct\b'
            stopBy: end
      - has:
          kind: line_comment
          regex: '//[^\n]*\bAct\b[^\n]*\bAssert\b'
          stopBy: end
```

`sgconfig.yaml` already has `ruleDirs: [rule]`, so no registration is needed.

### `script/rule_enforcer.sh`

Add the registry line to `RULES=( … )`, after the STY-RUST-024 entry (line 71):

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-027:check_sty_rust_027"
)
```

Add the check function after `check_sty_rust_024` (line 752), before the `# --- Runner ---` banner (line 754):

```bash
check_sty_rust_027() {
	local id="STY-RUST-027"
	local rule_id="sty-rust-027"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}
```

No `rule_enforcer.yaml` change: the check reads no `rules.STY-RUST-027` block.

## Verification

Fixtures in `/tmp/opencode/sty-027/fx/`, rule mounted at `/tmp/opencode/sty-027/sim/rule/sty-rust-027.yml` under a scratch `sgconfig.yaml` (the repository copy of the rule does not exist yet; the command is byte-identical to the wrapper's, with the scratch config in place of `sgconfig.yaml`).

- Red: `/tmp/opencode/sty-027/fx/red_missing_assert.rs` — `devenv shell -- ast-grep scan --config /tmp/opencode/sty-027/sim/sgconfig.yaml --filter sty-rust-027 --report-style=short --color=never /tmp/opencode/sty-027/fx/red_missing_assert.rs` — observed: `red_missing_assert.rs:2:1: error[sty-rust-027]` (has `// Act`, no `// Assert`). `red_wrong_order.rs` (`// Assert` before `// Act`) and `red_no_comments.rs` (no markers) also flag.
- Green: `/tmp/opencode/sty-027/fx/green_aaa.rs` (`// Arrange` / `// Act` / `// Assert`) and `/tmp/opencode/sty-027/fx/green_act_assert.rs` (`// Act & Assert`) — same command — observed: exit 0, no finding.
- Repository: `devenv shell -- ast-grep scan --config sgconfig.yaml --filter sty-rust-027 --report-style=short --color=never src` (exercised via the scratch config) — observed: 4 findings, all genuine deviations — `src/lib/infrastructure/outbound/sha2/content_hasher.rs:81`, `:87`, `:93`, and `src/lib/domain/port/content_hasher.rs:45`. The other 663 test functions (including async, `#[rstest]`/`#[case]`, and `#[tokio::test]` forms) pass, and no helper function is flagged.

## Next

- Apply: `rule/sty-rust-027.yml` (new), `script/rule_enforcer.sh` (registry line + `check_sty_rust_027`)
- Verify: `devenv shell -- ast-grep scan --config sgconfig.yaml --filter sty-rust-027 --report-style=short --color=never src`
