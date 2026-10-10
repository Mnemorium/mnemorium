# Add check — STY-RUST-028

## Rule guard — STY-RUST-028

- **Rule:** "Do not over-abstract test setup into deeply nested helper functions or distant global state." (`docs/development/TechnicalDesign.md` § 1 table, Section `Testing`; more info [§ 5](#5-testing))
- **Check state:** missing — `STY-RUST-028` is not in `RULES`, no `rule/sty-rust-028.yml` exists; `script/rule_enforcer.sh uncovered` lists it.
- **Mechanism:** ast-grep rule + thin Bash wrapper — both anti-patterns are syntactic: a `fn` item nested in another `fn` item ("deeply nested helper functions") and a module-level `static` item ("distant global state"), each confined to a `#[cfg(test)]` module. `ast-grep` sees both; the wrapper is the standard `--filter` runner. No `rule_enforcer.yaml` block — the target path (`src`) is fixed, like `check_sty_rust_003`/`004`.

## Files

### `rule/sty-rust-028.yml` (new)

```yaml
---
id: sty-rust-028
language: Rust
severity: error
message: >-
  keep test setup inline: no helper `fn` nested inside another `fn`, and no
  module-level `static` in a `#[cfg(test)]` module (STY-RUST-028)
rule:
  any:
    # A helper function nested inside another function, in a test module.
    - all:
        - kind: function_item
        - inside:
            kind: function_item
            stopBy: end
        - inside:
            kind: mod_item
            stopBy: end
            follows:
              kind: attribute_item
              regex: 'cfg\(test\)'
              stopBy: end
    # A module-level `static` — distant global state, in a test module.
    - all:
        - kind: static_item
        - inside:
            kind: declaration_list
            stopBy: neighbor
            inside:
              kind: mod_item
              stopBy: end
              follows:
                kind: attribute_item
                regex: 'cfg\(test\)'
                stopBy: end
```

Notes on the shape, drawn from `rule/sty-rust-024.yml`: `#[cfg(test)]` precedes the `mod` as a sibling, so the module is reached with `follows`, not `has`. The `static` arm is pinned to `declaration_list` (the module body) with `stopBy: neighbor` so a function-local `static` cached inside a helper function — `src/lib/application/use_case/login_user.rs:337,347` — is not "distant" and stays green; only a module-scope global is flagged.

### `script/rule_enforcer.sh`

Two edits. First, one registry line, after `STY-RUST-024`:

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-028:check_sty_rust_028"
)
```

Second, the check function, inserted after `check_sty_rust_024` and before `# --- Runner ---`, matching the other ast-grep wrappers:

```bash
check_sty_rust_028() {
	local id="STY-RUST-028"
	local rule_id="sty-rust-028"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}
```

### `rule_enforcer.yaml`

No change. No per-rule data; the target is the fixed `src` tree.

## Verification

- Red: `/tmp/opencode/s028/red.rs` (a `#[cfg(test)] mod tests` with module-level `static GLOBAL` at line 13 and a `fn inner` nested inside `#[test] fn nested_helper` at line 23) — `devenv shell -- ast-grep scan --rule /tmp/opencode/s028/sty-rust-028.yml --report-style=short --color=never /tmp/opencode/s028/red.rs` — observed: `red.rs:13:5: error[sty-rust-028]` and `red.rs:23:9: error[sty-rust-028]`; the scratch wrapper `devenv shell -- bash /tmp/opencode/s028/scratch.sh /tmp/opencode/s028/redroot` prints both and `FAIL STY-RUST-028`.
- Green: `/tmp/opencode/s028/green.rs` (an inline-setup `#[test] fn plain_test`, a permitted module-level helper `fn module_helper`, and a production-only nested `fn helper` outside any test module) — `devenv shell -- ast-grep scan --rule /tmp/opencode/s028/sty-rust-028.yml --report-style=short --color=never /tmp/opencode/s028/green.rs` — observed: no finding; the scratch wrapper over `greenroot` prints `ok STY-RUST-028`.
- Repository: `devenv shell -- ast-grep scan --rule /tmp/opencode/s028/sty-rust-028.yml --report-style=short --color=never src` (and the exact-form scratch wrapper `devenv shell -- bash /tmp/opencode/s028/scratch.sh /home/will/project/mine/Mnemorium/mnemorium`, which runs `ast-grep scan --config … --filter sty-rust-028 … src`) — observed: no findings, `ok STY-RUST-028`. The only test-module statics in the tree are function-local (`login_user.rs:337,347`) and are out of scope; the only other `static` is production (`media_probe.rs:244`). After applying, confirm with `devenv shell -- script/rule_enforcer.sh check -v`.

## Next

- Apply: `rule/sty-rust-028.yml` (new); `script/rule_enforcer.sh` (add the `STY-RUST-028:check_sty_rust_028` registry line and the `check_sty_rust_028` function). `rule_enforcer.yaml` unchanged.
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
