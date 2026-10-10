# Add check — STY-RUST-069

## Rule guard — STY-RUST-069

- **Rule:** "Always declare outbound port traits as `Send + Sync`." (`docs/development/TechnicalDesign.md` line 179, Section `Ports`, under the `### Section registry`; detail § 6.1 "Port traits are `Send` and `Sync`", line 631. The rulebook text is the source of truth and the ID has a row, so `validate_registry` accepts it.)
- **Check state:** missing — `script/rule_enforcer.sh uncovered` prints `STY-RUST-069 (Ports) — Always declare outbound port traits as 'Send + Sync'.`; the ID is absent from `RULES=(…)`, and `rule/sty-rust-069.yml` does not exist.
- **Mechanism:** ast-grep + bash — a hint rule extracts every `pub trait` declared under `src/lib/domain/port/` with its supertrait bounds; the Bash check decides whether the trait is `Send + Sync` directly or through a supertrait, which ast-grep cannot follow. § 6.1's property is about the port being movable between threads and shareable behind an `Arc`, and this repository encodes it with a `Send + Sync` base trait (`UnitOfWork: Send + Sync`, with the compile-time assertion in `unit_of_work.rs`) that the bounded-context unit-of-work traits extend; the check therefore resolves the supertrait instead of demanding the literal bound on every trait. No `rule_enforcer.yaml` block: the rule's data (the port directory, `Send`/`Sync`) is fixed by the rule text.

## Files

### `rule/sty-rust-069.yml` (new)

```yaml
---
id: sty-rust-069
language: Rust
severity: hint
message: extract outbound port trait declarations for STY-RUST-069
files:
  - src/lib/domain/port/**/*.rs
rule:
  any:
    - all:
        - kind: trait_item
        - pattern: 'pub trait $TRAIT: $$$BOUNDS { $$$BODY }'
    - all:
        - kind: trait_item
        - pattern: 'pub trait $TRAIT { $$$BODY }'
```

`sgconfig.yaml` already declares `ruleDirs: [rule]`, so no `sgconfig.yaml` change is needed.

### `script/rule_enforcer.sh`

Two edits, no runner change.

1. Register the check in `RULES=(…)` (append after the last entry):

```diff
 	"STY-RUST-067:check_sty_rust_067"
 	"STY-RUST-024:check_sty_rust_024"
+	"STY-RUST-069:check_sty_rust_069"
 )
```

2. Add the function in the `# --- Checks ---` section, immediately before the `# --- Runner ---` banner (after `check_sty_rust_024`):

```bash
check_sty_rust_069() {
	local id="STY-RUST-069"
	local rule_id="sty-rust-069"
	local port_dir="src/lib/domain/port"
	local matches trait bounds file result=0 changed super
	local -A supertraits=()
	local -A send_sync=()
	local -A flagged=()

	if [ ! -d "$port_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$port_dir" >&2
		return 1
	fi

	# sty-rust-069 extracts every outbound-port trait with its supertrait
	# bounds; whether a trait is `Send + Sync` directly or through a supertrait
	# is resolved here because an ast-grep rule cannot follow a supertrait.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no outbound port trait from %s\n' "$id" "$port_dir" >&2
		return 1
	fi

	while IFS=$'\t' read -r file trait bounds; do
		[ -n "$trait" ] || continue
		supertraits["$trait"]="$bounds"
		if [[ " $bounds " == *" Send "* ]] && [[ " $bounds " == *" Sync "* ]]; then
			send_sync["$trait"]=1
		fi
	done < <(printf '%s\n' "$matches" | jq -r '
		[.file, .metaVariables.single.TRAIT.text,
		 ([.metaVariables.multi.BOUNDS[]?.text] | join(" "))] | @tsv')

	# A supertrait that is itself `Send + Sync` makes the trait `Send + Sync`.
	# Iterating the known `Send + Sync` names and matching each against the
	# trait's padded bound string keeps the test on a whole token.
	changed=1
	while [ "$changed" -eq 1 ]; do
		changed=0
		for trait in "${!supertraits[@]}"; do
			[ -n "${send_sync[$trait]+set}" ] && continue
			for super in "${!send_sync[@]}"; do
				if [[ " ${supertraits[$trait]} " == *" $super "* ]]; then
					send_sync["$trait"]=1
					changed=1
					break
				fi
			done
		done
	done

	while IFS=$'\t' read -r file trait bounds; do
		[ -n "$trait" ] || continue
		if [ -z "${send_sync[$trait]+set}" ] && [ -z "${flagged[$trait]+set}" ]; then
			printf 'error: %s: %s: outbound port trait %s must be Send + Sync (found: %s)\n' \
				"$id" "$file" "$trait" "${bounds:-none}" >&2
			flagged["$trait"]=1
			result=1
		fi
	done < <(printf '%s\n' "$matches" | jq -r '
		[.file, .metaVariables.single.TRAIT.text,
		 ([.metaVariables.multi.BOUNDS[]?.text] | join(" "))] | @tsv')

	return "$result"
}
```

