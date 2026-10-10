# Add check — STY-RUST-036

## Rule guard — STY-RUST-036

- **Rule:** "Repository accessors return a short-lived view that borrows the unit of work." (`STY-RUST-036`, section Unit of Work; more info § 7.2 Context views: `fn users(&mut self) -> impl UserRepository + '_;`)
- **Check state:** missing — `STY-RUST-036` has a row in `docs/development/TechnicalDesign.md` (line 146) and is listed by `script/rule_enforcer.sh uncovered`; there is no `rule/sty-rust-036.yml`, no `check_sty_rust_036`, and no registry entry.
- **Mechanism:** ast-grep — the canonical shape in § 7.2 is a syntactic function signature (`fn <accessor>(&mut self) -> impl <Repository> + '_`); a single `error`-severity rule plus the standard thin wrapper is the smallest mechanical check. No `rule_enforcer.yaml` block: the check has no data that should grow independently of the rule.

Surface note: the rule constrains the context-view traits in `src/lib/domain/port/<context>_unit_of_work.rs`. The rule's `files` glob `src/lib/domain/port/**/*_unit_of_work.rs` selects exactly those five files and not the base `src/lib/domain/port/unit_of_work.rs` (verified: `unit_of_work.rs` does not match `*_unit_of_work.rs`).

## Files

### `rule/sty-rust-036.yml` (new)

The rule flags any `&mut self` trait-method signature in a context-view file whose return type is not the canonical borrowing view `impl <Repository> + '_`. `severity: error` is required so `ast-grep scan` exits non-zero on a finding (the wrapper relies on the exit code for the simple checks; `hint` returns 0).

```yaml
---
id: sty-rust-036
language: Rust
severity: error
message: >-
  a repository accessor on a context unit-of-work view must return a
  short-lived view that borrows the unit of work: `impl <Repository> + '_`
files:
  - src/lib/domain/port/**/*_unit_of_work.rs
rule:
  all:
    - kind: function_signature_item
    - pattern: 'fn $NAME(&mut self) -> $RET'
    - not:
        pattern: "fn $NAME(&mut self) -> impl $TRAIT + '_"
```

`sgconfig.yaml` already declares `ruleDirs: [rule]`, so no config change is needed to register the rule; the wrapper selects it with `--filter sty-rust-036`.

### `script/rule_enforcer.sh`

Two edits: one registry line and one check function. Nothing else in the runner changes.

**1. Registry** — append after the last entry (`"STY-RUST-024:check_sty_rust_024"`, currently line 71), before the closing `)`:

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-036:check_sty_rust_036"
)
```

**2. Check function** — insert after the closing brace of `check_sty_rust_024` (currently line 752), before the `# --- Runner ---` comment:

```bash
check_sty_rust_036() {
	local id="STY-RUST-036"
	local rule_id="sty-rust-036"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}
```

No `rule_enforcer.yaml` entry: `rule_config` is not read by this check, and the rule needs no per-rule data.

## Verification

All fixtures live under `/tmp/opencode/036/rep`, laid out as a miniature project so the exact check command shape (`--config sgconfig.yaml --filter sty-rust-036 … src`) runs verbatim:

```
/tmp/opencode/036/rep/
  sgconfig.yaml                        # ruleDirs: [rule]
  rule/sty-rust-036.yml                # candidate rule above
  src/lib/domain/port/user_unit_of_work.rs
```

- **Red:** `/tmp/opencode/036/rep/src/lib/domain/port/user_unit_of_work.rs` —
  ```rust
  pub trait UserUnitOfWork: UnitOfWork {
      fn users(&mut self) -> impl UserRepository;
  }
  ```
  `cd /tmp/opencode/036/rep && devenv shell -- ast-grep scan --config sgconfig.yaml --filter sty-rust-036 --report-style=short --color=never src`
  observed: `src/lib/domain/port/user_unit_of_work.rs:2:5: error[sty-rust-036]: a repository accessor on a context unit-of-work view must return a short-lived view that borrows the unit of work: `impl <Repository> + '_`` and `exit=1`. A four-method fixture (`-> UserRepositoryImpl`, `-> impl UserRepository`, `-> Box<dyn UserRepository>`, and the compliant `-> impl UserRepository + '_`) flagged exactly the three non-borrowing methods (lines 5, 6, 7) and skipped the compliant one (line 8).

- **Green:** same path with the compliant idiom (a single method, then a two-method variant and a multi-line return) —
  ```rust
  pub trait UserUnitOfWork: UnitOfWork {
      fn users(&mut self) -> impl UserRepository + '_;
  }
  ```
  `cd /tmp/opencode/036/rep && devenv shell -- ast-grep scan --config sgconfig.yaml --filter sty-rust-036 --report-style=short --color=never src`
  observed: no output, `exit=0`.

- **Repository:** from the repo root, the same rule content and file selection via an external `sgconfig.yaml` (`ruleDirs` → the candidate rule dir):
  `devenv shell -- ast-grep scan --config /tmp/opencode/036/ext-sgconfig.yaml --filter sty-rust-036 --report-style=short --color=never src`
  observed: no output, `exit=0` — the five real context-view files (`user`, `identity`, `asset`, `library`, `configuration`) are all compliant. A companion compliant-signature probe over `src` reported 8 accessor matches across those five files (`asset` 3, `library` 2, `user`/`identity`/`configuration` 1 each), confirming the `files` filter selects the intended surface. Once the plan is applied, the identical check runs as `devenv shell -- ast-grep scan --config sgconfig.yaml --filter sty-rust-036 --report-style=short --color=never src` (and via `devenv shell -- script/rule_enforcer.sh check -v`).

## Next

- Apply: `rule/sty-rust-036.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
