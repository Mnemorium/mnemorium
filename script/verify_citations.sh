#!/bin/bash
set -u -o pipefail

# Verify every `path:line` citation in a review report.
#
# A review cites locations as `path:line`, but nothing checked them, so a line
# number past the end of a short file could reach the report. This script
# extracts every citation, resolves it against the working tree (or a base ref
# for a path the working tree no longer has), and fails when one does not exist.
#
# Usage:
#   script/verify_citations.sh <report.md> [report.md ...] [--base <ref>]
#
# With --base <ref>, a path absent from the working tree is resolved against
# `git show <ref>:<path>`. See .opencode/agents/lead-reviewer.md, "Review task".

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
readonly REPO_ROOT

# A citation is a backticked path with an extension, ending in `:<digits>`. The
# extension keeps prose such as `HTTP:200` from reading as a citation. The
# backticks are literal, not command substitution.
# shellcheck disable=SC2016
readonly CITATION_PATTERN='`[A-Za-z0-9._/-]*\.[A-Za-z0-9_]+:[0-9]+`'

usage() {
	echo "Usage:"
	echo "  script/verify_citations.sh <report.md> [report.md ...] [--base <ref>]"
	echo ""
	echo "  Fail when a report cites a path:line that does not exist: the path"
	echo "  is absent, or the line is past the end of the file. Citations resolve"
	echo "  against the working tree; with --base <ref>, a path absent from the"
	echo "  working tree resolves against git show <ref>:<path>."
}

die() {
	echo "error: $*" >&2
	exit 1
}

line_count_file() {
	awk 'END { print NR + 0 }' "$1"
}

line_count_ref() {
	git -C "$REPO_ROOT" show "$1" | awk 'END { print NR + 0 }'
}

# check_citation <backticked-path:line> <base-ref> — print ok/BAD and return
# non-zero when the citation does not resolve.
check_citation() {
	local citation="$1" base="$2"
	local location path line total

	location="${citation#\`}"
	location="${location%\`}"
	path="${location%:*}"
	line="${location##*:}"

	if [ -f "$REPO_ROOT/$path" ]; then
		total=$(line_count_file "$REPO_ROOT/$path")
	elif [ -n "$base" ] && git -C "$REPO_ROOT" cat-file -e "$base:$path" 2>/dev/null; then
		total=$(line_count_ref "$base:$path")
	else
		printf 'BAD %s — path not found%s\n' "$location" "${base:+ in the working tree or at $base}"
		return 1
	fi

	if [ "$line" -gt "$total" ]; then
		printf 'BAD %s — file has %s line(s)\n' "$location" "$total"
		return 1
	fi

	printf 'ok  %s (%s lines)\n' "$location" "$total"
	return 0
}

main() {
	local base=""
	local -a reports=()
	local report citation status=0 checked=0 bad=0
	local -a citations

	while [ "$#" -gt 0 ]; do
		case "$1" in
		--)
			shift
			break
			;;
		--base)
			shift
			[ "$#" -gt 0 ] || die "--base needs a ref"
			base="$1"
			;;
		-h | --help | help)
			usage
			return 0
			;;
		-*)
			usage >&2
			return 64
			;;
		*)
			reports+=("$1")
			;;
		esac
		shift
	done

	while [ "$#" -gt 0 ]; do
		reports+=("$1")
		shift
	done

	[ "${#reports[@]}" -gt 0 ] || {
		usage >&2
		return 64
	}

	for report in "${reports[@]}"; do
		[ -f "$report" ] || die "no such report: $report"

		mapfile -t citations < <(grep -oE "$CITATION_PATTERN" "$report" | sort -u)

		for citation in "${citations[@]}"; do
			checked=$((checked + 1))
			if ! check_citation "$citation" "$base"; then
				status=1
				bad=$((bad + 1))
			fi
		done
	done

	if [ "$checked" -eq 0 ]; then
		echo "warning: no path:line citation found" >&2
		return 0
	fi

	printf '%s citation(s) checked, %s invalid\n' "$checked" "$bad"
	return "$status"
}

main "$@"
