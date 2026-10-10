# Add check — STY-RUST-070

## Rule guard — STY-RUST-070

- **Rule:** "Outbound ports that return `impl Future` are not dyn-safe: hold concrete adapters and `Arc<dyn <UseCaseName>UseCase>` trait objects, never `Arc<dyn UnitOfWorkFactory>` or `Arc<dyn TokenProvider>`." — Section *Ports*, `docs/development/TechnicalDesign.md` line 180; detail § 6.2 "Dyn-safety" (line 652).
- **Check state:** missing — `STY-RUST-070` has a rulebook row and detail section, but no `RULES=(…)` entry and no `rule/sty-rust-070.yml`; `script/rule_enforcer.sh uncovered` lists it.
- **Mechanism:** ast-grep + bash — an extraction rule collects every `Arc<dyn Trait>` site; the Bash wrapper relates each trait name to the outbound-port traits declared in `src/lib/domain/port/` that return `impl Future` (RPITIT), a cross-file relation an ast-grep rule cannot resolve. No `rule_enforcer.yaml` block: the rule's data (port directory, `impl Future` marker, `Arc<dyn …>` form) is fixed by the rule text, not forward-extensible.

## Files

### `rule/sty-rust-070.yml` (new)

An extraction rule (severity `hint`, like `sty-rust-006`/`sty-rust-012`/`sty-rust-013`/`sty-rust-019`/`sty-rust-023`): it captures every `Arc<dyn …>` generic type; the Bash check owns the decision. `kind: generic_type` with `regex: '^Arc<dyn '` is used because a bare `pattern: 'Arc<dyn $TRAIT>'` does not compile in ast-grep 0.45.1 (`dyn` does not parse as a standalone pattern).

```yaml
---
id: sty-rust-070
language: Rust
severity: hint
message: >-
  extract every `Arc<dyn Trait>` trait object for STY-RUST-070
files:
  - src/**/*.rs
rule:
  kind: generic_type
  regex: '^Arc<dyn '
```

### `script/rule_enforcer.sh`

Two edits; no runner change.

**1. Registry** — append one line to `RULES=( … )`, after `"STY-RUST-024:check_sty_rust_024"`:

```diff
 	"STY-RUST-067:check_sty_rust_067"
 	"STY-RUST-024:check_sty_rust_024"
+	"STY-RUST-070:check_sty_rust_070"
 )
```

**2. Check function** — add in the `# --- Checks ---` section, immediately before the `# --- Runner ---` banner (i.e. after `check_sty_rust_024`):

```bash
check_sty_rust_070() {
	local id="STY-RUST-070"
	local rule_id="sty-rust-070"
	local port_dir="src/lib/domain/port"
	local matches file text base result=0
	local -A dyn_unsafe=()

	if [ ! -d "$port_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$port_dir" >&2
		return 1
	fi

	# An outbound-port trait whose methods return `impl Future` (RPITIT) is not
	# dyn-safe. Collect those trait names from the port declarations; pairing
	# them with the `Arc<dyn …>` use sites below is Bash work because an
	# ast-grep rule cannot resolve the trait a `dyn` names.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		while IFS= read -r trait; do
			[ -n "$trait" ] || continue
			dyn_unsafe["$trait"]=1
		done < <(sed -nE 's/^[[:space:]]*pub trait ([A-Za-z0-9_]+).*/\1/p' "$file")
	done < <(grep -rlE 'impl Future' "$port_dir" --include='*.rs')

	if [ "${#dyn_unsafe[@]}" -eq 0 ]; then
		printf 'error: %s: found no outbound port returning impl Future under %s\n' "$id" "$port_dir" >&2
		return 1
	fi

	# sty-rust-070 extracts every `Arc<dyn Trait>` in the tree.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi

	while IFS=$'\t' read -r file text; do
		[ -n "$text" ] || continue
		base="${text#Arc<dyn }"
		base="${base%%>*}"
		base="${base%% *}"
		base="${base%%<*}"
		base="${base##*::}"
		if [ -n "${dyn_unsafe[$base]+set}" ]; then
			printf 'error: %s: %s: Arc<dyn %s> is not dyn-safe; hold a concrete adapter\n' \
				"$id" "$file" "$base" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" | jq -r '[.file, .text] | @tsv')

	return "$result"
}
```

