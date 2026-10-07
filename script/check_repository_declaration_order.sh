#!/bin/bash
set -u -o pipefail

# A repository port file declares the filter struct(s) first and the
# `<Aggregate>Repository` trait second; a read model or the `#[cfg(test)]`
# `&mut T` impl follows the trait. See docs/development/TechnicalDesign.md
# § 8.3 (Code Style Guidelines), STY-RUST-056 and STY-RUST-057.
readonly PORT_DIR="src/lib/domain/port"
readonly FILE_SUFFIX="_repository.rs"

# Top-level declarations start in column 0, so a `pub struct` quoted inside a
# doc comment (which begins with `///`) never matches.
readonly DECLARATION_PATTERN='^pub (struct|trait) '

usage() {
	echo "Usage:"
	echo "  script/check_repository_declaration_order.sh [file ...]"
	echo ""
	echo "  Fail when a repository port file declares a non-filter struct before"
	echo "  its repository trait. The file must declare the filter struct(s)"
	echo "  first and the trait second (STY-RUST-056, STY-RUST-057)."
	echo ""
	echo "  With no arguments, every *${FILE_SUFFIX} file under $PORT_DIR is"
	echo "  checked. Otherwise only the given files are checked; a file outside"
	echo "  $PORT_DIR is ignored."
}

die() {
	echo "error: $*" >&2
	exit 1
}

collect_targets() {
	if [ "$#" -gt 0 ]; then
		printf '%s\n' "$@"
	else
		find "$PORT_DIR" -type f -name "*${FILE_SUFFIX}" | sort
	fi
}

check_file() {
	local file="$1" entry line rest name trait_line=0
	local -a hits=()

	case "$file" in
	"$PORT_DIR"/*"$FILE_SUFFIX") ;;
	*) return 0 ;;
	esac
	[ -f "$file" ] || return 0

	while IFS= read -r entry; do
		line=${entry%%:*}
		rest=${entry#*:}
		case "$rest" in
		"pub trait "*)
			if [ "$trait_line" -eq 0 ]; then
				trait_line=$line
			fi
			;;
		"pub struct "*)
			if [ "$trait_line" -ne 0 ]; then
				continue
			fi
			name=${rest#pub struct }
			name=${name%% *}
			case "$name" in
			*Filter) ;;
			*) hits+=("${line}: ${name} is declared before the repository trait") ;;
			esac
			;;
		esac
	done < <(grep -nE "$DECLARATION_PATTERN" "$file")

	if [ "${#hits[@]}" -eq 0 ]; then
		return 0
	fi

	echo "error: a repository port file declares the filter struct(s) first and the trait second (STY-RUST-056, STY-RUST-057):" >&2
	printf '  %s\n' "$file" >&2
	printf '    %s\n' "${hits[@]}" >&2
	return 1
}

main() {
	local file status=0
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
		die "no *${FILE_SUFFIX} file found under $PORT_DIR"
	fi

	for file in "${targets[@]}"; do
		check_file "$file" || status=1
	done

	return "$status"
}

main "$@"
