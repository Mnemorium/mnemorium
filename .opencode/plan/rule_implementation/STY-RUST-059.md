# Add check — STY-RUST-059

## Rule guard — STY-RUST-059

- **Rule:** "Declare the use-case trait last in a use-case port file." (`docs/development/TechnicalDesign.md` line 169, section **Use cases**; § 9.1 enumerates the order: command object (`STY-RUST-056`), response object (`STY-RUST-057`), error enum (`STY-RUST-058`), use-case trait (`STY-RUST-059`)).
- **Check state:** missing — `STY-RUST-059` is absent from `RULES=(...)` in `script/rule_enforcer.sh`, `rule/sty-rust-059.yml` does not exist, and `script/rule_enforcer.sh uncovered` lists `STY-RUST-059 (Use cases) — Declare the use-case trait last in a use-case port file.`
- **Mechanism:** ast-grep + thin Bash wrapper — the rule is a purely syntactic intra-file ordering relation (a `*UseCase` trait that precedes any `struct`/`enum` declaration), which ast-grep's `precedes` relation expresses directly; no per-check data, so no `rule_enforcer.yaml` block.

Interpretation: § 9.1 fixes the order of the *type declarations* (command struct, response struct, error enum, trait); `impl` blocks and nested `mod` items are not part of that order (the real files interleave `impl` blocks between the structs). "Last" therefore means: no `struct_item` or `enum_item` may follow the use-case trait. The rule is scoped to `src/lib/application/port/*.rs` and to traits whose name ends in `UseCase`, so factory-port traits (`<Context>UseCaseFactory`, § 9.3) are unaffected.

## Files

### `rule/sty-rust-059.yml` (new)

```yaml
---
id: sty-rust-059
language: Rust
severity: error
message: the use-case trait $TRAIT must be declared last in a use-case port file
files:
  - src/lib/application/port/*.rs
rule:
  all:
    - kind: trait_item
    - pattern: pub trait $TRAIT
    - regex: '^pub trait \w+UseCase\b'
    - precedes:
        any:
          - kind: struct_item
          - kind: enum_item
        stopBy: end
```

`severity: error` matches the other failing (negative) rules (`sty-rust-003`, `-004`, `-007`, `-067`) and makes `ast-grep` exit non-zero on a finding. `pattern: pub trait $TRAIT` supplies the name interpolated into `message`; `regex: '^pub trait \w+UseCase\b'` restricts the rule to use-case traits and excludes `…UseCaseFactory`.

### `script/rule_enforcer.sh`

**Edit 1 — register the check.** In `RULES=( ... )`, after the final entry, append one line (the array is tab-indented):

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-059:check_sty_rust_059"
)
```

**Edit 2 — add the check function.** Insert between `check_sty_rust_024` (ends at current line 752) and the `# --- Runner ---` banner:

```bash
check_sty_rust_059() {
	local id="STY-RUST-059"
	local rule_id="sty-rust-059"
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

This mirrors the scoped-directory idiom of `check_sty_rust_018`/`check_sty_rust_022`. No `rule_enforcer.yaml` change is required (no per-check data), and the runner is untouched.

## Verification

The shipped `rule/sty-rust-059.yml` carries a `files:` glob, so the fixtures recreate the repo-relative path `src/lib/application/port/` under `/tmp/opencode/sty-rust-059/`.

- **Red:** `/tmp/opencode/sty-rust-059/red-root/src/lib/application/port/violation.rs`:

  ```rust
  /// Command.
  #[derive(Debug)]
  pub struct ViolationCommand;

  /// Use case for violating order.
  #[cfg_attr(test, mockall::automock)]
  pub trait ViolationUseCase: Send + Sync {
      fn execute(&self);
  }

  /// Response declared after the trait.
  #[derive(Debug)]
  pub struct ViolationResponse;
  ```

  Command: `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-059/red-root && ast-grep scan --rule /tmp/opencode/sty-rust-059/ruledir/sty-rust-059.yml --report-style=short --color=never src'` — observed: `src/lib/application/port/violation.rs:7:1: error[sty-rust-059]: the use-case trait ViolationUseCase must be declared last in a use-case port file` (exit 1). The same result through the literal wrapper function (`/tmp/opencode/sty-rust-059/check_scratch.sh`, run from `red-root`) exited 1.

- **Green:** `/tmp/opencode/sty-rust-059/green-root/src/lib/application/port/compliant.rs` (command struct, then response struct, then error enum, then `pub trait CompliantUseCase: Send + Sync`) plus `compliant_use_case_factory.rs` (`pub trait CompliantUseCaseFactory: Send + Sync`) — same rule, same command from `green-root` — observed: no finding (exit 0). The literal wrapper exited 0. Edge checks also returned no finding: a `struct`/`enum` nested in a `#[cfg(test)] mod` after the trait, an `impl` block after the trait, and a `*UseCaseFactory` trait followed by a struct.

- **Repository:** `devenv shell -- ast-grep scan --config /tmp/opencode/sty-rust-059/sgconfig-tmp.yaml --filter sty-rust-059 --report-style=short --color=never src` (scratch `sgconfig.yaml` with `ruleDirs: [ruledir]` pointing at the candidate rule, from the repo root) — observed: no finding, exit 0. After the plan is applied, `devenv shell -- script/rule_enforcer.sh check -v` runs the registered check over all 26 port files (`add_gallery_item.rs` … `write_upload_chunk.rs`) with the same result.

## Next

- Apply: `rule/sty-rust-059.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
