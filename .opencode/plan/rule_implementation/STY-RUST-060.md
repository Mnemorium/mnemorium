# Add check — STY-RUST-060

## Rule guard — STY-RUST-060

- **Rule:** `**Port** — `src/lib/application/port/<context>_use_case_factory.rs`. The trait is named `<Context>UseCaseFactory`, is `Send + Sync`, carries `#[cfg_attr(test, mockall::automock)]`, and exposes one method per use case — named after the use case — returning `Arc<dyn <UseCaseName>UseCase>`.` (Section: Use cases, `docs/development/TechnicalDesign.md` line 170; detail § 9.2 line 907)
- **Check state:** missing — `STY-RUST-060` has a rulebook row, is not in `RULES=(…)` (`list` prints nothing for it; `uncovered` prints it).
- **Mechanism:** ast-grep + bash — a hint rule extracts each factory trait (name, supertraits) and each accessor signature; Bash pairs them with the file path, the trait name and each method's return type, which ast-grep cannot relate.

## Files

### `rule/sty-rust-060.yml` (new)

```yaml
---
id: sty-rust-060
language: Rust
severity: hint
message: >-
  extract the use-case-factory traits and their accessor signatures, for
  STY-RUST-060
rule:
  any:
    # A use-case-factory trait with supertraits.
    - all:
        - kind: trait_item
        - regex: 'trait \w*UseCaseFactory\b'
        - pattern: 'pub trait $TRAIT: $$$BOUNDS { $$$BODY }'
    # A use-case-factory trait without supertraits.
    - all:
        - kind: trait_item
        - regex: 'trait \w*UseCaseFactory\b'
        - pattern: 'pub trait $TRAIT { $$$BODY }'
    # An accessor signature declared inside a use-case-factory trait.
    - all:
        - kind: function_signature_item
        - inside:
            kind: trait_item
            regex: 'trait \w*UseCaseFactory\b'
            stopBy: end
```

### `script/rule_enforcer.sh`

Two edits, no runner change.

1. Register the check in `RULES=(…)` (append after the last entry):

```diff
 	"STY-RUST-067:check_sty_rust_067"
 	"STY-RUST-024:check_sty_rust_024"
+	"STY-RUST-060:check_sty_rust_060"
 )
```

2. Add the function in the `# --- Checks ---` section, immediately before `# --- Runner ---` (after `check_sty_rust_024`):

