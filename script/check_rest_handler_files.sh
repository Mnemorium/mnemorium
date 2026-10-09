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

# An endpoint file is `<method>_<resource>.rs`, the method an HTTP verb.
readonly ENDPOINT_FILE_PATTERN='^(get|post|put|patch|delete)_[a-z0-9_]+\.rs$'

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

# Print every cross-submodule import of `$1` and exit non-zero when any is
# found. The file is parsed as a Rust use tree, so `pub`/`pub(crate)`, grouped,
# bare-submodule, relative sibling and rustfmt-wrapped imports are all seen.
analyze_imports() {
	local file="$1"
	python3 - "$file" "$CONTEXT_ALTERNATION" <<'PY'
import re
import sys

OP_RE = re.compile(r"^(get|post|put|patch|delete)_[a-z0-9_]+$")
USE_START_RE = re.compile(r"^([ \t]*)(?:pub(?:\s*\([^)]*\))?\s+)?use\s")
USE_BODY_RE = re.compile(r"^(?:pub(?:\s*\([^)]*\))?\s+)?use\s+(.*)$", re.S)


def split_top_level(text):
    parts, depth, current = [], 0, []
    for char in text:
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
        if char == "," and depth == 0:
            parts.append("".join(current))
            current = []
        else:
            current.append(char)
    if current:
        parts.append("".join(current))
    return [part.strip() for part in parts if part.strip()]


def expand(tree):
    tree = tree.strip()
    if not tree:
        return []
    index = tree.find("{")
    if index == -1:
        segments = [seg.strip() for seg in tree.split("::") if seg.strip()]
        return [segments] if segments else []
    prefix = tree[:index]
    if prefix.endswith("::"):
        prefix = prefix[:-2]
    base = [seg.strip() for seg in prefix.split("::") if seg.strip()]
    depth, end = 0, -1
    for offset in range(index, len(tree)):
        if tree[offset] == "{":
            depth += 1
        elif tree[offset] == "}":
            depth -= 1
            if depth == 0:
                end = offset
                break
    inner = tree[index + 1:end]
    suffix = tree[end + 1:].strip()
    paths = []
    for part in split_top_level(inner):
        for sub in expand(part):
            if sub == ["self"]:
                paths.append(list(base))
                continue
            path = list(base) + sub
            if suffix.startswith("::"):
                path += [seg.strip() for seg in suffix[2:].split("::") if seg.strip()]
            paths.append(path)
    return paths


def collect_statements(lines):
    statements, buffer, top_level = [], None, False
    for line in lines:
        if buffer is None:
            match = USE_START_RE.match(line)
            if match:
                buffer = line.strip()
                top_level = not match.group(1)
                if ";" in line:
                    statements.append((top_level, buffer))
                    buffer = None
        else:
            buffer += " " + line.strip()
            if ";" in line:
                statements.append((top_level, buffer))
                buffer = None
    return statements


def reaches_operation(path, contexts):
    if path[0] in ("super", "self") and len(path) > 1 and OP_RE.match(path[1]):
        return True
    if "handler" not in path:
        return False
    index = path.index("handler")
    return (
        index + 2 < len(path)
        and path[index + 1] in contexts
        and bool(OP_RE.match(path[index + 2]))
    )


def main():
    filename, context_argument = sys.argv[1], sys.argv[2]
    contexts = set(context_argument.split("|"))
    with open(filename, encoding="utf-8") as handle:
        lines = handle.read().splitlines()
    offenders = []
    for top_level, statement in collect_statements(lines):
        match = USE_BODY_RE.match(statement)
        if not match:
            continue
        body = match.group(1).rstrip().rstrip(";").strip()
        for path in expand(body):
            if not path:
                continue
            if path[0] in ("super", "self") and not top_level:
                continue
            if reaches_operation(path, contexts):
                offenders.append(statement)
                break
    if not offenders:
        return 0
    for offender in offenders:
        print(f"{filename}: {offender}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
PY
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
	)
	local -a accepted_labels=(
		"absolute module-level"
		"grouped module-level"
		"relative module-level"
		"module-level helper"
	)
	local -a accepted_imports=(
		$'use crate::infrastructure::inbound::rest::handler::library::GalleryResponse;\n'
		$'use crate::infrastructure::inbound::rest::handler::library::{GalleryResponse, parse_gallery_id};\n'
		$'use super::GalleryResponse;\n'
		$'use crate::infrastructure::inbound::rest::handler::asset::asset_routes;\n'
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
