#!/bin/bash
set -u -o pipefail

# Verify the numbered rules declared in TechnicalDesign.md.
#
# The rulebook tables in docs/development/TechnicalDesign.md are the source of
# truth: each row is `| `<ID>` | Section | Rule | More info |`. This script is
# the extensible harness that runs a check per rule; a check is a Bash function
# declared in this same file and registered below. Adding a rule means adding
# one registry line and one function — never editing the runner.
#
# A rule's per-check configuration lives in rule_enforcer.yaml at the repository
# root, under `rules:`. A check reads its own block through `rule_config <ID>`,
# which prints the block as JSON (or `{}`).
#
# Usage:
#   script/rule_enforcer.sh [check]   # run every registered check (default)
#   script/rule_enforcer.sh check -v  # ...and list every check that passed
#   script/rule_enforcer.sh uncovered # list rulebook rules with no check
#   script/rule_enforcer.sh list      # list registered rules
#   script/rule_enforcer.sh help
#
# See docs/development/TechnicalDesign.md, section anatomy, for the rule table.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
readonly REPO_ROOT

readonly RULEBOOK="docs/development/TechnicalDesign.md"
readonly CONFIG="rule_enforcer.yaml"
readonly SGCONFIG="sgconfig.yaml"

# Verbose mode: set by `-v`/`--verbose` with `check`; lists every check, pass or
# fail, instead of only the failures.
VERBOSE=0

# --- Registry ---------------------------------------------------------------
#
# Every check is one line, `<ID>:<function>`, in declaration order. The function
# returns 0 when the rule holds and non-zero when it is violated; it may print a
# diagnostic on stdout, which the runner indents under the failure. The ID must
# name a row in the rulebook (see read_rulebook), and the function must be
# defined in this file.
#
# RULES=(
# 	"DOC-001:check_doc_001"
# )
RULES=(
	"STY-RUST-001:check_sty_rust_001"
	"STY-RUST-002:check_sty_rust_002"
	"STY-RUST-003:check_sty_rust_003"
	"STY-RUST-004:check_sty_rust_004"
	"STY-RUST-005:check_sty_rust_005"
	"STY-RUST-006:check_sty_rust_006"
	"STY-RUST-007:check_sty_rust_007"
	"STY-RUST-008:check_sty_rust_008"
	"STY-RUST-009:check_sty_rust_009"
	"STY-RUST-010:check_sty_rust_010"
	"STY-RUST-011:check_sty_rust_011"
	"STY-RUST-012:check_sty_rust_012"
	"STY-RUST-013:check_sty_rust_013"
	"STY-RUST-014:check_sty_rust_014"
	"STY-RUST-019:check_sty_rust_019"
	"STY-RUST-015:check_sty_rust_015"
	"STY-RUST-016:check_sty_rust_016"
	"STY-RUST-018:check_sty_rust_018"
	"STY-RUST-020:check_sty_rust_020"
	"STY-RUST-023:check_sty_rust_023"
	"STY-RUST-022:check_sty_rust_022"
	"STY-RUST-067:check_sty_rust_067"
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-025:check_sty_rust_025"
	"STY-RUST-026:check_sty_rust_026"
	"STY-RUST-027:check_sty_rust_027"
	"STY-RUST-028:check_sty_rust_028"
	"STY-RUST-029:check_sty_rust_029"
	"STY-RUST-030:check_sty_rust_030"
	"STY-RUST-032:check_sty_rust_032"
	"STY-RUST-033:check_sty_rust_033"
	"STY-RUST-034:check_sty_rust_034"
	"STY-RUST-035:check_sty_rust_035"
	"STY-RUST-036:check_sty_rust_036"
	"STY-RUST-038:check_sty_rust_038"
	"STY-RUST-039:check_sty_rust_039"
	"STY-RUST-041:check_sty_rust_041"
	"STY-RUST-042:check_sty_rust_042"
	"STY-RUST-043:check_sty_rust_043"
	"STY-RUST-044:check_sty_rust_044"
	"STY-RUST-045:check_sty_rust_045"
	"STY-RUST-046:check_sty_rust_046"
	"STY-RUST-047:check_sty_rust_047"
	"STY-RUST-048:check_sty_rust_048"
	"STY-RUST-049:check_sty_rust_049"
	"STY-RUST-050:check_sty_rust_050"
	"STY-RUST-051:check_sty_rust_051"
	"STY-RUST-052:check_sty_rust_052"
	"STY-RUST-053:check_sty_rust_053"
	"STY-RUST-054:check_sty_rust_054"
	"STY-RUST-055:check_sty_rust_055"
	"STY-RUST-056:check_sty_rust_056"
	"STY-RUST-057:check_sty_rust_057"
	"STY-RUST-058:check_sty_rust_058"
	"STY-RUST-059:check_sty_rust_059"
	"STY-RUST-060:check_sty_rust_060"
	"STY-RUST-061:check_sty_rust_061"
	"STY-RUST-062:check_sty_rust_062"
	"STY-RUST-063:check_sty_rust_063"
	"STY-RUST-064:check_sty_rust_064"
	"STY-RUST-065:check_sty_rust_065"
	"STY-RUST-066:check_sty_rust_066"
	"STY-RUST-068:check_sty_rust_068"
	"STY-RUST-069:check_sty_rust_069"
	"STY-RUST-070:check_sty_rust_070"
	"STY-RUST-071:check_sty_rust_071"
	"STY-RUST-072:check_sty_rust_072"
	"STY-RUST-073:check_sty_rust_073"
	"STY-RUST-074:check_sty_rust_074"
	"STY-RUST-075:check_sty_rust_075"
	"STY-RUST-076:check_sty_rust_076"
	"STY-RUST-077:check_sty_rust_077"
	"STY-RUST-078:check_sty_rust_078"
	"STY-RUST-079:check_sty_rust_079"
	"STY-RUST-080:check_sty_rust_080"
	"STY-RUST-081:check_sty_rust_081"
	"STY-RUST-082:check_sty_rust_082"
	"STY-RUST-083:check_sty_rust_083"
	"DOC-003:check_doc_003"
)

# --- Rulebook ---------------------------------------------------------------

declare -A RULE_SECTION=()
declare -A RULE_TEXT=()

die() {
	printf 'error: %s\n' "$*" >&2
	exit 1
}

die_code() {
	local code="$1"
	shift
	printf 'error: %s\n' "$*" >&2
	exit "$code"
}

usage() {
	cat <<'EOF'
Usage:
  script/rule_enforcer.sh [check]   Run every registered check (default).
  script/rule_enforcer.sh uncovered List rulebook rules with no check.
  script/rule_enforcer.sh list      List registered rules.
  script/rule_enforcer.sh help      Show this help.

  -v, --verbose   With `check`, list every check that passed, not only failures.

A check is registered in this file as `<ID>:<function>` and reads its own
configuration block from rule_enforcer.yaml through `rule_config <ID>`.
EOF
}

