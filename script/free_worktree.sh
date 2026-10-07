#!/bin/bash
set -u -o pipefail

# Print the path of the first worktree the caller may branch in, skipping the
# one this script runs in. The implement/investigate commands call this so they
# never commandeer the checkout the session is already in: right after `main` is
# synced that checkout is the first clean one, so "first free" would otherwise
# always pick it. See .opencode/commands/implement.md, step 4.
#
# A worktree is free when it is not prunable, not the caller's own worktree,
# clean (`git status --porcelain` empty), and not ahead of its upstream
# (`git log --oneline @{u}..` empty). A worktree whose branch has no upstream is
# not free: the comparison cannot be made, so the caller treats it as unknown.

usage() {
	echo "Usage: script/free_worktree.sh"
	echo ""
	echo "  Print the path of the first free worktree that is not the one this"
	echo "  script runs in, or print nothing and exit non-zero when none is free."
	echo ""
	echo "  Free means: not prunable, not the current worktree, a clean tree"
	echo "  (git status --porcelain empty), and not ahead of its upstream"
	echo "  (git log --oneline @{u}.. empty). A worktree whose branch has no"
	echo "  upstream is not free."
}

die() {
	printf 'error: %s\n' "$*" >&2
	exit 1
}

# Return 0 when the worktree at $1 may be branched in. $2 is its prunable flag
# (1 when `git worktree list` marked it prunable), $3 the caller's own path.
is_free() {
	local path="$1" prunable="$2" current="$3"
	local status ahead

	[ "$prunable" -eq 0 ] || return 1
	[ "$path" != "$current" ] || return 1
	[ -d "$path" ] || return 1

	status="$(git -C "$path" status --porcelain 2>/dev/null)" || return 1
	[ -z "$status" ] || return 1

	ahead="$(git -C "$path" log --oneline '@{u}..' 2>/dev/null)" || return 1
	[ -z "$ahead" ] || return 1

	return 0
}

main() {
	case "${1:-}" in
	-h | --help | help)
		usage
		return 0
		;;
	--) ;;
	"") ;;
	*) die "unexpected argument: $1" ;;
	esac

	git rev-parse --is-inside-work-tree >/dev/null 2>&1 ||
		die "not inside a git work tree"

	local current
	current="$(git rev-parse --show-toplevel)" ||
		die "cannot resolve the current work tree"

	# `git worktree list --porcelain` emits one block per worktree, separated by
	# blank lines. Each block opens with `worktree <path>`; `prunable` follows
	# when the entry is stale. Flush a block's path when the next block begins
	# and after the last line.
	local path="" prunable=0 line
	while IFS= read -r line; do
		case "$line" in
		"worktree "*)
			if [ -n "$path" ] && is_free "$path" "$prunable" "$current"; then
				printf '%s\n' "$path"
				return 0
			fi
			path="${line#worktree }"
			prunable=0
			;;
		"prunable"*) prunable=1 ;;
		esac
	done < <(git worktree list --porcelain)

	if [ -n "$path" ] && is_free "$path" "$prunable" "$current"; then
		printf '%s\n' "$path"
		return 0
	fi

	return 1
}

main "$@"
