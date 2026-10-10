# Add check — STY-RUST-053

## Rule guard — STY-RUST-053

- **Rule:** "A use case trait has one and only one method, named `execute`." — `docs/development/TechnicalDesign.md`, section `Use cases` (line 163).
- **Check state:** missing — `STY-RUST-053` has a rulebook row but no `check_sty_rust_053` function and no `RULES=(...)` entry; it appears in `script/rule_enforcer.sh uncovered`.
- **Mechanism:** ast-grep — the trait's method set and the method name are a single-file syntactic shape, expressed by one rule at `rule/sty-rust-053.yml` plus a thin Bash wrapper. No config block: the rule carries no data that should vary.

## Files

### `rule/sty-rust-053.yml` (new)

```yaml
---
id: sty-rust-053
language: Rust
severity: error
message: >-
  a use case trait declares exactly one method, named `execute`
files:
  - src/**/*.rs
rule:
  kind: trait_item
  all:
    # A use case trait is `<UseCaseName>UseCase`; the `$` anchor excludes
    # `<Context>UseCaseFactory`, whose one-method-per-use-case shape is
    # required by STY-RUST-060.
    - has:
        field: name
        regex: 'UseCase$'
  any:
    # A method other than `execute` — a second method, or the sole method
    # under a different name.
    - has:
        field: body
        has:
          all:
            - any:
                - kind: function_signature_item
                - kind: function_item
            - not:
                has:
                  field: name
                  regex: '^execute$'
        stopBy: end
    # No `execute` method at all — a trait with zero methods, or a trait
    # whose only method is named otherwise.
    - not:
        has:
          field: body
          has:
            field: name
            regex: '^execute$'
          stopBy: end
```

Why the shape: `kind: trait_item` plus `has: {field: name, regex: 'UseCase$'}` selects use-case traits and excludes `*UseCaseFactory` traits (STY-RUST-060) and every non-trait item. The `any` is the violation condition: a non-`execute` method (covers two methods and a sole mis-named method), or the absence of `execute` (covers zero methods). Associated types and consts are not `function_signature_item`/`function_item`, so they are not counted as methods. Both `fn execute(...);` declarations and default-bodied `fn execute(...) {}` are covered.

### `script/rule_enforcer.sh`

Two edits; the runner is untouched.

1. Add one registry line in `RULES=(...)`, immediately after `"STY-RUST-024:check_sty_rust_024"` (line 71):

```bash
	"STY-RUST-053:check_sty_rust_053"
```

2. Add one check function immediately after `check_sty_rust_024` (before `# --- Runner ---`):

```bash
check_sty_rust_053() {
	local id="STY-RUST-053"
	local rule_id="sty-rust-053"
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

Scratch project at `/tmp/opencode/rule053/proj/` mirrors the repository layout: `sgconfig.yaml` (`ruleDirs: [rule]`), `rule/sty-rust-053.yml` (the candidate above), and `src/lib/*.rs` fixtures. The command is the harness's shape, run from that project root:

```
ast-grep scan --config sgconfig.yaml --filter sty-rust-053 --report-style=short --color=never src
```

- **Red:** `/tmp/opencode/rule053/proj/src/lib/red_two_methods.rs` (`pub trait CreateNoteUseCase { fn execute(&self); fn extra(&self); }`), `.../red_wrong_name.rs` (`pub trait CreateNoteUseCase { fn run(&self); }`), `.../red_zero_methods.rs` (`pub trait CreateNoteUseCase {}`) — observed:
  ```
  src/lib/red_zero_methods.rs:1:1: error[sty-rust-053]: a use case trait declares exactly one method, named `execute`
  src/lib/red_two_methods.rs:1:1: error[sty-rust-053]: a use case trait declares exactly one method, named `execute`
  src/lib/red_wrong_name.rs:1:1: error[sty-rust-053]: a use case trait declares exactly one method, named `execute`
  Error: 3 error(s) found in code.
  ```
  A fourth red, `red_default_execute_plus.rs` (`fn execute(&self) {}` + `fn extra(&self) {}`), was also flagged.
- **Green:** `/tmp/opencode/rule053/proj/src/lib/green_one_execute.rs` (one `fn execute(...) -> Pin<Box<dyn Future<...>>>;`), `.../green_factory.rs` (`pub trait UserUseCaseFactory` with two methods — excluded by the `UseCase$` anchor), `.../green_assoc_type.rs` (`type Output;` plus one `execute`) — with the red files removed, the same command observed: no finding, exit `0`.
- **Repository:** `devenv shell -- ast-grep scan --rule /tmp/opencode/rule053/nofiles.yml --report-style=short --color=never src` (the identical rule body; `files: src/**/*.rs` was separately proven to cover `src/lib/*.rs` in the scratch harness run above, so the result is the same) — observed: no findings. All 21 `*UseCase` traits under `src/lib/application/port/` declare exactly one `execute` method; the four `*UseCaseFactory` traits are correctly excluded.

## Next

- Apply: `rule/sty-rust-053.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