### `rule_enforcer.yaml`

No change. The check reads no per-rule config; `port_dir` is a fixed repository path, matching `check_sty_rust_018`/`check_sty_rust_019`. A `yq '.'` parse plus `shellcheck` and `shfmt -d -i 0 -ci` on the candidate function are clean.

## Verification

All runs used the candidate rule file verbatim and the exact function above (scratch harness `/tmp/opencode/sty-rust-069/check_scratch.sh`, which sets `SGCONFIG` and cds into the fixture root; fixtures under `/tmp/opencode/sty-rust-069/proj/`).

- Red: `/tmp/opencode/sty-rust-069/proj/red` — `pub trait BadRepository` (no bounds), `pub trait HalfSync: Send` (missing `Sync`), `pub trait Base: Send`, and `pub trait Child: Base` (supertrait not `Send + Sync`) — `devenv shell -- bash /tmp/opencode/sty-rust-069/check_scratch.sh /tmp/opencode/sty-rust-069/proj/red` — observed: `error: STY-RUST-069: src/lib/domain/port/half_sync.rs: outbound port trait HalfSync must be Send + Sync (found: Send)`, `.../base.rs: outbound port trait Base must be Send + Sync (found: Send)`, `.../child.rs: outbound port trait Child must be Send + Sync (found: Base)`, `.../bad_repository.rs: outbound port trait BadRepository must be Send + Sync (found: none)`, then `FAIL`, rc=1. The `Child` case proves the supertrait resolution rejects "extends something that is not itself `Send + Sync`".
- Green: `/tmp/opencode/sty-rust-069/proj/green` — `pub trait Base: Send + Sync`, `pub trait Child: Base`, and `pub trait GoodRepository: Send + Sync` — `devenv shell -- bash /tmp/opencode/sty-rust-069/check_scratch.sh /tmp/opencode/sty-rust-069/proj/green` — observed: `PASS: /tmp/opencode/sty-rust-069/proj/green`, rc=0 (the `Child: Base` case proves a trait inheriting `Send + Sync` from a compliant supertrait passes).
- Repository: the candidate rule mounted over `src` via `/tmp/opencode/sty-rust-069/repocfg/sgconfig.yaml` (`ruleDirs: [rule]` over the candidate) — `SGCONFIG_OVERRIDE=/tmp/opencode/sty-rust-069/repocfg/sgconfig.yaml devenv shell -- bash /tmp/opencode/sty-rust-069/check_scratch.sh /home/will/project/mine/Mnemorium/mnemorium` — observed: `PASS: /home/will/project/mine/Mnemorium/mnemorium`, rc=0. A raw scan of the rule over `src/lib/domain/port` returns 24 traits: 19 declare `Send + Sync` literally, and 5 (`AssetUnitOfWork`, `ConfigurationUnitOfWork`, `IdentityUnitOfWork`, `LibraryUnitOfWork`, `UserUnitOfWork`) extend `UnitOfWork`, itself `Send + Sync`; all 24 pass. After applying, confirm with `devenv shell -- script/rule_enforcer.sh check -v`.

## Next

- Apply: `rule/sty-rust-069.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
