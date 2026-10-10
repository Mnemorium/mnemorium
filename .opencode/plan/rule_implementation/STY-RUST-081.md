# Add check — STY-RUST-081

## Rule guard — STY-RUST-081

- **Rule:** "A query whose result carries data the aggregate does not declare returns a dedicated read-model type; the use case never stitches it from per-aggregate lookups." (`docs/development/TechnicalDesign.md`, § Code Style Guidelines, `Repository` group, row `STY-RUST-081`, line 191; § 8.3 adds "a read model returned by `search` … follows the trait").
- **Check state:** missing — `devenv shell -- script/rule_enforcer.sh uncovered` lists `STY-RUST-081 (Repository)`; it has no `RULES=(...)` entry, no `check_sty_rust_081`, and no `rule/sty-rust-081.yml`.
- **Mechanism:** ast-grep + bash — the enforceable half of the rule is that a `search*` query in a repository port file returns either an aggregate the repository already handles or a read-model struct declared in that same port file (§ 8.3); the identifier-to-declaration pairing is beyond ast-grep, so the harness idiom (`check_sty_rust_013`, `check_sty_rust_019`, `check_sty_rust_023`) extracts the signatures/structs with ast-grep and relates them in Bash. No `rule_enforcer.yaml` block is needed. The second clause ("the use case never stitches it") is enforced structurally: a query that would carry undeclared data must name a dedicated type at the port boundary, rather than returning an anonymous/foreign type the use case assembles.

## Files

### `rule/sty-rust-081.yml` (new)

Extracts every `RepositoryError`-returning trait method signature and every `pub struct` declared in a repository port file. The `files:` glob scopes it to the governed surface; being a `hint` extractor it emits findings for the Bash layer, it is not itself the verdict.

```yaml
---
id: sty-rust-081
language: Rust
severity: hint
message: extract repository-port query signatures and declared structs for STY-RUST-081
files:
  - src/lib/domain/port/*_repository.rs
rule:
  any:
    - all:
        - kind: function_signature_item
        - pattern: 'fn $METHOD($$$ARGS) -> impl Future<Output = Result<$RET, RepositoryError>> + Send;'
    - all:
        - kind: struct_item
        - pattern: 'pub struct $STRUCT'
```

`sgconfig.yaml` already sets `ruleDirs: [rule]`, so no registration is needed.

### `script/rule_enforcer.sh` (edit)

The runner is never edited. Two additions.

1. Register the check by adding one line to `RULES=(...)`, immediately after the existing `"STY-RUST-024:check_sty_rust_024"` entry (line 71), before the closing `)`:

```bash
	"STY-RUST-081:check_sty_rust_081"
```

2. Declare the function immediately after `check_sty_rust_024()` (which ends at line 752) and before the `# --- Runner ---` divider (line 754):

```bash
check_sty_rust_081() {
	local id="STY-RUST-081"
	local rule_id="sty-rust-081"
	local port_dir="src/lib/domain/port"
	local matches file method ret element key result=0
	local -A structs=()
	local -A aggregates=()

	if [ ! -d "$port_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$port_dir" >&2
		return 1
	fi

	# sty-rust-081 extracts each repository-port method signature and each
	# struct declared in a port file; the element-type-to-aggregate/read-model
	# pairing happens here because an ast-grep rule cannot relate two
	# identifiers.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$port_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no repository-port signature from %s\n' "$id" "$port_dir" >&2
		return 1
	fi

	# Read models: structs declared in the port file itself.
	while IFS=$'\t' read -r file element; do
		[ -n "$element" ] || continue
		structs["${file}:${element}"]=1
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.STRUCT) | [.file, .metaVariables.single.STRUCT.text] | @tsv')

	# Aggregates: what the trait's non-query methods return, so a query may
	# return one of the aggregates this repository owns.
	while IFS=$'\t' read -r file method ret; do
		[ -n "$method" ] || continue
		case "$method" in
		search*) continue ;;
		esac
		element="$(printf '%s' "$ret" | sed -E 's/^(Vec|Option)<(.*)>$/\2/')"
		aggregates["${file}:${element}"]=1
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.METHOD) | [.file, .metaVariables.single.METHOD.text, .metaVariables.single.RET.text] | @tsv')

	# Every query must return the aggregate or a read-model struct declared in
	# the same port file.
	while IFS=$'\t' read -r file method ret; do
		[ -n "$method" ] || continue
		case "$method" in
		search*) ;;
		*) continue ;;
		esac
		element="$(printf '%s' "$ret" | sed -E 's/^(Vec|Option)<(.*)>$/\2/')"
		key="${file}:${element}"
		if [ -z "${structs[$key]+set}" ] && [ -z "${aggregates[$key]+set}" ]; then
			printf 'error: %s: %s: query %s returns %s; a query result must be an aggregate the repository returns or a read-model struct declared in the port file\n' \
				"$id" "$file" "$method" "$ret" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.METHOD) | [.file, .metaVariables.single.METHOD.text, .metaVariables.single.RET.text] | @tsv')

	return "$result"
}
```

