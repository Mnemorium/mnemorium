# Add check — STY-RUST-068

## Rule guard — STY-RUST-068

- **Rule:** `STY-RUST-068` (Section: Application state) — "`AppState` is the single router state; `FromRef` impls expose exactly what middleware extracts, and getters return `Arc` clones, never borrows." (`docs/development/TechnicalDesign.md` line 178, table under `## Code Style Guidelines`; detail is § 1 Application state, lines 197–213.)
- **Check state:** missing — `STY-RUST-068` is absent from `RULES`, there is no `rule/sty-rust-068.yml`, and `script/rule_enforcer.sh uncovered` lists it.
- **Mechanism:** ast-grep + bash — the rule extracts the `AppState` struct, its `&self` getters and its `FromRef` impls; the Bash wrapper decides the single-declaration, file-location and getter-return/clone-body facts ast-grep cannot see. No `rule_enforcer.yaml` block: the check has no forward-extensible data.

## Files

### `rule/sty-rust-068.yml` (new)

An extraction rule (`severity: hint`, like `sty-rust-006`/`sty-rust-013`/`sty-rust-019`/`sty-rust-023`): one branch captures the state struct, one its getters, one its `FromRef` impls. The Bash check owns the classification. `sgconfig.yaml` already declares `ruleDirs: [rule]`, so no change is needed there.

```yaml
---
id: sty-rust-068
language: Rust
severity: hint
message: >-
  extract the AppState struct, its getters and its FromRef impls for
  STY-RUST-068
rule:
  any:
    # The single application state struct.
    - all:
        - kind: struct_item
        - pattern: 'pub struct AppState { $$$FIELDS }'
    # A getter: a public method on AppState borrowing self.
    - all:
        - kind: function_item
        - pattern: 'pub fn $GETTER(&self) -> $RET { $$$BODY }'
        - inside:
            kind: impl_item
            pattern: 'impl AppState { $$$IMPL_BODY }'
            stopBy: end
    # A FromRef impl exposing a piece of AppState to middleware.
    - all:
        - kind: impl_item
        - pattern: 'impl FromRef<AppState> for $REF { $$$REF_BODY }'
```

### `script/rule_enforcer.sh`

Two edits. First, append one registry line at the end of `RULES` (after `"STY-RUST-024:check_sty_rust_024"`):

```bash
	"STY-RUST-067:check_sty_rust_067"
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-068:check_sty_rust_068"
)
```

Second, add the function after `check_sty_rust_024()` and before the `# --- Runner ---` banner:

```bash
check_sty_rust_068() {
	local id="STY-RUST-068"
	local rule_id="sty-rust-068"
	local app_state="src/lib/infrastructure/inbound/rest/app_state.rs"
	local matches structs getters refs file name ret body result=0
	local clone_re='^Arc::clone\(&self\.[A-Za-z_][A-Za-z0-9_]*\)$'

	if [ ! -f "$app_state" ]; then
		printf 'error: %s: missing %s\n' "$id" "$app_state" >&2
		return 1
	fi

	# sty-rust-068 extracts the AppState struct, its `&self` getters and its
	# FromRef impls; the single-declaration, file-location and getter-return
	# decisions happen here because an ast-grep rule cannot see the file it
	# matched in and cannot classify a polymorphic return type.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no AppState declaration from src\n' "$id" >&2
		return 1
	fi

	structs="$(printf '%s\n' "$matches" | jq -c 'select(.metaVariables.multi.FIELDS)')"
	getters="$(printf '%s\n' "$matches" | jq -c 'select(.metaVariables.single.GETTER)')"
	refs="$(printf '%s\n' "$matches" | jq -c 'select(.metaVariables.single.REF)')"

	# Clause A: AppState is the single state struct, declared in app_state.rs.
	if [ "$(printf '%s\n' "$structs" | grep -c .)" -ne 1 ]; then
		printf 'error: %s: expected exactly one AppState declaration, found %s\n' \
			"$id" "$(printf '%s\n' "$structs" | grep -c .)" >&2
		result=1
	fi
	if [ -n "$structs" ]; then
		while IFS= read -r file; do
			[ -n "$file" ] || continue
			if [ "$file" != "$app_state" ]; then
				printf 'error: %s: AppState must be declared in %s (found in %s)\n' \
					"$id" "$app_state" "$file" >&2
				result=1
			fi
		done < <(printf '%s\n' "$structs" | jq -r '.file')
	fi

	# Clause B: every FromRef impl for AppState is declared with it.
	if [ -n "$refs" ]; then
		while IFS= read -r file; do
			[ -n "$file" ] || continue
			if [ "$file" != "$app_state" ]; then
				printf 'error: %s: FromRef<AppState> impl must be declared in %s (found in %s)\n' \
					"$id" "$app_state" "$file" >&2
				result=1
			fi
		done < <(printf '%s\n' "$refs" | jq -r '.file')
	fi

	# Clause C: every getter returns an Arc clone of a stored field.
	while IFS=$'\t' read -r file name ret body; do
		[ -n "$name" ] || continue
		if ! printf '%s' "$ret" | grep -q '^Arc<'; then
			printf 'error: %s: %s: getter %s returns %s; getters return an Arc clone, never a borrow\n' \
				"$id" "$file" "$name" "$ret" >&2
			result=1
			continue
		fi
		body="$(printf '%s' "$body" | tr -s '[:space:]' ' ' | sed -e 's/^ //' -e 's/ $//')"
		if ! [[ $body =~ $clone_re ]]; then
			printf 'error: %s: %s: getter %s returns %s but does not clone the stored Arc: %s\n' \
				"$id" "$file" "$name" "$ret" "$body" >&2
			result=1
		fi
	done < <(printf '%s\n' "$getters" |
		jq -r '[.file, .metaVariables.single.GETTER.text, .metaVariables.single.RET.text, .metaVariables.multi.BODY[0].text] | @tsv')

	return "$result"
}
```

