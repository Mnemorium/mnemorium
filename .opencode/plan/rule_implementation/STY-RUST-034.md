# Add check — STY-RUST-034

## Rule guard — STY-RUST-034

- **Rule:** "A commit failure maps to the use-case `Unknown(_)`." — `docs/development/TechnicalDesign.md`, § 7 rulebook, section **Unit of Work** (backed by § 7.1 Lifecycle; target surface `src/lib/application/use_case/**/*.rs`).
- **Check state:** missing — `STY-RUST-034` is absent from the `RULES` registry and `script/rule_enforcer.sh uncovered` lists it.
- **Mechanism:** ast-grep — the rule is a within-file syntactic pattern over `.commit()` call sites and how their error is handled; an ast-grep negative pattern expresses it exactly, so the Bash check is a thin `--filter` wrapper (no config block needed).

## Files

### `rule/sty-rust-034.yml`

New ast-grep rule. It matches every `commit()` call and rejects one whose failure is not mapped to `Unknown` in any of the four handling shapes the codebase uses: an inline `map_err(...)` closure, a `match ...commit().await` scrutinee, an `if let ... = ...commit().await`, and (by falling through `not: inside`) an unhandled/ignored commit. `--globs` is passed by the check, so the rule carries no `files:` block.

```yaml
---
id: sty-rust-034
language: Rust
severity: error
message: a commit failure must map to the use-case Unknown(_)
rule:
  pattern: $RECV.commit()
  not:
    inside:
      any:
        - all:
            - pattern: $R.commit().await.map_err($C)
            - has:
                regex: Unknown
                stopBy: end
        - all:
            - pattern: match $R.commit().await { $$$ARMS }
            - has:
                regex: Unknown
                stopBy: end
        - all:
            - pattern: if let $P = $R.commit().await { $$$BODY }
            - has:
                regex: Unknown
                stopBy: end
      stopBy: end
```

### `script/rule_enforcer.sh`

Add the registry entry. Append it to `RULES=( ... )`, after the existing last entry:

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-034:check_sty_rust_034"
)
```

Add the check function in the `# --- Checks ---` section, after `check_sty_rust_024()` and before `# --- Runner ---`:

```bash
check_sty_rust_034() {
	local id="STY-RUST-034"
	local rule_id="sty-rust-034"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs 'src/lib/application/use_case/**/*.rs' \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}
```

## Verification

- **Red:** `/tmp/opencode/sty-rust-034/fixture/src/lib/application/use_case/red.rs` — `devenv shell -- ast-grep scan --config /tmp/opencode/sty-rust-034/sg/sgconfig.yaml --filter sty-rust-034 --report-style=short --color=never /tmp/opencode/sty-rust-034/fixture/src/lib/application/use_case/red.rs` — observed: exit 1, five findings at lines 5, 14, 20, 28, 38 — `.map_err(|error| BadError::Conflict(...))`, an ignored `let _ = .commit().await`, a `match` on `commit()` whose `Err` arm is not `Unknown`, an `if let Err(...) = commit()` not returning `Unknown`, and a match-arm `.map_err` to a non-`Unknown` variant.
- **Green:** `/tmp/opencode/sty-rust-034/fixture/src/lib/application/use_case/green.rs` — same command against `.../green.rs` — observed: exit 0, no findings across the five compliant shapes (`map_err` to `Unknown`, nested `AttemptError::Business(WriteUploadChunkError::Unknown(...))`, `match` on `commit()` with `Err => Unknown`, `if let` returning `Unknown`, and a match-arm `.map_err` to `Unknown`).
- **Repository:** `devenv shell -- ast-grep scan --config sgconfig.yaml --filter sty-rust-034 --globs 'src/lib/application/use_case/**/*.rs' --report-style=short --color=never src` — observed: exit 0, no findings. Run against the candidate rule via the equivalent temp config (the rule is not yet in `rule/`), all 24 `commit()` sites under `src/lib/application/use_case/**` map to a use-case `Unknown(_)`; the sqlx adapter's `transaction.commit()` is outside the `--globs` and stays untouched. After applying the three edits, `devenv shell -- script/rule_enforcer.sh` registers and runs `STY-RUST-034` in place of its current `uncovered` listing.

## Next

- Apply: `rule/sty-rust-034.yml` (new), `script/rule_enforcer.sh` (registry entry + `check_sty_rust_034()`)
- Verify: `devenv shell -- ast-grep scan --config sgconfig.yaml --filter sty-rust-034 --globs 'src/lib/application/use_case/**/*.rs' --report-style=short --color=never src`