## Verification

Scratch proof under `/tmp/opencode/sty-rust-070/` (untracked). `rule/sty-rust-070.yml` is the candidate above; each fixture root holds `sgconfig.yaml` (`ruleDirs:\n  - rule`), a copy of the candidate at `rule/sty-rust-070.yml`, the four `src/lib/domain/port` declarations copied verbatim from the repository (`unit_of_work.rs`, `token_provider.rs`; both return `impl Future`), and its own `src/app.rs`. `check.sh` holds the candidate function above with a driver (`check_sty_rust_070; rc=$?; printf 'check rc=%s\n' "$rc"; exit "$rc"`); `repo-run.sh` is the same script with `SGCONFIG=/tmp/opencode/sty-rust-070/repo/sgconfig.yaml` (`ruleDirs: [/tmp/opencode/sty-rust-070/rule]`), run from the repository root.

- **Red:** `/tmp/opencode/sty-rust-070/red/src/app.rs` — a `BadState` with `factory: Arc<dyn UnitOfWorkFactory>` and `tokens: Arc<dyn crate::domain::port::token_provider::TokenProvider>` plus `build(factory: Arc<dyn UnitOfWorkFactory>, tokens: Arc<dyn TokenProvider>)`; `/tmp/opencode/sty-rust-070/red/sgconfig.yaml`, `rule/sty-rust-070.yml`.
  `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-070/red && bash ../check.sh'` — observed `check rc=1` with four findings (the qualified path is reduced to its final segment):
  ```
  error: STY-RUST-070: src/app.rs: Arc<dyn UnitOfWorkFactory> is not dyn-safe; hold a concrete adapter
  error: STY-RUST-070: src/app.rs: Arc<dyn TokenProvider> is not dyn-safe; hold a concrete adapter
  error: STY-RUST-070: src/app.rs: Arc<dyn UnitOfWorkFactory> is not dyn-safe; hold a concrete adapter
  error: STY-RUST-070: src/app.rs: Arc<dyn TokenProvider> is not dyn-safe; hold a concrete adapter
  ```
- **Green:** `/tmp/opencode/sty-rust-070/green/src/app.rs` — a `GoodState` holding `use_case: Arc<dyn RegisterUserUseCase>`, `factory: Arc<dyn IdentityUseCaseFactory>`, `adapter: Arc<SqlxUnitOfWorkFactory>`, and `take(_: Arc<dyn RegisterUserUseCase>, _: Arc<dyn IdentityUseCaseFactory>)`; same `sgconfig.yaml`/rule layout. A trailing doc comment `/// … Arc<dyn UnitOfWorkFactory> …` and string constant `"Arc<dyn TokenProvider>"` are added in `/tmp/opencode/sty-rust-070/commentred/` to prove comments/strings are ignored.
  `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-070/green && bash ../check.sh'` — observed `check rc=0`, no findings.
  `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-070/commentred && bash ../check.sh'` — observed `check rc=0`, no findings (AST-only matching skips the comment and the string literal).
- **Repository:** `devenv shell -- bash -c 'cd /home/will/project/mine/Mnemorium/mnemorium && bash /tmp/opencode/sty-rust-070/repo-run.sh'` — observed `check rc=0`, no findings. The extraction rule matched 78 `Arc<dyn …>` occurrences across `src`, all ending in `UseCase` or `UseCaseFactory` (e.g. `Arc<dyn GetGalleryUseCase>`, `Arc<dyn AssetUseCaseFactory>`), and none matching an outbound-port trait returning `impl Future` — so the green run is non-vacuous.

## Next

- Apply: `rule/sty-rust-070.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
