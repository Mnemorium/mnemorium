#!/bin/bash
set -u -o pipefail

# A REST handler bounded-context folder holds only endpoint files, and an
# endpoint file imports no context submodule. See
# docs/development/TechnicalDesign.md § 1 (Code Style Guidelines),
# STY-RUST-003, STY-RUST-005, STY-RUST-088 and STY-RUST-089.
readonly REST_DIR="src/lib/infrastructure/inbound/rest"
readonly HANDLER_DIR="${REST_DIR}/handler"
readonly HANDLER_RULES="STY-RUST-003, STY-RUST-005, STY-RUST-088"
readonly MEMBER_RULES="STY-RUST-089"

# The bounded contexts that own a handler folder.
readonly CONTEXTS=(asset identity library user)

# The context alternation used by the cross-submodule import guard, derived from
# CONTEXTS so a context is declared once.
CONTEXT_ALTERNATION=$(
	IFS='|'
	printf '%s' "${CONTEXTS[*]}"
)
readonly CONTEXT_ALTERNATION

# An endpoint file is `<method>_<resource>.rs`, the method an HTTP verb; a
# context operation submodule is the same name without the `.rs`.
readonly ENDPOINT_FILE_PATTERN='^(get|post|put|patch|delete)_[a-z0-9_]+\.rs$'
readonly OP_PATTERN='^(get|post|put|patch|delete)_[a-z0-9_]+$'
readonly CONTEXT_PATTERN="^(${CONTEXT_ALTERNATION})$"

# A `use` statement start (the leading whitespace is captured to test whether the
# statement is at column 0), an ` as <ident>` alias, and a `/* … */` block
# comment. Each is a variable so bash does not read its parentheses as `[[ ]]`
# conditional grouping.
readonly USE_START_PATTERN='^[[:space:]]*(pub([[:space:]]*\([^)]*\))?[[:space:]]+)?use[[:space:]]'
readonly ALIAS_PATTERN='(.*)[[:space:]]as[[:space:]]+([A-Za-z0-9_]+)(.*)'
readonly BLOCK_COMMENT_PATTERN='(.*)/\*([^*]|\*[^/])*\*/(.*)'

# The only members allowed directly under `rest/` and `handler/`. Each bounded
# context contributes its `<context>.rs` module file and its `<context>/`
# folder, so the context set is declared once.
readonly REST_ALLOWED=(api_error.rs app_state.rs hal.rs handler.rs handler middleware.rs middleware)
HANDLER_ALLOWED=(get_health.rs)
for context in "${CONTEXTS[@]}"; do
	HANDLER_ALLOWED+=("${context}.rs" "${context}")
done
readonly HANDLER_ALLOWED

usage() {
	echo "Usage:"
	echo "  script/check_rest_handler_files.sh [--self-test]"
	echo ""
	echo "  Fail when a REST handler bounded-context folder is not endpoint-only:"
	echo ""
	# shellcheck disable=SC2016
	echo '    - every file directly under ${REST_DIR}/handler/{asset,identity,library,user}/'
	echo "      is named <method>_<resource>.rs, with no subdirectories;"
	echo "    - no endpoint file imports a context submodule;"
	# shellcheck disable=SC2016
	echo '    - ${REST_DIR} and ${REST_DIR}/handler hold only their known members.'
	echo ""
	echo "  With --self-test, run the check against a temporary fixture that proves"
	echo "  each violating form is rejected and each clean form is accepted."
	echo ""
	echo "  See docs/development/TechnicalDesign.md § 1 (Code Style Guidelines),"
	echo "  $HANDLER_RULES and $MEMBER_RULES."
}

die() {
	echo "error: $*" >&2
	exit 1
}

in_list() {
	local needle="$1" item
	shift
	for item in "$@"; do
		if [ "$item" = "$needle" ]; then
			return 0
		fi
	done
	return 1
}

# Temporary fixture tree of the running self-test, removed by the EXIT trap.
SELF_TEST_ROOT=""

cleanup_self_test() {
	if [ -n "${SELF_TEST_ROOT:-}" ]; then
		rm -rf "${SELF_TEST_ROOT}"
		SELF_TEST_ROOT=""
	fi
}

assert_accepted() {
	local output
	if ! output=$("$@" 2>&1); then
		echo "self-test: expected acceptance: $output" >&2
		return 1
	fi
	return 0
}

assert_rejected() {
	local needle="$1" output
	shift
	if output=$("$@" 2>&1); then
		echo "self-test: expected a rejection containing '$needle'" >&2
		return 1
	fi
	case "$output" in
	*"$needle"*) return 0 ;;
	*)
		echo "self-test: rejection missed '$needle': $output" >&2
		return 1
		;;
	esac
}

