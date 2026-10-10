# Add check — STY-RUST-058

## Rule guard — STY-RUST-058

- **Rule:** "Declare the error enum third in a use-case port file." (`docs/development/TechnicalDesign.md`, § 9.1 "Declaration order in a use-case port file", row `STY-RUST-058`, line 168). § 9.1 elaborates the order: command (`STY-RUST-056`), response when non-empty (`STY-RUST-057`), error enum (`STY-RUST-058`), use-case trait (`STY-RUST-059`).
- **Check state:** missing — `devenv shell -- script/rule_enforcer.sh uncovered` lists `STY-RUST-058`; it is absent from `RULES=(...)` and no `check_sty_rust_058` function or `rule/sty-rust-058.yml` exists.
- **Mechanism:** ast-grep — the rule is a same-file ordering relation between top-level items, which ast-grep's `precedes`/`follows` relations express directly (mechanism 2). The wrapper only invokes the scan and passes the port-file glob; no `rule_enforcer.yaml` block is needed (the check reads no data).

The ordering is checked relative to the items § 9.1 names, not by counting all top-level structs, because two port files legitimately declare a read model before the response (`get_gallery.rs` has `GalleryItemSummary` at line 45, `list_galleries.rs` has `ListGalleriesItem` at line 87). A literal "third text declaration" count would falsely flag both. The check therefore requires the `*Error` enum to come **after the `*Command` and `*Response` structs (where present) and before the use-case trait**.

## Files

### `rule/sty-rust-058.yml`

New ast-grep rule. No `files:` key: the port-directory scope is passed by the wrapper through `--globs` (the idiom of `check_sty_rust_003`). An `enum_item` whose text begins `pub enum …Error` is a violation when it follows the trait, or precedes the command or response struct.

```yaml
---
id: sty-rust-058
language: Rust
severity: error
message: >-
  declare the error enum third in a use-case port file: after the command and
  response objects, before the use-case trait
rule:
  all:
    - kind: enum_item
    - regex: '^pub enum \w*Error\b'
    - any:
        - follows:
            kind: trait_item
            stopBy: end
        - precedes:
            kind: struct_item
            regex: 'struct \w*Command\b'
            stopBy: end
        - precedes:
            kind: struct_item
            regex: 'struct \w*Response\b'
            stopBy: end
```

### `script/rule_enforcer.sh`

Two additions, both inside this file; the runner is never edited.

1. Register the check by appending one line to `RULES=(...)`, immediately after the existing `"STY-RUST-024:check_sty_rust_024"` entry (line 71), before the closing `)`:

```bash
	"STY-RUST-058:check_sty_rust_058"
```

2. Declare the check function immediately after `check_sty_rust_024()` (which ends at line 752) and before the `# --- Runner ---` divider (line 754):

```bash
check_sty_rust_058() {
	local id="STY-RUST-058"
	local rule_id="sty-rust-058"
	local port_dir="src/lib/application/port"
	local output

	if [ ! -d "$port_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$port_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs 'src/lib/application/port/*.rs' \
		--globs '!src/lib/application/port/*_use_case_factory.rs' \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}
```

Notes: the function runs under the runner's `cd -- "$REPO_ROOT"`, so `src` and the globs resolve. It calls `ast-grep` directly with no `command -v` install guard, matching the other checks. The second `--globs` excludes the `*_use_case_factory.rs` port files, which carry no error enum.

`rule_enforcer.yaml` is unchanged: the check reads no per-check data, so it adds no `rules.STY-RUST-058` block.

## Verification

The candidate rule file is at `/tmp/opencode/sty-rust-058/rule/sty-rust-058.yml` (byte-identical to the proposal); the candidate check function is `/tmp/opencode/sty-rust-058/check.sh` (fixtures define `SGCONFIG=sgconfig.yaml` and `cd` into the fixture root) and `/tmp/opencode/sty-rust-058/check_repo.sh` (same function, `SGCONFIG` pointed at the `/tmp` config for the read-only repo run).

- **Red:** `/tmp/opencode/sty-rust-058/red/src/lib/application/port/` — three violating fixtures: `create_note.rs` (error enum first, before `CreateNoteCommand`), `before_response.rs` (error enum before `CreateNoteResponse`), `after_trait.rs` (error enum after `CreateNoteUseCase`). Command: `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-058/red && bash /tmp/opencode/sty-rust-058/check.sh'` — observed: three findings, `src/lib/application/port/after_trait.rs:12:1`, `before_response.rs:7:1`, `create_note.rs:7:1`, each `declare the error enum third in a use-case port file: after the command and response objects, before the use-case trait`; exit `1`.

- **Green:** `/tmp/opencode/sty-rust-058/green/src/lib/application/port/` — `create_note.rs` (command, response, error, trait), `no_command.rs` (response, error, trait), `no_response.rs` (command, error, trait). Command: `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-058/green && bash /tmp/opencode/sty-rust-058/check.sh'` — observed: no output, exit `0`.

- **Repository:** the exact check function run read-only against the working tree: `devenv shell -- bash /tmp/opencode/sty-rust-058/check_repo.sh` (from the repository root) — observed: no output, exit `0`. The twenty-one `src/lib/application/port/*.rs` use-case files, including `get_gallery.rs` and `list_galleries.rs` (read model declared before the response) and `initialize_root_admin.rs` / `load_configuration.rs` (no command), are all compliant. Equivalently, the raw scan was `devenv shell -- bash -c 'ast-grep scan --config /tmp/opencode/sty-rust-058/sgconfig.yaml --filter sty-rust-058 --globs "src/lib/application/port/*.rs" --globs "!src/lib/application/port/*_use_case_factory.rs" --report-style=short --color=never src'` — observed: no findings.

## Next

- Apply: `rule/sty-rust-058.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
