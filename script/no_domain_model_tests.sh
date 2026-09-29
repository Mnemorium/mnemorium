#!/bin/bash
set -u -o pipefail

readonly MODEL_DIR="src/lib/domain/model"
# Test constructs only: test gates, test attributes, test modules and mocks.
# See TEST-019 in docs/development/TechnicalDesign.md.
readonly TEST_PATTERN='#\[[[:space:]]*cfg[[:space:]]*\(.*\btest\b|#\[[[:space:]]*cfg_attr[[:space:]]*\([[:space:]]*test\b|#\[[[:space:]]*([A-Za-z_][A-Za-z0-9_]*::)*test[[:space:]]*[])(]|^[[:space:]]*(pub[[:space:]]+)?mod[[:space:]]+tests?\b|\bmockall\b|automock'

usage() {
	echo "Usage:"
	echo "  script/no_domain_model_tests.sh [file ...]"
	echo ""
	echo "  Fail when a Rust file under $MODEL_DIR contains test code."
	echo "  Domain models have no dedicated test of their own (TEST-019): they"
	echo "  are covered by the application-layer tests instead."
	echo ""
	echo "  With no arguments, every *.rs file under $MODEL_DIR is checked."
	echo "  Otherwise only the given files are checked."
}

die() {
	echo "error: $*" >&2
	exit 1
}

collect_targets() {
	if [ "$#" -gt 0 ]; then
		printf '%s\n' "$@"
	else
		find "$MODEL_DIR" -type f -name '*.rs' | sort
	fi
}

main() {
	local file hits
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
		die "no Rust file found under $MODEL_DIR"
	fi

	for file in "${targets[@]}"; do
		# A file removed in the commit cannot hold tests.
		[ -f "$file" ] || continue

		if hits=$(grep -nHE "$TEST_PATTERN" "$file"); then
			echo "error: tests are not allowed in domain models (TEST-019):" >&2
			printf '%s\n' "$hits" >&2
			status=1
		fi
	done

	return "$status"
}

main "$@"