# Print `$1` with leading and trailing whitespace removed.
trim() {
	local value="$1"
	value="${value#"${value%%[![:space:]]*}"}"
	value="${value%"${value##*[![:space:]]}"}"
	printf '%s' "$value"
}

# Split `$1` on commas at brace depth zero and store the parts in SPLIT_OUT.
SPLIT_OUT=()
split_top_level() {
	local text="$1" depth=0 i char part=""
	SPLIT_OUT=()
	for ((i = 0; i < ${#text}; i++)); do
		char="${text:i:1}"
		case "$char" in
		"{") depth=$((depth + 1)) ;;
		"}") depth=$((depth - 1)) ;;
		",")
			if [ "$depth" -eq 0 ]; then
				part="$(trim "$part")"
				[ -n "$part" ] && SPLIT_OUT+=("$part")
				part=""
				continue
			fi
			;;
		esac
		part+="$char"
	done
	part="$(trim "$part")"
	[ -n "$part" ] && SPLIT_OUT+=("$part")
}

# Expand a Rust use tree (`$1`) into one fully-qualified path per line.
expand_tree() {
	local tree prefix inner suffix depth=0 end=-1 index i char sub path part
	tree="$(trim "$1")"
	[ -z "$tree" ] && return 0
	if [[ $tree != *"{"* ]]; then
		printf '%s\n' "$tree"
		return 0
	fi
	prefix="${tree%%\{*}"
	index=${#prefix}
	prefix="$(trim "$prefix")"
	prefix="${prefix%::}"
	for ((i = 0; i < ${#tree}; i++)); do
		char="${tree:i:1}"
		if [ "$char" = "{" ]; then
			depth=$((depth + 1))
		elif [ "$char" = "}" ]; then
			depth=$((depth - 1))
			if [ "$depth" -eq 0 ]; then
				end=$i
				break
			fi
		fi
	done
	if [ "$end" -lt 0 ]; then
		printf '%s\n' "$tree"
		return 0
	fi
	inner="${tree:index+1:end-index-1}"
	suffix="$(trim "${tree:end+1}")"
	split_top_level "$inner"
	local -a parts=("${SPLIT_OUT[@]}")
	for part in "${parts[@]}"; do
		while IFS= read -r sub; do
			[ -z "$sub" ] && continue
			if [ "$sub" = "self" ]; then
				printf '%s\n' "$prefix"
				continue
			fi
			if [ -n "$prefix" ]; then
				path="${prefix}::${sub}"
			else
				path="$sub"
			fi
			if [[ $suffix == ::* ]]; then
				path="${path}${suffix}"
			fi
			printf '%s\n' "$path"
		done < <(expand_tree "$part")
	done
}

# Report whether an expanded `use` path (`$1`) reaches a context operation
# submodule: `handler::<context>::<operation>` anywhere, or a top-level
# `super::<operation>`/`self::<operation>`.
reaches_operation() {
	local rest="$1" part first
	local -a segs=()
	while [[ $rest == *"::"* ]]; do
		part="${rest%%::*}"
		rest="${rest#*::}"
		part="$(trim "$part")"
		[ -n "$part" ] && segs+=("$part")
	done
	rest="$(trim "$rest")"
	[ -n "$rest" ] && segs+=("$rest")
	local count=${#segs[@]} i handler_index=-1
	[ "$count" -eq 0 ] && return 1
	first="${segs[0]}"
	if { [ "$first" = "super" ] || [ "$first" = "self" ]; } &&
		[ "$count" -gt 1 ] && [[ ${segs[1]} =~ $OP_PATTERN ]]; then
		return 0
	fi
	for ((i = 0; i < count; i++)); do
		if [ "${segs[i]}" = "handler" ]; then
			handler_index=$i
			break
		fi
	done
	[ "$handler_index" -lt 0 ] && return 1
	if [ $((handler_index + 2)) -lt "$count" ] &&
		[[ ${segs[handler_index + 1]} =~ $CONTEXT_PATTERN ]] &&
		[[ ${segs[handler_index + 2]} =~ $OP_PATTERN ]]; then
		return 0
	fi
	return 1
}

# Print every cross-submodule import of `$1` and exit non-zero when any is
# found. The file is parsed as a Rust use tree, so `pub`/`pub(crate)`, grouped,
# bare-submodule, relative sibling, aliased, comment-trailed and rustfmt-wrapped
# imports are all seen.
analyze_imports() {
	local file="$1"
	local line buffer="" top_level=0 in_stmt=0
	local -a flags=() texts=()
	local i body path first offender=0

	while IFS= read -r line || [ -n "$line" ]; do
		# Drop block comments and line comments before parsing the use tree.
		while [[ $line =~ $BLOCK_COMMENT_PATTERN ]]; do
			line="${BASH_REMATCH[1]}${BASH_REMATCH[3]}"
		done
		line="${line%%//*}"
		if [ "$in_stmt" -eq 0 ]; then
			if [[ $line =~ $USE_START_PATTERN ]]; then
				if [[ $line =~ ^[[:space:]]+ ]]; then top_level=0; else top_level=1; fi
				buffer="$line"
				in_stmt=1
				if [[ $line == *";"* ]]; then
					flags+=("$top_level")
					texts+=("$buffer")
					in_stmt=0
					buffer=""
				fi
			fi
		else
			buffer="$buffer $line"
			if [[ $line == *";"* ]]; then
				flags+=("$top_level")
				texts+=("$buffer")
				in_stmt=0
				buffer=""
			fi
		fi
	done <"$file"

	for ((i = 0; i < ${#texts[@]}; i++)); do
		body="${texts[i]}"
		body="${body#*use}"
		body="$(trim "$body")"
		body="${body%;}"
		body="$(trim "$body")"
		# Strip ` as <ident>` aliases so the operation segment is bare.
		while [[ $body =~ $ALIAS_PATTERN ]]; do
			body="${BASH_REMATCH[1]}${BASH_REMATCH[3]}"
		done
		if [ "${flags[i]}" -eq 0 ] && { [ "${body:0:5}" = "super" ] || [ "${body:0:4}" = "self" ]; }; then
			continue
		fi
		while IFS= read -r path; do
			[ -z "$path" ] && continue
			first="${path%%::*}"
			if [ "${flags[i]}" -eq 0 ] && { [ "$first" = "super" ] || [ "$first" = "self" ]; }; then
				continue
			fi
			if reaches_operation "$path"; then
				offender=1
				printf '%s: %s\n' "$file" "${texts[i]}"
				break
			fi
		done < <(expand_tree "$body")
	done

	[ "$offender" -eq 0 ] && return 0
	return 1
}

check_context() {
	local handler_dir="$1" context="$2"
	local dir="${handler_dir}/${context}"
	local entry name hits
	local result=0

	[ -d "$dir" ] || die "missing bounded-context directory: $dir"

	while IFS= read -r entry; do
		if [ -d "$entry" ]; then
			echo "error: a bounded-context folder holds only endpoint files ($HANDLER_RULES):" >&2
			printf '  %s is a subdirectory\n' "$entry" >&2
			result=1
			continue
		fi
		name=${entry##*/}
		if ! [[ $name =~ $ENDPOINT_FILE_PATTERN ]]; then
			echo "error: an endpoint file is named <method>_<resource>.rs ($HANDLER_RULES):" >&2
			printf '  %s\n' "$entry" >&2
			result=1
		fi
	done < <(find "$dir" -mindepth 1 -maxdepth 1 | sort)

	while IFS= read -r entry; do
		[ -f "$entry" ] || continue
		if hits=$(analyze_imports "$entry"); then
			continue
		fi
		echo "error: an endpoint file imports a context submodule ($HANDLER_RULES):" >&2
		printf '%s\n' "$hits" >&2
		result=1
	done < <(find "$dir" -mindepth 1 -maxdepth 1 -type f | sort)

	return "$result"
}

check_allowed_members() {
	local dir="$1" entry name
	local result=0

	shift
	[ -d "$dir" ] || die "missing directory: $dir"

	while IFS= read -r entry; do
		name=${entry##*/}
		if ! in_list "$name" "$@"; then
			echo "error: $dir holds an unexpected member ($MEMBER_RULES):" >&2
			printf '  %s\n' "$name" >&2
			result=1
		fi
	done < <(find "$dir" -mindepth 1 -maxdepth 1 | sort)

	return "$result"
}

self_test() {
	local root handler endpoint context index result=0
	local -a labels=(
		"bare"
		"pub"
		"pub(crate)"
		"grouped-after-context"
		"grouped-before-context"
		"bare-submodule"
		"relative-super"
		"relative-self"
		"nested-absolute"
		"multi-line"
		"aliased"
		"aliased-grouped"
		"aliased-relative"
		"aliased-self"
		"comment-trailed"
		"comment-block"
	)
	local -a imports=(
		$'use crate::infrastructure::inbound::rest::handler::library::get_gallery::get_gallery;\n'
		$'pub use crate::infrastructure::inbound::rest::handler::asset::get_upload::PostUploadResponse;\n'
		$'pub(crate) use crate::infrastructure::inbound::rest::handler::user::get_user::GetUserResponse;\n'
		$'use crate::infrastructure::inbound::rest::handler::library::{get_gallery::get_gallery};\n'
		$'use crate::infrastructure::inbound::rest::handler::{library::get_gallery::get_gallery};\n'
		$'use crate::infrastructure::inbound::rest::handler::library::get_gallery_item;\n'
		$'use super::get_gallery_item::GalleryItemDetailResponse;\n'
		$'use self::get_gallery_item::GalleryItemDetailResponse;\n'
		$'    use crate::infrastructure::inbound::rest::handler::asset::get_upload::get_upload;\n'
		$'use crate::infrastructure::inbound::rest::handler::library::{\n    get_gallery::get_gallery,\n};\n'
		$'use crate::infrastructure::inbound::rest::handler::library::get_gallery as gg;\n'
		$'use crate::infrastructure::inbound::rest::handler::library::{get_gallery as gg};\n'
		$'use super::get_gallery_item as gi;\n'
		$'use crate::infrastructure::inbound::rest::handler::library::get_gallery::{self as gg};\n'
		$'use crate::infrastructure::inbound::rest::handler::library::get_gallery; // note\n'
		$'use crate::infrastructure::inbound::rest::handler::library::get_gallery /* note */;\n'
	)
	local -a accepted_labels=(
		"absolute module-level"
		"grouped module-level"
		"relative module-level"
		"module-level helper"
		"aliased module-level"
		"comment-trailed module-level"
	)
	local -a accepted_imports=(
		$'use crate::infrastructure::inbound::rest::handler::library::GalleryResponse;\n'
		$'use crate::infrastructure::inbound::rest::handler::library::{GalleryResponse, parse_gallery_id};\n'
		$'use super::GalleryResponse;\n'
		$'use crate::infrastructure::inbound::rest::handler::asset::asset_routes;\n'
		$'use crate::infrastructure::inbound::rest::handler::library::GalleryResponse as Resp;\n'
		$'use crate::infrastructure::inbound::rest::handler::library::GalleryResponse; // note\n'
	)

	root=$(mktemp -d)
	SELF_TEST_ROOT="$root"
	trap cleanup_self_test EXIT
	handler="${root}/handler"
	mkdir -p "$handler"

	for context in "${CONTEXTS[@]}"; do
		mkdir -p "${handler}/${context}"
		: >"${handler}/${context}.rs"
		: >"${handler}/${context}/get_thing.rs"
	done
	: >"${handler}/get_health.rs"

	if ! assert_accepted check_context "$handler" "library"; then
		echo "self-test: a clean context folder was rejected" >&2
		result=1
	fi
	if ! assert_accepted check_allowed_members "$handler" "${HANDLER_ALLOWED[@]}"; then
		echo "self-test: a clean handler tree was rejected" >&2
		result=1
	fi

	endpoint="${handler}/library/get_thing.rs"
	for index in "${!labels[@]}"; do
		printf '%s' "${imports[index]}" >"$endpoint"
		if ! assert_rejected "imports a context submodule" check_context "$handler" "library"; then
			echo "self-test: the ${labels[index]} import was not rejected" >&2
			result=1
		fi
	done
	for index in "${!accepted_labels[@]}"; do
		printf '%s' "${accepted_imports[index]}" >"$endpoint"
		if ! assert_accepted check_context "$handler" "library"; then
			echo "self-test: the ${accepted_labels[index]} import was rejected" >&2
			result=1
		fi
	done

	: >"${handler}/library/representation.rs"
	if ! assert_rejected "is named <method>_<resource>.rs" check_context "$handler" "library"; then
		echo "self-test: a non-endpoint file name was not rejected" >&2
		result=1
	fi
	rm -f "${handler}/library/representation.rs"

	mkdir -p "${handler}/library/nested"
	if ! assert_rejected "is a subdirectory" check_context "$handler" "library"; then
		echo "self-test: a context subdirectory was not rejected" >&2
		result=1
	fi
	rmdir "${handler}/library/nested"

	: >"${handler}/unexpected.rs"
	if ! assert_rejected "holds an unexpected member" check_allowed_members "$handler" "${HANDLER_ALLOWED[@]}"; then
		echo "self-test: an unexpected handler member was not rejected" >&2
		result=1
	fi
	rm -f "${handler}/unexpected.rs"

	rm -rf "${root:?}"
	SELF_TEST_ROOT=""
	return "$result"
}

main() {
	local context status=0

	while [ "$#" -gt 0 ]; do
		case "$1" in
		--self-test)
			shift
			self_test
			return $?
			;;
		-h | --help | help)
			usage
			return 0
			;;
		--)
			shift
			break
			;;
		-*)
			usage >&2
			return 64
			;;
		*)
			break
			;;
		esac
	done

	for context in "${CONTEXTS[@]}"; do
		check_context "$HANDLER_DIR" "$context" || status=1
	done

	check_allowed_members "$REST_DIR" "${REST_ALLOWED[@]}" || status=1
	check_allowed_members "$HANDLER_DIR" "${HANDLER_ALLOWED[@]}" || status=1

	return "$status"
}

main "$@"
