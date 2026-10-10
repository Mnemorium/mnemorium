# Add check — STY-RUST-045

## Rule guard — STY-RUST-045

- **Rule:** "`save` upserts an existing aggregate targeted by its identifier." (`docs/development/TechnicalDesign.md`, row `STY-RUST-045`, § 8.1 Repository port, category **Repository**). § 8.1 elaborates: the port's default mutation is `save(domain) -> Result<domain, _>`, an "Insert/Update (upsert by identity)"; an aggregate may instead expose narrow mutators (e.g. `UploadRepository` has no `save`), so the rule is conditional on a `save` existing.
- **Check state:** missing — `STY-RUST-045` has no registry line and no `check_sty_rust_045`; `script/rule_enforcer.sh list` omits it, `uncovered` lists it (`STY-RUST-045 (Repository) — \`save\` upserts an existing aggregate targeted by its identifier.`), and there is no `rule/sty-rust-045.yml`.
- **Mechanism:** ast-grep — the rule is a syntactic property of the adapter's `save` body: it must contain an upsert `INSERT … ON CONFLICT (<id>) DO UPDATE …`. One extraction-level ast-grep rule flags any `fn save` whose source lacks that pattern; the thin Bash wrapper runs it. No `rule_enforcer.yaml` block (no per-check data) and no `sgconfig.yaml` change (`ruleDirs: [rule]` already picks it up). The identity column target (`…_id`) is the project invariant from `STY-SQL-008` (primary keys are `<table_name>_id`).

## Files

### `rule/sty-rust-045.yml` (new)

New ast-grep rule. `severity: error` makes `ast-grep scan` exit non-zero on a match, the idiom the existing error-severity wrappers rely on (`rule/sty-rust-020.yml`, `sty-rust-024.yml`). `regex` is applied to the `function_item` node's whole source span, so `(?s)` lets `.`/quantifiers span the multi-line SQL literal; the negated `regex` fires when no identity-targeted upsert is present. `files` scopes the rule to the sqlx repository adapters, so the `#[cfg(test)] impl<T: …> … for &mut T` delegating `fn save` in `src/lib/domain/port/*.rs` is never matched.

```yaml
---
id: sty-rust-045
language: Rust
severity: error
message: >-
  save upserts an existing aggregate targeted by its identifier: use
  INSERT ... ON CONFLICT (<id>) DO UPDATE SET ..., never a plain INSERT or UPDATE
files:
  - src/lib/infrastructure/outbound/sqlx/*_repository.rs
rule:
  all:
    - kind: function_item
    - has:
        field: name
        regex: '^save$'
    - not:
        regex: '(?s)ON\s+CONFLICT\s*\(\s*[a-z_]*_id\s*\)\s*DO\s+UPDATE'
```

### `script/rule_enforcer.sh`

Two additions. Register the check in `RULES`, appending after `"STY-RUST-024:check_sty_rust_024"` (line 71):

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-045:check_sty_rust_045"
)
```

Declare the wrapper after `check_sty_rust_024()` (after line 752), before `# --- Runner ---` (line 754):

```bash
check_sty_rust_045() {
	local id="STY-RUST-045"
	local rule_id="sty-rust-045"
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

- Red (plain `INSERT`, no upsert): `/tmp/opencode/ruleguard045/work/red/src/lib/infrastructure/outbound/sqlx/user_repository.rs` (`save` issues `INSERT INTO user (…) VALUES (…) RETURNING …`) — `devenv shell -- bash -c 'cd /tmp/opencode/ruleguard045/work/red && ast-grep scan --config sgconfig.yaml --filter sty-rust-045 --report-style=short --color=never src'` — observed: `src/lib/infrastructure/outbound/sqlx/user_repository.rs:25:5: error[sty-rust-045]: save upserts an existing aggregate targeted by its identifier…`, rc=1.
- Red (upsert targeting the wrong column): `/tmp/opencode/ruleguard045/work/red/src/lib/infrastructure/outbound/sqlx/gallery_repository.rs` (`save` issues `ON CONFLICT (name) DO UPDATE …`, not the identity) — same command — observed: `src/lib/infrastructure/outbound/sqlx/gallery_repository.rs:15:5: error[sty-rust-045]: …`, rc=1 (2 errors total).
- Green: `/tmp/opencode/ruleguard045/work/green/src/lib/infrastructure/outbound/sqlx/user_repository.rs` (`save` issues `INSERT … ON CONFLICT (user_id) DO UPDATE SET …`) — `devenv shell -- bash -c 'cd /tmp/opencode/ruleguard045/work/green && ast-grep scan --config sgconfig.yaml --filter sty-rust-045 --report-style=short --color=never src'` — observed: no finding, rc=0. Wrapper run (`bash /tmp/opencode/ruleguard045/work/check045.sh`) observed `check rc=0`.
- Repository: `/tmp/opencode/ruleguard045/repo-check/` (candidate rule + `sgconfig.yaml`, `src` symlinked to the repo `src`) — `devenv shell -- bash -c 'cd /tmp/opencode/ruleguard045/repo-check && ast-grep scan --config sgconfig.yaml --filter sty-rust-045 --report-style=short --color=never src'` — observed: rc=0, no findings; the wrapper run observed `check rc=0`. A companion extraction rule over the same `files` glob and scan path matched the four real `save` sites (`configuration_repository.rs:152`, `gallery_repository.rs:122`, `credential_repository.rs:50`, `user_repository.rs:59`), confirming the green run scanned the adapters rather than skipping them.

## Next

- Apply: `rule/sty-rust-045.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
