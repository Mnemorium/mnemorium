# Add check — STY-RUST-026

## Rule guard — STY-RUST-026

- **Rule:** `Name tests <UnitOfWork>_<Scenario>_<ExpectedResult>`, e.g. `apply_discount_code_valid_code_reduces_total_price`. (Testing, § 5)
- **Check state:** missing — `rule/sty-rust-026.yml` does not exist and `STY-RUST-026` is not in the `RULES=(...)` registry (`script/rule_enforcer.sh uncovered` lists it).
- **Mechanism:** ast-grep — the whole constraint is a syntactic shape over `function_item` name + a sibling test attribute; a thin Bash wrapper runs the rule, matching the existing `check_sty_rust_00N` idiom. No `rule_enforcer.yaml` block: the required part count, forbidden prefix and test attributes are fixed by the rule, not forward-extensible data.

## Files

### `rule/sty-rust-026.yml` (new)

Flags any function carrying a test attribute whose name either starts with `test_` or has fewer than three underscore-separated parts. The `follows`/`stopBy` block attaches the attribute to *this* function: the backward search stops at the first non-attribute, non-comment sibling, so a function that merely sits below a test (or below another item) is not caught.

```yaml
---
id: sty-rust-026
language: Rust
severity: error
message: >-
  name tests `<UnitOfWork>_<Scenario>_<ExpectedResult>` — at least three
  underscore-separated parts, with no `test_` prefix
files:
  - src/**/*.rs
rule:
  all:
    - kind: function_item
    # A test carries a test attribute as a preceding sibling. `stopBy` halts the
    # backward search at the first real item (skipping the attribute/comment
    # block that decorates this function), so a function that merely follows a
    # test — or sits below an unrelated item — is not mistaken for one.
    - follows:
        kind: attribute_item
        regex: '#\[(test|tokio::test|rstest)\b'
        stopBy:
          all:
            - not:
                kind: attribute_item
            - not:
                kind: line_comment
            - not:
                kind: block_comment
    - any:
        # Reject the legacy `test_` prefix outright, even when the rest of the
        # name already has three parts.
        - has:
            field: name
            regex: '^test_'
        # Require at least three underscore-separated parts.
        - not:
            has:
              field: name
              regex: '^[a-z0-9]+(_[a-z0-9]+){2,}$'
```

### `script/rule_enforcer.sh`

Two edits. First, one line in the `RULES=(...)` registry (append after the current last entry, `"STY-RUST-024:check_sty_rust_024"`):

```bash
	"STY-RUST-026:check_sty_rust_026"
```

Second, the check function, declared after `check_sty_rust_024` (before `# --- Runner ---`):

```bash
check_sty_rust_026() {
	local id="STY-RUST-026"
	local rule_id="sty-rust-026"
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

All runs used the candidate rule under a scratch `sgconfig.yaml` (`ruleDirs: [rule]`) so the `files: src/**/*.rs` filter resolves as it will in the repo; the tool is the dev-shell `ast-grep` (0.45.1).

- **Red:** `/tmp/opencode/sty026/proj/src/red.rs` — `ast-grep scan --config sgconfig.yaml --filter sty-rust-026 --report-style=short --color=never src/red.rs` — observed: 4 findings.
  - line 2 `test_apply_discount_code_valid_code_reduces_total_price` — 8 valid parts but the forbidden `test_` prefix (the new requirement).
  - line 5 `apply_discount`, line 8 `get_user`, line 12 `validate_email` — two-part names.
- **Green:** `/tmp/opencode/sty026/proj/src/green.rs` — same command — observed: no finding, including `fn test_permits()` (a non-attributed helper with a `test_` name) and `fn helper()` sitting below a test.
- **Repository:** `ast-grep scan --config <scratch>/sgconfig.yaml --filter sty-rust-026 --report-style=short --color=never src` over a copy of the repo's `src` — observed: 0 findings, with a positive control rule confirming 667 test-attributed functions were actually inspected (so the green is a real pass, not a non-match). The current tree already satisfies the rule.

## Next

- Apply: `rule/sty-rust-026.yml` (new), `script/rule_enforcer.sh` (registry line + `check_sty_rust_026` function)
- Verify: `devenv shell -- ast-grep scan --config <scratch sgconfig, absolute ruleDir> --filter sty-rust-026 --report-style=short --color=never src` from the repo root (plan's red/green scratch runs cover the violating and compliant cases)
