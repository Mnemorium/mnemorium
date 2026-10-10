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
	"STY-RUST-002:check_sty_rust_002"
	"STY-RUST-003:check_sty_rust_003"
	"STY-RUST-004:check_sty_rust_004"
	"STY-RUST-005:check_sty_rust_005"
	"STY-RUST-006:check_sty_rust_006"
	"STY-RUST-007:check_sty_rust_007"
	"STY-RUST-014:check_sty_rust_014"
	"STY-RUST-013:check_sty_rust_013"
	"STY-RUST-008:check_sty_rust_008"
	"STY-RUST-011:check_sty_rust_011"
	"STY-RUST-012:check_sty_rust_012"
	"STY-RUST-068:check_sty_rust_068"
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

check_sty_rust_068() {
	local id="STY-RUST-068"
	local rule_id="sty-rust-068"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs '!src/bin/server.rs' \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
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
