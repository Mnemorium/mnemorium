# Add check — STY-RUST-025

## Rule guard — STY-RUST-025

- **Rule:** "Name variants in `UpperCamelCase`, one per allowed constraint value." (`docs/development/TechnicalDesign.md` § 4.1)
- **Check state:** missing — `STY-RUST-025` is not in `RULES`, no `rule/sty-rust-025.yml` exists; `script/rule_enforcer.sh uncovered` lists it (alongside `023`/`024`).
- **Mechanism:** ast-grep extraction rule + Bash cross-file check — variant names are syntax (ast-grep sees them), but "one per allowed constraint value" requires reading the SQL `CHECK (... IN (...))` lists in `migrations/`, which ast-grep cannot see. No `rule_enforcer.yaml` block: the two paths are fixed, like the other path-bound checks.

## Files

### `rule/sty-rust-025.yml` (new)

The extraction rule: every enum in the SQL data-model directory that maps its variants to uppercase SQL strings. `kg rep` output is consumed by the Bash check.

```yaml
---
id: sty-rust-025
language: Rust
severity: hint
message: >-
  extract SQL CHECK-constraint-backed enums and their variants for
  STY-RUST-025
files:
  - src/lib/infrastructure/outbound/sqlx/model/*.rs
rule:
  all:
    - kind: enum_item
    - pattern: pub enum $ENUM { $$$VARIANTS }
    - follows:
        kind: attribute_item
        regex: 'sqlx\(rename_all = "UPPERCASE"\)'
        stopBy: end
```

### `script/rule_enforcer.sh`

Two edits. First, one registry line (after `STY-RUST-020`):

```bash
	"STY-RUST-020:check_sty_rust_020"
	"STY-RUST-025:check_sty_rust_025"
	"STY-RUST-067:check_sty_rust_067"
)
```

Second, the check function, inserted after `check_sty_rust_067` and before `# --- Runner ---`:

```bash
check_sty_rust_025() {
	local id="STY-RUST-025"
	local rule_id="sty-rust-025"
	local model_dir="src/lib/infrastructure/outbound/sqlx/model"
	local migrations_dir="migrations"
	local matches raw values enum variants variant value result=0
	local -A allowed=()
	local -a sets=()

	if [ ! -d "$model_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$model_dir" >&2
		return 1
	fi

	# sty-rust-025 extracts each `sqlx::Type` enum that maps its variants to
	# uppercase SQL strings; the comparison against the allowed values of the
	# SQL CHECK constraints happens here because an ast-grep rule cannot read
	# migrations/.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$model_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no SQL CHECK-constraint enum from %s\n' "$id" "$model_dir" >&2
		return 1
	fi

	# Every allowed-value set of a SQL `CHECK (... IN ('A', 'B'))` constraint,
	# keyed by its sorted, uppercased values.
	while IFS= read -r raw; do
		[ -n "$raw" ] || continue
		values="$(printf '%s\n' "$raw" |
			grep -oE "'[A-Za-z0-9_]+'" | tr -d "'" |
			tr '[:lower:]' '[:upper:]' | sort | paste -sd, -)"
		[ -n "$values" ] || continue
		allowed["$values"]=1
		sets+=("$values")
	done < <(grep -rhoE "IN \('[A-Za-z0-9_]+'(, *'[A-Za-z0-9_]+')*\)" "$migrations_dir"/*.up.sql 2>/dev/null)

	if [ "${#sets[@]}" -eq 0 ]; then
		printf 'error: %s: no SQL CHECK (... IN (...)) string constraint found in %s\n' "$id" "$migrations_dir" >&2
		return 1
	fi

	while IFS=$'\t' read -r enum variants; do
		[ -n "$enum" ] || continue
		# Naming: every variant is UpperCamelCase.
		while IFS= read -r variant; do
			[ -n "$variant" ] || continue
			if [[ ! $variant =~ ^[A-Z][A-Za-z0-9]*$ ]]; then
				printf 'error: %s: %s: variant %s is not UpperCamelCase\n' \
					"$id" "$enum" "$variant" >&2
				result=1
			fi
		done <<<"$(printf '%s' "$variants" | tr ',' '\n')"

		# Cardinality: the variants, uppercased, are exactly one constraint's
		# allowed values.
		value="$(printf '%s' "$variants" | tr ',' '\n' |
			tr '[:lower:]' '[:upper:]' | sort | paste -sd, -)"
		if [ -z "${allowed[$value]+set}" ]; then
			printf 'error: %s: %s: variants (%s) do not name one variant per allowed CHECK constraint value\n' \
				"$id" "$enum" "$variants" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r '[.metaVariables.single.ENUM.text,
			([.metaVariables.multi.VARIANTS[].text | select(test("^[A-Za-z_][A-Za-z0-9_]*$"))] | join(","))] | @tsv')

	return "$result"
}
```

### `rule_enforcer.yaml`

No change. The check reads no per-rule config; `model_dir` and `migrations_dir` are fixed repository paths, matching `check_sty_rust_018`/`check_sty_rust_019`.

## Verification

- Red: `/tmp/opencode/sty025-red/` (`Role { admin, Standard }` — lowercase variant; `Kind { Alpha, Beta, Gamma }` against `CHECK (kind IN ('ALPHA', 'BETA'))` — extra variant) with `/tmp/opencode/sty025-red/migrations/20260101000000_create_thing.up.sql` — `devenv shell -- bash /tmp/opencode/check_sty_rust_025_scratch.sh /tmp/opencode/sty025-red` — observed: `error: STY-RUST-025: Role: variant admin is not UpperCamelCase` and `error: STY-RUST-025: Kind: variants (Alpha,Beta,Gamma) do not name one variant per allowed CHECK constraint value`, rc=1.
- Green: `/tmp/opencode/sty025-green/` (`Role { Admin, Standard }` and `Kind { Alpha, Beta }`, constraints matching) — same command with the green root — observed: `ok STY-RUST-025`, rc=0.
- Repository: extraction `devenv shell -- ast-grep scan --rule /tmp/opencode/sty-rust-025.yml --json=stream src/lib/infrastructure/outbound/sqlx/model` returns the four constraint-backed enums `Role`(Admin,Standard), `ScanType`(Interlaced,Mbaff,Paff,Progressive), `Orientation`(Landscape,Portrait,Square), `Rotation`(Daily,Hourly,Minutely,Never); the full candidate check run against the repository root — `devenv shell -- bash /tmp/opencode/check_sty_rust_025_repo.sh /home/will/project/mine/Mnemorium/mnemorium` — observed: `ok STY-RUST-025`, rc=0. Each variant set, uppercased, equals exactly one migration `CHECK (... IN (...))` set (`chk_user_role`, `chk_video_scan_type`, `chk_image_orientation`, `chk_configuration_log_rotation`); `chk_mime_type_type` has no modelled enum and is correctly out of scope. After applying, confirm with `devenv shell -- script/rule_enforcer.sh check -v`.

## Next

- Apply: `rule/sty-rust-025.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
