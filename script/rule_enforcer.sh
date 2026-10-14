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
	"STY-SQL-001:check_sty_sql_001"
	"STY-SQL-003:check_sty_sql_003"
	"STY-SQL-004:check_sty_sql_004"
	"STY-SQL-005:check_sty_sql_005"
	"STY-SQL-006:check_sty_sql_006"
	"STY-SQL-008:check_sty_sql_008"
	"STY-SQL-009:check_sty_sql_009"
	"STY-SQL-010:check_sty_sql_010"
	"STY-SQL-011:check_sty_sql_011"
	"STY-SQL-012:check_sty_sql_012"
	"TEST-001:check_test_001"
	"TEST-002:check_test_002"
	"TEST-004:check_test_004"
	"TEST-007:check_test_007"
	"TEST-008:check_test_008"
	"TEST-009:check_test_009"
	"TEST-013:check_test_013"
	"TEST-014:check_test_014"
	"TEST-015:check_test_015"
	"TEST-017:check_test_017"
	"TEST-019:check_test_019"
	"TEST-022:check_test_022"
	"TEST-024:check_test_024"
	"TEST-025:check_test_025"
	"TEST-026:check_test_026"
	"TEST-027:check_test_027"
	"TEST-028:check_test_028"
	"TEST-033:check_test_033"
	"TEST-034:check_test_034"
	"TEST-035:check_test_035"
	"TEST-036:check_test_036"
	"TEST-038:check_test_038"
	"TEST-040:check_test_040"
	"TEST-041:check_test_041"
	"TEST-042:check_test_042"
	"TEST-043:check_test_043"
	"TEST-045:check_test_045"
	"TEST-046:check_test_046"
	"TEST-048:check_test_048"
	"TEST-049:check_test_049"
	"TEST-052:check_test_052"
	"TEST-053:check_test_053"
	"TEST-054:check_test_054"
	"OBS-001:check_obs_001"
	"OBS-003:check_obs_003"
	"OBS-004:check_obs_004"
	"OBS-006:check_obs_006"
	"OBS-007:check_obs_007"
	"OBS-008:check_obs_008"
	"API-001:check_api_001"
	"API-002:check_api_002"
	"API-004:check_api_004"
	"API-005:check_api_005"
	"API-006:check_api_006"
	"API-007:check_api_007"
	"API-008:check_api_008"
	"API-009:check_api_009"
	"API-010:check_api_010"
	"API-011:check_api_011"
	"API-012:check_api_012"
	"API-013:check_api_013"
	"API-014:check_api_014"
	"API-015:check_api_015"
	"API-016:check_api_016"
	"API-020:check_api_020"
	"API-021:check_api_021"
	"API-023:check_api_023"
	"API-024:check_api_024"
	"API-025:check_api_025"
	"API-026:check_api_026"
	"API-027:check_api_027"
	"API-030:check_api_030"
	"API-031:check_api_031"
	"API-032:check_api_032"
	"API-033:check_api_033"
	"API-034:check_api_034"
	"API-035:check_api_035"
	"API-036:check_api_036"
	"API-037:check_api_037"
	"API-038:check_api_038"
	"API-039:check_api_039"
	"API-040:check_api_040"
	"API-041:check_api_041"
	"API-043:check_api_043"
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

check_sty_sql_001() {
	local id="STY-SQL-001"
	local config=".sqlfluff"
	local migrations_dir="migrations"
	local output

	if [ ! -f "$config" ]; then
		printf 'error: %s: missing %s\n' "$id" "$config" >&2
		return 1
	fi
	if [ ! -d "$migrations_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$migrations_dir" >&2
		return 1
	fi

	# sqlfluff already performs this check: CP02 holds every unquoted
	# identifier to the `.sqlfluff` `capitalisation_policy = lower`
	# (snake_case), and RF06 flags a quoted identifier whose quotes are
	# unnecessary. The policies live in `.sqlfluff`, read here through
	# `--config`; the wrapper only selects the rules and the surface.
	if output="$(sqlfluff lint --config "$config" --rules CP02,RF06 \
		--nocolor "$migrations_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_sql_003() {
	local id="STY-SQL-003"
	local migrations_dir="migrations"
	local output

	if [ ! -d "$migrations_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$migrations_dir" >&2
		return 1
	fi

	# sqlfluff's RF04 (`references.keywords`) is the SQLite dialect's
	# implementation of the rule: an unreserved keyword used as a name (date,
	# key) is reported as RF04, while a reserved keyword (order, group, select)
	# cannot be an unquoted identifier at all and surfaces as an unparsable PRS
	# section. The dialect's keyword set is this check's definition of "SQL
	# reserved word": SQLite's tolerated words (user) pass, and a prefixed or
	# suffixed name (purchase_order, created_at) is not a keyword. Quoted
	# identifiers are STY-SQL-001's surface (`references.quoting`, RF06), not
	# this rule's.
	if output="$(sqlfluff lint --dialect sqlite --rules RF04 --nocolor \
		"$migrations_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_sty_sql_004() {
	local id="STY-SQL-004"
	local migrations_dir="migrations"
	local parse file ident text result=0

	if [ ! -d "$migrations_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$migrations_dir" >&2
		return 1
	fi

	# sqlfluff parses the migrations with the SQLite dialect and classifies every
	# identifier as a `naked_identifier` or `quoted_identifier` segment; the
	# character-set test happens here because no ast-grep rule sees SQL and no
	# linter rule enforces the set (sqlfluff RF05 allows `A-Z` and non-ASCII
	# alphanumerics). The parse is read from stdout even when sqlfluff reports an
	# unparsable section, so that section is diagnosed below instead of aborting.
	parse="$(sqlfluff parse --dialect sqlite --format json "$migrations_dir" 2>/dev/null)"
	if ! printf '%s' "$parse" | jq -e . >/dev/null 2>&1; then
		printf 'error: %s: sqlfluff produced no parse of %s\n' "$id" "$migrations_dir" >&2
		return 2
	fi

	# Every identifier segment, keyed by the file that holds it. A quoted
	# identifier arrives with its quotes, so the outer pair is stripped before
	# the character-set test.
	while IFS=$'\t' read -r file ident; do
		[ -n "$ident" ] || continue
		if [[ ! $ident =~ ^[a-z0-9_]+$ ]]; then
			printf 'error: %s: %s: identifier %s uses characters outside [a-z0-9_]\n' \
				"$id" "$file" "$ident" >&2
			result=1
		fi
	done < <(printf '%s\n' "$parse" | jq -r '
		.[] | .filepath as $f |
		(.segments | .. | objects | to_entries[] |
		 select(.key == "naked_identifier" or .key == "quoted_identifier") |
		 [$f, (.value | tostring
			| gsub("^\"|\"$"; "") | gsub("^`|`$"; "") | gsub("^\\[|\\]$"; ""))] | @tsv)')

	# An unparsable section hides its identifiers: the dialect tokenises
	# `user$data` as `user` plus `$data`. Report it rather than pass silently.
	while IFS=$'\t' read -r file text; do
		[ -n "$text" ] || continue
		printf 'error: %s: %s: unparsable SQL section: %s\n' "$id" "$file" "$text" >&2
		result=1
	done < <(printf '%s\n' "$parse" | jq -r '
		.[] | .filepath as $f |
		(.segments | .. | objects | to_entries[] |
		 select(.key == "unparsable") |
		 [$f, ((.value | tostring) | gsub("\\s+"; " ") | .[0:80])] | @tsv)')

	return "$result"
}

check_sty_sql_005() {
	local id="STY-SQL-005"
	local migrations_dir="migrations"
	local file statement list value found=0 result=0

	if [ ! -d "$migrations_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$migrations_dir" >&2
		return 1
	fi

	# STY-SQL-005: an enum constraint is a SQL `CHECK (... IN (...))` whose
	# allowed values are string literals; each such literal must be uppercase.
	# ast-grep has no SQL grammar, so the extraction is textual: join each
	# migration to one line, split it into statements, and read the quoted
	# values of every `IN (...)` list in a `CHECK` statement. A list with no
	# string literal (e.g. the numeric `IN (0, 1)` boolean checks) is not an
	# enum constraint and is skipped.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		while IFS= read -r statement; do
			[ -n "$statement" ] || continue
			case "$statement" in
			*CHECK*) ;;
			*) continue ;;
			esac
			while IFS= read -r list; do
				[ -n "$list" ] || continue
				while IFS= read -r value; do
					[ -n "$value" ] || continue
					found=$((found + 1))
					if [ "$value" != "$(printf '%s' "$value" | tr '[:lower:]' '[:upper:]')" ]; then
						printf 'error: %s: %s: enum constraint string %s is not uppercase\n' \
							"$id" "$file" "$value" >&2
						result=1
					fi
				done < <(printf '%s\n' "$list" | grep -oE "'[^']*'" | tr -d "'")
			done < <(printf '%s\n' "$statement" | grep -oE "IN \([^)]*\)")
		done < <(awk '{ printf "%s ", $0 } END { printf "\n" }' "$file" | tr ';' '\n')
	done < <(find "$migrations_dir" -maxdepth 1 -name '*.sql' | sort)

	# An empty parse means the extraction moved, not that the rule holds.
	if [ "$found" -eq 0 ]; then
		printf 'error: %s: extracted no enum constraint string from %s\n' \
			"$id" "$migrations_dir" >&2
		return 1
	fi

	return "$result"
}

check_sty_sql_006() {
	local id="STY-SQL-006"
	local migrations_dir="migrations"
	local config table result=0 count=0
	local -A allowed=()

	if [ ! -d "$migrations_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$migrations_dir" >&2
		return 1
	fi

	config="$(rule_config "$id")" || return 2

	# Singular nouns that legitimately end in `s` (status, address, analysis,
	# series, species, news ...) are named here; an empty list approves none, so
	# every such name is reported until it is reviewed.
	while IFS= read -r table; do
		[ -n "$table" ] || continue
		allowed["$table"]=1
	done < <(jq -r '.singular_exceptions[]?' <<<"$config")

	# A table name is declared by `CREATE TABLE`; a plural final token is the
	# violation. References must equal the declared name, so checking the
	# declaration constrains every use. The final-token test is the plural
	# marker of English nouns, a suffix heuristic rather than a parser.
	while IFS= read -r table; do
		[ -n "$table" ] || continue
		count=$((count + 1))
		if [ -n "${allowed[$table]+set}" ]; then
			continue
		fi
		if [[ $table == *s ]]; then
			printf 'error: %s: table name %s is plural; use a singular name\n' \
				"$id" "$table" >&2
			result=1
		fi
	done < <(grep -rhoiE 'CREATE TABLE( IF NOT EXISTS)?[[:space:]]+[A-Za-z_][A-Za-z0-9_]*' \
		"$migrations_dir"/*.sql 2>/dev/null | awk '{ print tolower($NF) }' | sort -u)

	if [ "$count" -eq 0 ]; then
		printf 'error: %s: extracted no CREATE TABLE name from %s\n' "$id" "$migrations_dir" >&2
		return 1
	fi

	return "$result"
}

check_sty_sql_008() {
	local id="STY-SQL-008"
	local migrations_dir="migrations"
	local file table cols col expected found=0 result=0
	local -a keys=()

	if [ ! -d "$migrations_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$migrations_dir" >&2
		return 1
	fi

	# STY-SQL-008 constrains a table's own single-column primary key: the
	# column must be named `<table_name>_id`. The table name and its primary
	# key column are read out of each `CREATE TABLE` in migrations/*.up.sql;
	# the identifier-to-identifier comparison happens here because a syntactic
	# pattern cannot relate two names. A composite primary key (a junction
	# table, STY-SQL-007) has no single `<table_name>_id` column and is out of
	# this rule's scope.
	while IFS=$'\t' read -r file table cols; do
		[ -n "$table" ] || continue
		found=1
		IFS=',' read -r -a keys <<<"$cols"
		[ "${#keys[@]}" -eq 1 ] || continue
		col="${keys[0]}"
		expected="${table}_id"
		if [ "$col" != "$expected" ]; then
			printf 'error: %s: %s: table %s primary key column %s must be %s\n' \
				"$id" "$file" "$table" "$col" "$expected" >&2
			result=1
		fi
	done < <(for file in "$migrations_dir"/*.up.sql; do
		[ -f "$file" ] || continue
		awk -v file="$file" '
			{
				line = $0
				if (match(line, /CREATE[ \t]+TABLE[ \t]+(IF[ \t]+NOT[ \t]+EXISTS[ \t]+)?[A-Za-z_][A-Za-z0-9_]*/)) {
					name = substr(line, RSTART, RLENGTH)
					sub(/CREATE[ \t]+TABLE[ \t]+/, "", name)
					sub(/IF[ \t]+NOT[ \t]+EXISTS[ \t]+/, "", name)
					table = name
				}
				if (pk == 0 && match(line, /PRIMARY[ \t]+KEY[ \t]*\(/)) {
					rest = substr(line, RSTART + RLENGTH)
					pk = 1; depth = 1; acc = ""
				} else if (pk == 1) {
					rest = line
				} else next
				n = length(rest)
				for (i = 1; i <= n; i++) {
					c = substr(rest, i, 1)
					if (c == "(") { depth++; acc = acc c }
					else if (c == ")") { depth--; if (depth == 0) break; acc = acc c }
					else acc = acc c
				}
				if (depth == 0) {
					pk = 0
					gsub(/[ \t\r\n]+/, "", acc)
					printf "%s\t%s\t%s\n", file, table, acc
				}
			}
		' "$file"
	done)

	if [ "$found" -eq 0 ]; then
		printf 'error: %s: extracted no primary key under %s\n' "$id" "$migrations_dir" >&2
		return 1
	fi

	return "$result"
}

check_sty_sql_009() {
	local id="STY-SQL-009"
	local migrations_dir="migrations"
	local file stmt table pk locals ref_table expected locals_sorted expected_sorted
	local result=0 fk_count=0
	local -A pk_of=()

	if [ ! -d "$migrations_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$migrations_dir" >&2
		return 1
	fi

	# Each table's primary key, read from the table-level `PRIMARY KEY (...)`
	# of its `CREATE TABLE`. A migration is flattened and split on `;` first
	# because both the `CREATE TABLE` and the foreign-key clauses span lines.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		while IFS= read -r stmt; do
			[ -n "$stmt" ] || continue
			if printf '%s' "$stmt" | grep -q 'CREATE TABLE'; then
				table="$(printf '%s' "$stmt" |
					sed -nE 's/.*CREATE TABLE[[:space:]]+([A-Za-z_][A-Za-z0-9_]*).*/\1/p')"
				pk="$(printf '%s' "$stmt" |
					grep -oE 'PRIMARY KEY[[:space:]]*\([^)]*\)' | head -n 1 |
					sed -E 's/.*\(([^)]*)\).*/\1/' | tr -d '[:space:]')"
				[ -n "$table" ] && [ -n "$pk" ] && pk_of["$table"]="$pk"
			fi
		done < <(tr '\n;' ' \n' <"$file")
	done < <(find "$migrations_dir" -maxdepth 1 -name '*.up.sql' | sort)

	if [ "${#pk_of[@]}" -eq 0 ]; then
		printf 'error: %s: no table primary key found in %s/*.up.sql\n' \
			"$id" "$migrations_dir" >&2
		return 1
	fi

	# A foreign key column must carry the exact primary key name of the table
	# it references. The foreign key clause names the referenced table but not
	# that table's primary key, so the two are paired here: ast-grep has no
	# SQL grammar and cannot relate a clause to another table's declaration.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		while IFS= read -r stmt; do
			[ -n "$stmt" ] || continue
			while IFS= read -r clause; do
				[ -n "$clause" ] || continue
				fk_count=$((fk_count + 1))
				locals="$(printf '%s' "$clause" |
					sed -E 's/^.*FOREIGN KEY[[:space:]]*\(([^)]*)\).*$/\1/' | tr -d '[:space:]')"
				ref_table="$(printf '%s' "$clause" |
					sed -E 's/^.*REFERENCES[[:space:]]+([A-Za-z_][A-Za-z0-9_]*).*$/\1/')"
				expected="${pk_of[$ref_table]:-}"
				if [ -z "$expected" ]; then
					printf 'error: %s: %s: foreign key references %s, whose primary key is not declared in %s\n' \
						"$id" "$file" "$ref_table" "$migrations_dir" >&2
					result=1
					continue
				fi
				# A composite foreign key names the primary key's columns; the
				# order of the two lists is correspondence, not naming, so the
				# names are compared as a set.
				locals_sorted="$(printf '%s' "$locals" | tr ',' '\n' | sort | paste -sd, -)"
				expected_sorted="$(printf '%s' "$expected" | tr ',' '\n' | sort | paste -sd, -)"
				if [ "$locals_sorted" != "$expected_sorted" ]; then
					printf 'error: %s: %s: foreign key column %s must use the primary key name %s of %s\n' \
						"$id" "$file" "$locals" "$expected" "$ref_table" >&2
					result=1
				fi
			done < <(printf '%s' "$stmt" |
				grep -oE 'FOREIGN KEY[[:space:]]*\([^)]*\)[[:space:]]*REFERENCES[[:space:]]+[A-Za-z_][A-Za-z0-9_]*')
		done < <(tr '\n;' ' \n' <"$file")
	done < <(find "$migrations_dir" -maxdepth 1 -name '*.up.sql' | sort)

	if [ "$fk_count" -eq 0 ]; then
		printf 'error: %s: no FOREIGN KEY clause found in %s/*.up.sql\n' \
			"$id" "$migrations_dir" >&2
		return 1
	fi

	return "$result"
}

check_sty_sql_010() {
	local id="STY-SQL-010"
	local migrations_dir="migrations"
	local matches file line name type result=0

	if [ ! -d "$migrations_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$migrations_dir" >&2
		return 1
	fi

	# The declared column type is the only mechanical signal for the rule's
	# data-type categories: a BOOLEAN column must carry an is_/has_/can_ prefix
	# and a DATE column an _at/_date suffix. SQLite stores timestamps, counts
	# and totals as TEXT/INTEGER, so those clauses have no DDL signal and are
	# not checked here. Both CREATE TABLE columns and ALTER TABLE ADD COLUMN
	# definitions are covered.
	matches="$(grep -rEnE \
		'^[[:space:]]+[a-z][a-z0-9_]*[[:space:]]+(BOOLEAN|DATE)\b|^[[:space:]]*ADD[[:space:]]+COLUMN[[:space:]]+[a-z][a-z0-9_]*[[:space:]]+(BOOLEAN|DATE)\b' \
		--include='*.up.sql' "$migrations_dir" |
		sed -E 's/^([^:]+):([0-9]+):[[:space:]]*(ADD COLUMN[[:space:]]+)?([a-z][a-z0-9_]*)[[:space:]]+(BOOLEAN|DATE)\b.*/\1\t\2\t\4\t\5/')"

	while IFS=$'\t' read -r file line name type; do
		[ -n "$name" ] || continue
		case "$type" in
		BOOLEAN)
			if [[ ! $name =~ ^(is|has|can)_ ]]; then
				printf 'error: %s: %s:%s: boolean column %s must start with is_/has_/can_\n' \
					"$id" "$file" "$line" "$name" >&2
				result=1
			fi
			;;
		DATE)
			if [[ ! $name =~ _(at|date)$ ]]; then
				printf 'error: %s: %s:%s: date column %s must end with _at or _date\n' \
					"$id" "$file" "$line" "$name" >&2
				result=1
			fi
			;;
		esac
	done <<<"$matches"

	return "$result"
}

