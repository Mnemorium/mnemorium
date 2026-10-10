# Add check — STY-RUST-061

## Rule guard — STY-RUST-061

- **Rule:** `STY-RUST-061` (Use cases) — "**Adapter** — `src/lib/infrastructure/use_case_factory/<context>.rs`. The struct is named `Runtime<Context>UseCaseFactory` and holds what the context's use cases need: the live configuration (`Arc<ArcSwap<Configuration>>`) when they read it, and the unit-of-work factory (`Arc<SqlxUnitOfWorkFactory>`)." ([§ 9.2](#92-use-case-factory))
- **Check state:** missing — no `STY-RUST-061` entry in the registry, no `rule/sty-rust-061.yml`; `script/rule_enforcer.sh uncovered` lists it.
- **Mechanism:** ast-grep + bash — ast-grep extracts the factory struct and its fields; the Bash wrapper relates the struct name to the file name and checks the field types, which an ast-grep rule cannot see.

## Files

### `rule/sty-rust-061.yml` (new)

An extraction rule (severity `hint`, like `sty-rust-006`/`sty-rust-013`/`sty-rust-019`/`sty-rust-023`): one branch captures the public factory struct, the other its fields. The Bash check owns the file-name pairing and the type decisions.

```yaml
---
id: sty-rust-061
language: Rust
severity: hint
message: extract the use-case-factory adapter struct and its fields
files:
  - src/lib/infrastructure/use_case_factory/*.rs
rule:
  any:
    - all:
        - kind: struct_item
        - pattern: 'pub struct $FACTORY { $$$BODY }'
    - all:
        - kind: field_declaration
        - pattern: '$FIELD: $TYPE'
        - inside:
            kind: field_declaration_list
            inside:
              kind: struct_item
              pattern: 'pub struct $FACTORY { $$$BODY }'
              stopBy: end
```

### `script/rule_enforcer.sh`

Two edits. No change to `rule_enforcer.yaml` (the check needs no per-check data).

**1. Registry** — append one line to `RULES=( ... )`, after `"STY-RUST-024:check_sty_rust_024"`:

```bash
	"STY-RUST-061:check_sty_rust_061"
```

**2. Check function** — add immediately before the `# --- Runner ---` banner (i.e. after `check_sty_rust_024`):

```bash
check_sty_rust_061() {
	local id="STY-RUST-061"
	local rule_id="sty-rust-061"
	local factory_dir="src/lib/infrastructure/use_case_factory"
	local matches file name base expected field type result=0
	local -A struct_name=()
	local -A has_uow=()
	local -A has_config=()
	local -A reads_config=()

	if [ ! -d "$factory_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$factory_dir" >&2
		return 1
	fi

	# sty-rust-061 extracts each factory adapter struct and each of its fields;
	# the file-name-to-struct-name pairing and the field-type checks happen here
	# because an ast-grep rule cannot see the file it is matching in.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$factory_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no factory struct from %s\n' "$id" "$factory_dir" >&2
		return 1
	fi

	# The factory struct of each file, keyed by file.
	while IFS=$'\t' read -r file name; do
		[ -n "$name" ] || continue
		struct_name["$file"]="$name"
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.FACTORY) | [.file, .metaVariables.single.FACTORY.text] | @tsv')

	# The field types each file declares.
	while IFS=$'\t' read -r file field type; do
		[ -n "$field" ] || continue
		case "$type" in
		'Arc<SqlxUnitOfWorkFactory>') has_uow["$file"]=1 ;;
		'Arc<ArcSwap<Configuration>>') has_config["$file"]=1 ;;
		esac
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.FIELD) | [.file, .metaVariables.single.FIELD.text, .metaVariables.single.TYPE.text] | @tsv')

	# A factory whose accessors read the live configuration must hold it.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		if grep -q 'self\.configuration\.load()' "$file"; then
			reads_config["$file"]=1
		fi
	done < <(printf '%s\n' "$matches" | jq -r '.file' | sort -u)

	# Every factory file declares the Runtime<Context>UseCaseFactory struct named
	# after its context, holding the unit-of-work factory and, when its accessors
	# read the live configuration, that configuration.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		base="$(basename "$file" .rs)"
		expected="Runtime$(printf '%s' "$base" |
			awk -F_ '{ for (i = 1; i <= NF; i++) printf "%s%s", toupper(substr($i, 1, 1)), substr($i, 2) }')UseCaseFactory"
		name="${struct_name[$file]:-}"
		if [ -z "$name" ]; then
			printf 'error: %s: %s declares no factory struct\n' "$id" "$file" >&2
			result=1
			continue
		fi
		if [ "$name" != "$expected" ]; then
			printf 'error: %s: %s: factory struct %s must be named %s after the file\n' \
				"$id" "$file" "$name" "$expected" >&2
			result=1
		fi
		if [ -z "${has_uow[$file]+set}" ]; then
			printf 'error: %s: %s: factory struct %s must hold the unit-of-work factory (Arc<SqlxUnitOfWorkFactory>)\n' \
				"$id" "$file" "$name" >&2
			result=1
		fi
		if [ -n "${reads_config[$file]+set}" ] && [ -z "${has_config[$file]+set}" ]; then
			printf 'error: %s: %s: factory struct %s reads the live configuration but does not hold Arc<ArcSwap<Configuration>>\n' \
				"$id" "$file" "$name" >&2
			result=1
		fi
	done < <(find "$factory_dir" -maxdepth 1 -name '*.rs' | sort)

	return "$result"
}
```

## Verification

Fixtures live in `/tmp/opencode/sty-rust-061/` (scratch, untracked). `fixture/` is a copy of the four real factory files plus `sgconfig.yaml` and `rule/sty-rust-061.yml`; `check.sh` is the candidate function above, run with `cwd` at the fixture root so the rule's `files:` glob and the check's relative `factory_dir` resolve.

- **Red (wrong struct name)** — `/tmp/opencode/sty-rust-061/fixture-red/src/lib/infrastructure/use_case_factory/identity.rs` with `RuntimeIdentityUseCaseFactory` renamed to `IdentityUseCaseFactory`; also `library.rs` with the `unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>` field removed, and `asset.rs` with the `configuration: Arc<ArcSwap<Configuration>>` field removed — `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-061/fixture-red && bash /tmp/opencode/sty-rust-061/check.sh'` — observed:
  ```
  error: STY-RUST-061: src/lib/infrastructure/use_case_factory/asset.rs: factory struct RuntimeAssetUseCaseFactory reads the live configuration but does not hold Arc<ArcSwap<Configuration>>
  error: STY-RUST-061: src/lib/infrastructure/use_case_factory/identity.rs: factory struct IdentityUseCaseFactory must be named RuntimeIdentityUseCaseFactory after the file
  error: STY-RUST-061: src/lib/infrastructure/use_case_factory/library.rs: factory struct RuntimeLibraryUseCaseFactory must hold the unit-of-work factory (Arc<SqlxUnitOfWorkFactory>)
  ```
  exit `1`.
- **Red (no factory struct)** — `/tmp/opencode/sty-rust-061/fixture-nostruct/src/lib/infrastructure/use_case_factory/library.rs` with the struct body replaced by `pub struct NotAFactory;` — `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-061/fixture-nostruct && bash /tmp/opencode/sty-rust-061/check.sh'` — observed:
  ```
  error: STY-RUST-061: src/lib/infrastructure/use_case_factory/library.rs declares no factory struct
  ```
  exit `1`.
- **Green (compliant)** — `/tmp/opencode/sty-rust-061/fixture/src/lib/infrastructure/use_case_factory/{asset,identity,library,user}.rs`, byte copies of the repository files — `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-061/fixture && bash /tmp/opencode/sty-rust-061/check.sh'` — observed: no output, exit `0`.
- **Repository** — candidate check run from the repo root against `src/lib/infrastructure/use_case_factory` with a scratch `sgconfig.yaml` whose `ruleDirs` points at `/tmp/opencode/sty-rust-061/rule` — `devenv shell -- bash /tmp/opencode/sty-rust-061/check-repo.sh` — observed: exit `0`, no findings (all four factory files already satisfy the rule).

## Next

- Apply: `rule/sty-rust-061.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
