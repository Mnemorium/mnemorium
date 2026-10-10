# Add check — STY-RUST-038

## Rule guard — STY-RUST-038

- **Rule:** "Repositories are obtained **only** from a unit of work; concrete sqlx adapters never leave infrastructure." — `docs/development/TechnicalDesign.md`, § 7 rulebook, section **Unit of Work** (backed by § 7.2 Context views, which declares each bounded context's view over the unit of work, e.g. `fn users(&mut self) -> impl UserRepository + '_;`).
- **Check state:** missing — `STY-RUST-038` is absent from the `RULES` registry, no `check_sty_rust_038` exists, and `devenv shell -- script/rule_enforcer.sh uncovered` lists it.
- **Mechanism:** ast-grep + thin Bash path filter — the violation is syntactic (naming a concrete `Sqlx*` adapter type) plus path-scoped (outside infrastructure); ast-grep matches the identifier, and the wrapper's `--globs` confine the scan to everything except infrastructure and the composition root. No `rule_enforcer.yaml` block (the check reads no per-rule data).

### Scope decision (why the globs)

"Infrastructure" is `src/lib/infrastructure/**` plus the sibling module file `src/lib/infrastructure.rs`; the composition root `src/bin/server.rs` is infrastructure by role (`OBS-001`: "the composition root is infrastructure"; `STY-RUST-067` makes it the only place that constructs a process-lifetime concrete adapter, `SqlxUnitOfWorkFactory`). The forbidden surface is therefore the rest of `src` — `src/lib/domain`, `src/lib/application`, `src/lib/*.rs` module files, and `src/bin/*` other than `server.rs`. The lowercase `sqlx` crate path (`sqlx::Error`, `sqlx::query_as!`) is deliberately not matched; the regex is `^Sqlx`, and no current rule permits a `Sqlx`-prefixed type outside the adapters. The module file `src/lib/infrastructure.rs` is excluded by a separate glob because `src/lib/infrastructure/**` does not match the sibling `infrastructure.rs` (verified: a `SqlxUnitOfWorkFactory` re-export there was flagged until the extra glob was added).

## Files

### `rule/sty-rust-038.yml` (new)

New ast-grep rule, registered automatically by `sgconfig.yaml` (`ruleDirs: [rule]`); no `sgconfig.yaml` change. It matches a `Sqlx`-prefixed identifier in both positions the tree-sitter-rust grammar uses: `identifier` (a `use` path segment and the callee of `Sqlx*::new(...)`) and `type_identifier` (a type annotation, generic argument or return type). `severity: error` is required because the wrapper relies on the scan's exit code. No `files:` block: the wrapper's `--globs` scope it (the same idiom as `rule/sty-rust-067.yml`), which also keeps the rule directly runnable with `--rule` on a fixture.

```yaml
---
id: sty-rust-038
language: Rust
severity: error
message: >-
  a concrete sqlx adapter must not leave infrastructure; obtain repositories
  from a unit of work, never name a Sqlx adapter outside src/lib/infrastructure
rule:
  any:
    - kind: identifier
      regex: '^Sqlx'
    - kind: type_identifier
      regex: '^Sqlx'
```

### `script/rule_enforcer.sh`

Register the check in `RULES`, appending after the current last entry `"STY-RUST-024:check_sty_rust_024"`:

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-038:check_sty_rust_038"
)
```

Declare the wrapper in the `# --- Checks ---` section, after `check_sty_rust_024()` and before `# --- Runner ---`:

```bash
check_sty_rust_038() {
	local id="STY-RUST-038"
	local rule_id="sty-rust-038"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs '!src/lib/infrastructure/**' \
		--globs '!src/lib/infrastructure.rs' \
		--globs '!src/bin/server.rs' \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}
```

### `rule_enforcer.yaml`

No change. The check reads no per-rule configuration.

## Verification

- **Red:** `/tmp/opencode/sty-rust-038/fixture/src/lib/domain/model/red.rs` (a domain file that names `SqlxUserRepository` in a plain `use` and in `SqlxUserRepository::new(...)`, `SqlxUnitOfWork`/`SqlxUnitOfWorkFactory` in a braced `use`, an `Arc<SqlxUnitOfWorkFactory>` parameter and a return type, and the alias `Gallery as SqlxGallery`) — `devenv shell -- ast-grep scan --rule /tmp/opencode/sty-rust-038/rule.yml --report-style=short --color=never /tmp/opencode/sty-rust-038/fixture/src/lib/domain/model/red.rs` — observed: exit 1, nine `error[sty-rust-038]` findings at `red.rs:1:61`, `2:59`, `2:75`, `3:71`, `7:75`, `8:5`, `11:37`, `11:64`, `15:21`, i.e. the plain `use`, the two braced-`use` names, the aliased import, the return type, the constructor call, the generic argument and the return type.
- **Red (exact wrapper):** `/tmp/opencode/sty-rust-038/repo/src/lib/domain/model/red.rs`, with a matching `src/lib/infrastructure/outbound/sqlx/unit_of_work.rs` and `src/bin/server.rs` both naming `Sqlx*` — `devenv shell -- bash -c "cd /tmp/opencode/sty-rust-038/repo && SGCONFIG=/tmp/opencode/sty-rust-038/repo-sg/sgconfig.yaml bash /tmp/opencode/sty-rust-038/check038.sh"` (the candidate function verbatim) — observed: exit 1, the same nine findings, and **no** finding from the infrastructure adapter or `src/bin/server.rs`, confirming the globs exclude infrastructure and the composition root.
- **Green:** `/tmp/opencode/sty-rust-038/fixture/src/lib/application/use_case/green.rs` (obtains a repository from the unit of work — `factory.begin()`, then `unit_of_work.users()` — and names only the port traits `UnitOfWorkFactory`, `UserUnitOfWork`, `UserRepository`) — same `--rule` command against `.../green.rs` — observed: exit 0, no findings. **Green (exact wrapper):** `/tmp/opencode/sty-rust-038/repo-green/` (the same tree with `red.rs` removed, retaining the infrastructure and `server.rs` `Sqlx*` references) — `devenv shell -- bash -c "cd /tmp/opencode/sty-rust-038/repo-green && SGCONFIG=/tmp/opencode/sty-rust-038/repo-sg/sgconfig.yaml bash /tmp/opencode/sty-rust-038/check038.sh"` — observed: exit 0, no findings.
- **Repository:** `devenv shell -- ast-grep scan --config /tmp/opencode/sty-rust-038/real-sg/sgconfig.yaml --filter sty-rust-038 --globs '!src/lib/infrastructure/**' --globs '!src/lib/infrastructure.rs' --globs '!src/bin/server.rs' --report-style=short --color=never src` (the candidate rule via a temp config, since it is not yet in `rule/`) — observed: exit 0, no findings; corroborated by `grep -rn --include='*.rs' -E '\bSqlx' src` outside `src/lib/infrastructure/` and `src/bin/server.rs`, which returns nothing. After the two edits above, `devenv shell -- script/rule_enforcer.sh` registers and runs `STY-RUST-038` in place of its current `uncovered` listing.

## Next

- Apply: `rule/sty-rust-038.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
