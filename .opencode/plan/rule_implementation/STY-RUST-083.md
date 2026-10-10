# Add check — STY-RUST-083

## Rule guard — STY-RUST-083

- **Rule:** ``rest/`` holds only the known REST infrastructure (`api_error.rs`, `app_state.rs`, `hal.rs`, `handler.rs`, `handler`, `middleware.rs`, `middleware`); `handler/` holds only `get_health.rs` plus each bounded context's `<context>.rs` module file and `<context>/` endpoint folder.
- **Check state:** missing — `STY-RUST-083` has no registry entry (`list` omits it) and appears in `uncovered`; no `rule/sty-rust-083.yml`, no `rules.STY-RUST-083` block, no `check_sty_rust_083`.
- **Mechanism:** bash — the check enumerates directory entries and compares them against an allowlist, which ast-grep cannot see (it cannot list a directory or read `rule_enforcer.yaml`). The bounded-context names are the existing `rules.STY-RUST-002.bounded_contexts` registry; they are surfaced under `rules.STY-RUST-083` through a YAML alias so the check reads its own block without creating a second source of truth (the `STY-RUST-076` rationale). No `rule/*.yml` and no `sgconfig.yaml` change.

## Files

### `script/rule_enforcer.sh`

Add one registry line after `"STY-RUST-024:check_sty_rust_024"` (line 71, the last entry before the closing `)`):

```
 	"STY-RUST-024:check_sty_rust_024"
+	"STY-RUST-083:check_sty_rust_083"
 )
```

Add the check function after `check_sty_rust_024` (ends line 752) and before `# --- Runner ---` (line 754):

```bash
check_sty_rust_083() {
	local id="STY-RUST-083"
	local rest_dir="src/lib/infrastructure/inbound/rest"
	local handler_dir="$rest_dir/handler"
	local config context name entry result=0
	local -a contexts=()
	local -A allowed_rest=()
	local -A allowed_handler=()

	if [ ! -d "$rest_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$rest_dir" >&2
		return 1
	fi
	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	config="$(rule_config "$id")" || return 2
	mapfile -t contexts < <(jq -r '.bounded_contexts[]?' <<<"$config")
	if [ "${#contexts[@]}" -eq 0 ]; then
		printf 'error: %s: rule_enforcer.yaml declares no bounded_contexts\n' "$id" >&2
		return 2
	fi

	for name in api_error.rs app_state.rs hal.rs handler.rs handler middleware.rs middleware; do
		allowed_rest["$name"]=1
	done

	allowed_handler["get_health.rs"]=1
	for context in "${contexts[@]}"; do
		allowed_handler["${context}.rs"]=1
		allowed_handler["$context"]=1
	done

	while IFS= read -r entry; do
		name="${entry##*/}"
		if [ -z "${allowed_rest[$name]+set}" ]; then
			printf 'error: %s: %s is not known REST infrastructure\n' "$id" "$entry" >&2
			result=1
		fi
	done < <(find "$rest_dir" -mindepth 1 -maxdepth 1 | sort)

	while IFS= read -r entry; do
		name="${entry##*/}"
		if [ -z "${allowed_handler[$name]+set}" ]; then
			printf 'error: %s: %s is not a known handler module or bounded-context endpoint folder\n' "$id" "$entry" >&2
			result=1
		fi
	done < <(find "$handler_dir" -mindepth 1 -maxdepth 1 | sort)

	return "$result"
}
```

The rule's word "only" is enforced as an upper bound on the entries under each directory; the presence of each bounded context's module and folder is `STY-RUST-002`'s job, so this check does not duplicate it. The function calls only `find`, `jq` and `printf`, so it adds no `command -v` install guard.

### `rule_enforcer.yaml`

Anchor the existing bounded-context list on `STY-RUST-002` and alias it under `STY-RUST-083`, so the check's own block resolves to the same list (verified: `yq -c '.rules["STY-RUST-083"] // {}'` prints `{"bounded_contexts":["asset","identity","library","user"]}`):

```diff
 rules:
   STY-RUST-002:
-    bounded_contexts:
+    bounded_contexts: &bounded_contexts
       - asset
       - identity
       - library
       - user
+  STY-RUST-083:
+    bounded_contexts: *bounded_contexts
```

No `rule/sty-rust-083.yml` is added (the check calls no ast-grep) and `sgconfig.yaml` is unchanged.

## Verification

The candidate function was tested verbatim inside a scratch harness (`/tmp/opencode/sty083/check.sh`) that supplies the runner's `rule_config` backed by `yq` over the proposed config at `/tmp/opencode/sty083/rule_enforcer.yaml`, `cd`s into the fixture root, and runs `check_sty_rust_083`.

- Red: `/tmp/opencode/sty083/red1/src/lib/infrastructure/inbound/rest/route.rs` (an unknown file directly under `rest/`) — `devenv shell -- bash /tmp/opencode/sty083/check.sh /tmp/opencode/sty083/red1` — observed: rc=1 and `error: STY-RUST-083: src/lib/infrastructure/inbound/rest/route.rs is not known REST infrastructure`.
- Red: `/tmp/opencode/sty083/red2/src/lib/infrastructure/inbound/rest/handler/notes.rs` (an unknown context module file) — `devenv shell -- bash /tmp/opencode/sty083/check.sh /tmp/opencode/sty083/red2` — observed: rc=1 and `error: STY-RUST-083: src/lib/infrastructure/inbound/rest/handler/notes.rs is not a known handler module or bounded-context endpoint folder`.
- Red: `/tmp/opencode/sty083/red3/src/lib/infrastructure/inbound/rest/handler/notes` (an unknown context folder) — `devenv shell -- bash /tmp/opencode/sty083/check.sh /tmp/opencode/sty083/red3` — observed: rc=1 and `error: STY-RUST-083: src/lib/infrastructure/inbound/rest/handler/notes is not a known handler module or bounded-context endpoint folder`.
- Red: `/tmp/opencode/sty083/red4/src/lib/infrastructure/inbound/rest/routes` (an unknown directory under `rest/`) — `devenv shell -- bash /tmp/opencode/sty083/check.sh /tmp/opencode/sty083/red4` — observed: rc=1 and `error: STY-RUST-083: src/lib/infrastructure/inbound/rest/routes is not known REST infrastructure`.
- Green: `/tmp/opencode/sty083/green/` (exactly the seven `rest/` entries and `handler/` holding `get_health.rs`, `<context>.rs` and `<context>/` for `asset`/`identity`/`library`/`user`) — `devenv shell -- bash /tmp/opencode/sty083/check.sh /tmp/opencode/sty083/green` — observed: rc=0, no finding.
- Repository: `devenv shell -- bash /tmp/opencode/sty083/check.sh /home/will/project/mine/Mnemorium/mnemorium` — observed: rc=0, no findings; `rest/` lists exactly `api_error.rs`, `app_state.rs`, `hal.rs`, `handler`, `handler.rs`, `middleware`, `middleware.rs`, and `handler/` lists only `get_health.rs`, `{asset,identity,library,user}.rs` and `{asset,identity,library,user}/`.
- Lint: `devenv shell -- shfmt -d -i 0 -ci /tmp/opencode/sty083/check.sh` — observed: rc=0, no diff; `devenv shell -- shellcheck /tmp/opencode/sty083/check.sh` — observed: rc=0, no findings.

## Next

- Apply: `script/rule_enforcer.sh`, `rule_enforcer.yaml`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
