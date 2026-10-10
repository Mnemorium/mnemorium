# Add check — STY-RUST-054

## Rule guard — STY-RUST-054

- **Rule:** "Trait named `<UseCaseName>UseCase`, e.g. `CreateNoteUseCase`." (rulebook row `STY-RUST-054`, section **Use cases**, `docs/development/TechnicalDesign.md` line 164)
- **Check state:** missing — no `rule/sty-rust-054.yml`, no `STY-RUST-054` registry line in `script/rule_enforcer.sh`; `script/rule_enforcer.sh uncovered` lists it, `list` does not.
- **Mechanism:** ast-grep — both the trait name and its `execute` method are syntactic, so one rule expresses the whole check and the Bash check is a thin wrapper (matches the existing `check_sty_rust_014` idiom). No `rule_enforcer.yaml` block: there is no per-check data to externalise.

## Files

### `rule/sty-rust-054.yml`

New file. A use-case trait is identified structurally as a `trait_item` carrying a method named `execute` (the trait shape fixed by `STY-RUST-053`; factory traits carry no `execute`, so `*UseCaseFactory` is excluded). The rule fires when such a trait's name does not match `<UseCaseName>UseCase`. The file glob keeps the check to the use-case port directory where these traits live (`STY-RUST-056`–`STY-RUST-059`).

```yaml
---
id: sty-rust-054
language: Rust
severity: error
message: a use-case trait is named `<UseCaseName>UseCase`
files:
  - src/lib/application/port/**/*.rs
rule:
  all:
    - kind: trait_item
    - has:
        any:
          - kind: function_signature_item
          - kind: function_item
        has:
          field: name
          regex: '^execute$'
        stopBy: end
    - has:
        field: name
        not:
          regex: '^[A-Z][A-Za-z0-9]*UseCase$'
```

### `script/rule_enforcer.sh`

Two edits. First, append one registry line to the `RULES=( ... )` array (lines 49–72), after `"STY-RUST-024:check_sty_rust_024"`:

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-054:check_sty_rust_054"
)
```

Second, add the check function at the end of the `# --- Checks ---` block, immediately before `# --- Runner ---` (after `check_sty_rust_024`, line 752):

```bash
check_sty_rust_054() {
	local id="STY-RUST-054"
	local rule_id="sty-rust-054"
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

- Red: `/tmp/opencode/sty-rust-054/tree/src/lib/application/port/bad.rs` = `pub trait CreateNote: Send + Sync { fn execute<'future>(&'future self) -> u8; }` — `cd /tmp/opencode/sty-rust-054/tree && ast-grep scan --config sgconfig.yaml --filter sty-rust-054 --report-style=short src` — observed: `src/lib/application/port/bad.rs:1:1: error[sty-rust-054]: a use-case trait is named \`<UseCaseName>UseCase\``, exit 1. A default-body variant (`fn execute(...) { 0 }`) is flagged identically.
- Green: `/tmp/opencode/sty-rust-054/tree/src/lib/application/port/good.rs` = `pub trait CreateNoteUseCase: Send + Sync { fn execute<'future>(&'future self) -> u8; }` plus `pub trait IdentityUseCaseFactory: Send + Sync { fn create_note(&self) -> u8; }` plus a struct — same command — observed: no finding, exit 0. A trait named exactly `UseCase` (empty prefix) is flagged; the factory trait (no `execute`) and an `impl ...UseCase for ... { fn execute }` block are not.
- Repository: mirror of the real `src/` with the candidate rule registered at `/tmp/opencode/sty-rust-054/repomirror` (rule file + `sgconfig.yaml`, `ruleDirs: [rules]`) — `cd /tmp/opencode/sty-rust-054/repomirror && ast-grep scan --config sgconfig.yaml --filter sty-rust-054 --report-style=short src` — observed: no finding (exit 0) on all 190 real files; injecting `src/lib/application/port/bad_tmp.rs` yields `error[sty-rust-054]` on that file only (exit 1). The check's own command after registration is `script/rule_enforcer.sh check` (which runs `ast-grep scan --config sgconfig.yaml --filter sty-rust-054 --report-style=short --color=never src`), expected green.

## Next

- Apply: `rule/sty-rust-054.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
