# Add check — STY-RUST-056

## Rule guard — STY-RUST-056

- **Rule:** "Declare the command object first in a use-case port file."
  (`docs/development/TechnicalDesign.md`, rule row line 166, section **Use cases**; elaborated in § 9.1, lines 896–903.)
- **Check state:** missing — `script/rule_enforcer.sh uncovered` lists `STY-RUST-056 (Use cases) — Declare the command object first in a use-case port file.`; there is no `rule/sty-rust-056.yml`, no registry entry, and no function.
- **Mechanism:** ast-grep — the rule is local to one file and its top-level declaration order is visible to ast-grep as sibling relations (`follows`). A thin Bash wrapper scans the use-case port directory, exactly as `check_sty_rust_018`/`check_sty_rust_022` do.

## Files

### `rule/sty-rust-056.yml`

New file. The command object is the top-level `pub struct <Name>Command`; the rule matches one that is preceded (at the same level) by another top-level type declaration — a response struct, an error enum, or the use-case trait. A use-case port file with no command object (`load_configuration.rs`, `initialize_root_admin.rs`) yields no match, so nothing is flagged there. The rule deliberately has **no** `files:` key: `files:` is honoured even for an explicit `--rule <fixture>` scan, so it would suppress the red/green fixture proofs; the wrapper restricts the scan path instead (the `--globs`/directory idiom already used by `check_sty_rust_003`, `check_sty_rust_015`, `check_sty_rust_018`).

```yaml
---
id: sty-rust-056
language: Rust
severity: error
message: declare the command object first in a use-case port file
rule:
  all:
    - kind: struct_item
    - pattern: pub struct $COMMAND
    - regex: '^pub struct \w+Command\b'
    - follows:
        any:
          - kind: struct_item
          - kind: enum_item
          - kind: trait_item
        stopBy: end
```

### `script/rule_enforcer.sh`

Two edits; the runner itself is untouched.

1. Register the check in `RULES=( ... )` (after the `STY-RUST-024` line, which currently ends the array at line 71):

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-056:check_sty_rust_056"
)
```

2. Declare the check function after `check_sty_rust_024()` (which ends at line 752) and before the `# --- Runner ---` banner:

```bash
check_sty_rust_056() {
	local id="STY-RUST-056"
	local rule_id="sty-rust-056"
	local port_dir="src/lib/application/port"
	local output

	if [ ! -d "$port_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$port_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$port_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}
```

No `rule_enforcer.yaml` block is added: the check has no data that must grow without editing the script, and `sgconfig.yaml` already loads every file under `rule/`.

## Verification

All runs are from the repository root, through the dev shell. Fixtures live in `/tmp/opencode/sty-rust-056/`; the candidate rule was proved both as a direct `--rule` scan and through the exact wrapper mechanism (`--config` + `--filter`, using a scratch `sgconfig.yaml` with `ruleDirs: [rule]`).

- **Red (response declared before the command):** `/tmp/opencode/sty-rust-056/red_real.rs` —
  `devenv shell -- ast-grep scan --rule /tmp/opencode/sty-rust-056/final.yml --report-style=short --color=never /tmp/opencode/sty-rust-056/red_real.rs` —
  observed: `red_real.rs:14:1: error[sty-rust-056]: declare the command object first in a use-case port file`, exit 1.
- **Red (trait declared before the command):** `/tmp/opencode/sty-rust-056/red_trait.rs` — same command with `red_trait.rs` — observed: flags line 5, exit 1.
- **Red (enum declared before the command):** `/tmp/opencode/sty-rust-056/red_enum.rs` — same command with `red_enum.rs` — observed: flags the command, exit 1.
- **Green (command first, then response/error/trait, doc comments and derives present):** `/tmp/opencode/sty-rust-056/green_real.rs` — same command with `green_real.rs` — observed: no finding, exit 0.
- **Green (no command object — response first, like `load_configuration.rs`):** `/tmp/opencode/sty-rust-056/none.rs` — same command with `none.rs` — observed: no finding, exit 0.
- **Repository (wrapper mechanism against the real target):**
  `devenv shell -- ast-grep scan --config /tmp/opencode/sty-rust-056/sim/sgconfig.yaml --filter sty-rust-056 --report-style=short --color=never src/lib/application/port` —
  observed: no finding, exit 0; the port directory (including the `*_use_case_factory.rs` files, which declare no command) is currently compliant. Baseline `devenv shell -- script/rule_enforcer.sh check` reports `227 rulebook rule(s), 22 registered, 0 failed`.

## Next

- Apply: `rule/sty-rust-056.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
