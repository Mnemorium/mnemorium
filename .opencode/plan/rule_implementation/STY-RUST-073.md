# Add check — STY-RUST-073

## Rule guard — STY-RUST-073

- **Rule:** "Prefer sharing long-lived adapters through `Arc<T>` over adding `Clone`." — `docs/development/TechnicalDesign.md`, rule row `| `STY-RUST-073` | Ports | … | [§ 6.5](#65-prefer-arc-over-clone) |` (line 183), under `## Code Style Guidelines`; the detail is § 6.5 *Prefer `Arc` over `Clone`* (lines 697–700): "Do not add `Clone` just for frameworks — share long-lived adapters and the unit-of-work factory through `Arc<T>`. Repositories obtained from a unit of work are borrowed views, not cloned." The row exists, so `validate_registry` accepts the ID.
- **Check state:** missing — `script/rule_enforcer.sh list` does not register `STY-RUST-073` and `uncovered` prints `STY-RUST-073 (Ports) — Prefer sharing long-lived adapters through `Arc<T>` over adding `Clone`.`; there is no `check_sty_rust_073` function, no `RULES=(…)` entry, and no `rule/sty-rust-073.yml`.
- **Mechanism:** bash (cross-file) — the rule needs three facts an ast-grep rule cannot relate: a `pub trait` declared under the port directories, an `impl <Trait> for <Type>` elsewhere in the tree, and the struct's *contiguous attribute block* carrying a `Clone` derive. A `follows: { kind: attribute_item, regex: Clone }` match misses a derive separated from its struct by another attribute (the repository has 77 such structs); `stopBy: end` over-matches an unrelated earlier attribute (probed: `pub struct Other` with no derive was matched by the preceding item's derive). No ast-grep rule file and no `rule_enforcer.yaml` block: the forbidden set is derived from the source at run time, so there is no data to externalize.

An **adapter** is a concrete type that implements a workspace port trait — a trait declared under `src/lib/domain/port/` (outbound ports, the unit of work and its factory) or `src/lib/application/port/` (the use-case traits and the `<Context>UseCaseFactory` traits of `STY-RUST-060`). `STY-RUST-067` uses this same notion for the process-lifetime adapters it enumerates, and § 6.5 names "long-lived adapters and the unit-of-work factory". Such types are shared through `Arc<T>` (`src/bin/server.rs`, `AppState`, the factory accessors), so they must not derive `Clone`; § 6.5 extends that ban to repositories ("borrowed views, not cloned"), which the check also covers. Types that derive `Clone` for a third-party framework trait but are not adapters are out of scope because the rule's remedy (share through `Arc<T>`) does not apply: `DbSqlite3Source` implements the `config` crate's `Source` (its `clone_into_box` requires `Clone`) and `TrustedProxyKeyExtractor` implements `tower_governor`'s `KeyExtractor`; neither implements a workspace port. `AppState` (the axum router state) likewise implements no port.

## Files

### `script/rule_enforcer.sh`

Two edits; the runner is untouched.

1. Append one registry line to `RULES=(…)`, after the current last entry `"STY-RUST-024:check_sty_rust_024"` (line 71), before the closing `)` (line 72):

```diff
 	"STY-RUST-067:check_sty_rust_067"
 	"STY-RUST-024:check_sty_rust_024"
+	"STY-RUST-073:check_sty_rust_073"
 )
```

2. Add the function immediately before the `# --- Runner ---` banner (line 754), i.e. after `check_sty_rust_024()` (ends line 752). Tabs are used for indentation, matching the file:

