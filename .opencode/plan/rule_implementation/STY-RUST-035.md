# Add check — STY-RUST-035

## Rule guard — STY-RUST-035

- **Rule:** "Repository views borrow the unit of work mutably; drop them before `commit`/`rollback`." — `docs/development/TechnicalDesign.md`, rule row `STY-RUST-035`, section **Unit of Work**, detail § 7.1 Lifecycle.
- **Check state:** missing — `STY-RUST-035` has no registry line and no `check_sty_rust_035`; `script/rule_enforcer.sh uncovered` lists it (`STY-RUST-035 (Unit of Work) — Repository views borrow the unit of work mutably; drop them before `commit`/`rollback`.`).
- **Mechanism:** ast-grep + thin Bash wrapper — the enforceable half of the rule is the accessor declaration ("borrow the unit of work mutably" is exactly a `&mut self` receiver on a method returning a repository view); the "drop before `commit`/`rollback`" consequence is already guaranteed by rustc once the receiver is `&mut self`, because `commit`/`rollback` consume the unit of work (`STY-RUST-032`, `fn commit(self)` in `src/lib/domain/port/unit_of_work.rs`), so no view can be alive at a commit. The rule is one structural pattern over a method signature, so it is an ast-grep rule plus a `--filter` wrapper; no `rule_enforcer.yaml` block is needed.

A context-view accessor is a method whose return type is a repository view (`impl <Something>Repository + '_`). There are 24 such declarations in the tree (`src/lib/domain/port/*_unit_of_work.rs`, the `src/lib/infrastructure/outbound/sqlx/unit_of_work.rs` adapter, and the `src/lib/lib.rs` test mocks); all take `&mut self`. The rule flags any such method whose receiver is not `&mut self` (`&self`, `self`, or `mut self`). Methods that are not repository views — `begin(&self)`, `commit(self)`, `rollback(self)`, and any `impl Future`/other return — do not match the return-type discriminator and stay untouched.

## Files

### `rule/sty-rust-035.yml` (new)

New ast-grep rule. Registered automatically by `sgconfig.yaml` (`ruleDirs: [rule]`); no `sgconfig.yaml` change. It carries no `files:` block, so the wrapper's `--globs` scopes it and `--rule` runs it directly on a fixture.

```yaml
---
id: sty-rust-035
language: Rust
severity: error
message: >-
  a repository view borrows the unit of work mutably: the accessor must take
  `&mut self`, so no view can be alive across commit/rollback
rule:
  all:
    # A context-view accessor is a method returning a repository view; it is
    # declared without a body (trait) or with one (impl).
    - any:
        - kind: function_item
        - kind: function_signature_item
    - has:
        field: return_type
        regex: "Repository \\+ '_"
    # "borrow the unit of work mutably" is exactly `&mut self`: a shared or
    # by-value receiver does not borrow, so a view could outlive commit/rollback.
    - has:
        field: parameters
        has:
          kind: self_parameter
          not:
            regex: '^&mut '
```

### `script/rule_enforcer.sh`

Add the registry entry. Append it to `RULES=( ... )` after the existing last entry (line 71), before the closing `)`:

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-035:check_sty_rust_035"
)
```

Add the check function in the `# --- Checks ---` section, after `check_sty_rust_024()` (ends line 752) and before `# --- Runner ---` (line 754):

```bash
check_sty_rust_035() {
	local id="STY-RUST-035"
	local rule_id="sty-rust-035"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs 'src/**/*.rs' \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}
```

No `rule_enforcer.yaml` change: the check reads no per-rule data.

## Verification

- **Red:** `/tmp/opencode/sty-rust-035/fixture/red.rs` (a `UserUnitOfWork` trait declaring `fn users(&self) -> impl UserRepository + '_`, a `LibraryUnitOfWork` trait declaring `fn galleries(self) -> impl GalleryRepository + '_`, and an `impl` body `fn users(&self) -> impl UserRepository + '_`) — `devenv shell -- ast-grep scan --rule /tmp/opencode/sty-rust-035/rule/sty-rust-035.yml --report-style=short --color=never /tmp/opencode/sty-rust-035/fixture/red.rs` — observed: exit 1, three findings at `red.rs:7:5`, `red.rs:12:5`, `red.rs:19:5` (`error[sty-rust-035]`). The exact wrapper form was also run red: `--config /tmp/opencode/sty-rust-035/sgconfig.yaml --filter sty-rust-035 --globs 'src/**/*.rs' … src` with `red.rs` at `src/lib/domain/port/red.rs` — observed: exit 1, `src/lib/domain/port/red.rs:7:5`, `:12:5`, `:19:5`.
- **Green:** `/tmp/opencode/sty-rust-035/fixture/green.rs` (the same three accessors declared `&mut self`, plus an unrelated `fn begin(&self) -> impl Future<Output = ()> + Send` that must not be flagged) — same `--rule` command against `green.rs` — observed: exit 0, no findings. The wrapper form against `src/lib/domain/port/green.rs` — observed: exit 0, no findings.
- **Repository:** `devenv shell -- ast-grep scan --config /tmp/opencode/sty-rust-035/sgconfig.yaml --filter sty-rust-035 --globs 'src/**/*.rs' --report-style=short --color=never src` — observed: exit 0, no findings. A probe rule matching the return-type discriminator alone confirmed the check inspects all 24 repository-view accessor sites (`src/lib/domain/port/asset_unit_of_work.rs` ×3, `configuration_unit_of_work.rs` ×1, `identity_unit_of_work.rs` ×1, `library_unit_of_work.rs` ×2, `user_unit_of_work.rs` ×1, `src/lib/infrastructure/outbound/sqlx/unit_of_work.rs` ×8, `src/lib/lib.rs` ×8), every one `&mut self`. After applying the two edits, `devenv shell -- script/rule_enforcer.sh` registers `STY-RUST-035` in place of its current `uncovered` listing and reports it clean.

## Next

- Apply: `rule/sty-rust-035.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