check_sty_sql_011() {
	local id="STY-SQL-011"
	local migrations_dir="migrations"
	local sqlx_dir="src/lib/infrastructure/outbound/sqlx"
	local trigger_re='(CREATE|DROP)[[:space:]]+(TEMP[[:space:]]+|TEMPORARY[[:space:]]+)?TRIGGER[[:space:]]+(IF[[:space:]]+(NOT[[:space:]]+)?EXISTS[[:space:]]+)?["`[]?[A-Za-z_][A-Za-z0-9_]*'
	local file line text name count=0 result=0

	if [ ! -d "$migrations_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$migrations_dir" >&2
		return 1
	fi
	if [ ! -d "$sqlx_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$sqlx_dir" >&2
		return 1
	fi

	# Triggers are declared in the migration SQL and in the SQL string
	# literals of the sqlx outbound adapter tree. ast-grep cannot parse SQL,
	# so each declaration is read textually: line comments are dropped so
	# prose about a trigger is not read as one, then the name after the
	# TRIGGER keyword is compared to the required `tg_` prefix.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		while IFS=: read -r line text; do
			[ -n "$text" ] || continue
			name="${text##* }"
			name="${name#[\"\`\[]}"
			count=$((count + 1))
			case "$name" in
			tg_*) ;;
			*)
				printf 'error: %s: %s:%s: trigger %s must be named with the tg_ prefix\n' \
					"$id" "$file" "$line" "$name" >&2
				result=1
				;;
			esac
		done < <(sed -E 's@(--|//).*$@@' "$file" | grep -noiE "$trigger_re")
	done < <(find "$migrations_dir" "$sqlx_dir" -type f \( -name '*.sql' -o -name '*.rs' \) | sort)

	if [ "$count" -eq 0 ]; then
		printf 'error: %s: no trigger statement found in %s or %s\n' \
			"$id" "$migrations_dir" "$sqlx_dir" >&2
		return 1
	fi

	return "$result"
}

check_sty_sql_012() {
	local id="STY-SQL-012"
	local migrations_dir="migrations"
	local decl file name result=0
	local -a files=()

	if [ ! -d "$migrations_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$migrations_dir" >&2
		return 1
	fi

	mapfile -t files < <(find "$migrations_dir" -maxdepth 1 -name '*.sql' | sort)
	if [ "${#files[@]}" -eq 0 ]; then
		printf 'error: %s: no migration under %s\n' "$id" "$migrations_dir" >&2
		return 1
	fi

	# A function is declared by `CREATE [OR REPLACE] [TEMP|TEMPORARY] FUNCTION
	# [IF NOT EXISTS] <name>` and dropped by `DROP FUNCTION [IF EXISTS]
	# <name>`; the name that follows the `FUNCTION` keyword must carry the
	# `fn_` prefix. The declaration is matched here because ast-grep has no SQL
	# grammar (`language: Sql` is rejected), and whitespace is collapsed first
	# so a declaration split across lines is still one match.
	decl='(CREATE[[:space:]]+(OR[[:space:]]+REPLACE[[:space:]]+)?(TEMP(ORARY)?[[:space:]]+)?|DROP[[:space:]]+)FUNCTION[[:space:]]+(IF[[:space:]]+(NOT[[:space:]]+)?EXISTS[[:space:]]+)?"?[A-Za-z_][A-Za-z0-9_]*"?'

	for file in "${files[@]}"; do
		while IFS= read -r name; do
			[ -n "$name" ] || continue
			name="${name%\"}"
			name="${name#\"}"
			if [[ $name != fn_* ]]; then
				printf 'error: %s: %s: function %s must carry the fn_ prefix\n' \
					"$id" "$file" "$name" >&2
				result=1
			fi
		done < <(tr -s '[:space:]' ' ' <"$file" |
			grep -ioE "$decl" |
			sed -E 's/.*[[:space:]]//')
	done

	return "$result"
}

check_test_001() {
	local id="TEST-001"
	local rule_id="test-001"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_test_002() {
	local id="TEST-002"
	local rule_id="test-002"
	local port_dir="src/lib/domain/port"
	local app_port_dir="src/lib/application/port"
	local config matches file trait result=0
	local -A exempt=()

	if [ ! -d "$port_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$port_dir" >&2
		return 1
	fi
	if [ ! -d "$app_port_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$app_port_dir" >&2
		return 1
	fi

	config="$(rule_config "$id")" || return 2

	# The unit-of-work ports cannot be `mockall`-generated: their accessors
	# return borrowed opaque views, `commit`/`rollback` consume `self`, and the
	# factory carries an associated type. The hand-written
	# `TestUnitOfWork`/`TestUnitOfWorkFactory` doubles in `src/lib/lib.rs` stand
	# in for them (the reason is recorded there). The family is data: a port
	# that gains or loses that shape moves in or out of this list without
	# editing the script.
	while IFS= read -r trait; do
		[ -n "$trait" ] || continue
		exempt["$trait"]=1
	done < <(jq -r '.exempt_traits[]?' <<<"$config")

	# test-002 extracts every external-dependency trait (port) declared without
	# `mockall`'s automock attribute; the exemption comparison happens here
	# because an ast-grep rule cannot read rule_enforcer.yaml.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$port_dir" "$app_port_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	[ -n "$matches" ] || return 0

	while IFS=$'\t' read -r file trait; do
		[ -n "$trait" ] || continue
		if [ -n "${exempt[$trait]+set}" ]; then
			continue
		fi
		printf 'error: %s: %s: external dependency %s is declared without a mockall automock attribute; add #[cfg_attr(test, mockall::automock)]\n' \
			"$id" "$file" "$trait" >&2
		result=1
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, .metaVariables.single.TRAIT.text] | @tsv')

	return "$result"
}

check_test_004() {
	local id="TEST-004"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local file dto test_line line text found=0 result=0
	local -a dtos=()

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	# Every payload DTO declaration under the handler tree. A file that declares
	# none cannot hold an isolated DTO test.
	while IFS= read -r file; do
		[ -n "$file" ] || continue

		# The payload DTOs the file declares: top-level structs named with the
		# rule's `<Context>Request` / `<Context>Query` / `<Context>Response`
		# suffix. The declaration is the file-scoped anchor, so a same-named
		# type imported from another layer (e.g. the application port's
		# `GetUserResponse`) is not mistaken for a payload DTO.
		mapfile -t dtos < <(sed -nE \
			's/^pub struct ([A-Za-z0-9_]+(Request|Query|Response))[[:space:]]*[({].*/\1/p' "$file")
		[ "${#dtos[@]}" -gt 0 ] || continue
		found=$((found + 1))

		# The unit-test module lives in the file under test (TEST-001), so the
		# DTO's own file is where an isolated test of it would appear.
		test_line="$(grep -nE '^#\[cfg\(test\)\]' "$file" | head -n 1 | cut -d: -f1)"
		[ -n "$test_line" ] || continue

		# A test that names the payload DTO exercises it directly. The handler
		# tests drive the router (TEST-009) and assert the JSON body, so they
		# never name the DTO; a name here is the isolated test the rule forbids.
		for dto in "${dtos[@]}"; do
			while IFS=: read -r line text; do
				[ -n "$text" ] || continue
				printf 'error: %s: %s:%s: the test module names the payload DTO %s; test the handler that consumes it (found: %s)\n' \
					"$id" "$file" "$((test_line + line - 1))" "$dto" \
					"$(printf '%s' "$text" | sed 's/^[[:space:]]*//')" >&2
				result=1
			done < <(tail -n +"$test_line" "$file" |
				sed -E 's@//.*$@@' |
				grep -nE "(^|[^A-Za-z0-9_])${dto}([^A-Za-z0-9_]|$)")
		done
	done < <(find "$handler_dir" -name '*.rs' | sort)

	if [ "$found" -eq 0 ]; then
		printf 'error: %s: extracted no payload DTO from %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	return "$result"
}

check_test_007() {
	local id="TEST-007"
	local rule_id="test-007"
	local output

	# test-007 extracts every function that both uses SECRET_PASSWORD and
	# asserts a password outcome — a test whose subject is password behavior
	# (policy validation, wrong-password rejection, change-password, hashing,
	# verification). A test that merely stubs a password error, or binds one in
	# a case attribute, is not extracted.
	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_test_008() {
	local id="TEST-008"
	local rule_id="test-008"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_test_009() {
	local id="TEST-009"
	local rule_id="test-009"
	local surface="src/lib/infrastructure/inbound/rest/handler"
	local module="src/lib/infrastructure/inbound/rest/handler.rs"
	local matches file kind result=0
	local -A has_oneshot=()
	local -A has_builder=()

	for file in "$surface" "$module"; do
		if [ ! -e "$file" ]; then
			printf 'error: %s: missing %s\n' "$id" "$file" >&2
			return 1
		fi
	done

	# test-009 extracts the two syntactic facts the rule names — a request built
	# with `Request::builder()` and a dispatch through `.oneshot(...)`. The
	# per-file pairing happens here because an ast-grep rule cannot relate two
	# matches.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi

	while IFS=$'\t' read -r file kind; do
		[ -n "$file" ] || continue
		case "$kind" in
		oneshot) has_oneshot["$file"]=1 ;;
		builder) has_builder["$file"]=1 ;;
		esac
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, (if .metaVariables.single.RECV then "oneshot" else "builder" end)] | @tsv')

	# Every handler file that declares a test module must drive the router: a
	# request built with `Request::builder()`, sent through `.oneshot(...)` from
	# `tower::ServiceExt`.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		if [ -z "${has_oneshot[$file]+set}" ]; then
			printf 'error: %s: %s: handler test sends no request through .oneshot(...) from tower::ServiceExt\n' \
				"$id" "$file" >&2
			result=1
		fi
		if [ -z "${has_builder[$file]+set}" ]; then
			printf 'error: %s: %s: handler test builds no request with Request::builder()\n' \
				"$id" "$file" >&2
			result=1
		fi
	done < <(grep -rl --include='*.rs' '#\[cfg(test)\]' "$surface" "$module" 2>/dev/null | sort)

	return "$result"
}

check_test_013() {
	local id="TEST-013"
	local rule_id="test-013"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local matches file accessor expected result=0

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	# test-013 extracts the application use-case accessor each endpoint handler
	# drives (`state.<context>_use_case_factory().<accessor>().execute(...)`).
	# Pairing that accessor to the `Mock<PascalCase(accessor)>UseCase` its test
	# module must build happens here because an ast-grep rule cannot relate two
	# identifiers.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs 'src/lib/infrastructure/inbound/rest/handler/*/*.rs' \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no use-case-driven endpoint handler from %s\n' \
			"$id" "$handler_dir" >&2
		return 1
	fi

	while IFS=$'\t' read -r file accessor; do
		[ -n "$accessor" ] || continue
		expected="Mock$(printf '%s' "$accessor" |
			awk -F_ '{ for (i = 1; i <= NF; i++) printf "%s%s", toupper(substr($i, 1, 1)), substr($i, 2) }')UseCase"
		if ! grep -qF "$expected" "$file"; then
			printf 'error: %s: %s: handler test must mock the %s use case (expected %s)\n' \
				"$id" "$file" "$accessor" "$expected" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, .metaVariables.single.ACCESSOR.text] | @tsv')

	return "$result"
}

check_test_014() {
	local id="TEST-014"
	local rule_id="test-014"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local matches file line call rel context expected result=0

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	# test-014 extracts every router-state construction inside a `#[cfg(test)]`
	# module of the handler tree: the per-context helper, the all-mock helper
	# and a hand-built `AppState`. Comparing the construction against the
	# handler's own bounded context happens here because an ast-grep rule
	# cannot see the file path it matched in.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$handler_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no router-state construction from %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	while IFS=$'\t' read -r file line call; do
		[ -n "$call" ] || continue
		rel="${file#"$handler_dir"/}"
		context="${rel%%/*}"
		context="${context%.rs}"
		expected="app_state_with_${context}"
		if [ "$call" != "$expected" ]; then
			printf 'error: %s: %s:%s: the router state must be built with crate::test_helpers::%s (found %s)\n' \
				"$id" "$file" "$line" "$expected" "$call" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, ((.range.start.line + 1) | tostring),
			(.text | split("(")[0])] | @tsv')

	return "$result"
}