## Verification

Scratch mirror: `/tmp/opencode/sty-rust-081/mirror` holds the candidate `sgconfig.yaml` (`ruleDirs: [rule]`), the candidate `rule/sty-rust-081.yml`, and a byte-copy of the repository's `src/lib/domain/port/*.rs`. `/tmp/opencode/sty-rust-081/check.sh` holds the candidate function verbatim (plus `set -u -o pipefail` and `SGCONFIG="sgconfig.yaml"`) and runs it with cwd at the mirror root. `red-mirror`, `red2-mirror`, and `green-mirror` are copies with one port file patched.

- Red (undeclared read-model type): `/tmp/opencode/sty-rust-081/red-mirror/src/lib/domain/port/user_repository.rs` gains `fn search_details(…) -> impl Future<Output = Result<Vec<UserDetail>, RepositoryError>> + Send;` with no `UserDetail` declared in the file — `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-081/red-mirror && bash /tmp/opencode/sty-rust-081/check.sh; echo exit=$?'` — observed: `exit=1` and `error: STY-RUST-081: src/lib/domain/port/user_repository.rs: query search_details returns Vec<UserDetail>; a query result must be an aggregate the repository returns or a read-model struct declared in the port file`.
- Red (type not the repository's aggregate): `/tmp/opencode/sty-rust-081/red2-mirror/src/lib/domain/port/file_repository.rs` gains `fn search_credentials(…) -> … Result<Vec<Credential>, RepositoryError> …` (a foreign domain model, not a local read model) — same command with `…/red2-mirror` — observed: `exit=1`, the same diagnostic naming `search_credentials` / `Vec<Credential>`.
- Green (real port files): `/tmp/opencode/sty-rust-081/mirror` (copies of `src/lib/domain/port/*.rs`) — `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-081/mirror && bash /tmp/opencode/sty-rust-081/check.sh; echo exit=$?'` — observed: `exit=0`, no output. Every query resolves: `configuration_repository.search → Option<Configuration>` (`Configuration` is its aggregate), `credential_repository.search → Vec<Credential>`, `file_repository.search → Vec<File>`, `gallery_repository.search → Vec<Gallery>`, `gallery_repository.search_items → Vec<GalleryItemDetail>` (the read model declared in the file), `media_repository.search_images/search_videos → Vec<Image>/Vec<Video>`, `upload_repository.search → Vec<Upload>`, `user_repository.search → Vec<User>`.
- Green (declared read model): `/tmp/opencode/sty-rust-081/green-mirror/src/lib/domain/port/user_repository.rs` adds `search_details` and a matching `pub struct UserDetail` in the same port file — same command with `…/green-mirror` — observed: `exit=0`, no output.
- Repository: raw extraction over the real tree — `devenv shell -- ast-grep scan --rule /tmp/opencode/sty-rust-081/mirror/rule/sty-rust-081.yml --json=stream src/lib/domain/port` — observed: only the 35 method signatures and 9 struct declarations listed above; the candidate check run against the real repository (`SGCONFIG` pointed at `/tmp/opencode/sty-rust-081/repo-obs/sgconfig.yaml`, cwd at the repository root) — observed: `exit=0`, no findings.

## Next

- Apply: `rule/sty-rust-081.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
