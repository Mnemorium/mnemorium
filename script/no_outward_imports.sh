#!/bin/bash
set -u -o pipefail

# The hexagonal dependency direction: domain <- application <- infrastructure.
# Nothing points outward. See AGENTS.md, "Special Rules".
readonly DOMAIN_DIR="src/lib/domain"
readonly APPLICATION_DIR="src/lib/application"

# Only `use` statements are matched, so doc-comment prose like
# `[crate::infrastructure::...]` is not a false positive. The trailing
# boundary keeps a lookalike root such as `crate::application_helper` out.
readonly DOMAIN_USE_PATTERN='^[[:space:]]*(pub([[:space:]]*\([^)]*\))?[[:space:]]+)?use[[:space:]].*crate[[:space:]]*::[[:space:]]*(application|infrastructure)([^A-Za-z0-9_]|$)'
readonly APPLICATION_USE_PATTERN='^[[:space:]]*(pub([[:space:]]*\([^)]*\))?[[:space:]]+)?use[[:space:]].*crate[[:space:]]*::[[:space:]]*infrastructure([^A-Za-z0-9_]|$)'

usage() {
	echo "Usage:"
	echo "  script/no_outward_imports.sh [file ...]"
	echo ""
	echo "  Fail when a Rust file under a guarded directory imports a crate"
	echo "  root that points outward in the hexagonal architecture:"
	echo ""
	echo "    $DOMAIN_DIR       must not import crate::application or crate::infrastructure"
	echo "    $APPLICATION_DIR  must not import crate::infrastructure"
	echo ""
	echo '  See AGENTS.md, "Special Rules".'
	echo ""
	echo "  With no arguments, every *.rs file under both guarded directories is"
	echo "  checked. Otherwise only the given files are checked; a file outside"
	echo "  the guarded directories is ignored."
}

die() {
	echo "error: $*" >&2
	exit 1
}

collect_targets() {
	if [ "$#" -gt 0 ]; then
		printf '%s\n' "$@"
	else
		find "$DOMAIN_DIR" "$APPLICATION_DIR" -type f -name '*.rs' | sort
	fi
}

pattern_for() {
	case "$1" in
	"$DOMAIN_DIR"/*) printf '%s\n' "$DOMAIN_USE_PATTERN" ;;
	"$APPLICATION_DIR"/*) printf '%s\n' "$APPLICATION_USE_PATTERN" ;;
	*) return 1 ;;
	esac
}

main() {
	local file pattern hits
	local status=0
	local -a targets

	while [ "$#" -gt 0 ]; do
		case "$1" in
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

	mapfile -t targets < <(collect_targets "$@")

	if [ "${#targets[@]}" -eq 0 ]; then
		die "no Rust file found under $DOMAIN_DIR or $APPLICATION_DIR"
	fi

	for file in "${targets[@]}"; do
		# A file removed in the commit cannot import anything.
		[ -f "$file" ] || continue

		pattern=$(pattern_for "$file") || continue

		if hits=$(grep -nHE "$pattern" "$file"); then
			echo 'error: an import points outward in the hexagon (AGENTS.md, "Special Rules"):' >&2
			printf '%s\n' "$hits" >&2
			status=1
		fi
	done

	return "$status"
}

main "$@"
