#!/bin/bash
# Regenerate THIRD_PARTY_NOTICES.txt from the crate graph.
#
# cargo-bundle-licenses walks every target, so its output lists Windows/Android/
# UEFI crates the image never contains. This filters it to the packages actually
# compiled for the target, so the notices cover what the released binary ships.
set -u -o pipefail

readonly DEFAULT_TARGET="x86_64-unknown-linux-musl"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly REPO_ROOT
readonly NOTICES="$REPO_ROOT/THIRD_PARTY_NOTICES.txt"

tmp=""
cleanup() {
	[ -n "$tmp" ] && rm -rf "$tmp"
	return 0
}
trap cleanup EXIT

target="$DEFAULT_TARGET"
check=0

usage() {
	echo "Usage: script/generate_third_party_notices.sh [--check] [--target <triple>]"
	echo ""
	echo "  Regenerate $NOTICES from the crate graph, restricted to the"
	echo "  packages the given target compiles."
	echo ""
	echo "  --check    Fail when the file is stale instead of rewriting it."
	echo "  --target   Rust target triple (default: $DEFAULT_TARGET)."
}

die() {
	echo "error: $*" >&2
	exit 1
}

parse_args() {
	while [ "$#" -gt 0 ]; do
		case "$1" in
		--check) check=1 ;;
		--target)
			shift
			[ "$#" -gt 0 ] || die "--target needs a value"
			target="$1"
			;;
		-h | --help | help)
			usage
			exit 0
			;;
		*) die "unknown argument: $1" ;;
		esac
		shift
	done
}

shipped_names() {
	cargo tree --target "$target" -e normal --prefix none |
		awk 'NF {print $1}' | sort -u
}

full_bundle() {
	local out="$1"
	local -a args=(--format toml --output "$out")
	if [ -f "$NOTICES" ]; then
		args+=(--previous "$NOTICES")
	fi
	cargo bundle-licenses "${args[@]}"
}

filter_bundle() {
	awk -v names="$1" '
        BEGIN {
            while ((getline n < names) > 0) wanted[n] = 1
            close(names)
            keep = 1
        }
        $0 == "[[third_party_libraries]]" {
            if (keep) printf "%s", block
            keep = 0
            block = $0 "\n"
            next
        }
        {
            block = block $0 "\n"
            if (!keep && $0 ~ /^package_name = "/) {
                split($0, parts, "\"")
                if (parts[2] in wanted) keep = 1
            }
        }
        END { if (keep) printf "%s", block }
    ' "$2"
}

generate() {
	tmp="$(mktemp -d)"

	cd "$REPO_ROOT" || die "cannot enter $REPO_ROOT"

	shipped_names >"$tmp/names" || die "cargo tree failed for $target"
	full_bundle "$tmp/full.toml" || die "cargo bundle-licenses failed"
	filter_bundle "$tmp/names" "$tmp/full.toml" >"$tmp/notices"

	if [ "$check" -eq 1 ]; then
		if [ ! -f "$NOTICES" ] || ! cmp -s "$NOTICES" "$tmp/notices"; then
			die "$NOTICES is stale; run the licenses:generate task."
		fi
		echo "THIRD_PARTY_NOTICES.txt is up to date."
	else
		cp "$tmp/notices" "$NOTICES"
		echo "Wrote THIRD_PARTY_NOTICES.txt."
	fi
}

parse_args "$@"
generate
