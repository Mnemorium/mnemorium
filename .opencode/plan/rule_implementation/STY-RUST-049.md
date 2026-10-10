# Add check — STY-RUST-049

## Rule guard — STY-RUST-049

- **Rule:** "It holds the transaction it operates on (`&'transaction mut Transaction<'static, Sqlite>` for SQLite) and is built by the unit-of-work accessor — never with a connection pool." — rulebook row `| STY-RUST-049 | Repository | ... | [§ 8.2](#82-repository-adapter) |`, `docs/development/TechnicalDesign.md:159`; § 8.2 ("Repository adapter") states the implementation is named `<Tech><Aggregate>Repository` and lives in `infrastructure/outbound/<tech>/`.
- **Check state:** missing — `script/rule_enforcer.sh list` does not register `STY-RUST-049`, `script/rule_enforcer.sh uncovered` prints `STY-RUST-049 (Repository) — It holds the transaction ... never with a connection pool.`, and there is no `rule/sty-rust-049.yml`.
- **Mechanism:** ast-grep — the enforceable, local part of § 8.2 is the adapter struct's held type: every `<Tech><Aggregate>Repository` struct in the sqlx adapter directory must hold a `Transaction` field and must not hold a `Pool` field. One negative ast-grep rule plus the standard thin wrapper (the `check_sty_rust_004` shape) expresses both clauses syntactically; no `rule_enforcer.yaml` block (no per-check data) and no `sgconfig.yaml` change (`ruleDirs: [rule]` already picks it up). The cross-file "built by the unit-of-work accessor" clause is not directly checked: the constructor is forced to take the same field type, so the held type is the local proxy.

## Files

### `rule/sty-rust-049.yml` (new)

```yaml
---
id: sty-rust-049
language: Rust
severity: error
message: >-
  a repository adapter holds the unit-of-work transaction
  (`&'transaction mut Transaction<'static, Sqlite>` for SQLite), never a
  connection pool
files:
  - src/lib/infrastructure/outbound/sqlx/*.rs
rule:
  all:
    - kind: struct_item
    - has:
        field: name
        regex: 'Repository$'
    - any:
        - not:
            has:
              kind: field_declaration
              regex: 'Transaction'
              stopBy: end
        - has:
            kind: field_declaration
            regex: 'Pool'
            stopBy: end
```

`kind: struct_item` (not a full `pub struct $NAME { ... }` pattern) is required: every real adapter is generic (`pub struct SqlxUserRepository<'transaction> { ... }`), and a brace-only pattern silently skips it. `field: name` with `regex: 'Repository$'` scopes the rule to the adapter family without a meta-variable, so it holds for generic and non-generic structs alike. `stopBy: end` is required because `has` does not descend into the `field_declaration_list` by default. `severity: error` makes `ast-grep scan` exit non-zero on a match, which the wrapper relies on. `files` scopes the rule to the sqlx adapter directory, so the port traits under `src/lib/domain/port/*_repository.rs` and the data-model structs under `src/lib/infrastructure/outbound/sqlx/model/` are never matched. The `not`/`has` `regex` reads the field's declaration text (name and type), not the preceding doc comment, so a comment that merely says "Pool" or "Transaction" neither triggers nor masks a finding.

### `script/rule_enforcer.sh`

Two additions. Register the check in `RULES`, appending after `"STY-RUST-024:check_sty_rust_024"` (line 71):

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-049:check_sty_rust_049"
)
```

Declare the wrapper after `check_sty_rust_024()` closes (after line 752), before `# --- Runner ---` (line 754):

```bash
check_sty_rust_049() {
	local id="STY-RUST-049"
	local rule_id="sty-rust-049"
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

- Red: `/tmp/opencode/sty-rust-049/red/src/lib/infrastructure/outbound/sqlx/` holds four violating adapters — `media_repository.rs` (`pub struct SqlxMediaRepository<'transaction> { transaction: &'transaction mut Transaction<'static, Sqlite>, pool: SqlitePool }`, a transaction *and* a pool, proving the pool arm fires on its own), `user_repository.rs` (`pub struct SqlxUserRepository { pool: SqlitePool }`, no transaction), `upload_repository.rs` (`pub struct SqlxUploadRepository<'transaction> { transaction: SqlitePool, ... }`, generic and pool-typed), and `gallery_repository.rs` (`pub struct SqlxGalleryRepository { name: String }`, no transaction) — `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-049/red && ast-grep scan --config sgconfig.yaml --filter sty-rust-049 --report-style=short --color=never src'` — observed: `error[sty-rust-049]` at `media_repository.rs:6:1`, `user_repository.rs:4:1`, `upload_repository.rs:4:1`, `gallery_repository.rs:2:1` (`Error: 4 error(s) found in code.`). The `check_sty_rust_049` wrapper over the same tree (`bash /tmp/opencode/sty-rust-049/check049.sh`) printed those four lines and returned `check rc=1`.
- Green: `/tmp/opencode/sty-rust-049/green/src/lib/infrastructure/outbound/sqlx/user_repository.rs` declares `pub struct SqlxUserRepository<'transaction> { transaction: &'transaction mut Transaction<'static, Sqlite>, limit: usize }` and, in the same file, the non-repository `pub struct SqlxUnitOfWorkFactory { pool: SqlitePool }` — `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-049/green && ast-grep scan --config sgconfig.yaml --filter sty-rust-049 --report-style=short --color=never src'` — observed: no finding, `green_rc=0`. The wrapper returned `check rc=0`. A positive probe (`/tmp/opencode/sty-rust-049/green/rule/probe-049.yml`, `--filter probe-049`) reported `user_repository.rs:6:1`, confirming the compliant struct was scanned rather than skipped. A companion doc-comment probe (`/tmp/opencode/sty-rust-049/dbg2/src/comment_repo.rs`, glob widened to `src/*.rs`) showed a doc comment containing "Pool" on a transaction field produced no finding and a doc comment containing "Transaction" on a non-transaction field still produced one, so the regex reads the field declaration, not the comment.
- Repository: `/tmp/opencode/sty-rust-049/repo/` (candidate rule + `sgconfig.yaml`, `src` symlinked to the repository `src`) — `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-049/repo && ast-grep scan --config sgconfig.yaml --filter sty-rust-049 --report-style=short --color=never src'` — observed: no finding, `cand_rc=0`. The extraction probe (`--filter probe-049`) over the same tree listed all eight real sqlx adapters — `user_repository.rs:16`, `gallery_repository.rs:22`, `upload_repository.rs:18`, `credential_repository.rs:14`, `mime_type_repository.rs:8`, `configuration_repository.rs:24`, `media_repository.rs:21`, `file_repository.rs:14` — confirming the green run scanned the adapters rather than skipping them.

## Next

- Apply: `rule/sty-rust-049.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
