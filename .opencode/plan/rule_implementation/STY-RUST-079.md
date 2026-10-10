# Add check — STY-RUST-079

## Rule guard — STY-RUST-079

- **Rule:** "A repository persists and queries exactly one aggregate; a port that spans two aggregates is split per aggregate." (`STY-RUST-079`, Repository — `docs/development/TechnicalDesign.md` line 189)
- **Check state:** missing — `script/rule_enforcer.sh uncovered` lists `STY-RUST-079`; no registry entry, no function, no `rule/sty-rust-079.yml`.
- **Mechanism:** ast-grep + bash — a rule extracts each repository-port trait and its `create*` signatures; the Bash wrapper counts distinct `create*` methods per trait and fails a port with more than one. ast-grep cannot count within one trait, and the port's aggregate set is derivable from the port itself (`create*` inserts an aggregate, `STY-RUST-044`), so no `rule_enforcer.yaml` whitelist is needed.

**Clarification:** `MediaRepository` is intended to fail this check. It declares `create_image` → `Image` and `create_video` → `Video`, i.e. two aggregates in one port, and must be split into per-aggregate ports (e.g. `ImageRepository` and `VideoRepository`) or have a single declared `Media` aggregate root. The red repository result below is the desired outcome and is deliberately not exempted. There is no whitelist: the aggregate is derived from the port's own `create*` methods, so no `rule_enforcer.yaml` block is added.

## Files

### `rule/sty-rust-079.yml` (new)

```yaml
---
id: sty-rust-079
language: Rust
severity: hint
message: >-
  extract a repository-port trait and its `create*` method signatures for STY-RUST-079
files:
  - src/lib/domain/port/*_repository.rs
rule:
  all:
    - kind: function_signature_item
    - pattern: 'fn $METHOD(&mut self, $$$PARAMS) -> $RET;'
    - regex: '^fn create'
    - inside:
        kind: trait_item
        pattern: 'pub trait $TRAIT: $$$BOUNDS { $$$BODY }'
        stopBy: end
```

### `script/rule_enforcer.sh`

Add one registry line to `RULES=( … )` (append after `"STY-RUST-024:check_sty_rust_024"`):

```bash
	"STY-RUST-079:check_sty_rust_079"
```

Add one function after `check_sty_rust_024` and before `# --- Runner ---`:

```bash
check_sty_rust_079() {
	local id="STY-RUST-079"
	local rule_id="sty-rust-079"
	local port_dir="src/lib/domain/port"
	local matches counts file trait count methods result=0

	if [ ! -d "$port_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$port_dir" >&2
		return 1
	fi

	# sty-rust-079 extracts each repository-port trait and its `create*`
	# method signatures; the per-trait count happens here because an ast-grep
	# rule cannot count matches within one trait. A `create*` method inserts a
	# new aggregate (STY-RUST-044), so a port that declares two distinct
	# `create*` methods persists two aggregates (STY-RUST-079).
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$port_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi

	counts="$(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.METHOD) |
			[.file, .metaVariables.single.TRAIT.text,
			 .metaVariables.single.METHOD.text,
			 .metaVariables.single.RET.text] | @tsv' |
		awk -F'\t' '
			{
				key = $1 SUBSEP $2
				if (seen[key SUBSEP $3]++) next
				n[key]++
				agg = $4
				sub(/^.*Result</, "", agg); sub(/[,>].*$/, "", agg)
				if (n[key] > 1) list[key] = list[key] ", "
				list[key] = list[key] $3 " -> " agg
				file[key] = $1; trait[key] = $2
			}
			END {
				for (k in n) {
					if (n[k] > 1) printf "%s\t%s\t%d\t%s\n", file[k], trait[k], n[k], list[k]
				}
			}
		' | sort)"

	while IFS=$'\t' read -r file trait count methods; do
		[ -n "$trait" ] || continue
		printf 'error: %s: %s: %s spans %s aggregates (%s); split it into one <Aggregate>Repository per aggregate\n' \
			"$id" "$file" "$trait" "$count" "$methods" >&2
		result=1
	done <<<"$counts"

	return "$result"
}
```

No `rule_enforcer.yaml` block is added. `STY-RUST-079` already has a rulebook row, so `validate_registry` accepts the ID; the config file needs no change.

## Verification

- Red: `/tmp/opencode/sg079/red/src/lib/domain/port/media_repository.rs` (trait `MediaRepository` with `create_image -> Image` and `create_video -> Video`) — `devenv shell -- bash /tmp/opencode/sg079/check.sh /tmp/opencode/sg079/red` — observed: `error: STY-RUST-079: src/lib/domain/port/media_repository.rs: MediaRepository spans 2 aggregates (create_image -> Image, create_video -> Video); split it into one <Aggregate>Repository per aggregate` (exit 1).
- Green: `/tmp/opencode/sg079/green/src/lib/domain/port/{gallery_repository.rs,user_repository.rs}` (`GalleryRepository`: `create` → `Gallery` plus child-entity `add_item`; `UserRepository`: one `create` → `User`) — `devenv shell -- bash /tmp/opencode/sg079/check.sh /tmp/opencode/sg079/green` — observed: no finding (exit 0).
- Repository: `devenv shell -- ast-grep scan --config /tmp/opencode/sg079/sgconfig-repo.yaml --filter sty-rust-079 --json=stream src/lib/domain/port` (rule dir pointed at the candidate rule; then the same counting logic) — observed: **red, by design** — one finding — `src/lib/domain/port/media_repository.rs: MediaRepository spans 2 aggregates (create_image -> Image, create_video -> Video)`. The other seven ports each yield exactly one `create*` (`UserRepository`, `FileRepository`, `CredentialRepository`, `ConfigurationRepository`, `UploadRepository`, and `GalleryRepository` via `create`; `MimeTypeRepository` none) and are not flagged; the intended `MediaRepository` finding is the sole failure.

Scratch artifacts used (all under `/tmp/opencode/sg079/`, no tracked file touched): `sty-rust-079.yml`, `check.sh`, `check_repo.sh`, `sgconfig-repo.yaml`, `rule/`, and the `red/` and `green/` fixture roots.

## Next

- Apply: `rule/sty-rust-079.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v` — **expected to fail** on `MediaRepository` until it is split per aggregate. That failure is the rule's intent, not a defect in the check.