# read_rulebook — parse every rule row of the rulebook into RULE_SECTION and
# RULE_TEXT. A rule row is a table row whose first cell is a backticked ID, and
# it is accepted only under a `##` section named in the Section registry — this
# skips the illustrative table in the "Rule model" prose, which reuses a real
# ID. Each row's `Rule` cell is the description; a `|` inside it is joined back,
# then runs of whitespace collapse to single spaces.
read_rulebook() {
	[ -f "$RULEBOOK" ] || die "missing $RULEBOOK"

	local id section rule
	while IFS=$'\t' read -r id section rule; do
		if [ -n "${RULE_TEXT[$id]+set}" ]; then
			die "duplicate rule id in $RULEBOOK: $id"
		fi
		RULE_SECTION["$id"]="$section"
		RULE_TEXT["$id"]="$rule"
	done < <(awk '
		function trim(s) { gsub(/^[[:space:]]+|[[:space:]]+$/, "", s); return s }

		/^### Section registry$/ { in_registry = 1; next }
		in_registry && /^##/ { in_registry = 0 }
		in_registry && /^\|/ {
			n = split($0, c, "|")
			if (n >= 4) {
				num = trim(c[2]); name = trim(c[3])
				if (num ~ /^[0-9]+$/ && name != "") { registered[name] = 1 }
			}
			next
		}

		/^## / { heading = trim(substr($0, 4)); next }

		/^\|/ {
			n = split($0, c, "|")
			if (n < 5) next
			id = trim(c[2])
			if (id !~ /^`[A-Z][A-Z-]*-[0-9]+`$/) next
			if (!(heading in registered)) next
			gsub(/`/, "", id)
			section = trim(c[3])
			rule = ""
			for (i = 4; i <= n - 2; i++) {
				rule = rule (i > 4 ? "|" : "") c[i]
			}
			gsub(/[[:space:]]+/, " ", rule)
			printf "%s\t%s\t%s\n", id, section, trim(rule)
		}
	' "$RULEBOOK")

	[ "${#RULE_TEXT[@]}" -gt 0 ] || die "no rule rows parsed from $RULEBOOK"
}

# print_rule <id> — the shared `ID (Section) — Rule` format for failures, the
# registered list and the uncovered list.
print_rule() {
	printf '%s (%s) — %s\n' "$1" "${RULE_SECTION[$1]}" "${RULE_TEXT[$1]}"
}

# --- Configuration ----------------------------------------------------------

# require_yq — the harness reads the YAML config through yq, which the dev shell
# provides. A missing yq is a tool error, not a malformed config, so the two are
# told apart before yq is trusted.
require_yq() {
	command -v yq >/dev/null 2>&1
}

# rule_config <id> — print the rule's block from rule_enforcer.yaml as compact
# JSON, or `{}` when the file, the `rules:` map or the block is absent. Returns
# non-zero only on a tool or parse error, so a check can tell "no config" from
# "config broken".
rule_config() {
	local id="${1:-}"
	if [ -z "$id" ]; then
		printf 'error: rule_config needs a rule id\n' >&2
		return 2
	fi

	if [ ! -f "$CONFIG" ] || [ ! -s "$CONFIG" ]; then
		printf '{}\n'
		return 0
	fi

	if ! require_yq; then
		printf 'error: %s: yq not found on PATH (run inside the dev shell: devenv shell)\n' "$id" >&2
		return 2
	fi

	local block
	block="$(yq -c ".rules[\"$id\"] // {}" "$CONFIG")" || return 2
	[ -n "$block" ] || block='{}'
	printf '%s\n' "$block"
}

# validate_config — hard-fail on a malformed file; a missing or empty file is
# valid (all blocks are then `{}`). A missing yq is reported as the tool error it
# is, never as a malformed config.
validate_config() {
	[ -f "$CONFIG" ] || return 0
	[ -s "$CONFIG" ] || return 0
	if ! require_yq; then
		die_code 2 "required tool not found: yq (run inside the dev shell: devenv shell)"
	fi
	if ! yq '.' "$CONFIG" >/dev/null 2>&1; then
		die_code 2 "malformed config: $CONFIG"
	fi
}

# warn_unknown_config_rules — a `rules:` key absent from the rulebook is a
# warning, not a failure: the config may lead the registry during development.
warn_unknown_config_rules() {
	[ -f "$CONFIG" ] && [ -s "$CONFIG" ] || return 0
	local key
	while IFS= read -r key; do
		[ -n "$key" ] || continue
		if [ -z "${RULE_TEXT[$key]+set}" ]; then
			printf 'warning: config references unknown rule id: %s\n' "$key" >&2
		fi
	done < <(yq -r '.rules // {} | keys[]' "$CONFIG" 2>/dev/null)
}

# --- Checks -----------------------------------------------------------------

check_sty_rust_001() {
	local id="STY-RUST-001"
	local rule_id="sty-rust-001"
	local config matches file line func reason location result=0
	local -A allowed=()

	config="$(rule_config "$id")" || return 2

	# The allowlist is the set of code locations permitted to hold a single-use
	# extraction: a production `#[expect(clippy::single_call_fn, reason = "…")]`
	# is permitted only at a `<file>::<function>` listed here. Adding an entry is
	# adding that location; the diagnostic prints it verbatim. An empty list
	# approves nothing, so every occurrence is reported until it is reviewed.
	while IFS= read -r location; do
		[ -n "$location" ] || continue
		allowed["$location"]=1
	done < <(jq -r '.allowed_locations[]?' <<<"$config")

	# sty-rust-001 extracts every production `#[expect(clippy::single_call_fn,
	# reason = "…")]` together with the function it annotates; test modules and
	# `#[cfg(test)]` items are excluded by the rule. The location-vs-allowlist
	# comparison happens here because an ast-grep rule cannot read
	# rule_enforcer.yaml.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	[ -n "$matches" ] || return 0

	while IFS=$'\t' read -r file line func reason; do
		[ -n "$file" ] || continue
		location="${file}::${func}"
		if [ -z "$reason" ]; then
			printf 'error: %s: %s:%s: single-use extraction at %s carries no reason; add one\n' \
				"$id" "$file" "$line" "$location" >&2
			result=1
			continue
		fi
		if [ -z "${allowed[$location]+set}" ]; then
			printf 'error: %s: %s:%s: single-use extraction at %s is not allowlisted; add "%s" to allowed_locations\n' \
				"$id" "$file" "$line" "$location" "$location" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, ((.range.start.line + 1) | tostring),
			.metaVariables.single.FUNC.text,
			(.text | gsub("\\s+"; " ") |
				capture("reason *= *\"(?<r>[^\"]*)\"")? | .r // "")] | @tsv')

	return "$result"
}

check_sty_rust_002() {
	local id="STY-RUST-002"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local config context name dir
	local -a contexts=()
	local -A is_context=()
	local result=0

	config="$(rule_config "$id")" || return 2
	mapfile -t contexts < <(
		jq -r '.bounded_contexts[]?' <<<"$config"
	)
	if [ "${#contexts[@]}" -eq 0 ]; then
		printf 'error: %s: rule_enforcer.yaml declares no bounded_contexts\n' "$id" >&2
		return 2
	fi

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	for context in "${contexts[@]}"; do
		is_context["$context"]=1
		if [ ! -f "${handler_dir}/${context}.rs" ]; then
			printf 'error: %s: missing bounded-context module %s\n' "$id" "${handler_dir}/${context}.rs" >&2
			result=1
		fi
		if [ ! -d "${handler_dir}/${context}" ]; then
			printf 'error: %s: missing bounded-context folder %s\n' "$id" "${handler_dir}/${context}" >&2
			result=1
		fi
	done

	while IFS= read -r dir; do
		name="${dir##*/}"
		if [ -z "${is_context[$name]+set}" ]; then
			printf 'error: %s: %s is not a bounded-context folder\n' "$id" "$dir" >&2
			result=1
		fi
	done < <(find "$handler_dir" -mindepth 1 -maxdepth 1 -type d | sort)

	return "$result"
}

check_sty_rust_003() {
	local id="STY-RUST-003"
	local rule_id="sty-rust-003"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs 'src/lib/infrastructure/inbound/rest/handler/*/*.rs' \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_004() {
	local id="STY-RUST-004"
	local rule_id="sty-rust-004"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_005() {
	local id="STY-RUST-005"
	local rule_id="sty-rust-005"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_006() {
	local id="STY-RUST-006"
	local rule_id="sty-rust-006"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local matches declared file fn base result=0

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	# sty-rust-006 extracts the handler — the function carrying the endpoint's
	# `#[utoipa::path]` attribute. The file-name comparison happens here because
	# an ast-grep rule cannot see the file path it is matching in.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$handler_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no handler from %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	# Every file that declares an endpoint must yield a handler.
	declared="$(grep -rl '#\[utoipa::path' "$handler_dir" | sort -u)"
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		if ! printf '%s\n' "$matches" | jq -r '.file' | grep -qxF "$file"; then
			printf 'error: %s: %s declares an endpoint but no handler was extracted\n' "$id" "$file" >&2
			result=1
		fi
	done <<<"$declared"

	while IFS=$'\t' read -r file fn; do
		[ -n "$file" ] || continue
		base="$(basename "$file" .rs)"
		if [ "$fn" != "$base" ]; then
			printf 'error: %s: handler %s in %s must be named %s after the file\n' "$id" "$fn" "$file" "$base" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" | jq -r '[.file, .metaVariables.single.FUNC.text] | @tsv')

	return "$result"
}

check_sty_rust_007() {
	local id="STY-RUST-007"
	local rule_id="sty-rust-007"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_014() {
	local id="STY-RUST-014"
	local rule_id="sty-rust-014"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_015() {
	local id="STY-RUST-015"
	local rule_id="sty-rust-015"
	local target="src/lib/domain/port/error.rs"
	local matches file result=0

	# sty-rust-015 extracts every outbound-port error enum declared under
	# src/lib/domain/port/; the file comparison below holds them all to
	# error.rs, so a one-per-port family (e.g. UserRepositoryError beside
	# RepositoryError) is rejected. The path check lives here because an
	# ast-grep rule cannot see the file it is matching in.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src/lib/domain/port 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi

	while IFS= read -r file; do
		[ -n "$file" ] || continue
		if [ "$file" != "$target" ]; then
			printf 'error: %s: port error enum must be declared in %s (found in %s)\n' \
				"$id" "$target" "$file" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" | jq -r '.file')

	return "$result"
}

check_sty_rust_016() {
	local id="STY-RUST-016"
	local rule_id="sty-rust-016"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_018() {
	local id="STY-RUST-018"
	local rule_id="sty-rust-018"
	local model_dir="src/lib/domain/model"
	local output

	if [ ! -d "$model_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$model_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$model_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_020() {
	local id="STY-RUST-020"
	local rule_id="sty-rust-020"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_023() {
	local id="STY-RUST-023"
	local rule_id="sty-rust-023"
	local model_dir="src/lib/infrastructure/outbound/sqlx/model"
	local matches file enum field type base expected key result=0
	local -A enums=()

	if [ ! -d "$model_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$model_dir" >&2
		return 1
	fi

	# sty-rust-023 extracts each sqlx CHECK-constraint enum and each model
	# field; the field-name-to-enum-name pairing happens here because an
	# ast-grep rule cannot relate two identifiers.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$model_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no enum or field from %s\n' "$id" "$model_dir" >&2
		return 1
	fi

	# Enums declared in this file, keyed by file so an imported enum is not
	# mistaken for the attribute's own type.
	while IFS=$'\t' read -r file enum; do
		[ -n "$enum" ] || continue
		enums["${file}:${enum}"]=1
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.ENUM) | [.file, .metaVariables.single.ENUM.text] | @tsv')

	# A model attribute typed by a local enum must be named after it:
	# UpperCamelCase(field) == enum.
	while IFS=$'\t' read -r file field type; do
		[ -n "$type" ] || continue
		base="${type#Option<}"
		base="${base%>}"
		key="${file}:${base}"
		if [ -z "${enums[$key]+set}" ]; then
			continue
		fi
		expected="$(printf '%s' "$field" |
			awk -F_ '{ for (i = 1; i <= NF; i++) printf "%s%s", toupper(substr($i, 1, 1)), substr($i, 2) }')"
		if [ "$base" != "$expected" ]; then
			printf 'error: %s: %s: enum %s names the %s attribute; expected %s\n' \
				"$id" "$file" "$base" "$field" "$expected" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.FIELD) | [.file, .metaVariables.single.FIELD.text, .metaVariables.single.TYPE.text] | @tsv')

	return "$result"
}

check_sty_rust_022() {
	local id="STY-RUST-022"
	local rule_id="sty-rust-022"
	local middleware_dir="src/lib/infrastructure/inbound/rest/middleware"
	local output

	if [ ! -d "$middleware_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$middleware_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$middleware_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_013() {
	local id="STY-RUST-013"
	local rule_id="sty-rust-013"
	local model_dir="src/lib/domain/model"
	local matches file line name base key result=0
	local -A struct_line=()

	if [ ! -d "$model_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$model_dir" >&2
		return 1
	fi

	# sty-rust-013 extracts each domain error enum and each model struct; the
	# `<Model>Error`-before-`<Model>` pairing happens here because an ast-grep
	# rule cannot relate two identifiers.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$model_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no domain error or model struct from %s\n' "$id" "$model_dir" >&2
		return 1
	fi

	while IFS=$'\t' read -r file line name; do
		[ -n "$name" ] || continue
		struct_line["${file}:${name}"]="$line"
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.MODEL) | [.file, (.range.start.line|tostring), .metaVariables.single.MODEL.text] | @tsv')

	while IFS=$'\t' read -r file line name; do
		[ -n "$name" ] || continue
		base="${name%Error}"
		if [ "$base" = "$name" ]; then
			continue
		fi
		key="${file}:${base}"
		if [ -z "${struct_line[$key]+set}" ]; then
			printf 'error: %s: %s: domain error enum %s has no model struct %s in the same file\n' \
				"$id" "$file" "$name" "$base" >&2
			result=1
		elif [ "$line" -ge "${struct_line[$key]}" ]; then
			printf 'error: %s: %s: domain error enum %s must be declared before struct %s\n' \
				"$id" "$file" "$name" "$base" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.ERROR_ENUM) | [.file, (.range.start.line|tostring), .metaVariables.single.ERROR_ENUM.text] | @tsv')

	return "$result"
}

check_sty_rust_019() {
	local id="STY-RUST-019"
	local rule_id="sty-rust-019"
	local model_dir="src/lib/domain/model"
	local matches file struct impl name ret key result=0
	local -A fields=()
	# Return types that are never `Copy`: a `&self` getter returning one can
	# only hand back an owned clone, which the rule forbids.
	local owned='^(Option<)?(String|Vec<.*>|Box<.*>|PathBuf|OsString|CString|VecDeque<.*>|LinkedList<.*>|HashMap<.*>|HashSet<.*>|BTreeMap<.*>|BTreeSet<.*>)(>)?$'

	if [ ! -d "$model_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$model_dir" >&2
		return 1
	fi

	# sty-rust-019 extracts each struct field and each `&self` method; the
	# field-name-to-getter pairing happens here because an ast-grep rule cannot
	# relate two identifiers.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$model_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no field or getter from %s\n' "$id" "$model_dir" >&2
		return 1
	fi

	# Fields, keyed by the struct that declares them.
	while IFS=$'\t' read -r file struct name; do
		[ -n "$name" ] || continue
		fields["${file}:${struct}:${name}"]=1
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.FIELD) | [.file, .metaVariables.single.STRUCT.text, .metaVariables.single.FIELD.text] | @tsv')

	# A getter is a `&self` method named after a field of the same struct. It
	# must return a borrowed reference or a Copy value, never an owned type.
	while IFS=$'\t' read -r file impl name ret; do
		[ -n "$name" ] || continue
		key="${file}:${impl}:${name}"
		if [ -z "${fields[$key]+set}" ]; then
			continue
		fi
		if [[ $ret =~ $owned ]]; then
			printf 'error: %s: %s: getter %s returns owned %s; return a borrowed reference or a Copy value\n' \
				"$id" "$file" "$name" "$ret" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.METHOD) | [.file, .metaVariables.single.IMPL.text, .metaVariables.single.METHOD.text, .metaVariables.single.RET.text] | @tsv')

	return "$result"
}

check_sty_rust_008() {
	local id="STY-RUST-008"
	local rule_id="sty-rust-008"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_009() {
	local id="STY-RUST-009"
	local rule_id="sty-rust-009"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_010() {
	local id="STY-RUST-010"
	local rule_id="sty-rust-010"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_011() {
	local id="STY-RUST-011"
	local rule_id="sty-rust-011"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_012() {
	local id="STY-RUST-012"
	local rule_id="sty-rust-012"
	local target="src/lib/infrastructure/inbound/rest/api_error.rs"
	local matches file result=0

	# sty-rust-012 is a positive rule: it matches a compliant `enum ApiError` —
	# one not derived with `thiserror`. It scans all of `src` so the file
	# comparison below also catches a declaration in the wrong file.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi

	if [ -z "$matches" ]; then
		printf 'error: %s: ApiError is not declared in %s without a thiserror derive\n' \
			"$id" "$target" >&2
		return 1
	fi

	while IFS= read -r file; do
		[ -n "$file" ] || continue
		if [ "$file" != "$target" ]; then
			printf 'error: %s: ApiError must be declared only in %s (found in %s)\n' \
				"$id" "$target" "$file" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" | jq -r '.file')

	return "$result"
}

check_sty_rust_067() {
	local id="STY-RUST-067"
	local rule_id="sty-rust-067"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs '!src/bin/server.rs' \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_024() {
	local id="STY-RUST-024"
	local rule_id="sty-rust-024"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

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

check_sty_rust_026() {
	local id="STY-RUST-026"
	local rule_id="sty-rust-026"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_027() {
	local id="STY-RUST-027"
	local rule_id="sty-rust-027"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_028() {
	local id="STY-RUST-028"
	local rule_id="sty-rust-028"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_029() {
	local id="STY-RUST-029"
	local rule_id="sty-rust-029"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_030() {
	local id="STY-RUST-030"
	local rule_id="sty-rust-030"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_032() {
	local id="STY-RUST-032"
	local rule_id="sty-rust-032"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_033() {
	local id="STY-RUST-033"
	local rule_id="sty-rust-033"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs 'src/lib/application/use_case/**/*.rs' \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_034() {
	local id="STY-RUST-034"
	local rule_id="sty-rust-034"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs 'src/lib/application/use_case/**/*.rs' \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_035() {
	local id="STY-RUST-035"
	local rule_id="sty-rust-035"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs 'src/**/*.rs' \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_036() {
	local id="STY-RUST-036"
	local rule_id="sty-rust-036"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_038() {
	local id="STY-RUST-038"
	local rule_id="sty-rust-038"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs '!src/lib/infrastructure/**' \
		--globs '!src/lib/infrastructure.rs' \
		--globs '!src/bin/server.rs' \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_039() {
	local id="STY-RUST-039"
	local rule_id="sty-rust-039"
	local target="src/lib/application/use_case"
	local matches file func enc key line a b i text result=0
	local -a lines=()
	local -A begins=()
	local -A function_of=()

	if [ ! -d "$target" ]; then
		printf 'error: %s: missing %s\n' "$id" "$target" >&2
		return 1
	fi

	# sty-rust-039 extracts every unit-of-work `begin()` call with the function
	# that encloses it; counting and ordering happen here because an ast-grep
	# rule cannot compare two matches.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$target" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	[ -n "$matches" ] || return 0

	while IFS=$'\t' read -r file enc func line; do
		[ -n "$file" ] || continue
		key="${file}:${enc}"
		begins["$key"]+="${line}"$'\n'
		function_of["$key"]="$func"
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, ((.metaVariables.single.ENCLOSING.range.start.line) | tostring),
			.metaVariables.single.FUNC.text, ((.range.start.line + 1) | tostring)] | @tsv')

	# A function may open more than one unit of work only when every earlier one
	# is consumed before the next opens (commit/rollback, or a helper such as
	# discard/drop that wraps them): two live units of work are two concurrent
	# transactions and break atomicity.
	for key in "${!begins[@]}"; do
		mapfile -t lines < <(printf '%s' "${begins[$key]}" | sort -n)
		if [ "${#lines[@]}" -lt 2 ]; then
			continue
		fi
		file="${key%:*}"
		for ((i = 1; i < ${#lines[@]}; i++)); do
			a="${lines[$((i - 1))]}"
			b="${lines[$i]}"
			text="$(sed -n "$((a + 1)),$((b - 1))p" "$file")"
			if ! printf '%s\n' "$text" |
				grep -Eq '\.(commit|rollback)[[:space:]]*\(\)|(^|[^[:alnum:]_])(discard|drop)[[:space:]]*\('; then
				printf 'error: %s: %s: %s() opens a second unit of work before the previous one is consumed\n' \
					"$id" "$file" "${function_of[$key]}" >&2
				result=1
			fi
		done
	done

	return "$result"
}

check_sty_rust_041() {
	local id="STY-RUST-041"
	local rule_id="sty-rust-041"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_042() {
	local id="STY-RUST-042"
	local rule_id="sty-rust-042"
	local target="src/lib/domain/port/unit_of_work.rs"
	local output

	if [ ! -f "$target" ]; then
		printf 'error: %s: missing %s\n' "$id" "$target" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$target" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_043() {
	local id="STY-RUST-043"
	local rule_id="sty-rust-043"
	local target="src/lib/domain/port/unit_of_work.rs"
	local matches line name second="" count=0 result=0

	if [ ! -f "$target" ]; then
		printf 'error: %s: missing %s\n' "$id" "$target" >&2
		return 1
	fi

	# sty-rust-043 extracts every top-level trait declaration in the
	# unit-of-work port file; the "second" position is compared here because
	# an ast-grep rule cannot relate two declarations.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$target" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi

	while IFS=$'\t' read -r line name; do
		[ -n "$name" ] || continue
		count=$((count + 1))
		if [ "$count" -eq 2 ]; then
			second="$name"
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r '[.range.start.line, .metaVariables.single.NAME.text] | @tsv' | sort -n)

	if [ "$count" -lt 2 ] || [ "$second" != "UnitOfWorkFactory" ]; then
		printf 'error: %s: %s must declare UnitOfWorkFactory as the second trait\n' \
			"$id" "$target" >&2
		result=1
	fi

	return "$result"
}

check_sty_rust_044() {
	local id="STY-RUST-044"
	local rule_id="sty-rust-044"
	local dir="src/lib/infrastructure/outbound/sqlx"
	local matches file func sql table_name cols identity returning result=0

	if [ ! -d "$dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$dir" >&2
		return 1
	fi

	# sty-rust-044 extracts every `INSERT INTO` of a repository adapter `create`
	# together with the enclosing function; the identity-column check happens
	# here because an ast-grep rule cannot read inside the SQL string. The
	# `configuration` singleton (PERS-009, `configuration_id = 0`) is not a new
	# aggregate, so the datastore does not assign its identity and its adapter is
	# out of scope.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs 'src/lib/infrastructure/outbound/sqlx/*_repository.rs' \
		--globs '!src/lib/infrastructure/outbound/sqlx/configuration_repository.rs' \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no repository create insert\n' "$id" >&2
		return 1
	fi

	while IFS=$'\t' read -r file func sql; do
		[ -n "$sql" ] || continue
		# The identity column of the table named by `INSERT INTO <table>` is
		# `<table>_id`: the datastore assigns it, so `create` must neither bind
		# it nor omit it from `RETURNING`.
		table_name="$(printf '%s' "$sql" |
			sed -n 's/.*INSERT INTO[[:space:]]*\([A-Za-z_][A-Za-z0-9_]*\)[[:space:]]*(.*/\1/p')"
		cols="$(printf '%s' "$sql" |
			sed -n 's/.*INSERT INTO[[:space:]]*[A-Za-z_][A-Za-z0-9_]*[[:space:]]*(\([^)]*\)).*/\1/p')"
		identity="${table_name}_id"
		if printf '%s' "$cols" | tr ',' '\n' | grep -Eq "^[[:space:]]*${identity}[[:space:]]*$"; then
			printf 'error: %s: %s: %s must not bind the identity column %s\n' \
				"$id" "$file" "$func" "$identity" >&2
			result=1
		fi
		returning="${sql#*RETURNING}"
		if ! printf '%s' "$returning" | grep -Eq "(^|[ ,])${identity}([ ,\"]|$)"; then
			printf 'error: %s: %s: %s must return the identity column %s\n' \
				"$id" "$file" "$func" "$identity" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, .metaVariables.single.FUNC.text, (.text | gsub("\\s+"; " "))] | @tsv')

	return "$result"
}

check_sty_rust_045() {
	local id="STY-RUST-045"
	local rule_id="sty-rust-045"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_046() {
	local id="STY-RUST-046"
	local rule_id="sty-rust-046"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_047() {
	local id="STY-RUST-047"
	local rule_id="sty-rust-047"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_048() {
	local id="STY-RUST-048"
	local rule_id="sty-rust-048"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_049() {
	local id="STY-RUST-049"
	local rule_id="sty-rust-049"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_050() {
	local id="STY-RUST-050"
	local rule_id="sty-rust-050"
	local mapping="src/lib/infrastructure/outbound/sqlx/error_mapping.rs"
	local output result=0

	if [ ! -f "$mapping" ]; then
		printf 'error: %s: missing %s\n' "$id" "$mapping" >&2
		return 1
	fi

	# Clause 1: sqlx types never cross into the domain or application layers.
	# The rule is scoped to those two trees and matches only Rust paths,
	# `use` declarations and attributes — never a string literal such as the
	# `debug,sqlx=warn` logging directive.
	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		:
	else
		printf '%s\n' "$output"
		result=1
	fi

	# Clause 2: sqlx::Error is mapped to RepositoryError in error_mapping.rs.
	if ! grep -qE '^use sqlx::Error;' "$mapping" ||
		! grep -qE '^impl From<Error> for RepositoryError\b' "$mapping"; then
		printf 'error: %s: %s must map sqlx::Error to RepositoryError\n' "$id" "$mapping" >&2
		result=1
	fi

	return "$result"
}

check_sty_rust_051() {
	local id="STY-RUST-051"
	local rule_id="sty-rust-051"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs 'src/lib/domain/port/*_repository.rs' \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_052() {
	local id="STY-RUST-052"
	local rule_id="sty-rust-052"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_053() {
	local id="STY-RUST-053"
	local rule_id="sty-rust-053"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_054() {
	local id="STY-RUST-054"
	local rule_id="sty-rust-054"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_055() {
	local id="STY-RUST-055"
	local rule_id="sty-rust-055"
	local use_case_dir="src/lib/application/use_case"
	local matches file trait impl_type base expected result=0 count=0

	if [ ! -d "$use_case_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$use_case_dir" >&2
		return 1
	fi

	# sty-rust-055 extracts each use-case trait implementation; the
	# trait-to-struct comparison happens here because an ast-grep rule cannot
	# strip the `UseCase` suffix from one identifier and compare it to another.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$use_case_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no use-case implementation from %s\n' "$id" "$use_case_dir" >&2
		return 1
	fi

	while IFS=$'\t' read -r file trait impl_type; do
		[ -n "$trait" ] || continue
		count=$((count + 1))
		expected="${trait%UseCase}"
		base="${impl_type%%<*}"
		if [ "$base" != "$expected" ]; then
			printf 'error: %s: %s: %s implements %s; expected the implementation to be named %s\n' \
				"$id" "$file" "$base" "$trait" "$expected" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, .metaVariables.single.TRAIT.text, .metaVariables.single.TYPE.text] | @tsv')

	if [ "$count" -eq 0 ]; then
		printf 'error: %s: extracted no use-case implementation from %s\n' "$id" "$use_case_dir" >&2
		return 1
	fi

	return "$result"
}

check_sty_rust_056() {
	local id="STY-RUST-056"
	local rule_id="sty-rust-056"
	local port_dir="src/lib/application/port"
	local output

	if [ ! -d "$port_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$port_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$port_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_057() {
	local id="STY-RUST-057"
	local rule_id="sty-rust-057"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_058() {
	local id="STY-RUST-058"
	local rule_id="sty-rust-058"
	local port_dir="src/lib/application/port"
	local output

	if [ ! -d "$port_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$port_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs 'src/lib/application/port/*.rs' \
		--globs '!src/lib/application/port/*_use_case_factory.rs' \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_059() {
	local id="STY-RUST-059"
	local rule_id="sty-rust-059"
	local port_dir="src/lib/application/port"
	local output

	if [ ! -d "$port_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$port_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$port_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

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
			tr '[:upper:]' '[:lower:]')_use_case_factory.rs"
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

check_sty_rust_062() {
	local id="STY-RUST-062"
	local factory_dir="src/lib/infrastructure/use_case_factory"
	local file name struct_block result=0
	local -a forbidden=()

	if [ ! -d "$factory_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$factory_dir" >&2
		return 1
	fi

	while IFS= read -r file; do
		[ -n "$file" ] || continue

		# Types the file imports from the application, domain-port or outbound
		# layers are use cases and adapters: the factory must build them per
		# use, never store them. The unit-of-work factory is the one exception
		# STY-RUST-061 permits the struct to hold.
		forbidden=()
		while IFS= read -r name; do
			[ -n "$name" ] || continue
			[ "$name" = "SqlxUnitOfWorkFactory" ] && continue
			forbidden+=("$name")
		done < <(grep -oE 'crate::(application|domain::port|infrastructure::outbound)::[A-Za-z0-9_:]+' "$file" |
			sed 's/.*:://' | sort -u)

		# The long-lived factory struct's own fields, up to its closing brace.
		struct_block="$(awk '/^pub struct [A-Za-z0-9_]+ ?\{/{f=1} f{print} f&&/^}/{f=0}' "$file")"

		if [ "${#forbidden[@]}" -gt 0 ]; then
			for name in "${forbidden[@]}"; do
				if printf '%s\n' "$struct_block" | grep -qE ":.*\b${name}\b"; then
					printf 'error: %s: %s: the factory struct stores %s; build it in the accessor instead\n' \
						"$id" "$file" "$name" >&2
					result=1
				fi
			done
		fi
	done < <(find "$factory_dir" -maxdepth 1 -name '*.rs' | sort)

	return "$result"
}

check_sty_rust_063() {
	local id="STY-RUST-063"
	local rule_id="sty-rust-063"
	local factory_dir="src/lib/infrastructure/use_case_factory"
	local output

	if [ ! -d "$factory_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$factory_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$factory_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_rust_064() {
	local id="STY-RUST-064"
	local rule_id="sty-rust-064"
	local app_state="src/lib/infrastructure/inbound/rest/app_state.rs"
	local server="src/bin/server.rs"
	local matches app_matches file bindings binding_count binding executions result=0

	if [ ! -f "$app_state" ]; then
		printf 'error: %s: missing %s\n' "$id" "$app_state" >&2
		return 1
	fi

	# sty-rust-064 is a positive rule: it matches the compliant `AppState`
	# field and the composition root's `LoadConfiguration` binding. It scans
	# all of `src` so the file comparison below also catches an `AppState`
	# declared in the wrong file.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	# Clause A: AppState stores the live configuration as
	# `Arc<ArcSwap<Configuration>>`. The rule is positive, so a missing match
	# is the violation.
	app_matches="$(printf '%s\n' "$matches" |
		jq -c 'select(.metaVariables.single.VAR | not)')"
	if [ -z "$app_matches" ]; then
		printf 'error: %s: AppState does not store the configuration as Arc<ArcSwap<Configuration>>\n' \
			"$id" >&2
		return 1
	fi
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		if [ "$file" != "$app_state" ]; then
			printf 'error: %s: AppState must be declared in %s (found in %s)\n' \
				"$id" "$app_state" "$file" >&2
			result=1
		fi
	done < <(printf '%s\n' "$app_matches" | jq -r '.file')

	# Clause B: LoadConfiguration runs once at startup. The rule captures the
	# binding the composition root creates; the execute count is read here
	# because an ast-grep rule cannot relate the binding to a later call.
	bindings="$(printf '%s\n' "$matches" |
		jq -r --arg file "$server" \
			'select(.metaVariables.single.VAR and (.file == $file)) | .metaVariables.single.VAR.text')"
	binding_count="$(printf '%s\n' "$bindings" | grep -c .)"
	if [ "$binding_count" -ne 1 ]; then
		printf 'error: %s: %s must bind LoadConfiguration exactly once at startup (found %s)\n' \
			"$id" "$server" "$binding_count" >&2
		result=1
	fi
	if [ "$binding_count" -eq 1 ]; then
		binding="$(printf '%s\n' "$bindings" | head -n 1)"
		executions="$(grep -cF "${binding}.execute(" "$server")"
		if [ "$executions" -ne 1 ]; then
			printf 'error: %s: LoadConfiguration must run exactly once at startup (found %s execute call(s) on %s in %s)\n' \
				"$id" "$executions" "$binding" "$server" >&2
			result=1
		fi
	fi

	return "$result"
}

check_sty_rust_065() {
	local id="STY-RUST-065"
	local rule_id="sty-rust-065"
	local long_lived="src/lib/infrastructure"
	local matches file field type result=0
	local -a forbidden_names=("pepper" "secret" "ttl" "jwt_secret" "jwt_ttl")

	if [ ! -d "$long_lived" ]; then
		printf 'error: %s: missing %s\n' "$id" "$long_lived" >&2
		return 1
	fi

	# sty-rust-065 extracts every field of a long-lived adapter struct in the
	# inbound layer and the use-case factories. The value check happens here
	# because an ast-grep rule cannot decide whether a field's value was read
	# from the configuration. The per-use adapters (Argon2PasswordHasher,
	# JwtTokenProvider in src/lib/infrastructure/outbound) are deliberately out
	# of scope: the rule requires them to be rebuilt from the live configuration,
	# so only the long-lived surfaces are inspected.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no long-lived adapter field from %s\n' "$id" "$long_lived" >&2
		return 1
	fi

	while IFS=$'\t' read -r file field type; do
		[ -n "$field" ] || continue
		if printf '%s' "$type" | grep -Eq '(^|[^A-Za-z0-9_])(JwtTokenProvider|Argon2PasswordHasher)([^A-Za-z0-9_]|$)'; then
			printf 'error: %s: %s: long-lived adapter caches %s in field %s; read it from the live configuration\n' \
				"$id" "$file" "$type" "$field" >&2
			result=1
			continue
		fi
		for name in "${forbidden_names[@]}"; do
			if [ "$field" = "$name" ]; then
				printf 'error: %s: %s: long-lived adapter caches the configuration value %s; read it from the live configuration\n' \
					"$id" "$file" "$field" >&2
				result=1
			fi
		done
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, .metaVariables.single.FIELD.text, (.metaVariables.single.TYPE.text | gsub("\\s+"; ""))] | @tsv')

	return "$result"
}

check_sty_rust_066() {
	local id="STY-RUST-066"
	local rule_id="sty-rust-066"
	local reference="src/lib/infrastructure/outbound/config/configuration_source.rs"
	local bootstrap="src/lib/infrastructure/outbound/config/bootstrap.rs"
	local matches row file source result=0 i n
	local -a rows=() ref_sources=() boot_sources=() expected=()

	for file in "$reference" "$bootstrap"; do
		if [ ! -f "$file" ]; then
			printf 'error: %s: missing %s\n' "$id" "$file" >&2
			return 1
		fi
	done

	# sty-rust-066 extracts each `.add_source(...)` argument from the two
	# production chains; the text-and-order comparison happens here because an
	# ast-grep rule cannot relate two files.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$reference" "$bootstrap" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no configuration source\n' "$id" >&2
		return 1
	fi

	mapfile -t rows < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.SOURCE) |
			[.file,
			 (.metaVariables.single.SOURCE.range.byteOffset.start | tostring),
			 (.metaVariables.single.SOURCE.text | gsub("\\s+"; " ") | sub("^ "; "") | sub(" $"; ""))] | @tsv' |
		sort -t$'\t' -k1,1 -k2,2n)

	for row in "${rows[@]}"; do
		IFS=$'\t' read -r file _ source <<<"$row"
		[ -n "$source" ] || continue
		if [ "$file" = "$reference" ]; then
			ref_sources+=("$source")
		elif [ "$file" = "$bootstrap" ]; then
			boot_sources+=("$source")
		fi
	done

	if [ "${#ref_sources[@]}" -eq 0 ] || [ "${#boot_sources[@]}" -eq 0 ]; then
		printf 'error: %s: extracted no configuration source from %s or %s\n' \
			"$id" "$reference" "$bootstrap" >&2
		return 1
	fi

	# The datastore base layer is the one source the bootstrap cannot build:
	# it runs before the pool exists. Every other source must match, in order.
	for source in "${ref_sources[@]}"; do
		case "$source" in
		DbSqlite3Source*) ;;
		*) expected+=("$source") ;;
		esac
	done

	if [ "${#boot_sources[@]}" -ne "${#expected[@]}" ]; then
		printf 'error: %s: bootstrap adds %s file/environment source(s); ConfigConfigurationSource duplicates %s\n' \
			"$id" "${#boot_sources[@]}" "${#expected[@]}" >&2
		result=1
	fi

	n="${#expected[@]}"
	[ "${#boot_sources[@]}" -lt "$n" ] && n="${#boot_sources[@]}"
	for ((i = 0; i < n; i++)); do
		if [ "${boot_sources[$i]}" != "${expected[$i]}" ]; then
			printf 'error: %s: bootstrap source %s is %s; ConfigConfigurationSource has %s\n' \
				"$id" "$((i + 1))" "${boot_sources[$i]}" "${expected[$i]}" >&2
			result=1
		fi
	done

	return "$result"
}

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

check_sty_rust_072() {
	local id="STY-RUST-072"
	local rule_id="sty-rust-072"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

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

check_sty_rust_074() {
	local id="STY-RUST-074"
	local rule_id="sty-rust-074"
	local server="src/bin/server.rs"
	local matches installs install_count install_file var
	local install_line execute_line result=0

	if [ ! -f "$server" ]; then
		printf 'error: %s: missing %s\n' "$id" "$server" >&2
		return 1
	fi

	# sty-rust-074 extracts the `logging::setup` install call and the
	# composition root's `LoadConfiguration` binding; the count, path and
	# ordering logic happens here because an ast-grep rule cannot see how many
	# call sites exist, which file they are in, or the order of two statements.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi

	# Clause A: the subscriber is installed exactly once, by `logging::setup`,
	# from the composition root. A match carrying no VAR is an install call.
	installs="$(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.VAR | not) | [.file, (.range.start.line | tostring)] | @tsv')"
	install_count="$(printf '%s\n' "$installs" | grep -c .)"
	if [ "$install_count" -ne 1 ]; then
		printf 'error: %s: the subscriber must be installed exactly once via logging::setup (found %s)\n' \
			"$id" "$install_count" >&2
		return 1
	fi
	IFS=$'\t' read -r install_file _ <<<"$installs"
	if [ "$install_file" != "$server" ]; then
		printf 'error: %s: the subscriber must be installed from %s (found in %s)\n' \
			"$id" "$server" "$install_file" >&2
		return 1
	fi

	# Clause B: the install runs after LoadConfiguration. The binding variable
	# is read here so the check does not hard-code the local name.
	var="$(printf '%s\n' "$matches" |
		jq -r --arg file "$server" \
			'select(.metaVariables.single.VAR and (.file == $file)) | .metaVariables.single.VAR.text' |
		head -n 1)"
	if [ -z "$var" ]; then
		printf 'error: %s: %s binds no LoadConfiguration use case\n' "$id" "$server" >&2
		return 1
	fi
	execute_line="$(grep -nF "${var}.execute(" "$server" | head -n 1 | cut -d: -f1)"
	install_line="$(grep -nF "logging::setup(" "$server" | head -n 1 | cut -d: -f1)"
	if [ -z "$execute_line" ]; then
		printf 'error: %s: %s never runs %s.execute()\n' "$id" "$server" "$var" >&2
		return 1
	fi
	if [ -z "$install_line" ] || [ "$install_line" -le "$execute_line" ]; then
		printf 'error: %s: logging::setup must run after LoadConfiguration (%s.execute at line %s)\n' \
			"$id" "$var" "$execute_line" >&2
		result=1
	fi

	return "$result"
}

check_sty_rust_075() {
	local id="STY-RUST-075"
	local rule_id="sty-rust-075"
	local use_case_dir="src/lib/application/use_case"
	local config matches file text root result=0
	local -A allowed=()
	local -A seen=()
	local -a local_roots=(std core alloc crate self super)
	local use_root='^(pub[^[:space:]]*[[:space:]]+)?use[[:space:]]+(::)?([A-Za-z_][A-Za-z0-9_]*)'

	config="$(rule_config "$id")" || return 2

	# The crates a use case may reach directly (TechnicalDesign.md § 6.6) are
	# data: the list grows by adding a bullet there and an entry here.
	while IFS= read -r root; do
		[ -n "$root" ] || continue
		allowed["$root"]=1
	done < <(jq -r '.allowed_crates[]?' <<<"$config")

	if [ "${#allowed[@]}" -eq 0 ]; then
		printf 'error: %s: rule_enforcer.yaml declares no allowed_crates\n' "$id" >&2
		return 2
	fi
	for root in "${local_roots[@]}"; do
		allowed["$root"]=1
	done

	if [ ! -d "$use_case_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$use_case_dir" >&2
		return 1
	fi

	# sty-rust-075 extracts each non-test `use`; parsing the path root and
	# comparing it against the allowed set happens here because an ast-grep
	# rule cannot read rule_enforcer.yaml.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$use_case_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no use declaration from %s\n' "$id" "$use_case_dir" >&2
		return 1
	fi

	while IFS=$'\t' read -r file text; do
		[ -n "$text" ] || continue
		if [[ $text =~ $use_root ]]; then
			root="${BASH_REMATCH[3]}"
		else
			continue
		fi
		if [ -z "${allowed[$root]+set}" ] && [ -z "${seen["$file:$root"]+set}" ]; then
			printf 'error: %s: %s: use case reaches third-party crate %s directly; go through an outbound port\n' \
				"$id" "$file" "$root" >&2
			seen["$file:$root"]=1
			result=1
		fi
	done < <(printf '%s\n' "$matches" | jq -r '[.file, .text] | @tsv')

	return "$result"
}

check_sty_rust_076() {
	local id="STY-RUST-076"
	local rule_id="sty-rust-076"
	local bootstrap="src/lib/infrastructure/outbound/config/bootstrap.rs"
	local matches path feedback result=0
	local -A registered=()

	if [ ! -f "$bootstrap" ]; then
		printf 'error: %s: missing %s\n' "$id" "$bootstrap" >&2
		return 1
	fi

	# The § 10.1 table is the registry of startup-only settings; parse its rows
	# (setting, operator restart feedback) straight from the rulebook, as
	# read_rulebook parses the rule tables. An empty parse means the registry
	# moved, not that the rule holds.
	while IFS=$'\t' read -r path feedback; do
		[ -n "$path" ] || continue
		registered["$path"]=1
		if [ -z "$feedback" ]; then
			printf 'error: %s: startup-only setting %s records no operator restart feedback\n' \
				"$id" "$path" >&2
			result=1
		fi
	done < <(awk '
		function trim(s) { gsub(/^[[:space:]]+|[[:space:]]+$/, "", s); return s }

		/^##### 10\.1 Startup-only settings/ { in_registry = 1; next }
		in_registry && /^#/ { in_registry = 0 }
		in_registry && /^---/ { in_registry = 0 }
		in_registry && /^\|/ {
			n = split($0, c, "|")
			if (n < 5) next
			setting = trim(c[2]); feedback = trim(c[4])
			gsub(/`/, "", setting)
			if (setting == "Setting" || setting ~ /^-+$/) next
			if (setting !~ /^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$/) next
			printf "%s\t%s\n", setting, feedback
		}
	' "$RULEBOOK")

	if [ "${#registered[@]}" -eq 0 ]; then
		printf 'error: %s: parsed no startup-only setting from %s § 10.1\n' \
			"$id" "$RULEBOOK" >&2
		return 1
	fi

	# sty-rust-076 extracts the configuration paths the pre-pool bootstrap
	# reads. The bootstrap runs before the pool exists, so by construction
	# these settings cannot be read after the pool is built: the rule requires
	# each to be listed in the § 10.1 registry. The document comparison
	# happens here because an ast-grep rule cannot read a Markdown table.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$bootstrap" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no configuration path read from %s\n' "$id" "$bootstrap" >&2
		return 1
	fi

	while IFS= read -r path; do
		[ -n "$path" ] || continue
		if [ -z "${registered[$path]+set}" ]; then
			printf 'error: %s: startup-only setting %s is read before the pool but not listed in %s § 10.1\n' \
				"$id" "$path" "$RULEBOOK" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.PATH.text | startswith("\"")) |
			.metaVariables.single.PATH.text | gsub("^\"|\"$"; "")')

	return "$result"
}

check_sty_rust_077() {
	local id="STY-RUST-077"
	local model_dir="src/lib/domain/model"
	local output
	local -a files=()

	if [ ! -d "$model_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$model_dir" >&2
		return 1
	fi

	mapfile -t files < <(find "$model_dir" -maxdepth 1 -name '*.rs' | sort)
	if [ "${#files[@]}" -eq 0 ]; then
		printf 'error: %s: no model files under %s\n' "$id" "$model_dir" >&2
		return 1
	fi

	# STY-RUST-077 pairs a model struct's own attributes with the inherent
	# `impl` that declares `try_new`. The attribute-to-item association and
	# the struct-to-impl pairing are both beyond ast-grep, so the scan is
	# pure Bash: gawk walks each file, binds an attribute run to the item it
	# annotates, and records which types declare `try_new`.
	if output="$(awk '
		function self_type(s,   t, idx) {
			t = s
			sub(/^[ \t]*impl/, "", t)
			idx = index(t, " for ")
			if (idx > 0) t = substr(t, idx + 5)
			sub(/^[ \t]+/, "", t)
			if (match(t, /^[A-Za-z_][A-Za-z0-9_]*/)) return substr(t, 1, RLENGTH)
			return ""
		}

		FNR == 1 {
			attrs = ""
			attr_depth = 0
			in_impl = 0
			impl_depth = 0
			impl_type = ""
		}

		# An attribute opens with `#[`/`#![` and may span lines until its `]`.
		attr_depth > 0 || $0 ~ /^[ \t]*#!?\[/ {
			attrs = attrs "\n" $0
			line = $0; opens = gsub(/\[/, "", line)
			line = $0; closes = gsub(/\]/, "", line)
			attr_depth += opens - closes
			if (attr_depth < 0) attr_depth = 0
			next
		}

		# Blank lines and comments do not end an attribute run.
		$0 ~ /^[ \t]*$/ || $0 ~ /^[ \t]*\/\// { next }

		$0 ~ /^[ \t]*pub struct [A-Za-z_]/ {
			line = $0
			sub(/^[ \t]*pub struct[ \t]+/, "", line)
			match(line, /^[A-Za-z_][A-Za-z0-9_]*/)
			name = substr(line, 1, RLENGTH)
			key = FILENAME SUBSEP name
			names[key] = name
			files_of[key] = FILENAME
			if (attrs ~ /(^|[^A-Za-z0-9_])Deserialize([^A-Za-z0-9_]|$)/) deser[key] = 1
			if (attrs ~ /serde\(try_from/) routed[key] = 1
			attrs = ""
			next
		}

		# Any other code line ends the attribute run and may open an impl.
		{
			if (!in_impl && $0 ~ /^[ \t]*impl[^A-Za-z0-9_]/) {
				in_impl = 1
				impl_depth = 0
				impl_type = self_type($0)
			}
			if (in_impl && impl_type != "" && $0 ~ /fn[ \t]+try_new[ \t]*\(/) {
				validates[FILENAME SUBSEP impl_type] = 1
			}
			line = $0; opens = gsub(/\{/, "", line)
			line = $0; closes = gsub(/\}/, "", line)
			if (in_impl) {
				impl_depth += opens - closes
				if (impl_depth <= 0) {
					in_impl = 0
					impl_depth = 0
					impl_type = ""
				}
			}
			attrs = ""
			next
		}

		END {
			bad = 0
			for (key in names) {
				if (deser[key] && !routed[key] && validates[key]) {
					printf "error: STY-RUST-077: %s: %s derives Deserialize but validates in try_new; route serde through #[serde(try_from = \"…\")] plus TryFrom\n", files_of[key], names[key]
					bad = 1
				}
			}
			exit bad
		}
	' "${files[@]}" 2>&1)"; then
		return 0
	fi

	printf '%s\n' "$output"
	return 1
}

check_sty_rust_078() {
	local id="STY-RUST-078"
	local rule_id="sty-rust-078"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

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

check_sty_rust_080() {
	local id="STY-RUST-080"
	local rule_id="sty-rust-080"
	local port_dir="src/lib/domain/port"
	local model_dir="src/lib/domain/model"
	local matches filter field aggregate accessor result=0
	local -A accessors=()

	if [ ! -d "$port_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$port_dir" >&2
		return 1
	fi
	if [ ! -d "$model_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$model_dir" >&2
		return 1
	fi

	# sty-rust-080 extracts each public `*Filter` field and each aggregate
	# accessor; the field-to-aggregate pairing happens here because an ast-grep
	# rule cannot relate two identifiers.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no filter field or aggregate accessor\n' "$id" >&2
		return 1
	fi

	while IFS=$'\t' read -r aggregate accessor; do
		[ -n "$accessor" ] || continue
		accessors["${aggregate}:${accessor}"]=1
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.IMPL != null and .metaVariables.single.METHOD != null) | [.metaVariables.single.IMPL.text, .metaVariables.single.METHOD.text] | @tsv')

	while IFS=$'\t' read -r filter field; do
		[ -n "$field" ] || continue
		case "$filter" in
		*Filter) ;;
		*) continue ;;
		esac
		aggregate="${filter%Filter}"
		if [ -z "${accessors["${aggregate}:${field}"]+set}" ]; then
			printf 'error: %s: %s: field %s is not named after a field of %s (no %s::%s())\n' \
				"$id" "$filter" "$field" "$aggregate" "$aggregate" "$field" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.FILTER != null and .metaVariables.single.FIELD != null) | [.metaVariables.single.FILTER.text, .metaVariables.single.FIELD.text] | @tsv')

	return "$result"
}

check_sty_rust_081() {
	local id="STY-RUST-081"
	local rule_id="sty-rust-081"
	local port_dir="src/lib/domain/port"
	local matches file method ret element key result=0
	local -A structs=()
	local -A aggregates=()

	if [ ! -d "$port_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$port_dir" >&2
		return 1
	fi

	# sty-rust-081 extracts each repository-port method signature and each
	# struct declared in a port file; the element-type-to-aggregate/read-model
	# pairing happens here because an ast-grep rule cannot relate two
	# identifiers.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$port_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no repository-port signature from %s\n' "$id" "$port_dir" >&2
		return 1
	fi

	# Read models: structs declared in the port file itself.
	while IFS=$'\t' read -r file element; do
		[ -n "$element" ] || continue
		structs["${file}:${element}"]=1
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.STRUCT) | [.file, .metaVariables.single.STRUCT.text] | @tsv')

	# Aggregates: what the trait's non-query methods return, so a query may
	# return one of the aggregates this repository owns.
	while IFS=$'\t' read -r file method ret; do
		[ -n "$method" ] || continue
		case "$method" in
		search*) continue ;;
		esac
		element="$(printf '%s' "$ret" | sed -E 's/^(Vec|Option)<(.*)>$/\2/')"
		aggregates["${file}:${element}"]=1
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.METHOD) | [.file, .metaVariables.single.METHOD.text, .metaVariables.single.RET.text] | @tsv')

	# Every query must return the aggregate or a read-model struct declared in
	# the same port file.
	while IFS=$'\t' read -r file method ret; do
		[ -n "$method" ] || continue
		case "$method" in
		search*) ;;
		*) continue ;;
		esac
		element="$(printf '%s' "$ret" | sed -E 's/^(Vec|Option)<(.*)>$/\2/')"
		key="${file}:${element}"
		if [ -z "${structs[$key]+set}" ] && [ -z "${aggregates[$key]+set}" ]; then
			printf 'error: %s: %s: query %s returns %s; a query result must be an aggregate the repository returns or a read-model struct declared in the port file\n' \
				"$id" "$file" "$method" "$ret" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.METHOD) | [.file, .metaVariables.single.METHOD.text, .metaVariables.single.RET.text] | @tsv')

	return "$result"
}

check_sty_rust_082() {
	local id="STY-RUST-082"
	local rule_id="sty-rust-082"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local matches file context item op result=0

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	# sty-rust-082 extracts every endpoint-file path that reaches into a
	# bounded context (`handler::<context>::<next>`); the pairing below decides
	# whether `<next>` names the context's operation submodule — a fact an
	# ast-grep rule cannot see, because it cannot read the file system.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs 'src/lib/infrastructure/inbound/rest/handler/*/*.rs' \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi

	# Flat and bare imports: `handler::<context>::<next>`.
	while IFS=$'\t' read -r file context op; do
		[ -n "$op" ] || continue
		if [ -f "${handler_dir}/${context}/${op}.rs" ]; then
			printf 'error: %s: %s imports the %s::%s operation submodule; take shared items from %s.rs and hal\n' \
				"$id" "$file" "$context" "$op" "$context" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.OP) | [.file, .metaVariables.single.CTX.text, .metaVariables.single.OP.text] | @tsv')

	# Use-tree imports: `handler::<context>::{ <next>, ... }`.
	while IFS=$'\t' read -r file context item; do
		[ -n "$item" ] || continue
		op="${item%%[!A-Za-z0-9_]*}"
		if [ -f "${handler_dir}/${context}/${op}.rs" ]; then
			printf 'error: %s: %s imports the %s::%s operation submodule; take shared items from %s.rs and hal\n' \
				"$id" "$file" "$context" "$op" "$context" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.multi.ITEMS) | .file as $file | .metaVariables.single.CTX.text as $context | .metaVariables.multi.ITEMS[] | select(.text | test("^[A-Za-z_]")) | [$file, $context, .text] | @tsv')

	return "$result"
}

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

check_doc_003() {
	local id="DOC-003"
	local file line result=0

	# `.artifacts/` is machine-local, git-ignored scratch: the tooling that
	# produces it names it as a destination, while the documentation and the
	# production source never cite it. The surface is the Rust source under src/
	# and the Markdown documentation under docs/ and at the repository root. The
	# rulebook itself is excluded — it must name the directory to state the
	# prohibition.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		[ "$file" != "$RULEBOOK" ] || continue

		while IFS=: read -r line _; do
			[ -n "$line" ] || continue
			printf 'error: %s: %s:%s: cites the untracked .artifacts/ scratch directory; cite a path the repository tracks\n' \
				"$id" "$file" "$line" >&2
			result=1
		done < <(grep -nF '.artifacts/' -- "$file")
	done < <({
		git ls-files -- 'src/**/*.rs' 'docs/**/*.md'
		git ls-files -- '*.md' | grep -v /
	} | sort -u)

	return "$result"
}

# --- Runner -----------------------------------------------------------------

# validate_registry — every entry names a known rule and a defined function, and
# no ID is registered twice.
validate_registry() {
	local entry id fn
	local -A seen=()
	for entry in "${RULES[@]}"; do
		id="${entry%%:*}"
		fn="${entry#*:}"
		if [ -n "${seen[$id]+set}" ]; then
			die "duplicate rule in registry: $id"
		fi
		seen["$id"]=1
		if [ -z "${RULE_TEXT[$id]+set}" ]; then
			die "registry rule $id has no row in $RULEBOOK"
		fi
		if ! declare -F "$fn" >/dev/null; then
			die "registry function is not defined: $fn"
		fi
	done
}

# run_checks — run every registered check, print each failure as
# `FAIL <ID> (<Section>) — <Rule>` with the check's output indented below, then
# a tally. Returns non-zero when any check failed.
run_checks() {
	local entry id fn out rc failed=0 registered=0
	for entry in "${RULES[@]}"; do
		id="${entry%%:*}"
		fn="${entry#*:}"
		registered=$((registered + 1))

		if out="$("$fn" 2>&1)"; then
			rc=0
		else
			rc=$?
		fi

		if [ "$rc" -eq 0 ] && [ "$VERBOSE" -eq 1 ]; then
			printf 'ok   %s (%s)\n' "$id" "${RULE_SECTION[$id]}"
		fi

		if [ "$rc" -ne 0 ]; then
			failed=$((failed + 1))
			printf 'FAIL %s (%s) — %s\n' "$id" "${RULE_SECTION[$id]}" "${RULE_TEXT[$id]}"
			if [ -n "$out" ]; then
				printf '%s\n' "$out" | sed 's/^/    /'
			fi
		fi
	done

	printf '%s rulebook rule(s), %s registered, %s failed\n' \
		"${#RULE_TEXT[@]}" "$registered" "$failed"
	[ "$failed" -eq 0 ]
}

cmd_uncovered() {
	local entry id
	local -A registered=()
	for entry in "${RULES[@]}"; do
		registered["${entry%%:*}"]=1
	done

	while IFS= read -r id; do
		[ -n "$id" ] || continue
		if [ -z "${registered[$id]+set}" ]; then
			print_rule "$id"
		fi
	done < <(printf '%s\n' "${!RULE_TEXT[@]}" | sort)
	return 0
}

cmd_list() {
	local entry
	for entry in "${RULES[@]}"; do
		print_rule "${entry%%:*}"
	done
	return 0
}

main() {
	local cmd="check" arg
	while [ "$#" -gt 0 ]; do
		arg="$1"
		shift
		case "$arg" in
		-v | --verbose)
			VERBOSE=1
			;;
		check | uncovered | list)
			cmd="$arg"
			;;
		help | -h | --help)
			cmd="help"
			;;
		*)
			usage >&2
			return 64
			;;
		esac
	done

	case "$cmd" in
	check)
		cd -- "$REPO_ROOT" || die "cannot enter $REPO_ROOT"
		read_rulebook
		validate_config
		warn_unknown_config_rules
		validate_registry
		run_checks
		;;
	uncovered)
		cd -- "$REPO_ROOT" || die "cannot enter $REPO_ROOT"
		read_rulebook
		cmd_uncovered
		;;
	list)
		cd -- "$REPO_ROOT" || die "cannot enter $REPO_ROOT"
		read_rulebook
		cmd_list
		;;
	help)
		usage
		return 0
		;;
	esac
}

main "$@"
