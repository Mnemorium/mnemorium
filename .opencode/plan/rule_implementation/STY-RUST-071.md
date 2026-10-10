# Add check — STY-RUST-071

## Rule guard — STY-RUST-071

- **Rule:** "Repository methods mutate the shared transaction: take `&mut self` and return an explicitly `Send` future." (rulebook row `STY-RUST-071`, section *Ports*; detail in § 6.3 *Async methods take `&mut self` and return `Send` futures*, which anchors on `domain/port/<aggregate>_repository.rs` and the form `fn create(&mut self, ...) -> impl Future<Output = ...> + Send;`)
- **Check state:** missing — `script/rule_enforcer.sh list` has no `STY-RUST-071` entry, `uncovered` reports it, and `rule/sty-rust-071.yml` does not exist.
- **Mechanism:** ast-grep (extraction) + bash — the rule extracts every `<Aggregate>Repository` trait method signature (`function_signature_item` whose enclosing `trait_item` name ends `Repository`), and the Bash check validates each: receiver is literally `&mut self`, and the return type names a future with an explicit `+ Send` bound. The two assertions are per-method comparisons/literal-token tests ast-grep cannot express, matching the harness's existing extract-and-relate style (`check_sty_rust_019`, `check_sty_rust_023`). No `rule_enforcer.yaml` block: the check has no extensible data.

## Files

### `rule/sty-rust-071.yml` (new)

Extraction helper, `severity: hint` like `sty-rust-019`. It matches trait method declarations only (`;`-terminated `function_signature_item`), so the read-model `&self` getters and the `#[cfg(test)] impl … for &mut T` bodies are out of scope.

```yaml
---
id: sty-rust-071
language: Rust
severity: hint
message: >-
  extract every repository-trait method signature for STY-RUST-071
rule:
  all:
    - kind: function_signature_item
    - inside:
        kind: trait_item
        has:
          field: name
          regex: 'Repository$'
        stopBy: end
    - any:
        - pattern: 'fn $METHOD($$$PARAMS) -> $RET;'
        - pattern: 'fn $METHOD($$$PARAMS);'
```

`sgconfig.yaml` already declares `ruleDirs: [rule]`, so no change there.

### `script/rule_enforcer.sh`

Add the registry line after the last entry (`STY-RUST-024`), and declare the function alongside the other checks (e.g. just before `check_sty_rust_024`). The runner is untouched.

Registry:

```bash
	"STY-RUST-024:check_sty_rust_024",
	"STY-RUST-071:check_sty_rust_071",
)
```

Function:

```bash
check_sty_rust_071() {
	local id="STY-RUST-071"
	local rule_id="sty-rust-071"
	local port_dir="src/lib/domain/port"
	local matches file line method receiver ret result=0

	if [ ! -d "$port_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$port_dir" >&2
		return 1
	fi

	# sty-rust-071 extracts every repository-trait method signature; the
	# receiver-and-return validation runs here because an ast-grep rule cannot
	# compare a parameter to a literal or assert a bound token.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$port_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no repository method from %s\n' "$id" "$port_dir" >&2
		return 1
	fi

	while IFS=$'\t' read -r file line method receiver ret; do
		[ -n "$method" ] || continue
		if [ "$receiver" != "&mut self" ]; then
			printf 'error: %s: %s:%s: %s must take &mut self (found %s)\n' \
				"$id" "$file" "$line" "$method" "${receiver:-none}" >&2
			result=1
		fi
		if ! printf '%s' "$ret" | grep -Eq 'Future.*\+[[:space:]]*Send'; then
			printf 'error: %s: %s:%s: %s must return an explicitly Send future (found %s)\n' \
				"$id" "$file" "$line" "$method" "${ret:-none}" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, (.range.start.line|tostring), .metaVariables.single.METHOD.text, (.metaVariables.multi.PARAMS[0].text // ""), (.metaVariables.single.RET.text // "")] | @tsv')

	return "$result"
}
```

No `rule_enforcer.yaml` change is required.

## Verification

The candidate rule (`/tmp/opencode/sty-rust-071/sty-rust-071.yml`) and the candidate check (`/tmp/opencode/sty-rust-071/check_sty_rust_071.sh`, the function above verbatim) were exercised against scratch projects whose layout matches the repository (`sgconfig.yaml` + `rule/` + `src/lib/domain/port/`).

- **Red:** `/tmp/opencode/sty-rust-071/red/src/lib/domain/port/red_repository.rs` contains `fn bad_receiver(&self, …) -> … + Send;`, `fn missing_send(&mut self, …) -> impl Future<…>;`, and `fn no_return(&mut self, …);`.
  `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-071/red && bash /tmp/opencode/sty-rust-071/check_sty_rust_071.sh'`
  observed: `rc=1` with
  ```
  error: STY-RUST-071: src/lib/domain/port/red_repository.rs:3: bad_receiver must take &mut self (found &self)
  error: STY-RUST-071: src/lib/domain/port/red_repository.rs:4: missing_send must return an explicitly Send future (found impl Future<Output = Result<bool, RepositoryError>>)
  error: STY-RUST-071: src/lib/domain/port/red_repository.rs:5: no_return must return an explicitly Send future (found none)
  ```
- **Green:** `/tmp/opencode/sty-rust-071/green/src/lib/domain/port/green_repository.rs` holds a compliant `GreenRepository` (one single-line and one multiline `&mut self … -> impl Future<…> + Send;`), an inherent `impl Thing { pub fn id(&self) -> NumericID }` getter, and a non-repository trait `ThingProbe` with `fn probe(&self, id: NumericID) -> bool;`.
  `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-071/green && bash /tmp/opencode/sty-rust-071/check_sty_rust_071.sh'`
  observed: `rc=0`, no output (the read-model getter and the non-`Repository` trait are not flagged; a multiline `+ Send` return type is accepted; `impl Future<Output = Result<Send, …>>` without a `+ Send` bound is correctly *not* treated as explicit `Send`).
- **Repository:** the candidate rule registered through a scratch `sgconfig.yaml` (`/tmp/opencode/sty-rust-071/repocheck/`) and run against the real tree:
  `devenv shell -- ast-grep scan --config /tmp/opencode/sty-rust-071/repocheck/sgconfig.yaml --filter sty-rust-071 --json=stream src/lib/domain/port` — observed: 35 method signatures across the 8 `src/lib/domain/port/*_repository.rs` files (`configuration`, `credential`, `file`, `gallery`, `media`, `mime_type`, `upload`, `user`), and the check body over them:
  `devenv shell -- bash -c 'cd /home/will/project/mine/Mnemorium/mnemorium && SGCONFIG=/tmp/opencode/sty-rust-071/repocheck/sgconfig.yaml bash /tmp/opencode/sty-rust-071/check_sty_rust_071.sh'` — observed: `rc=0` (no violations; all 35 methods take `&mut self` and return `impl Future<…> + Send`).

## Next

- Apply: `rule/sty-rust-071.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
