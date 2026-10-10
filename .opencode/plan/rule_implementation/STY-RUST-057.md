# Add check — STY-RUST-057

## Rule guard — STY-RUST-057

- **Rule:** "Declare the response object second in a use-case port file (only when non-empty)." (`docs/development/TechnicalDesign.md`, section `Use cases`, row `STY-RUST-057`, line 167; section [§ 9.1](#91-declaration-order-in-a-use-case-port-file) lists it as item 2 of the use-case port order: command, then response, then error enum, then trait.)
- **Check state:** missing — `devenv shell -- script/rule_enforcer.sh uncovered` lists `STY-RUST-057`; the working-tree `script/rule_enforcer.sh` has no `STY-RUST-057` registry entry and no `check_sty_rust_057` function, and `rule/sty-rust-057.yml` does not exist.
- **Mechanism:** ast-grep — the response object's position relative to its sibling declarations is a purely syntactic relation, which ast-grep expresses with the sibling `follows`/`precedes` relations (the idiom of `rule/sty-rust-007.yml` and `rule/sty-rust-042.yml`); a thin Bash wrapper runs the rule, as `check_sty_rust_014` does for a `files:`-scoped rule.

**Semantics of "second".** A `<Name>Response` struct violates the rule when any declaration that must follow it already precedes it, or when it precedes the `<Name>Command` it must follow. Concretely, the `<Name>Response` struct must not be preceded by the `<Name>Error` enum or the `<Name>UseCase` trait, and must not precede a `<Name>Command` struct. This is the negative/relative form used for the repository order (`script/check_repository_declaration_order.sh`) and `STY-RUST-042`: a file with no command object (e.g. `initialize_root_admin.rs`, `load_configuration.rs`, where the response can only be first) is not a finding — a missing earlier item is out of scope of *this* rule. The `(only when non-empty)` clause is handled structurally: a use case that returns `()` declares no `<Name>Response` struct, so nothing matches. The rule is scoped to `src/lib/application/port/**/*.rs` by its own `files:`, so the wrapper needs no `--globs` argument and no `rule_enforcer.yaml` block.

## Files

### `rule/sty-rust-057.yml` (new)

```yaml
---
id: sty-rust-057
language: Rust
severity: error
message: >-
  declare the response object second in a use-case port file: after the command
  object and before the error enum and the use-case trait
files:
  - src/lib/application/port/**/*.rs
rule:
  all:
    - kind: struct_item
    - regex: '^pub struct \w+Response\b'
    - not:
        inside:
          kind: mod_item
          stopBy: end
          follows:
            kind: attribute_item
            regex: cfg\(test\)
    - any:
        # A response declared after the error enum is not second.
        - follows:
            kind: enum_item
            regex: '^pub enum \w+Error\b'
            stopBy: end
        # A response declared after the use-case trait is not second.
        - follows:
            kind: trait_item
            regex: '^pub trait \w+UseCase\b'
            stopBy: end
        # A response declared before the command object is not second.
        - precedes:
            kind: struct_item
            regex: '^pub struct \w+Command\b'
            stopBy: end
```

`severity: error` is what makes `ast-grep scan` exit non-zero on a match, which the Bash check relies on. `sgconfig.yaml` already has `ruleDirs: [rule]`, so no change there.

### `script/rule_enforcer.sh` (edit)

Two additions inside this file. The runner is never edited; no `rule_enforcer.yaml` change.

1. Append one registry line inside `RULES=( ... )`, immediately after the current last entry `"STY-RUST-024:check_sty_rust_024"` (working-tree line 71, before the closing `)` on line 72):

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-057:check_sty_rust_057"
)
```

2. Declare the check function immediately after `check_sty_rust_024()` (ends at working-tree line 752) and before the `# --- Runner ---` divider (line 754). It mirrors `check_sty_rust_014`: the rule's `files:` scoping does the path filtering, so the wrapper runs the filtered scan over `src`.

```bash
check_sty_rust_057() {
	local id="STY-RUST-057"
	local rule_id="sty-rust-057"
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

- Red: `/tmp/opencode/rg057/fix/red/src/lib/application/port/create_note.rs` — command, run with cwd `/tmp/opencode/rg057/fix/red`:
  `devenv shell -- bash -c 'cd /tmp/opencode/rg057/fix/red && ast-grep scan --rule /tmp/opencode/rg057/repo/rule/sty-rust-057.yml --report-style=short --color=never src/lib/application/port/create_note.rs'` — observed: `src/lib/application/port/create_note.rs:22:1: error[sty-rust-057]: declare the response object second in a use-case port file: after the command object and before the error enum and the use-case trait`, exit `1`. The fixture declares `CreateNoteCommand`, then `CreateNoteError`, then `CreateNoteResponse` (line 22), then the trait. Two further misplacements were verified against the same candidate: a response declared first, before the command (`precedes` branch), and a response declared after the trait (`follows` branch); both were flagged at their response line.

- Green: `/tmp/opencode/rg057/fix/green/src/lib/application/port/create_note.rs` — command, run with cwd `/tmp/opencode/rg057/fix/green`:
  `devenv shell -- bash -c 'cd /tmp/opencode/rg057/fix/green && ast-grep scan --rule /tmp/opencode/rg057/repo/rule/sty-rust-057.yml --report-style=short --color=never src/lib/application/port/create_note.rs'` — observed: no finding, exit `0`. The fixture declares `CreateNoteCommand`, `CreateNoteResponse`, `CreateNoteError`, then the trait. The same candidate also left unscanned/clean a command-less file (response, error, trait) and a factory port file (trait only), confirming the `(only when non-empty)` and directory-scope behaviour.

- Repository: `devenv shell -- bash -c 'cd /tmp/opencode/rg057/repomirror && ast-grep scan --config sgconfig.yaml --filter sty-rust-057 --report-style=short --color=never src'`, where `/tmp/opencode/rg057/repomirror/` holds the candidate `sgconfig.yaml` (`ruleDirs: [rule]`), the candidate `rule/sty-rust-057.yml`, and `src` symlinked to the repository's real `src` — this is the exact command `check_sty_rust_057` issues, only with the rule supplied through the scratch config instead of the applied `sgconfig.yaml` — observed: no finding, exit `0`. All 24 files under `src/lib/application/port/` satisfy the rule, including the two command-less use-case port files whose response is first.

## Next

- Apply: `rule/sty-rust-057.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
