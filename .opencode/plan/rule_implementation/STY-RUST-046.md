# Add check — STY-RUST-046

## Rule guard — STY-RUST-046

- **Rule:** "`delete` returns `Ok(false)` when no row matched; a missing entity is not an error." — rulebook row `| STY-RUST-046 | Repository | ... | § 8.1 Repository port |`, `docs/development/TechnicalDesign.md:156`.
- **Check state:** missing — `script/rule_enforcer.sh list` does not register `STY-RUST-046`, and `script/rule_enforcer.sh uncovered` lists it among the unguarded rows.
- **Mechanism:** ast-grep — the enforceable part of § 8.1 is a signature: a repository delete must yield `Result<bool, _>` so a no-match can be `Ok(false)`. A negative ast-grep rule plus the standard thin wrapper (the `check_sty_rust_004` shape) expresses this syntactically; no `rule_enforcer.yaml` block is needed.

The rule constrains the port in `src/lib/domain/port/<aggregate>_repository.rs`, whose methods are trait declarations, i.e. tree-sitter kind `function_signature_item`. The name regex `^delete(_|$)` covers the delete-operation family the ports actually expose — `delete`, `delete_item` (`gallery_repository.rs`), `delete_image`, `delete_video` (`media_repository.rs`) — while excluding unrelated names such as `deleted`. The adapter is forced to match the trait by the compiler, so the port is the check surface.

## Files

### `rule/sty-rust-046.yml` (new)

```yaml
---
id: sty-rust-046
language: Rust
severity: error
message: >-
  a repository `delete` returns `Result<bool, _>` so a no-match is `Ok(false)`,
  not an error
files:
  - src/lib/domain/port/*_repository.rs
rule:
  kind: function_signature_item
  has:
    field: name
    regex: '^delete(_|$)'
  not:
    has:
      field: return_type
      regex: 'Result<\s*bool\b'
```

`sgconfig.yaml` already declares `ruleDirs: [rule]`, so no change is needed there.

### `script/rule_enforcer.sh`

Add one line to the `RULES=( ... )` registry (append after the last entry):

```bash
	"STY-RUST-067:check_sty_rust_067"
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-046:check_sty_rust_046"
)
```

Add one function after `check_sty_rust_024` closes (line 752), before `# --- Runner ---`:

```bash
check_sty_rust_046() {
	local id="STY-RUST-046"
	local rule_id="sty-rust-046"
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

- Red: `/tmp/opencode/sty-rust-046/final/src/lib/domain/port/user_repository.rs` declaring `fn delete_item(&mut self, id: u64) -> impl Future<Output = Result<(), RepositoryError>> + Send;` — `devenv shell -- bash -c "cd /tmp/opencode/sty-rust-046/final && ast-grep scan --config sgconfig.yaml --filter sty-rust-046 --report-style=short --color=never src"` — observed: `src/lib/domain/port/user_repository.rs:3:5: error[sty-rust-046]: a repository `delete` returns `Result<bool, _>` so a no-match is `Ok(false)`, not an error`, exit 1. The `check_sty_rust_046` wrapper over the same tree returns `rc=1` and prints that line. Variants `delete -> Result<(),_>`, `delete -> Result<User,_>`, and `delete -> u64` are each flagged too.
- Green: `/tmp/opencode/sty-rust-046/final/src/lib/domain/port/user_repository.rs` (a `delete` and a `delete_item`, both `-> ... Result<bool, RepositoryError> ...`, plus a `deleted` method returning `Result<(),_>` and an out-of-glob `helper_ports.rs` whose `delete` returns `Result<(),_>`) with the same command — observed: no finding, exit 0; the wrapper returns `rc=0`. This shows the family regex does not over-match `deleted`, and the file glob excludes `helper_ports.rs`.
- Repository: `devenv shell -- bash -c "ast-grep scan --config /tmp/opencode/sty-rust-046/repo_probe/sgconfig.yaml --filter sty-rust-046 --report-style=short --color=never src"` (candidate rule mounted through a scratch config at the repo root) — observed: no finding, exit 0. A positive probe over the same glob lists all seven delete-family signatures, each `Result<bool, RepositoryError>`: `gallery_repository.rs:65` (`delete`), `gallery_repository.rs:75` (`delete_item`), `media_repository.rs:70` (`delete_image`), `media_repository.rs:80` (`delete_video`), `upload_repository.rs:36`, `user_repository.rs:37`, `credential_repository.rs:34`.

## Next

- Apply: `rule/sty-rust-046.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
