# Add check — STY-RUST-082

## Rule guard — STY-RUST-082

- **Rule:** "Shared representations, links and helpers live in the bounded context module file (`<context>.rs`); a context folder holds only endpoint files, and an endpoint file imports no `<context>::<operation>` submodule — it takes shared items from the `<context>.rs` module file and `hal`." — `STY-RUST-082`, section **REST handlers** (`docs/development/TechnicalDesign.md`, line 192).
- **Check state:** missing — `STY-RUST-082` has no entry in `RULES=(...)` and no `check_sty_rust_082`; `script/rule_enforcer.sh uncovered` lists it.
- **Mechanism:** ast-grep (extraction) + bash pairing — the forbidden shape is a syntactic `use` path, but whether the segment after the context names an *operation* depends on the context folder's contents, which an ast-grep rule cannot read (the harness already pairs ast-grep output with filesystem facts in `check_sty_rust_006`, `_013`, `_015`, `_019`, `_023`).

## Files

### `rule/sty-rust-082.yml` (new)

```yaml
---
id: sty-rust-082
language: Rust
severity: hint
message: >-
  extract a bounded-context path an endpoint file reaches
  (`handler::<context>::<next>`), for STY-RUST-082
rule:
  any:
    # A flat or bare path: `handler::<context>::<next>` (and deeper).
    - pattern: crate::infrastructure::inbound::rest::handler::$CTX::$OP
    # A use tree: `handler::<context>::{ <next>, ... }`.
    - pattern: use crate::infrastructure::inbound::rest::handler::$CTX::{$$$ITEMS};
```

`severity: hint` (not `error`) so `ast-grep scan --json=stream` still exits 0 when it matches; the wrapper, not ast-grep, decides pass/fail.

### `script/rule_enforcer.sh`

Add one registry line after `"STY-RUST-024:check_sty_rust_024"` (line 71), inside `RULES=( ... )`:

```bash
	"STY-RUST-082:check_sty_rust_082"
```

Add the function after `check_sty_rust_024` (ends line 752), before `# --- Runner ---`:

```bash
check_sty_rust_082() {
	local id="STY-RUST-082"
	local rule_id="sty-rust-082"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local matches file context item op result=0

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	# sty-rust-082 extracts every endpoint-file path that reaches into a
	# bounded context (`handler::<context>::<next>`); the pairing below decides
	# whether `<next>` names the context's operation submodule — a fact an
	# ast-grep rule cannot see, because it cannot read the file system.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs 'src/lib/infrastructure/inbound/rest/handler/*/*.rs' \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi

	# Flat and bare imports: `handler::<context>::<next>`.
	while IFS=$'\t' read -r file context op; do
		[ -n "$op" ] || continue
		if [ -f "${handler_dir}/${context}/${op}.rs" ]; then
			printf 'error: %s: %s imports the %s::%s operation submodule; take shared items from %s.rs and hal\n' \
				"$id" "$file" "$context" "$op" "$context" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.OP) | [.file, .metaVariables.single.CTX.text, .metaVariables.single.OP.text] | @tsv')

	# Use-tree imports: `handler::<context>::{ <next>, ... }`.
	while IFS=$'\t' read -r file context item; do
		[ -n "$item" ] || continue
		op="${item%%[!A-Za-z0-9_]*}"
		if [ -f "${handler_dir}/${context}/${op}.rs" ]; then
			printf 'error: %s: %s imports the %s::%s operation submodule; take shared items from %s.rs and hal\n' \
				"$id" "$file" "$context" "$op" "$context" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.multi.ITEMS) | .file as $file | .metaVariables.single.CTX.text as $context | .metaVariables.multi.ITEMS[] | select(.text | test("^[A-Za-z_]")) | [$file, $context, .text] | @tsv')

	return "$result"
}
```

No `rule_enforcer.yaml` block is needed: the operation set is read from the context folder (`handler/<context>/<op>.rs`), not from config. `sgconfig.yaml` already discovers `rule/` (`ruleDirs: [rule]`), so it is unchanged.

## Verification

- **Red:** `/tmp/opencode/sc082/red/src/lib/infrastructure/inbound/rest/handler/user/get_me.rs` (with a sibling operation file `.../handler/user/get_user.rs` present) containing `use crate::infrastructure::inbound::rest::handler::user::get_user::get_user;` — `devenv shell -- bash /tmp/opencode/sc082/check.sh /tmp/opencode/sc082/red /tmp/opencode/sc082/sty-rust-082.yml` — observed: `error: STY-RUST-082: src/lib/infrastructure/inbound/rest/handler/user/get_me.rs imports the user::get_user operation submodule` (exit 1). The use-tree form `use crate::infrastructure::inbound::rest::handler::user::{ get_user::get_user, user_self_href };` in `/tmp/opencode/sc082/red_tree/...` is likewise flagged (exit 1).
- **Green:** `/tmp/opencode/sc082/green/src/lib/infrastructure/inbound/rest/handler/user/get_me.rs` containing only `use crate::infrastructure::inbound::rest::handler::user::user_self_href;`, `use crate::infrastructure::inbound::rest::hal::SelfLinks;` and `use serde::{Deserialize, Serialize};` — `devenv shell -- bash /tmp/opencode/sc082/check.sh /tmp/opencode/sc082/green /tmp/opencode/sc082/sty-rust-082.yml` — observed: no finding (exit 0). The `<context>.rs` module file at `/tmp/opencode/sc082/green_module/src/lib/infrastructure/inbound/rest/handler/user.rs`, which legitimately imports `handler::user::get_user::get_user`, is not scanned (exit 0), because the globs match only `handler/<context>/*.rs`.
- **Repository:** `devenv shell -- bash /tmp/opencode/sc082/check.sh /home/will/project/mine/Mnemorium/mnemorium /tmp/opencode/sc082/sty-rust-082.yml` — observed: no finding (exit 0); the extraction run returned 25 matches, every one a shared item (`user_self_href`, `GetUserResponse`, `parse_gallery_id`, `UploadSessionResponse`, …), none naming an operation module such as `get_user` or `post_gallery`.

## Next

- Apply: `rule/sty-rust-082.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
