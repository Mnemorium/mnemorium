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
RULES=()

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

# rule_config <id> — print the rule's block from rule_enforcer.yaml as compact
# JSON, or `{}` when the file, the `rules:` map or the block is absent. Returns
# non-zero only when the file is malformed, so a check can tell "no config" from
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

	local block
	block="$(yq -c ".rules[\"$id\"] // {}" "$CONFIG")" || return 2
	[ -n "$block" ] || block='{}'
	printf '%s\n' "$block"
}

# validate_config — hard-fail on a malformed file; a missing or empty file is
# valid (all blocks are then `{}`).
validate_config() {
	[ -f "$CONFIG" ] || return 0
	[ -s "$CONFIG" ] || return 0
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
	case "${1:-check}" in
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
	help | -h | --help)
		usage
		return 0
		;;
	*)
		usage >&2
		return 64
		;;
	esac
}

main "$@"