```bash
check_sty_rust_060() {
	local id="STY-RUST-060"
	local rule_id="sty-rust-060"
	local port_dir="src/lib/application/port"
	local method_re='^fn ([a-z_][a-z0-9_]*)\(&self\) -> Arc<dyn ([A-Za-z0-9_]+)>;$'
	local matches file expected trait bounds line text method use_case result=0

	if [ ! -d "$port_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$port_dir" >&2
		return 1
	fi

	# sty-rust-060 extracts each use-case-factory trait and each accessor
	# declared inside one; the trait's file, name, supertraits and the
	# accessor's signature are paired here because an ast-grep rule cannot see
	# a file path or relate two identifiers.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no use-case-factory trait under %s\n' "$id" "$port_dir" >&2
		return 1
	fi

	# Every factory port file lives directly in the port directory.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		if [ "$(dirname "$file")" != "$port_dir" ]; then
			printf 'error: %s: factory port file must be %s/<context>_use_case_factory.rs (found %s)\n' \
				"$id" "$port_dir" "$file" >&2
			result=1
		fi
	done < <(find src -name '*_use_case_factory.rs' | sort)

	# The trait is named <Context>UseCaseFactory, is Send + Sync, carries the
	# automock attribute, and lives in <context>_use_case_factory.rs.
	while IFS=$'\t' read -r file line trait bounds; do
		[ -n "$trait" ] || continue
		expected="${port_dir}/$(printf '%s' "$trait" |
			sed -E 's/UseCaseFactory$//' |
			sed -E 's/([a-z0-9])([A-Z])/\1_\2/g' |
			tr 'A-Z' 'a-z')_use_case_factory.rs"
		if [ "$file" != "$expected" ]; then
			printf 'error: %s: %s: factory trait %s must be declared in %s\n' \
				"$id" "$file" "$trait" "$expected" >&2
			result=1
		fi
		if [[ " $bounds " != *" Send "* ]] || [[ " $bounds " != *" Sync "* ]]; then
			printf 'error: %s: %s: factory trait %s must be Send + Sync (found: %s)\n' \
				"$id" "$file" "$trait" "${bounds:-none}" >&2
			result=1
		fi
		if ! grep -qF '#[cfg_attr(test, mockall::automock)]' "$file"; then
			printf 'error: %s: %s: factory trait %s must carry #[cfg_attr(test, mockall::automock)]\n' \
				"$id" "$file" "$trait" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" | jq -r '
		select(.metaVariables.single.TRAIT) |
		[.file, (.range.start.line | tostring),
		 .metaVariables.single.TRAIT.text,
		 ([.metaVariables.multi.BOUNDS[]?.text] | join(" "))] | @tsv')

	# Every factory port file declares its own factory trait.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		expected="$(basename "$file" _use_case_factory.rs |
			awk -F_ '{ for (i = 1; i <= NF; i++) printf "%s%s", toupper(substr($i, 1, 1)), substr($i, 2) }')UseCaseFactory"
		if ! printf '%s\n' "$matches" | jq -e -s --arg f "$file" --arg t "$expected" \
			'any(.[]; .file == $f and .metaVariables.single.TRAIT.text == $t)' >/dev/null; then
			printf 'error: %s: %s must declare trait %s\n' "$id" "$file" "$expected" >&2
			result=1
		fi
	done < <(find "$port_dir" -maxdepth 1 -name '*_use_case_factory.rs' | sort)

	# Each accessor is named after the use case it returns.
	while IFS=$'\t' read -r file line text; do
		[ -n "$text" ] || continue
		if [[ $text =~ $method_re ]]; then
			method="${BASH_REMATCH[1]}"
			use_case="${BASH_REMATCH[2]}"
			expected="$(printf '%s' "$method" |
				awk -F_ '{ for (i = 1; i <= NF; i++) printf "%s%s", toupper(substr($i, 1, 1)), substr($i, 2) }')UseCase"
			if [ "$use_case" != "$expected" ]; then
				printf 'error: %s: %s: accessor %s must return Arc<dyn %s> (found Arc<dyn %s>)\n' \
					"$id" "$file" "$method" "$expected" "$use_case" >&2
				result=1
			fi
			if ! grep -rqE "pub trait ${use_case}\b" "$port_dir"; then
				printf 'error: %s: %s: accessor %s returns %s, not a use-case trait under %s\n' \
					"$id" "$file" "$method" "$use_case" "$port_dir" >&2
				result=1
			fi
		else
			printf 'error: %s: %s: accessor has an unexpected signature: %s\n' \
				"$id" "$file" "$text" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" | jq -r '
		select(.metaVariables.single.TRAIT | not) |
		[.file, (.range.start.line | tostring), .text] | @tsv')

	return "$result"
}
```

No `rule_enforcer.yaml` change: the rule's data (path, trait shape, attribute) is fixed by the rule text, not forward-extensible.

## Verification

- **Red:** `/tmp/opencode/sty-rust-060/red` (copy of the real port dir, plus: `asset_use_case_factory.rs` with `AssetUseCaseFactory` lacking `Send + Sync` and the automock attribute and carrying `do_stuff(&self) -> Arc<dyn WrongUseCase>` and `bad(&self, limit: u8) -> Arc<dyn BeginUploadUseCase>`; `gamma_use_case_factory.rs` declaring `GammaFactory`; `random.rs` declaring `ZetaUseCaseFactory`; `sub/nested_use_case_factory.rs`) — `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-060/red && bash check.sh'` — observed `check rc=1`, flagging: factory file not directly under `src/lib/application/port`; `ZetaUseCaseFactory`/`NestedUseCaseFactory` declared outside `<context>_use_case_factory.rs`; `AssetUseCaseFactory` not `Send + Sync`; missing `#[cfg_attr(test, mockall::automock)]`; `gamma_use_case_factory.rs` missing `GammaUseCaseFactory`; `go` returning `Arc<dyn GetUserUseCase>` instead of `Arc<dyn GoUseCase>`; `do_stuff` returning `WrongUseCase` (and `WrongUseCase` absent from the port dir); and `fn bad(&self, limit: u8) -> Arc<dyn BeginUploadUseCase>;` as an unexpected signature.
- **Green:** `/tmp/opencode/sty-rust-060/green` (a verbatim copy of `src/lib/application/port`, `sgconfig.yaml`, `rule/sty-rust-060.yml`) — `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-060/green && bash check.sh'` — observed `check rc=0`, no findings.
- **Repository:** the same function run against the tree with the candidate rule mounted via `/tmp/opencode/sty-rust-060/repo-scan/sgconfig.yaml` — `devenv shell -- bash -c 'cd /home/will/project/mine/Mnemorium/mnemorium && bash /tmp/opencode/sty-rust-060/repo-run.sh'` — observed `check rc=0` (all four factories — `asset`, `identity`, `library`, `user` — pass).

## Next

- Apply: `rule/sty-rust-060.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