## Verification

Candidate rule file used verbatim (`/tmp/opencode/sty-rust-068/rule/sty-rust-068.yml`); scratch harness `/tmp/opencode/sty-rust-068/check_scratch.sh` defines the check above with `SGCONFIG=/tmp/opencode/sty-rust-068/ruleonly/sgconfig.yaml` (`ruleDirs: [/tmp/opencode/sty-rust-068/rule]`), cds to the fixture root, and runs the function. Fixtures under `/tmp/opencode/sty-rust-068/proj/`, each holding `src/lib/infrastructure/inbound/rest/app_state.rs`.

- Red (borrowed getter): `/tmp/opencode/sty-rust-068/proj/red_borrow` — `configuration` returns `&Arc<ArcSwap<Configuration>>` with body `&self.configuration` — `devenv shell -- bash /tmp/opencode/sty-rust-068/check_scratch.sh /tmp/opencode/sty-rust-068/proj/red_borrow` — observed: `error: STY-RUST-068: src/lib/infrastructure/inbound/rest/app_state.rs: getter configuration returns &Arc<ArcSwap<Configuration>>; getters return an Arc clone, never a borrow` / `FAIL`.
- Red (non-clone getter body): `/tmp/opencode/sty-rust-068/proj/red_new` — `configuration` returns `Arc<ArcSwap<Configuration>>` with body `Arc::new(ArcSwap::from_pointee(Configuration::default()))` — same command with `red_new` — observed: `error: STY-RUST-068: src/lib/infrastructure/inbound/rest/app_state.rs: getter configuration returns Arc<ArcSwap<Configuration>> but does not clone the stored Arc: Arc::new(ArcSwap::from_pointee(Configuration::default()))` / `FAIL`.
- Red (second state struct): `/tmp/opencode/sty-rust-068/proj/red_dup` — an extra `pub struct AppState { pub other: Arc<()> }` in `src/lib/infrastructure/inbound/rest/other_state.rs` — same command with `red_dup` — observed: `error: STY-RUST-068: expected exactly one AppState declaration, found 2` and `error: STY-RUST-068: AppState must be declared in src/lib/infrastructure/inbound/rest/app_state.rs (found in src/lib/infrastructure/inbound/rest/other_state.rs)` / `FAIL`.
- Red (state in the wrong file): `/tmp/opencode/sty-rust-068/proj/red_state_misplaced` — the `AppState` struct lives only in `other_state.rs` — same command with `red_state_misplaced` — observed: `error: STY-RUST-068: AppState must be declared in src/lib/infrastructure/inbound/rest/app_state.rs (found in src/lib/infrastructure/inbound/rest/other_state.rs)` / `FAIL`.
- Red (FromRef impl in the wrong file): `/tmp/opencode/sty-rust-068/proj/red_ref_misplaced` — `impl FromRef<AppState> for Arc<JwtTokenProvider>` moved to `src/lib/infrastructure/inbound/rest/other_ref.rs` — same command with `red_ref_misplaced` — observed: `error: STY-RUST-068: FromRef<AppState> impl must be declared in src/lib/infrastructure/inbound/rest/app_state.rs (found in src/lib/infrastructure/inbound/rest/other_ref.rs)` / `FAIL`.
- Green: `/tmp/opencode/sty-rust-068/proj/green` — a copy of the repository `app_state.rs` (one `AppState`, five `Arc::clone` getters, one `FromRef` impl) — same command with `green` — observed: `PASS: /tmp/opencode/sty-rust-068/proj/green`, exit `0`.
- Repository: `devenv shell -- bash /tmp/opencode/sty-rust-068/check_scratch.sh /home/will/project/mine/Mnemorium/mnemorium` (harness cds to the repo root; `SGCONFIG` points at the candidate rule) — observed: `PASS: /home/will/project/mine/Mnemorium/mnemorium`, exit `0`. The raw candidate scan `devenv shell -- ast-grep scan --config /tmp/opencode/sty-rust-068/ruleonly/sgconfig.yaml --filter sty-rust-068 --report-style=short --color=never src` returns exactly 7 matches, all in `src/lib/infrastructure/inbound/rest/app_state.rs` (the struct at line 21, five getters at 37/43/49/55/79, the `FromRef` impl at 84); no violation.

Clause B is enforced as the structural half of "`FromRef` impls expose … "; set-equality with what middleware extracts is not decidable from source shape (the concrete `State<T>` is bound at `authenticate::<JwtTokenProvider>` in `handler/identity.rs`), and the reverse direction — a middleware `State<T>` with no `impl FromRef<AppState> for T` — is a compile error, not a lint. The check therefore requires every `FromRef<AppState>` impl to be declared in `app_state.rs` and leaves the concrete-extraction equivalence to the compiler.

## Next

- Apply: `rule/sty-rust-068.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
