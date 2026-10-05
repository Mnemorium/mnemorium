#!/bin/bash
# Regenerate THIRD_PARTY_NOTICES.txt from the crate graph.
#
# cargo-about reads `about.toml` (accepted license set, target scope, per-crate
# clarifications) and renders it through `about.hbs`. The target scope lists the
# packages the image actually compiles, so the notices cover what the released
# binary ships.
set -u -o pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly REPO_ROOT
readonly NOTICES="$REPO_ROOT/THIRD_PARTY_NOTICES.txt"
readonly CONFIG="$REPO_ROOT/about.toml"
readonly TEMPLATE="$REPO_ROOT/about.hbs"

tmp=""
cleanup() {
	[ -n "$tmp" ] && rm -rf "$tmp"
	return 0
}
trap cleanup EXIT

check=0

usage() {
	echo "Usage: script/generate_third_party_notices.sh [--check]"
	echo ""
	echo "  Regenerate $NOTICES from the crate graph."
	echo ""
	echo "  --check    Fail when the file is stale instead of rewriting it."
}

die() {
	echo "error: $*" >&2
	exit 1
}

parse_args() {
	while [ "$#" -gt 0 ]; do
		case "$1" in
		--check) check=1 ;;
		-h | --help | help)
			usage
			exit 0
			;;
		*) die "unknown argument: $1" ;;
		esac
		shift
	done
}

generate() {
	tmp="$(mktemp -d)"

	cd "$REPO_ROOT" || die "cannot enter $REPO_ROOT"

	cargo about generate \
		--config "$CONFIG" \
		--fail \
		-o "$tmp/notices" \
		"$TEMPLATE" || die "cargo-about failed"

	if grep -q 'Copyright (c) <year> <copyright holders>' "$tmp/notices"; then
		echo "warning: a crate resolved to cargo-about's generic MIT text" >&2
	fi

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
