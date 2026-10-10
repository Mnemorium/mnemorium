# Add check — STY-RUST-048

## Rule guard — STY-RUST-048

- **Rule:** "Missing entities are never errors (see [Port error](#26-port-errors))." — rulebook row `| STY-RUST-048 | Repository | … | § 8.1 Repository port |`, `docs/development/TechnicalDesign.md:158`. The harness parses this row as `STY-RUST-048 (Repository) — Missing entities are never errors (see [Port error](#26-port-errors)).`
- **Check state:** missing — `script/rule_enforcer.sh list` does not register `STY-RUST-048`, `script/rule_enforcer.sh uncovered` prints it, there is no `check_sty_rust_048`, and no `rule/sty-rust-048.yml` exists.
- **Mechanism:** ast-grep — the enforceable part of § 8.1 is a port signature: a single-entity lookup can report absence only as a value (`Ok(None)`), so it must return `Result<Option<…>, _>`. A repository lookup that returns a bare aggregate (`Result<User, RepositoryError>`) forces absence to become an error. A negative ast-grep rule over the repository ports, run by the standard thin wrapper (`check_sty_rust_004` shape), expresses this syntactically; no `rule_enforcer.yaml` block is needed and `sgconfig.yaml` already declares `ruleDirs: [rule]`.

The check surface is `src/lib/domain/port/*_repository.rs`, where the port contract lives (the adapter is forced to match it by the compiler). Lookups are the `get_*`/`find_*` family the project uses (`get_image`, `get_video` in `media_repository.rs`); the regex `^(get|find)(_|$)` covers that family and excludes unrelated names. This is the same idiom as the `STY-RUST-046` check, which guards the sibling `delete` signature with `^delete(_|$)`. The error-variant facet of "missing is never an error" (a `NotFound`-style variant on `RepositoryError`) is already guarded by `STY-RUST-016`, whose `rule/sty-rust-016.yml` scans `src/lib/domain/port/error.rs`; this check adds the repository-port lookup facet.

## Files

### `rule/sty-rust-048.yml` (new)

```yaml
---
id: sty-rust-048
language: Rust
severity: error
message: >-
  a repository lookup reports a missing entity as a value: return
  Result<Option<...>, _>, never a bare aggregate
files:
  - src/lib/domain/port/*_repository.rs
rule:
  kind: function_signature_item
  has:
    field: name
    regex: '^(get|find)(_|$)'
  not:
    has:
      field: return_type
      regex: 'Result<\s*Option<'
```

`severity: error` makes `ast-grep scan` exit non-zero on a match, the idiom the error-severity wrappers rely on. `files` scopes the rule to the repository ports, so the `#[cfg(test)] impl<T: …> … for &mut T` delegating methods (tree-sitter `function_item`, with bodies) are not matched. `sgconfig.yaml` already declares `ruleDirs: [rule]`, so no change is needed there.

### `script/rule_enforcer.sh`

Add one line to the `RULES=( … )` registry, appending after `"STY-RUST-024:check_sty_rust_024"` (line 71, before the closing `)` on line 72):

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-048:check_sty_rust_048"
)
```

Add one function after `check_sty_rust_024` closes (line 752), before `# --- Runner ---` (line 754):

```bash
check_sty_rust_048() {
	local id="STY-RUST-048"
	local rule_id="sty-rust-048"
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

- Red: `/tmp/opencode/sty-rust-048/red_tree/src/lib/domain/port/user_repository.rs` — a `UserRepository` trait declaring `fn get_user(&mut self, id: NumericID) -> impl Future<Output = Result<User, RepositoryError>> + Send;` (line 10) and `fn find_user(&mut self, email: &str) -> impl Future<Output = Result<User, RepositoryError>> + Send;` (line 16), plus a compliant `search` returning `Result<Vec<User>, RepositoryError>` — `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-048/red_tree && SGCONFIG=/tmp/opencode/sty-rust-048/sg/sgconfig.yaml bash /tmp/opencode/sty-rust-048/check048.sh'` — observed: two findings and rc=1, `src/lib/domain/port/user_repository.rs:10:5: error[sty-rust-048]: a repository lookup reports a missing entity as a value: return Result<Option<...>, _>, never a bare aggregate` and the same at `:16:5`. The direct scan (`ast-grep scan --rule /tmp/opencode/sty-rust-048/sg/rules/sty-rust-048.yml --report-style=short --color=never src` from `red_tree`) reports the identical two lines, rc=1.
- Green: `/tmp/opencode/sty-rust-048/green_tree/src/lib/domain/port/media_repository.rs` — a `MediaRepository` trait with `get_image` (line 11), `get_video` (line 17) and `find_image` (line 23), each `-> impl Future<Output = Result<Option<…>, RepositoryError>> + Send`, plus `search_images` returning `Result<Vec<Image>, RepositoryError>` — `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-048/green_tree && SGCONFIG=/tmp/opencode/sty-rust-048/sg/sgconfig.yaml bash /tmp/opencode/sty-rust-048/check048.sh'` — observed: no finding, rc=0. The direct scan over `green_tree` is likewise empty, rc=0; the `Result<\s*Option<` return-type filter accepts all three lookups and the `search_images` name is out of the lookup family.
- Repository: `devenv shell -- bash -c 'cd /home/will/project/mine/Mnemorium/mnemorium && SGCONFIG=/tmp/opencode/sty-rust-048/sg/sgconfig.yaml bash /tmp/opencode/sty-rust-048/check048.sh'` — observed: no finding, rc=0. A positive probe (`probe-048`, same `files` glob and `^(get|find)(_|$)` name filter) over the repository `src` lists the two real lookup signatures — `src/lib/domain/port/media_repository.rs:88` (`get_image`) and `:96` (`get_video`) — confirming the scan reaches the repository ports rather than skipping them; both carry `Result<Option<…>, RepositoryError>` and so are not flagged. After applying the two `script/rule_enforcer.sh` edits and adding `rule/sty-rust-048.yml`, `devenv shell -- script/rule_enforcer.sh check -v` registers and runs `STY-RUST-048` in place of its current `uncovered` listing.

## Next

- Apply: `rule/sty-rust-048.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
