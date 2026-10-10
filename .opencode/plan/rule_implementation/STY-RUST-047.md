# Add check — STY-RUST-047

## Rule guard — STY-RUST-047

- **Rule:** "`search` returns an empty collection when nothing matches." — rulebook row `| STY-RUST-047 | Repository | ... | § 8.1 Repository port |`, `docs/development/TechnicalDesign.md:157` (detail `##### 8.1 Repository port`, `docs/development/TechnicalDesign.md:836`).
- **Check state:** missing — `script/rule_enforcer.sh list` does not register `STY-RUST-047`, `script/rule_enforcer.sh uncovered` lists it among the unguarded rows, and `rule/sty-rust-047.yml` does not exist.
- **Mechanism:** ast-grep — the enforceable part of § 8.1 is the port signature: `search` is `search(filter: &SomeFilter)` and must yield a collection (`Result<Vec<_>, _>`) so that a no-match is an empty `Vec`, never an `Option` or a single value. A negative ast-grep rule plus the standard thin wrapper (the `check_sty_rust_004` shape) expresses this syntactically; no `rule_enforcer.yaml` block is needed (no per-check data) and no `sgconfig.yaml` change (`ruleDirs: [rule]` already picks the file up). The adapter is forced to match the trait by the compiler, so the port is the check surface — the same surface `STY-RUST-046` uses for the sibling `delete` rule.

The `search` family is identified by name `^search(_|$)` (`search`, `search_items`, `search_images`, `search_videos`) **and** a parameter typed with a `Filter` struct, which is exactly the § 8.1 `search(filter: &SomeFilter)` contract. The filter clause excludes the one parameterless sibling, `ConfigurationRepository::search()` (`src/lib/domain/port/configuration_repository.rs:29`), which returns `Result<Option<Configuration>, RepositoryError>` for the configuration singleton: it takes no filter, so "nothing matches" does not apply and it is outside § 8.1's `search(filter:)` contract. Without the filter clause the rule would falsely flag that singleton.

## Files

### `rule/sty-rust-047.yml` (new)

```yaml
---
id: sty-rust-047
language: Rust
severity: error
message: >-
  a repository `search` returns a collection (`Result<Vec<_>, _>`) so that
  nothing matching yields an empty collection, never an `Option` or a single
  value
files:
  - src/lib/domain/port/*_repository.rs
rule:
  kind: function_signature_item
  all:
    - has:
        field: name
        regex: '^search(_|$)'
    - has:
        field: parameters
        has:
          kind: parameter
          has:
            field: type
            regex: 'Filter'
    - not:
        has:
          field: return_type
          regex: 'Vec\s*<'
```

`severity: error` makes `ast-grep scan` exit non-zero on a match, the idiom the existing wrappers rely on. `kind: function_signature_item` targets trait method declarations (the `#[cfg(test)] impl<T: …> … for &mut T` delegating methods are `function_item` and are compiler-mirrored, so they are not the check surface). `regex` on the `return_type` node matches its whole text (`impl Future<Output = Result<Vec<User>, RepositoryError>> + Send`), so `Vec\s*<` accepts every compliant `search` and the negated `not` fires on `Option<…>` or a bare single value. `files` scopes the scan to the repository port directory.

### `script/rule_enforcer.sh`

Two additions; the runner is not edited.

Append one line to the `RULES=( ... )` registry, after `"STY-RUST-024:check_sty_rust_024"` (line 71):

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-047:check_sty_rust_047"
)
```

Declare the wrapper after `check_sty_rust_024()` closes (line 752), before `# --- Runner ---` (line 754):

```bash
check_sty_rust_047() {
	local id="STY-RUST-047"
	local rule_id="sty-rust-047"
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

- Red: `/tmp/opencode/sty-rust-047/red/src/lib/domain/port/user_repository.rs` — a filter-taking `fn search(&mut self, filter: &UserFilter) -> impl Future<Output = Result<Option<User>, RepositoryError>> + Send` (line 10) and `fn search_items(...) -> impl Future<Output = Result<User, RepositoryError>> + Send` (line 16) — `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-047/red && ast-grep scan --config sgconfig.yaml --filter sty-rust-047 --report-style=short --color=never src'` — observed: two findings, `src/lib/domain/port/user_repository.rs:10:5: error[sty-rust-047]: a repository \`search\` returns a collection (\`Result<Vec<_>, _>\`) so that nothing matching yields an empty collection, never an \`Option\` or a single value` and the same at `16:5`, exit 1. The candidate wrapper over the same tree (`devenv shell -- bash /tmp/opencode/sty-rust-047/check047.sh /tmp/opencode/sty-rust-047/red`) observed `check rc=1`.
- Green: `/tmp/opencode/sty-rust-047/green/src/lib/domain/port/user_repository.rs` — the same `search` and `search_items`, both `-> impl Future<Output = Result<Vec<User>, RepositoryError>> + Send` — same command with `cd /tmp/opencode/sty-rust-047/green` — observed: no finding, exit 0; the wrapper observed `check rc=0`.
- Repository: `/tmp/opencode/sty-rust-047/repo-check/` (candidate rule + `sgconfig.yaml`, `src` symlinked to the repo `src`) — `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-047/repo-check && ast-grep scan --config sgconfig.yaml --filter sty-rust-047 --report-style=short --color=never src'` — observed: no finding, exit 0; the wrapper observed `check rc=0`. A companion positive probe over the same `files` glob and scan path listed all nine `search`-family signature items — `user_repository.rs:50`, `credential_repository.rs:50`, `file_repository.rs:34`, `upload_repository.rs:73`, `gallery_repository.rs:99` (`search`) and `:108` (`search_items`), `media_repository.rs:105` (`search_images`) and `:114` (`search_videos`), and the parameterless `configuration_repository.rs:29` — confirming the scan reads the port files; the eight filter-taking signatures each return `Result<Vec<…>, RepositoryError>` and the singleton is excluded by the filter clause.

## Next

- Apply: `rule/sty-rust-047.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