check_test_015() {
	local id="TEST-015"
	local rule_id="test-015"
	local output

	# test-015 flags a per-context use-case factory expectation whose
	# `times(...)` argument is not `0..=1`: the handler can reject the request
	# (invalid path id, malformed body) before the factory is reached, so the
	# expectation must tolerate zero calls.
	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_test_017 — an application test is `async` (TEST-017). Every test
# function under src/lib/application/ carries `async`; the `#[tokio::test]`
# attribute that starts it requires it. The directory is the scan target
# because the rule is generic — ast-grep matches the function, not the path.
check_test_017() {
	local id="TEST-017"
	local rule_id="test-017"
	local target="src/lib/application"
	local output

	if [ ! -d "$target" ]; then
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

check_test_019() {
	local id="TEST-019"
	local rule_id="test-019"
	local model_dir="src/lib/domain/model"
	local output

	if [ ! -d "$model_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$model_dir" >&2
		return 1
	fi

	# test-019 matches a test construct — a `#[cfg(test)]`/`#[cfg_attr(test, …)]`
	# attribute, a `#[test]` (or `#[<path>::test]`), a `mod test`/`mod tests`,
	# or a `mockall`/`automock` name. Domain models have no dedicated test of
	# their own (TEST-019), so the model directory is the only scope the rule is
	# run over.
	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$model_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_test_053 — a use case whose catalog entry names more than one bounded
# context keeps a single E2E file, placed in a folder named by that entry
# (TEST-053). The catalog (docs/development/UseCases.md) is the rule's source:
# each `## UC-<seq> - <Name>` entry's `### Bounded context(s)` bullet list is
# read at run time, as check_test_041 reads the entry headings. TEST-041 owns
# "one file per use case"; this check owns the multi-context case — the file
# must sit in one of the contexts the entry names, so a use case that spans
# contexts is neither duplicated across them nor filed under a context it does
# not touch.
check_test_053() {
	local id="TEST-053"
	local e2e_dir="test/e2e"
	local catalog="docs/development/UseCases.md"
	local seq num ctx file base folder list
	local result=0
	local -A contexts=()
	local -A multi=()
	local -A named=()
	local -A files=()

	if [ ! -d "$e2e_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$e2e_dir" >&2
		return 1
	fi
	if [ ! -f "$catalog" ]; then
		printf 'error: %s: missing %s\n' "$id" "$catalog" >&2
		return 1
	fi

	# The bounded contexts each catalogued use case names: one `seq<TAB>context`
	# row per bullet under the entry's `### Bounded context(s)` heading. The
	# "Field rules" prose at the top of the catalog also carries that heading;
	# it sits before the first `## UC-` entry, so a row without an entry is
	# dropped.
	while IFS=$'\t' read -r seq ctx; do
		[ -n "$seq" ] || continue
		num=$((10#$seq))
		ctx="$(printf '%s' "$ctx" | tr '[:upper:]' '[:lower:]')"
		contexts["$num"]+="$ctx"$'\n'
	done < <(awk '
		function trim(s) { gsub(/^[[:space:]]+|[[:space:]]+$/, "", s); return s }

		/^## UC-[0-9]+ - / { seq = $0; sub(/^## UC-/, "", seq); sub(/ - .*$/, "", seq); in_bc = 0; next }
		/^## / { in_bc = 0; next }
		/^### Bounded context\(s\)/ { in_bc = 1; next }
		/^### / { in_bc = 0; next }
		in_bc && /^- / {
			ctx = trim(substr($0, 3))
			if (seq != "" && ctx != "") printf "%s\t%s\n", seq, ctx
		}
	' "$catalog")

	if [ "${#contexts[@]}" -eq 0 ]; then
		printf 'error: %s: parsed no bounded context from %s\n' "$id" "$catalog" >&2
		return 1
	fi

	# Keep the use cases that name more than one context; those are the ones
	# TEST-053 governs.
	for num in "${!contexts[@]}"; do
		mapfile -t list < <(printf '%s' "${contexts[$num]}" | sed '/^$/d' | sort -u)
		if [ "${#list[@]}" -le 1 ]; then
			continue
		fi
		multi["$num"]=1
		for ctx in "${list[@]}"; do
			named["$num:$ctx"]=1
		done
	done

	if [ "${#multi[@]}" -eq 0 ]; then
		printf 'error: %s: %s names no use case with more than one bounded context\n' \
			"$id" "$catalog" >&2
		return 1
	fi

	# Every E2E test module, grouped by the catalogued use case it names.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		base="${file##*/}"
		[[ $base =~ ^uc([0-9]+)_ ]] || continue
		num=$((10#${BASH_REMATCH[1]}))
		[ -n "${multi[$num]+set}" ] || continue
		files["$num"]+="$file"$'\n'
	done < <(find "$e2e_dir" -type f -name 'uc*.py' \
		! -path '*/__pycache__/*' | sort)

	# A multi-context use case keeps one file, in a context its entry names.
	for num in "${!multi[@]}"; do
		mapfile -t list < <(printf '%s' "${files[$num]:-}" | sed '/^$/d')
		if [ "${#list[@]}" -eq 0 ]; then
			# "every use case has its file" is TEST-041's clause.
			continue
		fi

		if [ "${#list[@]}" -gt 1 ]; then
			printf 'error: %s: UC-%03d names more than one bounded context but keeps %s E2E files (%s); keep a single file and record the other context in the catalog\n' \
				"$id" "$num" "${#list[@]}" "$(printf '%s' "${list[*]}" | tr ' ' ',')" >&2
			result=1
			continue
		fi

		file="${list[0]}"
		folder="$(basename "$(dirname "$file")")"
		if [ -z "${named["$num:$folder"]+set}" ]; then
			printf 'error: %s: %s sits in test/e2e/%s/, a context UC-%03d does not name; place it in the context of the primary endpoint\n' \
				"$id" "$file" "$folder" "$num" >&2
			result=1
		fi
	done

	return "$result"
}

# check_test_054 — an outbound adapter that drives a native binary is covered by
# an integration test in the same file (TEST-054). The surface is discovered: an
# outbound adapter "drives a native binary" when it spawns a child with
# `Command::new`. For each such file the check asserts a `#[cfg(test)]`
# integration test module in the same file, a test-only program seam (a
# constructor that binds an explicit program) the tests drive, an executable
# stub the tests write, and an assertion for every child outcome.
#
# "Every child outcome" is a syntactic proxy. The adapter imports its domain
# port error type (`use crate::domain::port::error::<Type>;`), so the variants
# the production code returns can be enumerated; each must be asserted by the
# test module, and the success path must be unwrapped and asserted. Whether a
# given assertion actually reaches the variant, and whether the test drives the
# stub through the seam rather than `PATH`, is beyond the syntax (coverage
# limit). The final clause — the real binary is exercised by the container E2E
# suite — is not asserted here: it needs the built image, out of a static
# check's reach (coverage limit).
check_test_054() {
	local id="TEST-054"
	local outbound="src/lib/infrastructure/outbound"
	local file test_line test_text seam error_type variant
	local found=0 result=0
	local -A production_variants=()

	if [ ! -d "$outbound" ]; then
		printf 'error: %s: missing %s\n' "$id" "$outbound" >&2
		return 1
	fi

	# A native-binary adapter spawns a child process.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		found=$((found + 1))

		# Clause 1 — an integration test module in the same file.
		test_line="$(grep -nE '^[[:space:]]*(pub[[:space:]]+)?mod tests?\b' "$file" |
			head -n 1 | cut -d: -f1)"
		if [ -z "$test_line" ] ||
			! sed -n "1,${test_line}p" "$file" | grep -qE '#\[cfg\(test\)\]'; then
			printf 'error: %s: %s: a native-binary adapter must carry a #[cfg(test)] integration test module in the same file\n' \
				"$id" "$file" >&2
			result=1
			continue
		fi
		test_text="$(tail -n +"$test_line" "$file")"

		# Clause 2 — the test-only program seam: a constructor that binds an
		# explicit `program`, driven by the tests instead of the real binary.
		seam="$(grep -oE 'fn [A-Za-z_][A-Za-z0-9_]*\([^)]*\bprogram\b' "$file" |
			head -n 1 | sed -E 's/^fn ([A-Za-z_][A-Za-z0-9_]*).*/\1/')"
		if [ -z "$seam" ]; then
			printf 'error: %s: %s: the adapter declares no test-only program seam (a constructor taking the program)\n' \
				"$id" "$file" >&2
			result=1
		elif ! printf '%s\n' "$test_text" | grep -qE "\b${seam}\("; then
			printf 'error: %s: %s: the test module never drives the program seam %s\n' \
				"$id" "$file" "$seam" >&2
			result=1
		fi

		# Clause 3 — a stub executable: the tests write an executable script.
		if ! printf '%s\n' "$test_text" | grep -qE 'set_mode\(0o' ||
			! printf '%s\n' "$test_text" | grep -qF '#!/bin/sh'; then
			printf 'error: %s: %s: the test module does not write an executable stub (needs a #!/bin/sh script and set_mode)\n' \
				"$id" "$file" >&2
			result=1
		fi

		# Clause 4 — every child outcome is asserted. Enumerate the variants of
		# the adapter's domain port error type that the production code returns,
		# and require the test module to assert each, plus the success path.
		error_type="$(grep -oE 'use crate::domain::port::error::[A-Za-z0-9_]+' "$file" |
			head -n 1 | sed 's/.*:://')"
		if [ -z "$error_type" ]; then
			printf 'error: %s: %s: the adapter imports no domain port error type\n' \
				"$id" "$file" >&2
			result=1
			continue
		fi

		production_variants=()
		while IFS= read -r variant; do
			[ -n "$variant" ] || continue
			production_variants["$variant"]=1
		done < <(sed -n "1,$((test_line - 1))p" "$file" |
			grep -oE "${error_type}::[A-Za-z0-9_]+" |
			sed "s/^${error_type}:://" | sort -u)

		for variant in "${!production_variants[@]}"; do
			if ! printf '%s\n' "$test_text" | grep -qF "${error_type}::${variant}"; then
				printf 'error: %s: %s: child outcome %s::%s is returned but not asserted by the test module\n' \
					"$id" "$file" "$error_type" "$variant" >&2
				result=1
			fi
		done
		if ! printf '%s\n' "$test_text" | grep -qE '\.ok_or\(|Ok\(Some\('; then
			printf 'error: %s: %s: the test module asserts no successful child outcome\n' \
				"$id" "$file" >&2
			result=1
		fi
	done < <(grep -rlE 'Command::new\(' "$outbound" --include='*.rs' | sort)

	if [ "$found" -eq 0 ]; then
		printf 'error: %s: no native-binary adapter found under %s\n' "$id" "$outbound" >&2
		return 1
	fi

	return "$result"
}

# check_obs_001 — the domain must not log: emit log events only from the
# application and infrastructure layers (OBS-001). `tracing` is the single
# logging facade, so obs-001 matches any dependency on it — an import, a
# fully-qualified path (macro or attribute) or an `extern crate` — under a tree
# that must not log. The application and infrastructure layers are the permitted
# emitters, so only the domain is scanned.
check_obs_001() {
	local id="OBS-001"
	local rule_id="obs-001"
	local domain_dir="src/lib/domain"
	local output

	if [ ! -d "$domain_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$domain_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$domain_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_obs_003() {
	local id="OBS-003"
	local rule_id="obs-003"
	local config matches file line text masked name hit result=0
	local -a names=()

	# The values OBS-003 forbids are named here as data: a new error variable
	# name or raw client field is added to rule_enforcer.yaml, never by editing
	# this function. An empty list approves none, so every formatted value is
	# reported until the list is reviewed.
	config="$(rule_config "$id")" || return 2
	while IFS= read -r name; do
		[ -n "$name" ] || continue
		names+=("$name")
	done < <(jq -r '.denied_values[]?' <<<"$config")
	if [ "${#names[@]}" -eq 0 ]; then
		printf 'error: %s: rule_enforcer.yaml declares no denied_values\n' "$id" >&2
		return 2
	fi

	# obs-003 extracts every non-test tracing log macro invocation. A macro's
	# arguments are a token tree, not typed expressions, so the `?`/`%` sigil
	# and an interpolated name survive only in the node text; the
	# name-vs-denylist comparison happens here because an ast-grep rule cannot
	# read rule_enforcer.yaml. This is a syntactic proxy: it catches a formatted
	# value named as an error or a raw client value and cannot know a value's
	# type (an error on an unrecognised name, or `.to_string()`/`.display()` on
	# one, is outside it).
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no tracing log macro invocation from src\n' "$id" >&2
		return 1
	fi

	while IFS=$'\t' read -r file line text; do
		[ -n "$text" ] || continue
		# Message strings are masked so a prose word (`error`, `body`) inside one
		# is not read as a formatted value; interpolation (`{name}`) is the one
		# form that lives inside the string, so it is tested on the raw text.
		masked="$(sed -E 's/"[^"]*"/""/g' <<<"$text")"
		for name in "${names[@]}"; do
			hit=""
			if [[ $masked =~ [%?][[:space:]]*${name}[[:space:]]*([^.A-Za-z0-9_]|$) ]]; then
				hit="the Display/Debug sigil on a value"
			fi
			if [[ $text =~ \{[[:space:]]*${name}[[:space:]]*[:}] ]]; then
				hit="an interpolated value"
			fi
			if [[ $masked =~ [,(][[:space:]]*${name}[[:space:]]*[,)] ]]; then
				hit="a Display argument"
			fi
			if [ -n "$hit" ]; then
				printf 'error: %s: %s:%s: logs %s named %s; log a stable classification instead (found: %s)\n' \
					"$id" "$file" "$line" "$hit" "$name" "$text" >&2
				result=1
			fi
		done
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, ((.range.start.line + 1) | tostring),
			(.text | gsub("\\s+"; " "))] | @tsv')

	return "$result"
}

# check_obs_004 — a type that holds a secret must not derive a `Debug` that
# exposes it (OBS-004). § 4 names the secret-bearing types; obs-004 extracts
# each struct declaration, each struct that derives `Debug`, and each manual
# `Debug` impl under src/lib/domain/model/. The secret-field comparison happens
# here because an ast-grep rule cannot read rule_enforcer.yaml.
check_obs_004() {
	local id="OBS-004"
	local rule_id="obs-004"
	local model_dir="src/lib/domain/model"
	local config matches file type name body result=0
	local -a secret_names=()
	local -A secret_types=()
	local -A declared=()

	config="$(rule_config "$id")" || return 2

	while IFS= read -r type; do
		[ -n "$type" ] || continue
		secret_types["$type"]=1
	done < <(jq -r '.secret_types[]?' <<<"$config")
	while IFS= read -r name; do
		[ -n "$name" ] || continue
		secret_names+=("$name")
	done < <(jq -r '.secret_fields[]?' <<<"$config")

	if [ "${#secret_types[@]}" -eq 0 ] || [ "${#secret_names[@]}" -eq 0 ]; then
		printf 'error: %s: rule_enforcer.yaml declares no secret_types or secret_fields\n' "$id" >&2
		return 2
	fi

	if [ ! -d "$model_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$model_dir" >&2
		return 1
	fi

	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no struct or Debug impl from %s\n' "$id" "$model_dir" >&2
		return 1
	fi

	# Presence: every type § 4 names is declared under the model directory.
	while IFS= read -r type; do
		[ -n "$type" ] || continue
		declared["$type"]=1
	done < <(printf '%s\n' "$matches" |
		jq -r '.metaVariables.single.DECL.text // empty,
			.metaVariables.single.DERIVED.text // empty,
			.metaVariables.single.IMPL.text // empty')
	for type in "${!secret_types[@]}"; do
		if [ -z "${declared[$type]+set}" ]; then
			printf 'error: %s: secret-bearing type %s is not declared in %s\n' \
				"$id" "$type" "$model_dir" >&2
			result=1
		fi
	done

	# A `Debug` derive prints every field; a field named after a secret exposes it.
	while IFS=$'\t' read -r file type name; do
		[ -n "$name" ] || continue
		if printf '%s\n' "${secret_names[@]}" | grep -qxF "$name"; then
			printf 'error: %s: %s: %s derives Debug, exposing secret field %s\n' \
				"$id" "$file" "$type" "$name" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.DERIVED) |
			.file as $file | .metaVariables.single.DERIVED.text as $type |
			.metaVariables.multi.DERIVED_FIELDS[]?.text |
			select(test(":")) |
			[$file, $type,
				(capture("(?<name>[A-Za-z_][A-Za-z0-9_]*)[[:space:]]*:").name)] | @tsv')

	# A manual `Debug` impl must not print a secret field.
	while IFS=$'\t' read -r file type body; do
		[ -n "$type" ] || continue
		for name in "${secret_names[@]}"; do
			if printf '%s' "$body" | grep -Eq "self\\.${name}\\b"; then
				printf 'error: %s: %s: impl Debug for %s prints secret field %s\n' \
					"$id" "$file" "$type" "$name" >&2
				result=1
			fi
		done
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.IMPL) |
			[.file, .metaVariables.single.IMPL.text, (.text | gsub("\\s+"; " "))] | @tsv')

	return "$result"
}

check_obs_006() {
	local id="OBS-006"
	local rule_id="obs-006"
	local matches file line b64 text level event expected exp_level exp_fields
	local entry body fields sites=0
	local -A catalog=()
	local -A seen=()
	local -a findings=()

	# OBS-006's catalog is the § 6 table, whose columns are event, level and
	# fields. The table is Markdown, so it is parsed here, as `check_sty_rust_076`
	# parses § 10.1; an ast-grep rule cannot read it.
	while IFS=$'\t' read -r event level fields; do
		[ -n "$event" ] || continue
		catalog["$event"]="$level|$fields"
		seen["$event"]=0
	done < <(awk '
		function trim(s) { gsub(/^[[:space:]]+|[[:space:]]+$/, "", s); return s }

		/^### 6\. Security-event catalog/ { in_cat = 1; next }
		in_cat && /^### / { in_cat = 0 }
		in_cat && /^---/ { in_cat = 0 }
		in_cat && /^\|/ {
			n = split($0, c, "|")
			if (n < 6) next
			event = trim(c[2]); level = trim(c[3]); fields = trim(c[4])
			gsub(/`/, "", event); gsub(/`/, "", level); gsub(/`/, "", fields)
			if (event == "Event" || event ~ /^-+$/) next
			if (event !~ /^[a-z][a-z0-9_]*$/) next
			nf = split(fields, f, ",")
			list = ""
			for (i = 1; i <= nf; i++) {
				name = trim(f[i])
				if (name != "") list = list (list == "" ? "" : " ") name
			}
			printf "%s\t%s\t%s\n", event, level, list
		}
	' "$RULEBOOK")

	if [ "${#catalog[@]}" -eq 0 ]; then
		printf 'error: %s: parsed no event from %s § 6\n' "$id" "$RULEBOOK" >&2
		return 1
	fi

	# obs-006 extracts every security-target macro invocation. The catalog
	# pairing happens here because an ast-grep rule reads neither the Markdown
	# table nor a macro body's field set.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi

	while IFS=$'\t' read -r file line b64; do
		[ -n "$b64" ] || continue
		# `@tsv` escapes the macro's newlines, so the body travels base64.
		text="$(printf '%s' "$b64" | base64 -d)"
		sites=$((sites + 1))

		level="$(printf '%s' "${text%%!*}" | tr -d '[:space:]')"
		event="$(printf '%s\n' "$text" |
			grep -oE 'event[[:space:]]*=[[:space:]]*"[A-Za-z0-9_]+"' | head -n 1 |
			sed -E 's/.*"([A-Za-z0-9_]+)"/\1/')"

		# A field is a named entry in the macro body: `<name> = <expr>` or the
		# shorthand `<name>`. The `target:`/`event` pseudo-fields, the macro
		# name and the trailing message literal are dropped; splitting on
		# commas is exact because no field value in the tree carries one.
		body="$(printf '%s' "$text" |
			sed -E 's/^[^(]*\(//; s/\)[[:space:]]*$//' |
			sed -E 's/target[[:space:]]*:[[:space:]]*"security"[[:space:]]*,?//g' |
			sed -E 's/event[[:space:]]*=[[:space:]]*"[A-Za-z0-9_]+"[[:space:]]*,?//g' |
			sed -E 's/"[^"]*"[[:space:]]*$//' |
			tr ',' '\n')"
		fields=""
		while IFS= read -r entry; do
			entry="$(printf '%s' "$entry" | sed -E 's/^[[:space:]]+|[[:space:]]+$//g')"
			[ -n "$entry" ] || continue
			case "$entry" in
			'"'*) continue ;;
			esac
			if [[ $entry =~ ^([a-z_][a-z0-9_]*)[[:space:]]*= ]]; then
				fields="$fields ${BASH_REMATCH[1]}"
			elif [[ $entry =~ ^([a-z_][a-z0-9_]*)$ ]]; then
				fields="$fields $entry"
			fi
		done <<<"$body"
		# shellcheck disable=SC2086  # word-split the space-joined field list
		fields="$(printf '%s\n' $fields | LC_ALL=C sort -u | paste -sd' ' -)"

		if [ -z "${catalog[$event]+set}" ]; then
			findings+=("$id: $file:$line: emitted event $event is not in the catalog")
			continue
		fi
		seen["$event"]=1
		expected="${catalog[$event]}"
		exp_level="${expected%%|*}"
		# shellcheck disable=SC2086  # word-split the space-joined field list
		exp_fields="$(printf '%s\n' ${expected#*|} | LC_ALL=C sort -u | paste -sd' ' -)"
		if [ "$level" != "$exp_level" ]; then
			findings+=("$id: $file:$line: $event must be emitted at $exp_level, not $level")
		fi
		if [ "$fields" != "$exp_fields" ]; then
			findings+=("$id: $file:$line: $event must carry fields [$exp_fields], not [$fields]")
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.text != null) |
			[.file, ((.range.start.line + 1) | tostring), (.text | @base64)] | @tsv')

	if [ "$sites" -eq 0 ]; then
		printf 'error: %s: extracted no security-target emission from src\n' "$id" >&2
		return 1
	fi

	# Completeness: every catalogued event has an emission site.
	for event in "${!seen[@]}"; do
		if [ "${seen[$event]}" -eq 0 ]; then
			findings+=("$id: emitted: catalogued event $event is never emitted")
		fi
	done

	if [ "${#findings[@]}" -eq 0 ]; then
		return 0
	fi
	printf '%s\n' "${findings[@]}" | LC_ALL=C sort >&2
	return 1
}

check_obs_007() {
	local id="OBS-007"
	local rule_id="obs-007"
	local domain="src/lib/domain/model/logging.rs"
	local infra="src/lib/infrastructure/logging.rs"
	local matches result=0

	if [ ! -f "$domain" ] || [ ! -f "$infra" ]; then
		printf 'error: %s: missing %s or %s\n' "$id" "$domain" "$infra" >&2
		return 1
	fi

	# obs-007 extracts the guards and the floor that keep a catalog event from
	# being suppressed; the per-file placement and the pinned values are
	# asserted here because an ast-grep rule cannot assert a conjunction of
	# constructs across two files and two functions.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi

	if ! printf '%s\n' "$matches" | jq -e -s --arg f "$domain" \
		'any(.[]; .file == $f and .metaVariables.single.DISABLING_LEVEL != null)' >/dev/null; then
		printf 'error: %s: Logging::try_new must reject off/none as a whole-level value\n' "$id" >&2
		result=1
	fi
	if ! printf '%s\n' "$matches" | jq -e -s --arg f "$domain" \
		'any(.[]; .file == $f and .metaVariables.single.SECURITY_DIRECTIVES != null)' >/dev/null; then
		printf 'error: %s: Logging::try_new must reject a directive that targets the security target\n' "$id" >&2
		result=1
	fi
	if ! printf '%s\n' "$matches" | jq -e -s --arg f "$domain" \
		'any(.[]; .file == $f and (.text | test("^const SECURITY_TARGET: &str = \"security\";$")))' >/dev/null; then
		printf 'error: %s: the reserved security target constant must be "security"\n' "$id" >&2
		result=1
	fi
	if ! printf '%s\n' "$matches" | jq -e -s --arg f "$infra" \
		'any(.[]; .file == $f and (.text | test("^const SECURITY_FLOOR: &str = \"security=debug\";$")))' >/dev/null; then
		printf 'error: %s: the security floor constant must be "security=debug"\n' "$id" >&2
		result=1
	fi
	if ! printf '%s\n' "$matches" | jq -e -s --arg f "$infra" \
		'any(.[]; .file == $f and (.text | test("^format!")))' >/dev/null; then
		printf 'error: %s: logging::setup must append the security floor to the filter\n' "$id" >&2
		result=1
	fi

	return "$result"
}

# check_obs_008 — an adapter that hands caller-supplied content to an external
# native parser restricts its protocols to an explicit allow-list, bounds the
# child's wall clock and output, and records the residual isolation gap
# (TechnicalDesign.md § 9, OBS-008; detail § 8). An adapter is a file under the
# infrastructure layer that spawns a child process, detected by its `process`
# import. The per-parser allow-list is data (rule_enforcer.yaml); the tracking
# issue recording the gap is read from the rulebook § 8 detail, so the check
# hard-codes neither. The multi-statement conjunction and the comment citation
# are beyond ast-grep.
check_obs_008() {
	local id="OBS-008"
	local infra="src/lib/infrastructure"
	local config proto whitelist issue file found_issue count=0 result=0
	local -a allowed_protocols=()
	local -A is_allowed=()
	local -a issues=()
	local -A is_issue=()

	config="$(rule_config "$id")" || return 2
	while IFS= read -r proto; do
		[ -n "$proto" ] || continue
		allowed_protocols+=("$proto")
		is_allowed["$proto"]=1
	done < <(jq -r '.allowed_protocols[]?' <<<"$config")
	if [ "${#allowed_protocols[@]}" -eq 0 ]; then
		printf 'error: %s: rule_enforcer.yaml declares no allowed_protocols\n' "$id" >&2
		return 2
	fi

	# The tracked end-state issues that record the residual isolation gap are
	# read from the rulebook § 8 detail, so the check hard-codes none.
	while IFS= read -r issue; do
		[ -n "$issue" ] || continue
		issues+=("$issue")
		is_issue["$issue"]=1
	done < <(awk '
		/^### 8\. External parsers on untrusted input/ { in_detail = 1; next }
		in_detail && (/^##[^#]/ || /^---/) { in_detail = 0 }
		in_detail {
			while (match($0, /https:\/\/github\.com\/Mnemorium\/mnemorium\/issues\/[0-9]+/)) {
				print substr($0, RSTART, RLENGTH)
				$0 = substr($0, RSTART + RLENGTH)
			}
		}
	' "$RULEBOOK")
	if [ "${#issues[@]}" -eq 0 ]; then
		printf 'error: %s: %s § 8 names no tracking issue for the residual isolation gap\n' \
			"$id" "$RULEBOOK" >&2
		return 1
	fi

	if [ ! -d "$infra" ]; then
		printf 'error: %s: missing %s\n' "$id" "$infra" >&2
		return 1
	fi

	# An adapter that hands caller-supplied content to an external native parser
	# spawns a child process, so it imports a `process` module; every such file
	# under the infrastructure layer is an external-parser adapter.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		count=$((count + 1))

		# Clause 1 — an explicit protocol allow-list.
		whitelist="$(awk '
			{
				line = $0
				while (match(line, /"[^"]*"/)) {
					token = substr(line, RSTART + 1, RLENGTH - 2)
					if (want) { print token; exit }
					if (token == "-protocol_whitelist") want = 1
					line = substr(line, RSTART + RLENGTH)
				}
			}
		' "$file")"
		if [ -z "$whitelist" ]; then
			printf 'error: %s: %s: external parser is invoked without an explicit -protocol_whitelist allow-list\n' \
				"$id" "$file" >&2
			result=1
		else
			while IFS= read -r proto; do
				[ -n "$proto" ] || continue
				if [ -z "${is_allowed[$proto]+set}" ]; then
					printf 'error: %s: %s: protocol %s is not on the explicit allow-list\n' \
						"$id" "$file" "$proto" >&2
					result=1
				fi
			done < <(printf '%s\n' "$whitelist" | tr ',' '\n')
		fi

		# Clause 2 — the child's wall clock is bounded.
		if ! grep -qE '(^|[^A-Za-z0-9_])timeout\(' "$file" ||
			! grep -qF 'kill_on_drop(true)' "$file"; then
			printf 'error: %s: %s: the external parser is not bounded by a wall clock (timeout + kill_on_drop)\n' \
				"$id" "$file" >&2
			result=1
		fi

		# Clause 3 — the child's output is bounded.
		if ! grep -qE '\.take\(' "$file"; then
			printf 'error: %s: %s: the external parser standard output is not capped\n' \
				"$id" "$file" >&2
			result=1
		fi

		# Clause 4 — the residual isolation gap is recorded.
		found_issue=0
		while IFS= read -r url; do
			[ -n "$url" ] || continue
			if [ -n "${is_issue[$url]+set}" ]; then
				found_issue=1
			fi
		done < <(grep -oE 'https://github\.com/Mnemorium/mnemorium/issues/[0-9]+' "$file")
		if [ "$found_issue" -eq 0 ]; then
			printf 'error: %s: %s: the residual isolation gap is not recorded; cite a tracking issue named in %s § 8\n' \
				"$id" "$file" "$RULEBOOK" >&2
			result=1
		fi
	done < <(grep -rlE '^use (std|tokio)::process' "$infra" --include='*.rs' | sort)

	if [ "$count" -eq 0 ]; then
		printf 'error: %s: found no external-parser adapter under %s\n' "$id" "$infra" >&2
		return 1
	fi

	return "$result"
}

# check_test_022 — every application use-case test module covers a dependency
# failure (TEST-022): at least one test configures a mocked outbound port to
# return an error. An application test exercises a use case with its ports
# replaced by mocks (Application strategy), so a module that drives a use case
# through a mock must exercise the path where one of those ports fails.
check_test_022() {
	local id="TEST-022"
	local rule_id="test-022"
	local target="src/lib/application/use_case"
	local matches file result=0
	local -A covered=()

	if [ ! -d "$target" ]; then
		printf 'error: %s: missing %s\n' "$id" "$target" >&2
		return 1
	fi

	# test-022 extracts every mock expectation configured to return an error —
	# a `.returning(…)`/`.return_once(…)` whose arguments yield `Err(…)`. The
	# per-file coverage decision happens here because an ast-grep rule cannot
	# count matches within a file.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$target" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi

	while IFS= read -r file; do
		[ -n "$file" ] || continue
		covered["$file"]=1
	done < <(printf '%s\n' "$matches" | jq -r '.file')

	# Every use-case file whose test module drives it through a mock must cover
	# a dependency failure.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		if [ -z "${covered[$file]+set}" ]; then
			printf 'error: %s: %s: test module covers no dependency failure; configure a mocked port to return an error\n' \
				"$id" "$file" >&2
			result=1
		fi
	done < <(grep -rl --include='*.rs' 'Mock' "$target" | sort)

	return "$result"
}

# check_test_024 — a domain service has its own inline unit test module
# (TEST-024). Every `.rs` file under src/lib/domain/service/ is a domain
# service; it must declare an inline `#[cfg(test)] mod tests { … }`. The rule
# is positive: ast-grep extracts the compliant module, and the per-file
# requirement — the service file that yields no match — is resolved here
# because an ast-grep rule cannot assert the absence of a match in a file.
check_test_024() {
	local id="TEST-024"
	local rule_id="test-024"
	local service_dir="src/lib/domain/service"
	local matches file found=0 result=0
	local -A has_test_module=()

	if [ ! -d "$service_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$service_dir" >&2
		return 1
	fi

	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$service_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi

	# Every service file that carries its own inline test module.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		has_test_module["$file"]=1
	done < <(printf '%s\n' "$matches" | jq -r '.file')

	# Every domain service must appear above; a file absent from the map has
	# no dedicated test module of its own.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		found=$((found + 1))
		if [ -z "${has_test_module[$file]+set}" ]; then
			printf 'error: %s: %s: domain service has no dedicated unit test module; declare an inline #[cfg(test)] mod tests\n' \
				"$id" "$file" >&2
			result=1
		fi
	done < <(find "$service_dir" -maxdepth 1 -name '*.rs' | sort)

	if [ "$found" -eq 0 ]; then
		printf 'error: %s: no domain service found under %s\n' "$id" "$service_dir" >&2
		return 1
	fi

	return "$result"
}

check_test_025() {
	local id="TEST-025"
	local rule_id="test-025"
	local service_dir="src/lib/domain/service"
	local matches file name variant enum_count=0 result=0
	local -A test_ids=()

	if [ ! -d "$service_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$service_dir" >&2
		return 1
	fi

	# test-025 extracts each domain service's error-enum variants — the
	# business rules the service enforces — and the identifiers its own
	# `#[cfg(test)]` module names. Pairing each variant with the test module
	# happens here because an ast-grep rule cannot compare two sets within one
	# file.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$service_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no domain service from %s\n' "$id" "$service_dir" >&2
		return 1
	fi

	# The identifiers the service's own test module names, keyed by file.
	while IFS=$'\t' read -r file name; do
		[ -n "$name" ] || continue
		test_ids["${file}:${name}"]=1
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.IDENT != null) | [.file, .metaVariables.single.IDENT.text] | @tsv')

	# Each error variant is one business rule; it must be named in the
	# service's own test module. A tuple or struct variant's text carries its
	# payload, so only the leading identifier is the variant name.
	while IFS=$'\t' read -r file variant; do
		[ -n "$variant" ] || continue
		variant="${variant%%[!A-Za-z0-9_]*}"
		enum_count=$((enum_count + 1))
		if [ -z "${test_ids["${file}:${variant}"]+set}" ]; then
			printf 'error: %s: %s: business rule %s is not covered by the service test module; assert the error it encodes\n' \
				"$id" "$file" "$variant" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.VARIANT != null) | [.file, .metaVariables.single.VARIANT.text] | @tsv')

	# An empty variant set means the extraction moved, not that the rule holds.
	if [ "$enum_count" -eq 0 ]; then
		printf 'error: %s: extracted no service error variant from %s\n' "$id" "$service_dir" >&2
		return 1
	fi

	return "$result"
}

# check_test_026 — a domain service is pure, so its tests need no doubles
# (TEST-026). Every mockall construct — the `mockall`/`automock` name, a
# `Mock*` type, or a `mock!` macro — is a double, and the service directory is
# the scope the rule names.
check_test_026() {
	local id="TEST-026"
	local rule_id="test-026"
	local service_dir="src/lib/domain/service"
	local output

	if [ ! -d "$service_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$service_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$service_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_test_027 — integration tests exercise the outbound port adapters
# (TEST-027): every repository (SQLite, moka cache) and configuration-source
# adapter carries its inline `#[cfg(test)] mod tests { … }`, the module the
# integration test lives in (TEST-028). The surfaces are the rule's own
# families, transcribed as globs in rule_enforcer.yaml so a new adapter is
# added as data. The rule is positive: ast-grep extracts the compliant inline
# module, and the per-file requirement — the adapter that yields no match — is
# resolved here because an ast-grep rule cannot assert the absence of a match
# in a file.
check_test_027() {
	local id="TEST-027"
	local rule_id="test-027"
	local config pattern file matches found=0 result=0
	local -a surfaces=()
	local -a files=()

	config="$(rule_config "$id")" || return 2

	# The rule names the families, not the paths: the repository (SQLite, moka
	# cache) and the configuration source are transcribed as globs in
	# rule_enforcer.yaml. The moka cache adapter is declared but not
	# implemented, so its path is listed and skipped below until it holds code.
	mapfile -t surfaces < <(jq -r '.adapter_surfaces[]?' <<<"$config")
	if [ "${#surfaces[@]}" -eq 0 ]; then
		printf 'error: %s: rule_enforcer.yaml declares no adapter_surfaces\n' "$id" >&2
		return 2
	fi

	for pattern in "${surfaces[@]}"; do
		mapfile -t files < <(compgen -G "$pattern" || true)
		for file in "${files[@]}"; do
			[ -f "$file" ] || continue
			# A module that holds no code at all (blank lines and comments
			# only) declares no adapter: there is nothing to exercise yet.
			if ! grep -qE '^[^/[:space:]]' "$file"; then
				continue
			fi
			found=$((found + 1))
			if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
				--json=stream "$file" 2>&1)"; then
				printf '%s\n' "$matches"
				return 1
			fi
			if [ -z "$matches" ]; then
				printf 'error: %s: %s: outbound port adapter has no inline #[cfg(test)] test module; add its integration test to this file\n' \
					"$id" "$file" >&2
				result=1
			fi
		done
	done

	if [ "$found" -eq 0 ]; then
		printf 'error: %s: extracted no outbound port adapter from the declared surfaces\n' "$id" >&2
		return 1
	fi

	return "$result"
}

# check_test_028 — an integration test module lives in the same file as the
# code under test (TEST-028). The integration tests exercise the outbound port
# adapters (TEST-027), so the adapter tree is the surface. The violation is a
# test-gated file module (`#[cfg(test)] mod tests;`): it has no body, so it
# lives in another file.
check_test_028() {
	local id="TEST-028"
	local rule_id="test-028"
	local surface="src/lib/infrastructure/outbound"
	local output

	if [ ! -d "$surface" ]; then
		printf 'error: %s: missing %s\n' "$id" "$surface" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_test_033 — the SQLite repository is exercised against an in-memory
# database, one `:memory:` connection per test so each test runs against a
# fresh database (TEST-033). The surface is the SQLite adapter tree
# (src/lib/infrastructure/outbound/sqlx/). test-033 extracts every connection
# call made from a `#[cfg(test)]` module together with its enclosing function;
# the literal-vs-`:memory:` comparison and the "per test, not shared" decision
# happen here because an ast-grep rule cannot compare a literal or see which
# tests a helper serves.
check_test_033() {
	local id="TEST-033"
	local rule_id="test-033"
	local surface="src/lib/infrastructure/outbound/sqlx"
	local matches file line arg helper result=0 modules=0
	local -A in_memory=()

	if [ ! -d "$surface" ]; then
		printf 'error: %s: missing %s\n' "$id" "$surface" >&2
		return 1
	fi

	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi

	while IFS=$'\t' read -r file line arg helper; do
		[ -n "$file" ] || continue
		if [ "$arg" != '"sqlite::memory:"' ]; then
			printf 'error: %s: %s:%s: test connects to %s; the SQLite repository must use an in-memory database (:memory:)\n' \
				"$id" "$file" "$line" "${arg:-none}" >&2
			result=1
			continue
		fi
		if [ -z "$helper" ]; then
			printf 'error: %s: %s:%s: the :memory: connection is created outside a function; each test must open its own connection so it runs against a fresh database\n' \
				"$id" "$file" "$line" >&2
			result=1
			continue
		fi
		in_memory["$file"]=1
	done < <(printf '%s\n' "$matches" | jq -r \
		'[.file, ((.range.start.line + 1) | tostring),
		 (.metaVariables.single.ARG.text // ""),
		 (.metaVariables.single.HELPER.text // "")] | @tsv')

	# Every test module that builds a SQLite pool must open an in-memory
	# connection; a module with none runs against an on-disk file.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		grep -qE 'SqlitePool|SqlitePoolOptions|init_db|sqlx::migrate!' "$file" || continue
		modules=$((modules + 1))
		if [ -z "${in_memory[$file]+set}" ]; then
			printf 'error: %s: %s: SQLite test module opens no :memory: connection; its datastore tests must run against a fresh in-memory database\n' \
				"$id" "$file" >&2
			result=1
		fi
	done < <(grep -rl '#\[cfg(test)\]' "$surface" --include='*.rs' | sort)

	if [ "$modules" -eq 0 ]; then
		printf 'error: %s: extracted no SQLite test module from %s\n' "$id" "$surface" >&2
		return 1
	fi

	return "$result"
}

# check_test_034 — a SQLite repository integration test caps its in-memory pool
# at one connection (TEST-034). Each test module builds the pool with
# `SqlitePoolOptions::new().max_connections(1)`: a larger pool opens a separate
# private `:memory:` database per connection, so the test would no longer run
# against the one migrated database it seeded. The production `init_db` pool is
# configurable by design and out of scope; the ast-grep rule confines its
# matches to `#[cfg(test)]` modules.
check_test_034() {
	local id="TEST-034"
	local rule_id="test-034"
	local target="src/lib/infrastructure/outbound/sqlx"
	local output

	if [ ! -d "$target" ]; then
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

# check_test_035 — the moka cache adapter's test mocks the wrapped repository
# with mockall (TEST-035). The moka cache adapter wraps another repository
# (§ Moka), so the test supplies a mockall mock as that inner repository: the
# `Mock<…>Repository` type TEST-002 generates for every repository port. The
# rule's surface is the moka adapter — path-fixed, so no rule_enforcer.yaml
# block. A surface file that holds no code declares no adapter yet (the moka
# adapter is declared in outbound.rs but unimplemented), so it is skipped until
# it lands; a file that holds production code but no in-file test module has no
# test to mock anything, which TEST-027 owns.
check_test_035() {
	local id="TEST-035"
	local rule_id="test-035"
	local surface="src/lib/infrastructure/outbound/moka.rs"
	local module="src/lib/infrastructure/outbound/moka"
	local file matches found=0 result=0
	local -a files=()

	if [ ! -f "$surface" ] && [ ! -d "$module" ]; then
		printf 'error: %s: missing %s\n' "$id" "$surface" >&2
		return 1
	fi

	mapfile -t files < <({
		[ -f "$surface" ] && printf '%s\n' "$surface"
		[ -d "$module" ] && find "$module" -name '*.rs'
	} | sort -u)

	for file in "${files[@]}"; do
		[ -f "$file" ] || continue
		# A module that holds no code (blank lines and comments only) declares
		# no adapter: there is nothing to exercise yet.
		if ! grep -qE '^[^/[:space:]]' "$file"; then
			continue
		fi
		found=$((found + 1))
		# The rule is about the adapter's test. A file with production code but
		# no in-file test module has no test to mock anything; TEST-027 owns
		# that missing module.
		if ! grep -qE '^#\[cfg\(test\)\]' "$file"; then
			continue
		fi
		# test-035 extracts the mockall mock the test supplies as the wrapped
		# repository; the per-file requirement is resolved here because an
		# ast-grep rule cannot assert the absence of a match in a file.
		if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
			--json=stream "$file" 2>&1)"; then
			printf '%s\n' "$matches"
			return 1
		fi
		if [ -z "$matches" ]; then
			printf 'error: %s: %s: the moka cache adapter test does not mock the wrapped repository with mockall; supply a Mock<…>Repository double as the wrapped repository\n' \
				"$id" "$file" >&2
			result=1
		fi
	done

	if [ "$found" -eq 0 ]; then
		# The moka cache adapter is declared but unimplemented; TEST-027 owns
		# the adapter's existence, and the rule has no subject until it lands.
		return 0
	fi

	return "$result"
}

# check_test_036 — an external-service test stubs the service with `wiremock`
# and loads its canned responses from golden files (TEST-036). The
# external-service clients are the outbound adapters that map a dependency
# fault to the domain's `ExternalServiceError` (§ 6 error table); test-036
# extracts each one's wiremock stub and any canned response built from an
# inline literal, and the per-file requirement — a client whose test stubs
# with something other than `wiremock` — is resolved here because an ast-grep
# rule cannot assert the absence of a match in a file. The tree holds no
# external-service client yet, so the check is green and vacuous until the
# first one lands.
check_test_036() {
	local id="TEST-036"
	local rule_id="test-036"
	local surface="src/lib/infrastructure/outbound"
	local file matches kind line text stubbed result=0

	if [ ! -d "$surface" ]; then
		printf 'error: %s: missing %s\n' "$id" "$surface" >&2
		return 1
	fi

	while IFS= read -r file; do
		[ -n "$file" ] || continue

		if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
			--json=stream "$file" 2>&1)"; then
			printf '%s\n' "$matches"
			return 1
		fi

		stubbed=0
		if [ -n "$matches" ]; then
			while IFS=$'\t' read -r kind line text; do
				[ -n "$kind" ] || continue
				if [ "$kind" = "inline" ]; then
					printf 'error: %s: %s:%s: canned response is inline; load it from a golden file (found: %s)\n' \
						"$id" "$file" "$line" "$text" >&2
					result=1
				else
					stubbed=1
				fi
			done < <(printf '%s\n' "$matches" | jq -r '
				[(if .metaVariables.single.INLINE then "inline" else "stub" end),
				 ((.range.start.line + 1) | tostring),
				 (.metaVariables.single.INLINE.text // "" | gsub("\\s+"; " "))] | @tsv')
		fi

		if [ "$stubbed" -eq 0 ]; then
			printf 'error: %s: %s: external-service test does not stub the service with wiremock; start a MockServer\n' \
				"$id" "$file" >&2
			result=1
		fi
	done < <(grep -rl 'ExternalServiceError' --include='*.rs' "$surface" 2>/dev/null | sort)

	return "$result"
}

# check_test_038 — an external-service test module lives in the same file as
# the client under test (TEST-038). The external-service clients are the
# outbound adapters that map a dependency fault to the domain's
# `ExternalServiceError` (§ 6 error table), the surface the sibling TEST-036
# plan records. The violation is a test-gated file module
# (`#[cfg(test)] mod tests;`): it has no body, so it lives in another file. The
# client discovery is content-based — an ast-grep rule cannot see whether the
# file it matches names `ExternalServiceError` — so the wrapper resolves it.
# The tree holds no external-service client yet, so the check is green and
# vacuous until the first one lands.
check_test_038() {
	local id="TEST-038"
	local rule_id="test-038"
	local surface="src/lib/infrastructure/outbound"
	local file output result=0

	if [ ! -d "$surface" ]; then
		printf 'error: %s: missing %s\n' "$id" "$surface" >&2
		return 1
	fi

	while IFS= read -r file; do
		[ -n "$file" ] || continue
		if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
			--report-style=short --color=never "$file" 2>&1)"; then
			continue
		fi
		printf '%s\n' "$output"
		result=1
	done < <(grep -rl 'ExternalServiceError' --include='*.rs' "$surface" 2>/dev/null | sort)

	return "$result"
}

# check_test_040 — every pytest test module lives under `test/e2e/` (TEST-040).
# pytest.ini's `python_files` is the suite's own definition of a test module, so
# it is read here rather than duplicated: a tracked Python file matching one of
# those globs is a test, and a test outside `test/e2e/` is the violation.
check_test_040() {
	local id="TEST-040"
	local e2e_dir="test/e2e"
	local pytest_ini="pytest.ini"
	local file base line pattern result=0
	local -a patterns=()

	if [ ! -d "$e2e_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$e2e_dir" >&2
		return 1
	fi
	if [ ! -f "$pytest_ini" ]; then
		printf 'error: %s: missing %s\n' "$id" "$pytest_ini" >&2
		return 1
	fi

	while IFS= read -r line || [ -n "$line" ]; do
		for pattern in $line; do
			patterns+=("$pattern")
		done
	done < <(sed -nE \
		's/^[[:space:]]*python_files[[:space:]]*=[[:space:]]*(.*)$/\1/p' "$pytest_ini")
	if [ "${#patterns[@]}" -eq 0 ]; then
		printf 'error: %s: no python_files patterns parsed from %s\n' \
			"$id" "$pytest_ini" >&2
		return 1
	fi

	while IFS= read -r file; do
		[ -n "$file" ] || continue
		case "$file" in
		"$e2e_dir"/*) continue ;;
		esac
		base="${file##*/}"
		for pattern in "${patterns[@]}"; do
			# The glob on the right is intentional: match as a pattern.
			# shellcheck disable=SC2053
			if [[ $base == $pattern ]]; then
				printf 'error: %s: %s: pytest test module lives outside %s; move it under %s\n' \
					"$id" "$file" "$e2e_dir" "$e2e_dir" >&2
				result=1
				break
			fi
		done
	done < <(git ls-files -- '*.py' | sort)

	return "$result"
}

# check_test_041 — one E2E file per catalogued business use case (TEST-041).
# The catalog is the rulebook's own data: the `## UC-<seq> - <Name>` headings of
# docs/development/UseCases.md are read at run time, as check_sty_rust_076 reads
# § 10.1, so a new use case is covered by cataloguing it, never by editing this
# function. Every Python file under test/e2e/ is named `uc<seq>_<name>.py` after
# a catalogued use case, one file per use case, and every use case has its file.
# `conftest.py` holds the shared fixtures, not a use case, so it is exempt.
check_test_041() {
	local id="TEST-041"
	local e2e_dir="test/e2e"
	local catalog="docs/development/UseCases.md"
	local file base seq num result=0
	local -A use_cases=()
	local -A files_of=()

	if [ ! -d "$e2e_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$e2e_dir" >&2
		return 1
	fi
	if [ ! -f "$catalog" ]; then
		printf 'error: %s: missing %s\n' "$id" "$catalog" >&2
		return 1
	fi

	# The catalogued business use cases: `## UC-<seq> - <Name>` headings. The
	# leading-zero form is kept for diagnostics; the numeric key joins the
	# heading to the file name.
	while IFS= read -r seq; do
		[ -n "$seq" ] || continue
		num=$((10#$seq))
		use_cases["$num"]="UC-$seq"
	done < <(sed -nE 's/^## UC-([0-9]+) - .*$/\1/p' "$catalog")

	if [ "${#use_cases[@]}" -eq 0 ]; then
		printf 'error: %s: parsed no use case from %s\n' "$id" "$catalog" >&2
		return 1
	fi

	# Every E2E file is named after a catalogued use case, and one use case
	# keeps one file: `uc<seq>_<name>.py`.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		base="${file##*/}"
		if [[ ! $base =~ ^uc([0-9]+)_ ]]; then
			printf 'error: %s: %s: E2E file is not named after a catalogued use case (expected uc<seq>_<name>.py)\n' \
				"$id" "$file" >&2
			result=1
			continue
		fi
		seq="${BASH_REMATCH[1]}"
		num=$((10#$seq))
		if [ -z "${use_cases[$num]+set}" ]; then
			printf 'error: %s: %s: names UC-%s, which is not in %s\n' \
				"$id" "$file" "$seq" "$catalog" >&2
			result=1
		elif [ -n "${files_of[$num]+set}" ]; then
			printf 'error: %s: %s: %s already has %s; keep one file per use case\n' \
				"$id" "$file" "${use_cases[$num]}" "${files_of[$num]}" >&2
			result=1
		else
			files_of["$num"]="$file"
		fi
	done < <(find "$e2e_dir" -type f -name '*.py' \
		! -name 'conftest.py' ! -path "${e2e_dir}/fixture/*" \
		! -path '*/__pycache__/*' | sort)

	# Every catalogued business use case has its one file.
	while IFS= read -r num; do
		[ -n "$num" ] || continue
		if [ -z "${files_of[$num]+set}" ]; then
			printf 'error: %s: catalogued use case %s has no E2E file under %s\n' \
				"$id" "${use_cases[$num]}" "$e2e_dir" >&2
			result=1
		fi
	done < <(printf '%s\n' "${!use_cases[@]}" | sort -n)

	return "$result"
}

# check_test_042 — the E2E suite is split into one folder per bounded context
# (TEST-042). The bounded contexts are the Overview's § Bounded context table,
# read here as the rule's own source of truth; `test/e2e/fixture/` is the
# sanctioned home of golden and media files (TEST-052), and `__pycache__` is the
# interpreter's own directory, so neither is a context folder.
check_test_042() {
	local id="TEST-042"
	local e2e_dir="test/e2e"
	local overview="docs/development/Overview.md"
	local entry name result=0
	local -A is_context=()

	if [ ! -d "$e2e_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$e2e_dir" >&2
		return 1
	fi
	if [ ! -f "$overview" ]; then
		printf 'error: %s: missing %s\n' "$id" "$overview" >&2
		return 1
	fi

	# The Overview's § Bounded context table is the rule's list: its first
	# column, lowercased, is the set of folder names the E2E suite may use.
	while IFS= read -r name; do
		[ -n "$name" ] || continue
		is_context["$name"]=1
	done < <(awk '
		function trim(s) { gsub(/^[[:space:]]+|[[:space:]]+$/, "", s); return s }

		/^## Bounded context[[:space:]]*$/ { in_table = 1; next }
		in_table && /^##/ { in_table = 0 }
		in_table && /^\|/ {
			n = split($0, c, "|")
			if (n < 3) next
			name = trim(c[2])
			if (name == "" || name == "Name" || name ~ /^-+$/) next
			printf "%s\n", tolower(name)
		}
	' "$overview")

	if [ "${#is_context[@]}" -eq 0 ]; then
		printf 'error: %s: no bounded contexts parsed from %s\n' "$id" "$overview" >&2
		return 1
	fi

	while IFS= read -r entry; do
		name="${entry##*/}"
		# TEST-052 owns the fixture folder; __pycache__ is generated.
		case "$name" in
		fixture | __pycache__) continue ;;
		esac
		if [ -z "${is_context[$name]+set}" ]; then
			printf 'error: %s: %s is not a bounded-context folder; place its tests in the folder of the bounded context that owns them (see Overview.md, Bounded context)\n' \
				"$id" "$entry" >&2
			result=1
		fi
	done < <(find "$e2e_dir" -mindepth 1 -maxdepth 1 -type d | sort)

	# A test module belongs inside its bounded-context folder; `conftest.py` is
	# the suite's own fixture module (TEST-043) and stays at the root.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		printf 'error: %s: %s sits directly under %s; move it into its bounded-context folder\n' \
			"$id" "$file" "$e2e_dir" >&2
		result=1
	done < <(find "$e2e_dir" -mindepth 1 -maxdepth 1 -type f -name '*.py' \
		! -name 'conftest.py' | sort)

	return "$result"
}

# check_test_043 — a generic E2E test provisions or registers its users with the
# shared `SECRET_PASSWORD` (TEST-043). test-043 extracts every `register(…)`
# call and its password argument; the shared-secret comparison happens here
# because an ast-grep rule cannot read rule_enforcer.yaml. A name bound to the
# shared secret (`X = SECRET_PASSWORD`) is the secret by another name, and a
# test whose subject is password behavior (TEST-044) is exempt through
# allowed_locations.
check_test_043() {
	local id="TEST-043"
	local rule_id="test-043"
	local e2e_dir="test/e2e"
	local conftest="$e2e_dir/conftest.py"
	local config matches file line password location func result=0
	local -A secret_aliases=()
	local -A allowed=()

	if [ ! -f "$conftest" ]; then
		printf 'error: %s: missing %s\n' "$id" "$conftest" >&2
		return 1
	fi
	if ! grep -qE '^SECRET_PASSWORD[[:space:]]*=' "$conftest"; then
		printf 'error: %s: %s does not define SECRET_PASSWORD\n' "$id" "$conftest" >&2
		return 1
	fi

	config="$(rule_config "$id")" || return 2

	# The exempt locations are the password-behavior tests (TEST-044): a test
	# that picks its own explicit password is named here, never inferred. An
	# empty list exempts none, so every non-secret password is reported until it
	# is reviewed.
	while IFS= read -r location; do
		[ -n "$location" ] || continue
		allowed["$location"]=1
	done < <(jq -r '.allowed_locations[]?' <<<"$config")

	# Every name bound to the shared secret (`X = SECRET_PASSWORD`) is the shared
	# secret by another name, so a provisioning call may pass it.
	secret_aliases[SECRET_PASSWORD]=1
	while IFS= read -r name; do
		[ -n "$name" ] || continue
		secret_aliases["$name"]=1
	done < <(grep -rhoE '^[A-Za-z_][A-Za-z0-9_]*[[:space:]]*=[[:space:]]*SECRET_PASSWORD([[:space:]]|$)' \
		"$e2e_dir" --include='*.py' | sed -E 's/[[:space:]]*=.*//')

	# test-043 extracts every `register(…)` call and its password argument.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$e2e_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	[ -n "$matches" ] || return 0

	while IFS=$'\t' read -r file line password; do
		[ -n "$file" ] || continue
		# conftest.py is the fixture module, not a test: its `_register` helper
		# forwards whatever password its caller passes.
		case "$file" in
		conftest.py | */conftest.py) continue ;;
		esac
		if [ -n "${secret_aliases[$password]+set}" ]; then
			continue
		fi
		func="$(awk -v target="$line" '
			/^(async[[:space:]]+)?def[[:space:]]/ {
				name=$0
				sub(/^.*def[[:space:]]+/, "", name)
				sub(/[[:space:](].*$/, "", name)
			}
			NR==target { print name; exit }
			END { if (NR < target) print name }
		' "$file")"
		location="${file}::${func}"
		if [ -n "${allowed[$location]+set}" ]; then
			continue
		fi
		printf 'error: %s: %s:%s: user-provisioning call passes %s, not the shared SECRET_PASSWORD; use SECRET_PASSWORD or add "%s" to allowed_locations\n' \
			"$id" "$file" "$line" "$password" "$location" >&2
		result=1
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, ((.range.start.line + 1) | tostring),
			.metaVariables.single.PASSWORD.text] | @tsv')

	return "$result"
}

check_test_045() {
	local id="TEST-045"
	local rule_id="test-045"
	local e2e_dir="test/e2e"
	local conftest="$e2e_dir/conftest.py"
	local matches file line text name secret_re found_fixture=0 result=0
	local -a shared=("SECRET_PASSWORD" "secret_password")

	if [ ! -d "$e2e_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$e2e_dir" >&2
		return 1
	fi

	# The shared secret is SECRET_PASSWORD and every name bound to it — the
	# conftest constants and the per-file aliases. The set is read from the
	# suite itself because an ast-grep rule cannot read another file's
	# assignments.
	while IFS= read -r name; do
		[ -n "$name" ] || continue
		shared+=("$name")
	done < <(grep -rhoE '^[A-Za-z_][A-Za-z0-9_]*[[:space:]]*=[[:space:]]*SECRET_PASSWORD[[:space:]]*$' \
		"$e2e_dir" --include='*.py' | sed -E 's/[[:space:]]*=.*$//' | sort -u)

	# test-045 extracts every call that names the Root Admin — by the
	# `ROOT_ADMIN_USERNAME` constant or the `"root"` literal. Comparing the
	# extracted call against the shared secret happens here because an ast-grep
	# rule cannot read the value a name is bound to.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$e2e_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no Root Admin call from %s\n' "$id" "$e2e_dir" >&2
		return 1
	fi

	while IFS=$'\t' read -r file line text; do
		[ -n "$text" ] || continue
		text="$(printf '%s' "$text" | tr -s '[:space:]' ' ')"
		for name in "${shared[@]}"; do
			secret_re="(^|[^A-Za-z0-9_])${name}([^A-Za-z0-9_]|\$)"
			if [[ $text =~ $secret_re ]]; then
				printf 'error: %s: %s:%s: the Root Admin password is the shared secret %s; obtain it through the default_password fixture\n' \
					"$id" "$file" "$line" "$name" >&2
				result=1
			fi
		done
		if printf '%s' "$text" | grep -qE '(^|[^A-Za-z0-9_])default_password([^A-Za-z0-9_]|$)'; then
			found_fixture=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, ((.range.start.line + 1) | tostring), .text] | @tsv')

	if [ "$found_fixture" -eq 0 ]; then
		printf 'error: %s: no Root Admin call obtains its password through the default_password fixture\n' "$id" >&2
		result=1
	fi

	# The fixture itself must hand back the password the container printed, not
	# the shared secret.
	if [ -f "$conftest" ]; then
		while IFS=: read -r line text; do
			[ -n "$text" ] || continue
			for name in "${shared[@]}"; do
				secret_re="(^|[^A-Za-z0-9_])${name}([^A-Za-z0-9_]|\$)"
				if [[ $text =~ $secret_re ]]; then
					printf 'error: %s: %s:%s: the default_password fixture hands back the shared secret %s; return the password the container generated\n' \
						"$id" "$conftest" "$line" "$name" >&2
					result=1
				fi
			done
		done < <(awk '
			/^def default_password\(/ { in_fixture = 1 }
			in_fixture && $0 !~ /^def default_password\(/ && $0 ~ /^[^[:space:]]/ { in_fixture = 0 }
			in_fixture { print NR ":" $0 }
		' "$conftest")
	fi

	return "$result"
}

# check_test_046 — shared Rust test helpers live only in the single
# `#[cfg(test)] mod test_helpers` in src/lib/lib.rs (TEST-046); a separate
# helper file or module is forbidden. The decisive facts are the file path, the
# number of declarations across src/ and the presence of a separate helper
# file, so the scan is pure Bash. The module/file name `test_helper` is the
# mechanical anchor of the rule's named example (`test_helper.rs`); a helper
# file under a different name is outside it.
check_test_046() {
	local id="TEST-046"
	local module="src/lib/lib.rs"
	local src_dir="src"
	local file line inline gated result=0 count=0
	local -a files=()

	if [ ! -d "$src_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$src_dir" >&2
		return 1
	fi
	if [ ! -f "$module" ]; then
		printf 'error: %s: missing %s\n' "$id" "$module" >&2
		return 1
	fi

	# Clause A: no separate helper file anywhere under src/ — the named failure
	# mode is a `test_helper.rs` beside the code under test, which is the file
	# form of the helper module.
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		printf 'error: %s: %s is a separate test-helper file; add shared helpers to the single #[cfg(test)] mod test_helpers in %s\n' \
			"$id" "$file" "$module" >&2
		result=1
	done < <(find "$src_dir" \( -name 'test_helper.rs' -o -name 'test_helpers.rs' \) | sort)

	# Clause B: every `mod test_helper`/`mod test_helpers` declaration, with the
	# file and line that hold it, whether it is inline (has a body) and whether
	# a `#[cfg(test)]` attribute gates it. The attribute-run tracking is the
	# awk idiom of check_sty_rust_077: a run continues across blank and comment
	# lines and across a multi-line attribute, and ends at the first code line.
	mapfile -t files < <(find "$src_dir" -name '*.rs' | sort)
	if [ "${#files[@]}" -eq 0 ]; then
		printf 'error: %s: no Rust file under %s\n' "$id" "$src_dir" >&2
		return 1
	fi
	while IFS=$'\t' read -r file line inline gated; do
		[ -n "$file" ] || continue
		count=$((count + 1))
		if [ "$file" != "$module" ]; then
			printf 'error: %s: %s:%s: test helpers must live only in the single #[cfg(test)] mod test_helpers in %s\n' \
				"$id" "$file" "$line" "$module" >&2
			result=1
			continue
		fi
		if [ "$inline" != "inline" ]; then
			printf 'error: %s: %s:%s: mod test_helpers must be declared inline; a file module (mod test_helpers;) is a separate helper file\n' \
				"$id" "$file" "$line" >&2
			result=1
		fi
		if [ "$gated" != "gated" ]; then
			printf 'error: %s: %s:%s: mod test_helpers must be gated by #[cfg(test)]\n' \
				"$id" "$file" "$line" >&2
			result=1
		fi
	done < <(awk '
			function is_helper(line,   t) {
				t = line
				sub(/^[[:space:]]*/, "", t)
				return t ~ /^(pub([[:space:]]*\([^)]*\))?[[:space:]]+)?mod[[:space:]]+test_helpers?([[:space:];{]|$)/
			}
			FNR == 1 { attrs = ""; attr_depth = 0 }
			attr_depth > 0 || /^[[:space:]]*#!?\[/ {
				attrs = attrs "\n" $0
				line = $0; opens = gsub(/\[/, "", line)
				line = $0; closes = gsub(/\]/, "", line)
				attr_depth += opens - closes
				if (attr_depth < 0) attr_depth = 0
				next
			}
			/^[[:space:]]*$/ || /^[[:space:]]*\/\// { next }
			{
				if (is_helper($0)) {
					code = $0
					sub(/\/\/.*$/, "", code)
					gated = (attrs ~ /cfg\([^)]*test/) ? "gated" : "ungated"
					inline = (code ~ /\{/) ? "inline" : "file"
					printf "%s\t%d\t%s\t%s\n", FILENAME, FNR, inline, gated
				}
				attrs = ""
				next
			}
		' "${files[@]}")

	if [ "$count" -eq 0 ]; then
		printf 'error: %s: %s declares no mod test_helpers; shared helpers live in the single #[cfg(test)] mod test_helpers there\n' \
			"$id" "$module" >&2
		result=1
	elif [ "$count" -gt 1 ]; then
		printf 'error: %s: found %s mod test_helpers declarations; there must be exactly one\n' \
			"$id" "$count" >&2
		result=1
	fi

	return "$result"
}

# check_test_048 — a production factory struct has no dedicated test of its own
# (TEST-048): factories only wire use cases, adapters and transactions, so the
# assembled parts are tested where the behavior lives. test-048 matches a test
# construct — a `#[cfg(test)]`/`#[cfg_attr(test, …)]` attribute, a `#[test]` (or
# `#[<path>::test]`), a `mod test`/`mod tests`, or a `mockall`/`automock` name.
# The scope is every file under src/lib/infrastructure/use_case_factory/ (the
# per-context use-case factories) plus the unit-of-work factory's file,
# src/lib/infrastructure/outbound/sqlx/unit_of_work.rs. The test doubles in
# crate::test_helpers are scaffolding for other tests and are not scanned.
check_test_048() {
	local id="TEST-048"
	local rule_id="test-048"
	local factory_dir="src/lib/infrastructure/use_case_factory"
	local unit_of_work="src/lib/infrastructure/outbound/sqlx/unit_of_work.rs"
	local output

	if [ ! -d "$factory_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$factory_dir" >&2
		return 1
	fi
	if [ ! -f "$unit_of_work" ]; then
		printf 'error: %s: missing %s\n' "$id" "$unit_of_work" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$factory_dir" "$unit_of_work" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_test_049 — an endpoint test does not assert behaviour owned by
# middleware (TEST-049): authentication, body-size limits, the HAL error
# envelope, or cross-origin handling. The project's middleware modules own
# those assertions in their own test modules, so a handler test drives the
# bare router. test-049 matches a middleware-owned surface inside a
# `#[cfg(test)]` module — the `hal_errors` envelope, the `authenticate`
# middleware, a body-limit layer, or a CORS layer/header. The handler tree is
# the scan scope because the middleware's own test modules
# (`middleware/auth.rs`, `middleware/hal_errors.rs`) name those surfaces
# legitimately; the router composition root `handler.rs` is out of scope, as
# the rule names the endpoint test.
check_test_049() {
	local id="TEST-049"
	local rule_id="test-049"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local output

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs 'src/lib/infrastructure/inbound/rest/handler/**/*.rs' \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_test_052 — a golden file or a test media file the E2E suite consumes is
# stored under test/e2e/fixture/ (TEST-052). The E2E suite is Python (TEST-039),
# so the only tracked file that may live under test/e2e/ outside the fixture
# folder is a Python test source; every other tracked file is a golden or test
# media file the suite consumes. The storage-location test is filesystem logic
# an ast-grep rule cannot see, and no existing command performs it.
check_test_052() {
	local id="TEST-052"
	local e2e_dir="test/e2e"
	local fixture_dir="test/e2e/fixture"
	local file count=0 result=0

	if [ ! -d "$e2e_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$e2e_dir" >&2
		return 1
	fi

	while IFS= read -r file; do
		[ -n "$file" ] || continue
		count=$((count + 1))
		case "$file" in
		*.py) continue ;;
		"$fixture_dir"/*) continue ;;
		esac
		printf 'error: %s: %s: a golden or test media file must be stored under %s/\n' \
			"$id" "$file" "$fixture_dir" >&2
		result=1
	done < <(git ls-files -- "$e2e_dir" | sort)

	# An empty parse means the surface moved, not that the rule holds.
	if [ "$count" -eq 0 ]; then
		printf 'error: %s: no tracked file under %s\n' "$id" "$e2e_dir" >&2
		return 1
	fi

	return "$result"
}

# check_api_001 — every REST handler is annotated with `#[utoipa::path(...)]`
# (API-001). A handler is the endpoint function of its file: the top-level
# `pub async fn` shape STY-RUST-006 recognises. The bounded-context modules in
# this tree declare their routers as `pub fn`, and their tests use private
# `async fn` inside a `#[cfg(test)]` module, so the shape names the handlers
# exactly. api-001 matches a handler whose own attribute run carries no
# `#[utoipa::path]`; the handler tree is the whole surface, so a new endpoint
# file cannot escape the check.
check_api_001() {
	local id="API-001"
	local rule_id="api-001"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local output

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$handler_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_api_002 — the specification's path items, tags and reusable schemas are
# aggregated in one struct deriving `utoipa::OpenApi` (TechnicalDesign.md § API,
# API-002). api-002 extracts that struct and its `#[openapi(...)]` attribute;
# the clause and uniqueness decisions happen here because an ast-grep rule
# cannot assert three sub-blocks inside one attribute, nor count declarations.
check_api_002() {
	local id="API-002"
	local rule_id="api-002"
	local surface="src"
	local matches aggregators aggregator_count file text result=0
	local -A aggregator_file=()
	local -A openapi_attr=()

	if [ ! -d "$surface" ]; then
		printf 'error: %s: missing %s\n' "$id" "$surface" >&2
		return 1
	fi

	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$surface" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi

	aggregators="$(printf '%s\n' "$matches" |
		jq -c 'select(.metaVariables.single.AGGREGATOR != null)')"
	aggregator_count="$(printf '%s\n' "$aggregators" | grep -c .)"
	if [ "$aggregator_count" -eq 0 ]; then
		printf 'error: %s: no struct derives utoipa::OpenApi to aggregate the path items, tags and reusable schemas\n' \
			"$id" >&2
		return 1
	fi
	if [ "$aggregator_count" -ne 1 ]; then
		printf 'error: %s: expected exactly one struct deriving utoipa::OpenApi, found %s\n' \
			"$id" "$aggregator_count" >&2
		result=1
	fi

	while IFS= read -r file; do
		[ -n "$file" ] || continue
		aggregator_file["$file"]=1
	done < <(printf '%s\n' "$aggregators" | jq -r '.file')

	while IFS=$'\t' read -r file text; do
		[ -n "$file" ] || continue
		openapi_attr["$file"]="$text"
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.AGGREGATOR == null) |
			[.file, (.text | gsub("\\s+"; " "))] | @tsv')

	for file in "${!aggregator_file[@]}"; do
		text="${openapi_attr[$file]:-}"
		if [ -z "$text" ]; then
			printf 'error: %s: %s: the utoipa::OpenApi aggregator carries no #[openapi(...)] attribute\n' \
				"$id" "$file" >&2
			result=1
			continue
		fi
		for clause in 'paths(' 'components(schemas(' 'tags('; do
			if [[ $text != *"$clause"* ]]; then
				printf 'error: %s: %s: #[openapi(...)] aggregates no %s clause\n' \
					"$id" "$file" "$clause" >&2
				result=1
			fi
		done
	done

	return "$result"
}

check_api_004() {
	local id="API-004"
	local rule_id="api-004"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local matches file line value result=0
	local -A seen=()

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	# api-004 extracts each `#[utoipa::path(...)]` attribute of the handler
	# tree; presence and uniqueness of its operation_id are compared here
	# because an ast-grep rule cannot relate two matches.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$handler_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no #[utoipa::path] attribute from %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	while IFS=$'\t' read -r file line value; do
		[ -n "$file" ] || continue
		if [ -z "$value" ]; then
			printf 'error: %s: %s:%s: #[utoipa::path] must declare a non-empty operation_id\n' \
				"$id" "$file" "$line" >&2
			result=1
			continue
		fi
		if [ -n "${seen[$value]+set}" ]; then
			printf 'error: %s: %s:%s: operation_id "%s" is already declared at %s\n' \
				"$id" "$file" "$line" "$value" "${seen[$value]}" >&2
			result=1
			continue
		fi
		seen["$value"]="${file}:${line}"
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, ((.range.start.line + 1) | tostring),
			(.text | gsub("\\s+"; " ") |
				capture("operation_id *= *\"(?<op>[^\"]*)\"")? | .op // "")] | @tsv')

	return "$result"
}

# check_api_005 — every endpoint handler's `#[utoipa::path(...)]` declares the
# HTTP method (`get`/`post`/`put`/`patch`/`delete`) as the bare method token
# (TechnicalDesign.md API-005). The rule is negative: it matches an annotation
# that carries none of the five method identifiers. Test modules are excluded,
# as in STY-RUST-007. Config carries no data, so no rule_enforcer.yaml block.
check_api_005() {
	local id="API-005"
	local rule_id="api-005"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local output

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$handler_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_api_006 — every endpoint handler annotation declares `path`, the route
# of the endpoint (API-006). Handlers live under the REST handler directory and
# are documented inline with `#[utoipa::path(...)]`; api-006 flags an annotation
# whose argument list carries no `path = "..."` item.
check_api_006() {
	local id="API-006"
	local rule_id="api-006"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local output

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$handler_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_api_007 — every endpoint's `#[utoipa::path]` attribute declares `tag`,
# the bounded context the endpoint belongs to (API-007). The tag is the
# endpoint's bounded-context folder under the handler tree, or the reserved
# `system` tag for a system-level endpoint — a file directly under the handler
# tree, e.g. get_health.rs. The path-to-context pairing and the value
# comparison happen here because an ast-grep rule cannot see the file it is
# matching in, nor read a value back out of the attribute.
check_api_007() {
	local id="API-007"
	local rule_id="api-007"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local matches file text tag rel expected result=0

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	# api-007 extracts each endpoint's `#[utoipa::path]` attribute; the
	# attribute text carries the declared keys.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$handler_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no endpoint from %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	while IFS=$'\t' read -r file text; do
		[ -n "$file" ] || continue

		# The `tag` key is matched at a key boundary (start of the collapsed
		# attribute, `(` or `,`) so a literal `tag` inside a description is not
		# mistaken for the key.
		tag="$(printf '%s\n' "$text" |
			grep -oE '(^|[(,][[:space:]]*)tag[[:space:]]*=[[:space:]]*"[^"]*"' |
			head -n1 | sed -E 's/.*"([^"]*)"$/\1/')"

		# The endpoint's bounded context is its folder; a file directly under
		# the handler tree is the system-level endpoint and carries `system`.
		rel="${file#"$handler_dir"/}"
		case "$rel" in
		*/*) expected="${rel%%/*}" ;;
		*) expected="system" ;;
		esac

		if [ -z "$tag" ]; then
			printf 'error: %s: %s: endpoint declares no tag; declare tag = "%s" (its bounded context) or tag = "system"\n' \
				"$id" "$file" "$expected" >&2
			result=1
		elif [ "$tag" != "$expected" ]; then
			printf 'error: %s: %s: tag "%s" is not the endpoint'"'"'s bounded context; expected "%s"\n' \
				"$id" "$file" "$tag" "$expected" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, (.text | gsub("\\s+"; " "))] | @tsv')

	return "$result"
}

# check_api_008 — an endpoint whose handler extracts a request body declares
# `request_body` in its `#[utoipa::path(...)]` annotation (TechnicalDesign.md
# § API, API-008). A handler carries a body when it takes a body-consuming axum
# extractor parameter (`Json<T>`, `Form<T>`, `Bytes`, `RawBody`, `Multipart`);
# `api-008` matches such a handler whose own `#[utoipa::path]` attribute carries
# no `request_body`. The handler tree is the whole surface, so a new endpoint
# file cannot escape the check.
check_api_008() {
	local id="API-008"
	local rule_id="api-008"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local output

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$handler_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_api_009 — an endpoint that carries a path, query, header or cookie
# parameter declares it in `params(...)` inside its `#[utoipa::path(...)]`
# (TechnicalDesign.md § API, API-009). api-009 matches a `#[utoipa::path]`
# annotation that omits `params(...)` while the route template names a path
# parameter (`{name}`) or the handler signature extracts one through
# `Path`/`Query`/`HeaderMap`/`TypedHeader`/`Cookie`. The handler tree is the
# whole surface, so a new endpoint file cannot escape the check.
check_api_009() {
	local id="API-009"
	local rule_id="api-009"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local output

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$handler_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_api_010 — every endpoint handler annotation declares `responses(...)`
# with at least one status row and at most one row per status code
# (TechnicalDesign.md § API, API-010). api-010 extracts each
# `#[utoipa::path(...)]` attribute of the handler tree; the presence of
# `responses(...)` and the one-row-per-status-code comparison happen here
# because an ast-grep rule cannot relate two matches inside one attribute token
# tree. The handler tree is the whole surface, so a new endpoint file cannot
# escape the check.
check_api_010() {
	local id="API-010"
	local rule_id="api-010"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local matches file line block body statuses dups result=0

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$handler_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no #[utoipa::path] attribute from %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	while IFS=$'\t' read -r file line block; do
		[ -n "$file" ] || continue
		if ! printf '%s' "$block" | grep -qE '\bresponses\s*\('; then
			printf 'error: %s: %s:%s: #[utoipa::path] must declare responses(...) with every possible response\n' \
				"$id" "$file" "$line" >&2
			result=1
			continue
		fi
		# The argument of `responses(...)`, string literals left intact and
		# their parentheses (a description such as "note (draft)") counted as
		# text, not structure.
		body="$(printf '%s\n' "$block" | awk '
			{
				line = $0
				pos = index(line, "responses(")
				if (pos == 0) next
				rest = substr(line, pos + length("responses("))
				n = length(rest)
				depth = 1
				in_string = 0
				out = ""
				for (i = 1; i <= n; i++) {
					c = substr(rest, i, 1)
					if (in_string) {
						out = out c
						if (c == "\\") { i++; out = out substr(rest, i, 1); continue }
						if (c == "\"") in_string = 0
						continue
					}
					if (c == "\"") { in_string = 1; out = out c; continue }
					if (c == "(") depth++
					if (c == ")") { depth--; if (depth == 0) break }
					out = out c
				}
				printf "%s\n", out
			}
		')"
		statuses="$({
			# Quoted status codes: `status = "200"`.
			printf '%s\n' "$body" |
				grep -oE 'status[[:space:]]*=[[:space:]]*"[^"]*"' |
				sed -E 's/status[[:space:]]*=[[:space:]]*//; s/^"//; s/"$//'
			# Bare status codes, with every string literal blanked so prose
			# ("description = \"status = OK\"") cannot look like a row.
			printf '%s\n' "$body" |
				sed -E 's/"[^"]*"/#/g' |
				grep -oE 'status[[:space:]]*=[[:space:]]*[0-9A-Za-z_]+' |
				sed -E 's/status[[:space:]]*=[[:space:]]*//'
		} | grep -v '^$')"
		if [ -z "$statuses" ]; then
			printf 'error: %s: %s:%s: responses(...) declares no status row\n' \
				"$id" "$file" "$line" >&2
			result=1
			continue
		fi
		dups="$(printf '%s\n' "$statuses" | sort | uniq -d | paste -sd, -)"
		if [ -n "$dups" ]; then
			printf 'error: %s: %s:%s: responses(...) declares more than one row for status code(s): %s\n' \
				"$id" "$file" "$line" "$dups" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, ((.range.start.line + 1) | tostring),
			(.text | gsub("\\s+"; " "))] | @tsv')

	return "$result"
}

# check_api_011 — every operation declares its security requirement (API-011).
# api-011 reports an `#[utoipa::path(...)]` attribute whose `security` is absent
# or is not one of the two sanctioned requirements. The declaration-vs-router
# correspondence in the rule's second sentence spans the attribute and the
# router builder, which ast-grep cannot relate, so it stays for review.
check_api_011() {
	local id="API-011"
	local rule_id="api-011"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local output

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$handler_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_api_012 — every endpoint handler declares `summary`, a one-line
# human-readable summary, in its `#[utoipa::path(...)]` annotation
# (TechnicalDesign.md § API, API-012). api-012 matches a `#[utoipa::path]`
# attribute that carries no top-level `summary` key. The handler tree is the
# whole surface, so a new endpoint file cannot escape the check.
check_api_012() {
	local id="API-012"
	local rule_id="api-012"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local output

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$handler_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_api_013 — every parameter declared as an inline tuple inside
# `params(...)` in a `#[utoipa::path(...)]` attribute carries `name`, the
# parameter name as it appears in the URL, header or cookie (TechnicalDesign.md
# § API, API-013). api-013 matches a parameter tuple whose first element is not
# a string literal; `params(<IntoParams>)` names its parameters through the
# struct's fields and is not an inline tuple. The handler tree is the whole
# surface, so a new endpoint file cannot escape the check.
check_api_013() {
	local id="API-013"
	local rule_id="api-013"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local output

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$handler_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_api_014 — every inline parameter declared by a `#[utoipa::path(...)]`
# annotation names its location (API-014). An inline parameter is the tuple
# `("<name>" [= Type], <in>, ...)` and `in` is its second element; utoipa
# defaults a missing one to `Path` (utoipa-gen `ParameterIn::default`), so the
# compiler accepts a declaration the rule rejects. The annotation body is a
# token tree ast-grep cannot see into, so rule/api-014.yml anchors the
# `params(...)` token tree and the tuples are read here. A `params(<Query>)`
# reference to an `IntoParams` struct is out of scope: it declares no inline
# tuple, and § 2's worked example requires no `parameter_in` on the struct.
check_api_014() {
	local id="API-014"
	local rule_id="api-014"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local matches file line b64 text name location tuples=0 result=0

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$handler_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no params(...) declaration from %s\n' \
			"$id" "$handler_dir" >&2
		return 1
	fi

	while IFS=$'\t' read -r file line b64; do
		[ -n "$b64" ] || continue
		text="$(printf '%s' "$b64" | base64 -d)"
		while IFS=$'\t' read -r name location; do
			[ -n "$name" ] || continue
			tuples=$((tuples + 1))
			case "$location" in
			Path | Query | Header | Cookie) ;;
			*)
				printf 'error: %s: %s:%s: parameter %s must declare its location as Path, Query, Header or Cookie (found: %s)\n' \
					"$id" "$file" "$line" "$name" "${location:-none}" >&2
				result=1
				;;
			esac
		done < <(printf '%s\n' "$text" | awk '
			function trim(s) { gsub(/^[[:space:]]+|[[:space:]]+$/, "", s); return s }

			# split_top <s> <out> — split on commas outside any nesting or string
			# literal; returns the element count.
			function split_top(s, out,   i, n, c, depth, cur, str, k) {
				n = length(s); depth = 0; cur = ""; str = 0; k = 0
				for (i = 1; i <= n; i++) {
					c = substr(s, i, 1)
					if (str) {
						cur = cur c
						if (c == "\\") { i++; cur = cur substr(s, i, 1) }
						else if (c == "\"") str = 0
						continue
					}
					if (c == "\"") { str = 1; cur = cur c; continue }
					if (c == "(" || c == "[" || c == "{") { depth++; cur = cur c; continue }
					if (c == ")" || c == "]" || c == "}") { depth--; cur = cur c; continue }
					if (c == "," && depth == 0) { k++; out[k] = cur; cur = ""; continue }
					cur = cur c
				}
				k++; out[k] = cur
				return k
			}

			{ text = text $0 "\n" }

			END {
				gsub(/^[[:space:]]+|[[:space:]]+$/, "", text)
				body = substr(text, 2, length(text) - 2)
				n = split_top(body, element)
				for (i = 1; i <= n; i++) {
					el = trim(element[i])
					if (el == "" || substr(el, 1, 1) != "(") continue
					m = split_top(substr(el, 2, length(el) - 2), part)
					name = trim(part[1])
					if (name ~ /^"/) { sub(/^"/, "", name); sub(/".*$/, "", name) }
					location = (m >= 2) ? trim(part[2]) : ""
					printf "%s\t%s\n", name, location
				}
			}
		')
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, ((.range.start.line + 1) | tostring), (.text | @base64)] | @tsv')

	if [ "$tuples" -eq 0 ]; then
		printf 'error: %s: extracted no inline parameter from %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	return "$result"
}

# check_api_015 — an inline parameter in `params(...)` writes its Rust type as
# `= Type` immediately after the name: the binding is introduced by `=`, never
# by `:` or a bare element (TechnicalDesign.md § API, API-015). api-015 matches
# a parameter tuple whose quoted name is followed by anything other than `=` or
# the element separator, so a type written some other way is reported. The type
# statement is optional (utoipa resolves an untyped parameter), so a tuple that
# binds no type is not a violation. The handler tree is the whole surface, so a
# new endpoint file cannot escape the check.
check_api_015() {
	local id="API-015"
	local rule_id="api-015"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local output

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$handler_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

check_api_016() {
	local id="API-016"
	local rule_id="api-016"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_api_020 — a `request_body(...)` block in a `#[utoipa::path(...)]`
# annotation declares `content_type`, the media type of the body
# (TechnicalDesign.md § API, API-020). api-020 matches a request_body token
# tree that carries no `content_type`; the media type must be read from the
# request body, never from a `responses(...)` row. The documented shorthand
# `request_body = <Type>` carries utoipa's default application/json media type
# (§ 1's complete-declaration example uses it) and declares no block, so it is
# out of scope. The handler tree is the whole surface, so a new endpoint file
# cannot escape the check.
check_api_020() {
	local id="API-020"
	local rule_id="api-020"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local output

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$handler_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_api_021 — a `request_body(...)` declaration names the schema of the
# body (TechnicalDesign.md § API, API-021). `api-021` matches the argument group
# of a `request_body(...)` that declares no `content` attribute, and a
# `content(...)` entry that names a media type but no schema; the
# `request_body = <Type>` shorthand names the schema directly and is not
# matched. The handler tree is the whole surface, so a new endpoint file cannot
# escape the check.
check_api_021() {
	local id="API-021"
	local rule_id="api-021"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local output

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$handler_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_api_023 — every response declared inside a handler's `responses(...)`
# must name its HTTP `status` (API-023). The ast-grep rule matches a response
# group — the bracketed token tree that is a direct child of the
# `responses(...)` group and that carries no `status =` key — so the wrapper
# only selects the rule and the surface.
check_api_023() {
	local id="API-023"
	local rule_id="api-023"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_api_024 — every response declared by a `responses(...)` entry in a
# `#[utoipa::path(...)]` annotation carries `description`, a human-readable
# description (TechnicalDesign.md § API, API-024). api-024 matches a response
# entry token tree that declares no top-level `description`; a `description`
# nested in `headers(...)`/`content(...)` or a `"description"` string inside an
# `example` does not satisfy it. The handler tree is the whole surface, so a new
# endpoint file cannot escape the check.
check_api_024() {
	local id="API-024"
	local rule_id="api-024"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local output

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$handler_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_api_025 — a response row that declares a payload media type declares the
# schema of that payload (TechnicalDesign.md § API, API-025). api-025 matches a
# row inside a `responses(...)` group that carries a `content_type` but no
# `body`; a row without a media type (a `NO_CONTENT` response) carries no
# payload and is out of scope. The handler tree is the whole surface, so a new
# endpoint file cannot escape the check.
check_api_025() {
	local id="API-025"
	local rule_id="api-025"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local output

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$handler_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_api_026 — a response in a `#[utoipa::path(...)]` annotation that
# declares a payload declares the payload media type (TechnicalDesign.md § API,
# API-026). api-026 matches a response row of `responses(...)` that carries a
# `body` identifier and no `content_type` identifier; a bodyless row, and a
# `content(...)` entry that names its media type as the entry key, are not
# matched. The handler tree is the whole surface, so a new endpoint file cannot
# escape the check.
check_api_026() {
	local id="API-026"
	local rule_id="api-026"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local output

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$handler_dir" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_api_027 — every response header a handler sets at runtime is declared in
# the endpoint's `#[utoipa::path]` `headers(...)` (TechnicalDesign.md § API,
# API-027). api-027 extracts each `#[utoipa::path]` attribute and each
# response-header insert (`…headers_mut().insert(header::CONST, …)` /
# `headers.insert(header::CONST, …)`); pairing the constant with the name a
# `headers(...)` block declares happens here because an ast-grep rule cannot
# relate two matches, and because the names are compared case-insensitively
# (HTTP header names are). `Content-Type` is expressed through `content_type =`
# (API-026), not `headers(...)`, so it is not required here. The handler tree is
# the whole surface, so a new endpoint file cannot escape the check.
check_api_027() {
	local id="API-027"
	local rule_id="api-027"
	local handler_dir="${1:-src/lib/infrastructure/inbound/rest/handler}"
	local matches file text const name names result=0
	local -A declared_by=()

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$handler_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no #[utoipa::path] or response-header insert from %s\n' \
			"$id" "$handler_dir" >&2
		return 1
	fi

	# The header names each file declares inside a `headers(...)` block. Each
	# block's argument is spliced with a paren-aware, string-aware scan so a `)`
	# inside a description cannot truncate it.
	while IFS=$'\t' read -r file names; do
		[ -n "$file" ] || continue
		declared_by["$file"]="$names"
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.CONST | not) | [.file, .text] | @tsv' |
		while IFS=$'\t' read -r file text; do
			names="$(printf '%s' "$text" | awk '
				{ text = text $0 "\n" }
				END {
					n = length(text)
					i = 1
					while (i <= n) {
						p = index(substr(text, i), "headers(")
						if (p == 0) break
						j = i + p - 1 + length("headers(")
						depth = 1
						in_string = 0
						out = ""
						while (j <= n && depth > 0) {
							c = substr(text, j, 1)
							if (in_string) {
								out = out c
								if (c == "\\") { j++; out = out substr(text, j, 1) }
								else if (c == "\"") in_string = 0
								j++
								continue
							}
							if (c == "\"") { in_string = 1; out = out c; j++; continue }
							if (c == "(") { depth++; out = out c; j++; continue }
							if (c == ")") { depth--; if (depth == 0) break; out = out c; j++; continue }
							out = out c
							j++
						}
						printf "%s\n", out
						i = j + 1
					}
				}
			' |
				grep -oE '"[A-Za-z][A-Za-z0-9-]*"[[:space:]]*=' |
				sed -E 's/^"//; s/"[[:space:]]*=.*$//' |
				tr '[:upper:]' '[:lower:]' | sort -u | paste -sd, -)"
			printf '%s\t%s\n' "$file" "$names"
		done)

	# Every non-Content-Type header a handler inserts into a response must be
	# declared. Both sides are lowercased before the comparison because HTTP
	# header names are case-insensitive.
	while IFS=$'\t' read -r file const; do
		[ -n "$const" ] || continue
		name="$(printf '%s' "$const" | tr '[:upper:]_' '[:lower:]-')"
		[ "$name" = "content-type" ] && continue
		case ",${declared_by[$file]:-}," in
		*",$name,"*) ;;
		*)
			printf 'error: %s: %s: the handler sets the %s response header but the endpoint declares no headers(...) entry for it\n' \
				"$id" "$file" "$const" >&2
			result=1
			;;
		esac
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.CONST) | [.file, .metaVariables.single.CONST.text] | @tsv')

	return "$result"
}

# check_api_030 — every handler's `responses(...)` declares the success case and
# every error status the handler can return (TechnicalDesign.md § API, API-030).
# api-030 extracts each `#[utoipa::path]` attribute, each `impl … for ApiError`
# mapping and each `ApiError::Variant` value a handler constructs; the
# variant-to-status table is read from `api_error.rs`'s exhaustive
# `status_and_message` match, and the per-file comparison happens here because
# an ast-grep rule cannot relate two matches or read a second file. Extractor
# rejections (400/413/415/422/500) and the authentication middleware (401) are
# supplied by layer code, not the handler body, and stay a review residual, as
# does a handler that reuses a `From<…Error> for ApiError` impl another file
# declares. The handler tree is the whole surface, so a new endpoint file cannot
# escape the check.
check_api_030() {
	local id="API-030"
	local rule_id="api-030"
	local handler_dir="src/lib/infrastructure/inbound/rest/handler"
	local api_error="src/lib/infrastructure/inbound/rest/api_error.rs"
	local matches file payload kind variant status declared result=0
	local -A variant_status=()
	local -A declared_by=()
	local -A required_by=()

	if [ ! -d "$handler_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$handler_dir" >&2
		return 1
	fi
	if [ ! -f "$api_error" ]; then
		printf 'error: %s: missing %s\n' "$id" "$api_error" >&2
		return 1
	fi

	# The ApiError variant -> status table, read from the exhaustive
	# `status_and_message` match rather than restated here.
	while IFS=$'\t' read -r variant status; do
		[ -n "$variant" ] || continue
		variant_status["$variant"]="$status"
	done < <(tr '\n' ' ' <"$api_error" |
		grep -oE 'Self::[A-Za-z][A-Za-z0-9_]*\(?[^)]*\)? *(,|)=> *\(? *StatusCode::[A-Z_]+' |
		sed -E 's/^Self::([A-Za-z0-9_]+).*StatusCode::([A-Z_]+).*/\1\t\2/' | sort -u)

	if [ "${#variant_status[@]}" -eq 0 ]; then
		printf 'error: %s: extracted no ApiError status mapping from %s\n' "$id" "$api_error" >&2
		return 2
	fi

	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$handler_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no #[utoipa::path] attribute from %s\n' "$id" "$handler_dir" >&2
		return 1
	fi

	# Classify each match: the annotation's declared statuses, the variants an
	# `impl … for ApiError` maps, and the variants the body constructs directly.
	while IFS=$'\t' read -r kind file payload; do
		[ -n "$file" ] || continue
		case "$kind" in
		attr)
			while IFS= read -r status; do
				[ -n "$status" ] || continue
				declared_by["$file"]+=" $status"
			done < <(printf '%s' "$payload" |
				sed -E 's/"[^"]*"/#/g' |
				grep -oE 'status[[:space:]]*=[[:space:]]*[A-Z][A-Z0-9_]*' |
				sed -E 's/.*=[[:space:]]*//')
			;;
		impl)
			while IFS= read -r variant; do
				[ -n "$variant" ] || continue
				required_by["$file"]+=" $variant"
			done < <(printf '%s' "$payload" |
				grep -oE 'Self::[A-Z][A-Za-z0-9_]*' | sed 's/^Self:://')
			;;
		direct)
			case "$payload" in
			[A-Z]*) required_by["$file"]+=" $payload" ;;
			esac
			;;
		esac
	done < <(printf '%s\n' "$matches" | jq -r '
		if .metaVariables.single.VARIANT then
			["direct", .file, .metaVariables.single.VARIANT.text] | @tsv
		elif (.text | startswith("#[")) then
			["attr", .file, (.text | gsub("\\s+"; " "))] | @tsv
		else
			["impl", .file, (.text | gsub("\\s+"; " "))] | @tsv
		end')

	for file in "${!declared_by[@]}"; do
		declared=" ${declared_by[$file]} "
		for variant in ${required_by[$file]:-}; do
			status="${variant_status[$variant]:-}"
			if [ -z "$status" ]; then
				printf 'error: %s: %s: ApiError::%s has no status mapping in %s\n' \
					"$id" "$file" "$variant" "$api_error" >&2
				result=1
				continue
			fi
			case "$declared" in
			*" $status "*) ;;
			*)
				printf 'error: %s: %s: the handler returns ApiError::%s (%s) but responses(...) declares no %s row\n' \
					"$id" "$file" "$variant" "$status" "$status" >&2
				result=1
				;;
			esac
		done
		if ! printf '%s\n' "$declared" |
			grep -qE ' (OK|CREATED|ACCEPTED|NO_CONTENT|PARTIAL_CONTENT|20[0-9]) '; then
			printf 'error: %s: %s: responses(...) declares no success (2xx) row\n' \
				"$id" "$file" >&2
			result=1
		fi
	done

	return "$result"
}

check_api_031() {
	local id="API-031"
	local rule_id="api-031"
	local config matches body file line result=0
	local -A non_hal=()

	config="$(rule_config "$id")" || return 2

	# The bodies that are not HAL responses — token issues and liveness probes —
	# keep their plain `application/json` representation (§ HAL payload
	# guidelines). Every other response body is a resource representation or the
	# error envelope and must be declared `application/hal+json` (`API-031`).
	# The list is data: a new non-representation body is added here, not in the
	# ast-grep rule.
	while IFS= read -r body; do
		[ -n "$body" ] || continue
		non_hal["$body"]=1
	done < <(jq -r '.non_hal_bodies[]?' <<<"$config")

	if [ "${#non_hal[@]}" -eq 0 ]; then
		printf 'error: %s: rule_enforcer.yaml declares no non_hal_bodies\n' "$id" >&2
		return 2
	fi

	# api-031 extracts every response entry that carries a body but does not
	# declare the HAL media type; excluding the allowlisted non-HAL bodies
	# happens here because an ast-grep rule cannot read rule_enforcer.yaml.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	[ -n "$matches" ] || return 0

	while IFS=$'\t' read -r file line body; do
		[ -n "$file" ] || continue
		if [ -z "${non_hal[$body]+set}" ]; then
			printf 'error: %s: %s:%s: response body %s is not served as application/hal+json; declare content_type = "application/hal+json" or list %s in non_hal_bodies\n' \
				"$id" "$file" "$line" "$body" "$body" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, ((.range.start.line + 1) | tostring),
			(.text | capture("\\bbody\\s*=\\s*(?<b>[A-Za-z_][A-Za-z0-9_]*)")?.b // "?")] | @tsv')

	return "$result"
}

# check_api_032 — every resource representation carries `_links.self`
# (TechnicalDesign.md § HAL payload guidelines, API-032). api-032 extracts each
# response row of a `#[utoipa::path]` annotation and each serde-renamed `_links`
# or `self` struct field. Pairing a representation with its `_links` type, and a
# `_links` type with its `self` member, happens here because an ast-grep rule
# cannot relate two matches. A representation is a `responses(...)` row served
# as `application/hal+json` that names a body other than the error envelope;
# token issues and probes (plain `application/json`) and `204` rows (no body)
# are not representations and are out of scope. The rest tree is the whole
# surface, so a new representation file cannot escape the check.
check_api_032() {
	local id="API-032"
	local rule_id="api-032"
	local target="${1:-src/lib/infrastructure/inbound/rest}"
	local matches file text rep rename ty body result=0
	local -A links_type=()
	local -A has_self=()
	local -A is_rep=()

	if [ ! -d "$target" ]; then
		printf 'error: %s: missing %s\n' "$id" "$target" >&2
		return 1
	fi

	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$target" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no HAL response row or serde-renamed field from %s\n' \
			"$id" "$target" >&2
		return 1
	fi

	while IFS=$'\t' read -r rep rename ty; do
		[ -n "$rep" ] || continue
		if [ "$rename" = '"self"' ]; then
			has_self["$rep"]=1
		else
			links_type["$rep"]="$ty"
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.REP) |
			[.metaVariables.single.REP.text,
			 .metaVariables.single.RENAME.text,
			 .metaVariables.single.TYPE.text] | @tsv')

	while IFS=$'\t' read -r file text; do
		[ -n "$file" ] || continue
		printf '%s' "$text" | grep -q 'application/hal+json' || continue
		body="$(printf '%s' "$text" |
			sed -nE 's/.*[^_a-z]body = ([A-Za-z0-9_]+).*/\1/p')"
		[ -n "$body" ] || continue
		[ "$body" = "ErrorBody" ] && continue
		is_rep["$body"]=1
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.REP | not) |
			[.file, (.text | gsub("\\s+"; " "))] | @tsv')

	for rep in "${!is_rep[@]}"; do
		if [ -z "${links_type[$rep]+set}" ]; then
			printf 'error: %s: representation %s declares no _links field\n' \
				"$id" "$rep" >&2
			result=1
		fi
	done

	for rep in "${!links_type[@]}"; do
		ty="${links_type[$rep]}"
		if [ -z "${has_self[$ty]+set}" ]; then
			printf 'error: %s: %s._links (%s) declares no self member\n' \
				"$id" "$rep" "$ty" >&2
			result=1
		fi
	done

	return "$result"
}

# check_api_033 — a HAL representation keeps its business data at the root of the
# payload, beside `_links` (TechnicalDesign.md § API, API-033; detail § HAL
# payload guidelines). A representation is the Rust struct serialized at the JSON
# root; its `#[serde(rename = "_links")]` field is the marker. api-033 extracts
# each such struct and this function reads its serialized root field names,
# rejecting a key that is a wrapper (the envelope the rule forbids) rather than a
# business field. The wrapper keys are data in rule_enforcer.yaml, so a new
# envelope key is added there, never by editing this function. The rest surface
# (the shared HAL module, the error envelope and the handler tree) is the whole
# payload surface, so a new representation cannot escape the check.
check_api_033() {
	local id="API-033"
	local rule_id="api-033"
	local surface="${1:-src/lib/infrastructure/inbound/rest}"
	local config file line b64 text key name result=0
	local -a wrapper_keys=()

	if [ ! -d "$surface" ]; then
		printf 'error: %s: missing %s\n' "$id" "$surface" >&2
		return 1
	fi

	config="$(rule_config "$id")" || return 2
	while IFS= read -r key; do
		[ -n "$key" ] || continue
		wrapper_keys+=("$key")
	done < <(jq -r '.wrapper_keys[]?' <<<"$config")
	if [ "${#wrapper_keys[@]}" -eq 0 ]; then
		printf 'error: %s: rule_enforcer.yaml declares no wrapper_keys\n' "$id" >&2
		return 2
	fi

	# api-033 extracts each HAL representation struct; the wrapper-key comparison
	# happens here because an ast-grep rule cannot read rule_enforcer.yaml.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$surface" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no HAL representation struct from %s\n' "$id" "$surface" >&2
		return 1
	fi

	while IFS=$'\t' read -r file line b64; do
		[ -n "$b64" ] || continue
		text="$(printf '%s' "$b64" | base64 -d)"
		while IFS= read -r name; do
			[ -n "$name" ] || continue
			for key in "${wrapper_keys[@]}"; do
				if [ "$name" = "$key" ]; then
					printf 'error: %s: %s:%s: HAL representation nests its business data under %s; keep business data at the root beside _links\n' \
						"$id" "$file" "$line" "$name" >&2
					result=1
				fi
			done
		done < <(printf '%s\n' "$text" | awk '
			{
				line = $0
				sub(/^[[:space:]]+/, "", line)
				if (line ~ /^#/) {
					if (match(line, /rename[[:space:]]*=[[:space:]]*"[^"]*"/)) {
						r = substr(line, RSTART, RLENGTH)
						sub(/^rename[[:space:]]*=[[:space:]]*"/, "", r)
						sub(/"$/, "", r)
						rename = r
					}
					next
				}
				n = line
				sub(/^pub([[:space:]]*\([^)]*\))?[[:space:]]+/, "", n)
				if (n ~ /^[a-z_][A-Za-z0-9_]*[[:space:]]*:/) {
					name = n
					sub(/[[:space:]]*:.*/, "", name)
					print (rename != "" ? rename : name)
					rename = ""
				}
			}
		')
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, ((.range.start.line + 1) | tostring), (.text | @base64)] | @tsv')

	return "$result"
}

# check_api_034 — `_links` is reserved for navigation and related resources
# (TechnicalDesign.md § API, API-034; detail § HAL payload guidelines). A HAL
# representation carries links under `_links`; the field is marked by
# `#[serde(rename = "_links")]` and its type holds one HAL link per relation.
# api-034 extracts each `_links` field with its type and every struct field with
# its owner and type; resolving a `_links` type to the members it holds happens
# here because an ast-grep rule cannot relate two matches. A member that is not a
# HAL `Link` (or an `Option<Link>`) is non-link data placed under `_links`. The
# HAL surface (hal.rs and the handler tree) is the whole payload surface, so a
# new representation cannot escape the check.
check_api_034() {
	local id="API-034"
	local rule_id="api-034"
	local surface="${1:-src/lib/infrastructure/inbound/rest}"
	local matches rep ty member result=0
	local -A links_type=()
	local -A members=()

	if [ ! -d "$surface" ]; then
		printf 'error: %s: missing %s\n' "$id" "$surface" >&2
		return 1
	fi

	# api-034 extracts each `_links` field and every struct field; the
	# type-to-members resolution happens here because an ast-grep rule cannot
	# relate two matches.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$surface" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no HAL links field from %s\n' "$id" "$surface" >&2
		return 1
	fi

	while IFS=$'\t' read -r rep ty; do
		[ -n "$rep" ] || continue
		links_type["$rep"]="$ty"
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.REP) |
			[.metaVariables.single.REP.text, .metaVariables.single.TYPE.text] | @tsv')

	while IFS=$'\t' read -r rep ty; do
		[ -n "$rep" ] || continue
		members["$rep"]+="${ty}"$'\n'
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.STRUCT) |
			[.metaVariables.single.STRUCT.text, .metaVariables.single.TYPE.text] | @tsv')

	if [ "${#links_type[@]}" -eq 0 ]; then
		printf 'error: %s: extracted no _links field from %s\n' "$id" "$surface" >&2
		return 1
	fi

	for rep in "${!links_type[@]}"; do
		ty="${links_type[$rep]}"
		if [ -z "${members[$ty]+set}" ]; then
			printf 'error: %s: %s._links (%s) is not a HAL links struct; reserve _links for navigation and related resources\n' \
				"$id" "$rep" "$ty" >&2
			result=1
			continue
		fi
		while IFS= read -r member; do
			[ -n "$member" ] || continue
			case "$member" in
			Link | 'Option<Link>') ;;
			*)
				printf 'error: %s: %s._links (%s) carries a %s member; reserve _links for navigation and related resources (a HAL Link)\n' \
					"$id" "$rep" "$ty" "$member" >&2
				result=1
				;;
			esac
		done <<<"${members[$ty]}"
	done

	return "$result"
}

# check_api_035 — a HAL payload does not use `_embedded` (TechnicalDesign.md
# § API, API-035; detail § HAL payload guidelines). `_embedded` is HAL's nesting
# key; this API keeps business data at the representation root (API-033) and
# exposes related resources through `_links` (API-034), so an `_embedded` is a
# deliberate omission. api-035 extracts every `_embedded` use in the rest
# surface — a serde-renamed field, a bare field, or a `json!` key — with its
# enclosing struct or function, and excludes `#[cfg(test)]` items. A use is
# permitted only at a location listed in allowed_locations, the record of a
# proven performance need; the list is empty by default, so every use is
# reported until it is reviewed. The comparison happens here because an
# ast-grep rule cannot read rule_enforcer.yaml.
check_api_035() {
	local id="API-035"
	local rule_id="api-035"
	local surface="${1:-src/lib/infrastructure/inbound/rest}"
	local config matches file line item location result=0
	local -A allowed=()

	if [ ! -d "$surface" ]; then
		printf 'error: %s: missing %s\n' "$id" "$surface" >&2
		return 1
	fi

	config="$(rule_config "$id")" || return 2
	while IFS= read -r location; do
		[ -n "$location" ] || continue
		allowed["$location"]=1
	done < <(jq -r '.allowed_locations[]?' <<<"$config")

	# api-035 extracts every `_embedded` use together with its enclosing struct
	# or function; the waiver comparison happens here because an ast-grep rule
	# cannot read rule_enforcer.yaml. An empty match set is the passing state: a
	# payload that never uses `_embedded` satisfies the rule.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$surface" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	[ -n "$matches" ] || return 0

	while IFS=$'\t' read -r file line item; do
		[ -n "$file" ] || continue
		if [ -n "$item" ]; then
			location="${file}::${item}"
		else
			location="$file"
		fi
		if [ -z "${allowed[$location]+set}" ]; then
			printf 'error: %s: %s:%s: HAL payload uses _embedded; keep business data at the root and expose related resources through _links, or record a proven performance need by adding "%s" to allowed_locations\n' \
				"$id" "$file" "$line" "$location" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" | jq -r \
		'[.file, ((.range.start.line + 1) | tostring),
			(.metaVariables.single.STRUCT.text // .metaVariables.single.FUNC.text // "")] | @tsv')

	return "$result"
}

# check_api_036 — a link that exposes search and filtering is a URI template and
# carries `templated: true` (TechnicalDesign.md § API, API-036; detail § HAL
# payload guidelines, "Search and filtering"). api-036 matches two violation
# shapes: a URI-template href built with `Link::new` (which serializes no
# `templated`), and the `search` link built with `Link::new` (the field that
# exposes the collection filters). Building either with `Link::templated` sets
# `templated: Some(true)`, which is the rule's prescribed form. The rest surface
# is the whole HAL link surface, so a new representation cannot escape the
# check.
check_api_036() {
	local id="API-036"
	local rule_id="api-036"
	local surface="${1:-src/lib/infrastructure/inbound/rest}"
	local output

	if [ ! -d "$surface" ]; then
		printf 'error: %s: missing %s\n' "$id" "$surface" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$surface" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}

# check_api_037 — every HAL link href is a root-relative URI reference, never a
# hardcoded absolute URL (TechnicalDesign.md § API, API-037; detail § HAL
# payload guidelines, which shows `self` = `/orders/123` and the search link as
# `/orders{?status,page,size}`). api-037 extracts each HAL link constructor
# (`Link::new`, `Link::templated`, `SelfLinks::new`) and each href helper
# (`fn *_href`), and this function reads the href literal each carries, flagging
# a URL that hardcodes an origin (`scheme://`) or that is not root-relative (so
# the client would have to construct the URL itself). The literal extraction and
# the reference comparison happen here because an ast-grep rule cannot read the
# leading literal of a format template.
check_api_037() {
	local id="API-037"
	local rule_id="api-037"
	local surface="${1:-src/lib/infrastructure/inbound/rest}"
	local matches file line b64 text href result=0 count=0

	if [ ! -d "$surface" ]; then
		printf 'error: %s: missing %s\n' "$id" "$surface" >&2
		return 1
	fi

	# api-037 extracts each HAL link constructor and each href helper.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$surface" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no HAL link or href helper from %s\n' "$id" "$surface" >&2
		return 1
	fi

	while IFS=$'\t' read -r file line b64; do
		[ -n "$b64" ] || continue
		text="$(printf '%s' "$b64" | base64 -d)"
		# The href of a link constructor or helper is the URL-looking string
		# literal it carries; format-template fragments without a slash (query
		# keys, `true`/`false`) are not hrefs.
		while IFS= read -r href; do
			[ -n "$href" ] || continue
			count=$((count + 1))
			if printf '%s' "$href" | grep -q '://'; then
				printf 'error: %s: %s:%s: HAL link hardcodes the absolute URL %s; expose a root-relative reference so the client resolves it against its own base\n' \
					"$id" "$file" "$line" "$href" >&2
				result=1
			elif [ "${href#/}" = "$href" ]; then
				printf 'error: %s: %s:%s: HAL link %s is not a root-relative reference; it must begin with "/" so the client does not construct the URL\n' \
					"$id" "$file" "$line" "$href" >&2
				result=1
			fi
		done < <(printf '%s\n' "$text" |
			grep -oE '"[^"]*"' | sed -E 's/^"//; s/"$//' | grep -E '/' || true)
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, ((.range.start.line + 1) | tostring), (.text | @base64)] | @tsv')

	if [ "$count" -eq 0 ]; then
		printf 'error: %s: extracted no HAL link href from %s\n' "$id" "$surface" >&2
		return 1
	fi

	return "$result"
}

check_api_038() {
	local id="API-038"
	local rule_id="api-038"
	local surface="${1:-src/lib/infrastructure/inbound/rest}"
	local matches file line name result=0
	local -A hal=()
	local -A body=()
	local -a findings=()

	if [ ! -d "$surface" ]; then
		printf 'error: %s: missing %s\n' "$id" "$surface" >&2
		return 1
	fi

	# api-038 extracts every struct that carries a HAL navigation field
	# (`_links`/`_embedded`) — the marker of a HAL representation (`API-033`).
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$surface" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi

	while IFS=$'\t' read -r file line name; do
		[ -n "$name" ] || continue
		hal["$name"]="$file:$line"
	done < <(printf '%s\n' "$matches" | jq -r \
		'select(.metaVariables.single.NAME != null) |
		 [.file, ((.range.start.line + 1) | tostring),
		  .metaVariables.single.NAME.text] | @tsv')

	# The request-body types the handlers declare. An ast-grep rule cannot read
	# the `#[utoipa::path]` token tree or a function parameter list, so the
	# markers are parsed here: the `request_body = X` / `request_body(content =
	# X, …)` annotation (`API-008`), the `Result<Json<X>, JsonRejection>` /
	# `Result<Form<X>, FormRejection>` extractor, and the `*Request` naming
	# convention. Every marker names the request body type by name; the
	# definition's field list decides whether it is a HAL representation.
	while IFS= read -r name; do
		[ -n "$name" ] || continue
		body["$name"]=1
	done < <({
		grep -rhoE 'struct[[:space:]]+[A-Za-z0-9_]*Request\b' "$surface" --include='*.rs' |
			sed -E 's/.*struct[[:space:]]+//'
		grep -rhoE 'request_body[[:space:]]*=[[:space:]]*[A-Z][A-Za-z0-9_]*' "$surface" --include='*.rs' |
			sed -E 's/.*=[[:space:]]*//'
		grep -rhoE 'content[[:space:]]*=[[:space:]]*[A-Z][A-Za-z0-9_]*' "$surface" --include='*.rs' |
			sed -E 's/.*=[[:space:]]*//'
		grep -rhoE 'Result<[[:space:]]*Json<[A-Z][A-Za-z0-9_]*>[[:space:]]*,[[:space:]]*JsonRejection>' \
			"$surface" --include='*.rs' | sed -E 's/.*Json<([A-Za-z0-9_]+)>.*/\1/'
		grep -rhoE 'Result<[[:space:]]*Form<[A-Z][A-Za-z0-9_]*>[[:space:]]*,[[:space:]]*FormRejection>' \
			"$surface" --include='*.rs' | sed -E 's/.*Form<([A-Za-z0-9_]+)>.*/\1/'
	} | sort -u)
	if [ "${#body[@]}" -eq 0 ]; then
		printf 'error: %s: found no request body type under %s\n' "$id" "$surface" >&2
		return 1
	fi

	# A request body type that is a HAL representation reuses the response
	# representation format; requests carry only business data.
	for name in "${!body[@]}"; do
		if [ -n "${hal[$name]+set}" ]; then
			findings+=("$id: ${hal[$name]}: request body type $name carries a HAL navigation field; requests carry only business data")
		fi
	done

	if [ "${#findings[@]}" -eq 0 ]; then
		return "$result"
	fi
	printf '%s\n' "${findings[@]}" | LC_ALL=C sort >&2
	return 1
}

# check_api_039 — the HAL error envelope is the one response returned for every
# error the server emits (TechnicalDesign.md § 2.5, API-039). api-039 extracts
# the `ErrorBody` envelope struct and the root router's `setup_routes` middleware
# layers; the envelope's serialized root keys and the outermost layer are decided
# here because an ast-grep rule can neither enumerate a struct's keys nor order a
# method chain. The envelope must carry exactly `error` (the message at the root,
# `API-033`) and `_links` (the `self` link, `API-032`/`API-043`), and `hal_errors`
# must be applied last so it rewraps the routing fallback and extractor
# rejections as that envelope. The media type (`API-031`) and the request-path
# `self` (`API-043`) are the sibling rules' surfaces.
check_api_039() {
	local id="API-039"
	local rule_id="api-039"
	local surface="${1:-src/lib/infrastructure/inbound/rest}"
	local matches result=0 envelopes=0 layers=0 file line mw b64 text sorted key
	local -a keys=()
	local -A layer_last_line=()
	local -A layer_last_mw=()

	if [ ! -d "$surface" ]; then
		printf 'error: %s: missing %s\n' "$id" "$surface" >&2
		return 1
	fi

	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$surface" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi

	# The outermost `setup_routes` layer is the one whose middleware call sits on
	# the greatest source line: a later `.layer(...)` wraps the earlier ones, so
	# it sees every response, including the routing fallback and framework
	# rejections.
	while IFS=$'\t' read -r file line mw; do
		[ -n "$file" ] || continue
		layers=$((layers + 1))
		if [ -z "${layer_last_line[$file]+set}" ] ||
			[ "$line" -gt "${layer_last_line[$file]}" ]; then
			layer_last_line["$file"]="$line"
			layer_last_mw["$file"]="$mw"
		fi
	done < <(printf '%s\n' "$matches" | jq -r '
		select(.metaVariables.single.MIDDLEWARE != null) |
		[.file,
		 (.metaVariables.single.MIDDLEWARE.range.start.line | tostring),
		 .metaVariables.single.MIDDLEWARE.text] | @tsv')

	# The envelope's serialized root keys must be exactly `error` and `_links`; a
	# field's serialized name honours `#[serde(rename = "…")]`.
	while IFS=$'\t' read -r file b64; do
		[ -n "$b64" ] || continue
		envelopes=$((envelopes + 1))
		text="$(printf '%s' "$b64" | base64 -d)"
		keys=()
		while IFS= read -r key; do
			[ -n "$key" ] || continue
			keys+=("$key")
		done < <(printf '%s\n' "$text" | awk '
			{
				line = $0
				sub(/^[[:space:]]+/, "", line)
				if (line ~ /^#/) {
					if (match(line, /rename[[:space:]]*=[[:space:]]*"[^"]*"/)) {
						r = substr(line, RSTART, RLENGTH)
						sub(/^rename[[:space:]]*=[[:space:]]*"/, "", r)
						sub(/"$/, "", r)
						rename = r
					}
					next
				}
				n = line
				sub(/^pub([[:space:]]*\([^)]*\))?[[:space:]]+/, "", n)
				if (n ~ /^[a-z_][A-Za-z0-9_]*[[:space:]]*:/) {
					name = n
					sub(/[[:space:]]*:.*/, "", name)
					print (rename != "" ? rename : name)
					rename = ""
				}
			}
		')
		sorted="$(printf '%s\n' "${keys[@]}" | LC_ALL=C sort | paste -sd' ' -)"
		if [ "$sorted" != "_links error" ]; then
			printf 'error: %s: %s: the error envelope root keys are [%s]; they must be exactly error and _links (API-039)\n' \
				"$id" "$file" "$sorted" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" | jq -r '
		select(.metaVariables.multi.FIELDS != null) |
		[.file, (.text | @base64)] | @tsv')

	if [ "$envelopes" -eq 0 ]; then
		printf 'error: %s: extracted no ErrorBody envelope from %s\n' "$id" "$surface" >&2
		return 1
	fi
	if [ "$layers" -eq 0 ]; then
		printf 'error: %s: extracted no setup_routes middleware layer from %s\n' "$id" "$surface" >&2
		return 1
	fi

	while IFS= read -r file; do
		[ -n "$file" ] || continue
		mw="${layer_last_mw[$file]}"
		if ! printf '%s' "$mw" | grep -qE '(^|[^A-Za-z0-9_])hal_errors([^A-Za-z0-9_]|$)'; then
			printf 'error: %s: %s: the outermost setup_routes layer is %s, not hal_errors; the error envelope must be applied last so it wraps routing fallbacks and extractor rejections (API-039)\n' \
				"$id" "$file" "$mw" >&2
			result=1
		fi
	done < <(printf '%s\n' "${!layer_last_mw[@]}" | LC_ALL=C sort)

	return "$result"
}

# check_api_040 — the HAL `error` message is a stable, server-authored,
# client-safe string: it carries no extractor or framework wording and no
# internal detail — field paths, expected types, parse positions, type names
# (API-040, § 2.5). Every rejection is translated into a message in
# `api_error.rs`, whose `From<*Rejection>`/`From<ErrorKind>` impls attach the
# string an `ApiError` later carries into the envelope, so the rule's
# "Rejection detail is logged, never returned" holds there: each attached
# message is a literal, never the rejection's `Display`/`Debug`, a field of it,
# or a parse position. api-040 matches a message-carrying `ApiError`
# construction whose argument is not a string literal.
check_api_040() {
	local id="API-040"
	local rule_id="api-040"
	local target="src/lib/infrastructure/inbound/rest/api_error.rs"
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

# check_api_041 — the status code a failed request returns is the one its
# classification names (TechnicalDesign.md § API, API-041). api-041 extracts the
# `ApiError` variant -> `StatusCode` arms of `status_and_message` and the
# `JsonRejection` arms of `impl From<JsonRejection> for ApiError`; the
# name-to-status correspondence and the class-to-status comparison happen here
# because an ast-grep rule can neither transform an identifier nor relate two
# matches. The single mapping is what makes the code independent of the layer
# that caught the failure: every layer returns an `ApiError`, and this table
# fixes the status of each variant. The framework rejections the rule's detail
# section names are checked at their classification site: a body that is not
# JSON (`JsonSyntaxError`) is a `400`, a body that parses but does not match the
# schema (`JsonDataError`) is a `422`, and a missing or unsupported media type
# (`MissingJsonContentType`) is a `415`. The mapping of a domain or framing
# failure to an `ApiError` variant is a semantic judgement and stays a review
# residual, as does a name that does not derive from a status code.
check_api_041() {
	local id="API-041"
	local rule_id="api-041"
	local api_error="src/lib/infrastructure/inbound/rest/api_error.rs"
	local matches file line text variant status classifier expected result=0
	local table=0 rejections=0

	if [ ! -f "$api_error" ]; then
		printf 'error: %s: missing %s\n' "$id" "$api_error" >&2
		return 1
	fi

	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$api_error" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no ApiError status mapping from %s\n' "$id" "$api_error" >&2
		return 1
	fi

	while IFS=$'\t' read -r file line text; do
		[ -n "$text" ] || continue

		# A framework rejection site: the failure class the rule names must be
		# classified as the ApiError variant whose status is the rule's code.
		if [[ $text == *"JsonRejection::"* ]]; then
			rejections=$((rejections + 1))
			classifier="$(printf '%s' "$text" |
				sed -nE 's/.*JsonRejection::([A-Za-z0-9_]+).*/\1/p')"
			variant="$(printf '%s' "$text" |
				sed -nE 's/.*Self::([A-Za-z0-9_]+).*/\1/p')"
			case "$classifier" in
			JsonSyntaxError) expected="BadRequest" ;;
			JsonDataError) expected="UnprocessableEntity" ;;
			MissingJsonContentType) expected="UnsupportedMediaType" ;;
			*) continue ;;
			esac
			if [ "$variant" != "$expected" ]; then
				status="$(printf '%s' "$expected" |
					sed -E 's/([a-z0-9])([A-Z])/\1_\2/g' | tr '[:lower:]' '[:upper:]')"
				printf 'error: %s: %s:%s: %s is classified as ApiError::%s; API-041 assigns %s to that failure class\n' \
					"$id" "$file" "$line" "$classifier" "$variant" "$status" >&2
				result=1
			fi
			continue
		fi

		# The status table: the status must name the failure class the variant
		# encodes (`UnprocessableEntity` -> `UNPROCESSABLE_ENTITY`).
		variant="$(printf '%s' "$text" |
			sed -nE 's/.*Self::([A-Za-z0-9_]+).*/\1/p')"
		status="$(printf '%s' "$text" |
			sed -nE 's/.*StatusCode::([A-Z_]+).*/\1/p')"
		if [ -z "$variant" ] || [ -z "$status" ]; then
			continue
		fi
		table=$((table + 1))
		expected="$(printf '%s' "$variant" |
			sed -E 's/([a-z0-9])([A-Z])/\1_\2/g' | tr '[:lower:]' '[:upper:]')"
		if [ "$status" != "$expected" ]; then
			printf 'error: %s: %s:%s: ApiError::%s maps to %s; API-041 requires the status that names the failure class (%s)\n' \
				"$id" "$file" "$line" "$variant" "$status" "$expected" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, ((.range.start.line + 1) | tostring),
			(.text | gsub("\\s+"; " "))] | @tsv')

	if [ "$table" -eq 0 ]; then
		printf 'error: %s: extracted no ApiError variant/status mapping from %s\n' \
			"$id" "$api_error" >&2
		return 1
	fi
	if [ "$rejections" -eq 0 ]; then
		printf 'error: %s: extracted no JsonRejection classification from %s\n' \
			"$id" "$api_error" >&2
		return 1
	fi

	return "$result"
}

# check_api_043 — an error envelope's `_links.self` carries the request path
# component of the request URI, never the query string (API-043, § 2.5). The
# single envelope builder is `error_response`; api-043 extracts each call to it
# with the path argument, and this check resolves that argument to its source
# expression because an ast-grep rule cannot follow a value to its binding. The
# source must be the request path (`Uri::path()`), never the whole request URI
# or its query.
check_api_043() {
	local id="API-043"
	local rule_id="api-043"
	local rest="src/lib/infrastructure/inbound/rest"
	local matches file line arg name expr result=0

	if [ ! -d "$rest" ]; then
		printf 'error: %s: missing %s\n' "$id" "$rest" >&2
		return 1
	fi

	# api-043 extracts every call to the one error-envelope builder, with its
	# path argument; the binding that argument names is resolved below because
	# an ast-grep rule cannot follow a value to its source.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$rest" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: no call to the error-envelope builder error_response found under %s\n' \
			"$id" "$rest" >&2
		return 1
	fi

	while IFS=$'\t' read -r file line arg; do
		[ -n "$file" ] || continue
		name="${arg#&}"
		if [[ $name =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]]; then
			expr="$(awk -v name="$name" -v target="$line" '
				$0 ~ ("(^|[^A-Za-z0-9_])let[[:space:]]+" name "[[:space:]]*=") {
					last = $0
					sub(/^.*=[[:space:]]*/, "", last)
					sub(/;[[:space:]]*$/, "", last)
				}
				NR == target { print last; exit }
			' "$file")"
		else
			expr="$arg"
		fi
		if [[ $expr != *".path()"* ]] ||
			[[ $expr == *".query("* || $expr == *".path_and_query("* ||
				$expr == *"uri().to_string()"* || $expr == *"uri().to_owned()"* ]]; then
			printf 'error: %s: %s:%s: the error envelope builds _links.self from %s; it must carry the request path component (request.uri().path()), never the whole request URI or its query\n' \
				"$id" "$file" "$line" "$arg" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, ((.range.start.line + 1) | tostring),
			.metaVariables.single.PATH.text] | @tsv')

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