```bash
check_sty_rust_073() {
	local id="STY-RUST-073"
	local domain_port="src/lib/domain/port"
	local app_port="src/lib/application/port"
	local file trait type name result=0
	local -A ports=()
	local -A adapters=()
	local -A clone_structs=()
	local -A flagged=()

	if [ ! -d "$domain_port" ] || [ ! -d "$app_port" ]; then
		printf 'error: %s: missing %s or %s\n' "$id" "$domain_port" "$app_port" >&2
		return 1
	fi

	# A concrete type that implements a workspace port trait is an adapter
	# (outbound port, unit of work, or use-case factory). Sharing is through
	# `Arc<T>`, so the type must not derive `Clone`. The port set and the
	# adapter set span files, so the relation is resolved here rather than in
	# an ast-grep rule.
	while IFS= read -r trait; do
		[ -n "$trait" ] || continue
		ports["$trait"]=1
	done < <(grep -rhoE '^[[:space:]]*pub trait [A-Za-z0-9_]+' \
		"$domain_port" "$app_port" --include='*.rs' | sed 's/.*trait //')

	if [ "${#ports[@]}" -eq 0 ]; then
		printf 'error: %s: found no port trait under %s or %s\n' \
			"$id" "$domain_port" "$app_port" >&2
		return 1
	fi

	# The base name of every struct implementing a port trait. `impl` lines are
	# read field-wise so an `impl<...> Trait for Type` and a lifetime on the
	# type (`SqlxRepository<'_>`) both reduce to the bare type name.
	while IFS=$'\t' read -r trait type; do
		[ -n "$type" ] || continue
		if [ -n "${ports[$trait]+set}" ]; then
			adapters["${type%%<*}"]=1
		fi
	done < <(grep -rhE '^impl ' src --include='*.rs' |
		awk '{ for (i = 1; i <= NF; i++) if ($i == "for") { print $(i-1) "\t" $(i+1) } }')

	if [ "${#adapters[@]}" -eq 0 ]; then
		printf 'error: %s: found no adapter implementing a port trait\n' "$id" >&2
		return 1
	fi

	# Every struct whose contiguous attribute block carries a `Clone` derive.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		while IFS= read -r name; do
			[ -n "$name" ] || continue
			clone_structs["$name"]=1
		done < <(awk '
			/^[[:space:]]*#\[/ {
				if ($0 ~ /derive\([^)]*Clone([^A-Za-z0-9_]|$)/) { clone = 1 }
				next
			}
			/^[[:space:]]*(pub[[:space:]]+)?struct[[:space:]]+[A-Za-z0-9_]+/ {
				if (clone) {
					name = $0
					sub(/^[[:space:]]*(pub[[:space:]]+)?struct[[:space:]]+/, "", name)
					sub(/[^A-Za-z0-9_].*$/, "", name)
					print name
				}
				clone = 0
				next
			}
			{ clone = 0 }
		' "$file")
	done < <(find src -name '*.rs' | sort)

	for name in "${!adapters[@]}"; do
		if [ -n "${clone_structs[$name]+set}" ] && [ -z "${flagged[$name]+set}" ]; then
			printf 'error: %s: adapter %s derives Clone; share it through Arc<T> instead\n' \
				"$id" "$name" >&2
			flagged["$name"]=1
			result=1
		fi
	done

	return "$result"
}
```

No `rule/sty-rust-073.yml` is added (the check calls no ast-grep), and `rule_enforcer.yaml` is unchanged. The function calls only `find`, `grep`, `sed` and `awk`, so it adds no `command -v` install guard. `shellcheck` and `shfmt -d -i 0 -ci` are clean on the function.

## Verification

All runs used the function above verbatim inside the scratch driver `/tmp/opencode/sty-rust-073/check.sh` (the function plus `cd -- "$1"; check_sty_rust_073; rc=$?; printf 'check rc=%s\n' "$rc"; exit "$rc"`).

- Red (adapter derives `Clone`, derive separated from struct by another attribute; and a use-case factory): `/tmp/opencode/sty-rust-073/proj/red` — `JwtTokenProvider` declared `#[derive(Debug, Clone)]` + `#[expect(…)]` and implementing `TokenProvider`, `RuntimeIdentityUseCaseFactory` declared `#[derive(Debug, Clone)]` and implementing `IdentityUseCaseFactory`, plus a non-adapter `User` model that derives `Clone`. Command: `devenv shell -- bash /tmp/opencode/sty-rust-073/check.sh /tmp/opencode/sty-rust-073/proj/red` — observed `rc=1`:
  ```
  error: STY-RUST-073: adapter RuntimeIdentityUseCaseFactory derives Clone; share it through Arc<T> instead
  error: STY-RUST-073: adapter JwtTokenProvider derives Clone; share it through Arc<T> instead
  ```
  (the `User` model is not flagged). A second red fixture `/tmp/opencode/sty-rust-073/proj/red_repo` puts `#[derive(Clone)] pub struct SqlxUserRepository<'transaction>` implementing `UserRepository` (a borrowed view, § 6.5); `devenv shell -- bash /tmp/opencode/sty-rust-073/check.sh /tmp/opencode/sty-rust-073/proj/red_repo` — observed `rc=1`, `error: STY-RUST-073: adapter SqlxUserRepository derives Clone; share it through Arc<T> instead`.
- Green: `/tmp/opencode/sty-rust-073/proj/green` — `JwtTokenProvider` (`#[derive(Debug)]`) and `FileSystemStorage` (`#[derive(Debug, Default)]`) implement their ports without `Clone`, `RuntimeIdentityUseCaseFactory` holds `Arc<JwtTokenProvider>`, a `User` model and a test-only `CaptureWriter` both derive `Clone` but implement no port. Command: `devenv shell -- bash /tmp/opencode/sty-rust-073/check.sh /tmp/opencode/sty-rust-073/proj/green` — observed `check rc=0`, no finding.
- Repository: `devenv shell -- bash /tmp/opencode/sty-rust-073/check.sh /home/will/project/mine/Mnemorium/mnemorium` — observed `check rc=0`, no finding; the adapter set is non-vacuous (23 structs implementing a workspace port trait: `Argon2PasswordHasher`, `ChaChaSecretGenerator`, `ConfigConfigurationSource`, `FileSystemStorage`, `JwtTokenProvider`, `LocalMediaProbe`, `RandomPasswordGenerator`, `Sha2ContentHasher`, `Sha2ContentHasherSession`, `Sqlx{Configuration,Credential,File,Gallery,Media,MimeType,Upload,User}Repository`, `SqlxUnitOfWork`, `SqlxUnitOfWorkFactory`, and `Runtime{Asset,Identity,Library,User}UseCaseFactory`), and none of them appears in the `Clone`-derive set. Baseline: `devenv shell -- script/rule_enforcer.sh list | wc -l` → `22`, `devenv shell -- script/rule_enforcer.sh uncovered | grep -c 'STY-RUST-073'` → `1`.

## Next

- Apply: `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
