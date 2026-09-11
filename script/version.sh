#!/bin/bash
set -u -o pipefail

readonly CARGO_TOML="Cargo.toml"
readonly REST_RS="src/lib/infrastructure/inbound/rest.rs"
readonly SEMVER_REGEX='^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.]+)?$'

usage() {
	echo "Usage:"
	echo "  script/version.sh set <version>"
	echo "  script/version.sh check [expected]"
	echo ""
	echo "  set    Update the version in Cargo.toml and src/lib/infrastructure/inbound/rest.rs"
	echo "  check  Verify both files agree; with an argument, verify they equal it"
	echo ""
	echo "Examples:"
	echo "  script/version.sh set 0.2.0"
	echo "  script/version.sh check          # both files must agree"
	echo "  script/version.sh check v0.2.0   # both files must equal the tag"
}

die() {
	echo "error: $*" >&2
	exit 1
}

is_semver() {
	echo "$1" | grep -Eq "$SEMVER_REGEX"
}

cargo_version() {
	local line
	line=$(awk '/^\[package\]$/{in_pkg=1; next} in_pkg && /^\[/{in_pkg=0} in_pkg && /^version = /{ sub(/^version = /,""); gsub(/"/,""); print; exit }' "$CARGO_TOML")
	if [ -z "$line" ]; then
		die "could not find a version = \"...\" line under [package] in $CARGO_TOML"
	fi
	echo "$line"
}

rest_rs_version() {
	local line
	line=$(sed -n 's/^[[:space:]]*version = "\(.*\)",$/\1/p' "$REST_RS" | head -n 1)
	if [ -z "$line" ]; then
		die "could not find a version = \"...\" line in $REST_RS"
	fi
	echo "$line"
}

cmd_set() {
	local version="$1"

	if ! is_semver "$version"; then
		die "$version is not a valid semver string (expected e.g. 0.2.0 or 0.2.0-rc.1)"
	fi

	local current_cargo current_rest
	current_cargo=$(cargo_version)
	current_rest=$(rest_rs_version)

	if [ "$current_cargo" != "$current_rest" ]; then
		die "$CARGO_TOML has $current_cargo but $REST_RS has $current_rest; fix the inconsistency first"
	fi

	if ! sed -i "s/^version = \"${current_cargo}\"$/version = \"${version}\"/" "$CARGO_TOML"; then
		die "failed to rewrite the version in $CARGO_TOML"
	fi
	if ! sed -E -i "s/^([[:space:]]*)version = \"${current_rest}\",$/\1version = \"${version}\",/" "$REST_RS"; then
		die "failed to rewrite the version in $REST_RS"
	fi

	echo "regenerating docs/development/api/openapi.json"
	cargo run --bin openapi_gen >/dev/null 2>&1 ||
		die "failed to regenerate openapi.json — run 'cargo run --bin openapi_gen' manually"

	echo "version set to ${version} in:"
	echo "  - $CARGO_TOML"
	echo "  - $REST_RS"
}

cmd_check() {
	local expected="${1:-}"
	local cargo_v rest_v

	cargo_v=$(cargo_version 2>/dev/null) || cargo_v=""
	rest_v=$(rest_rs_version 2>/dev/null) || rest_v=""

	if [ -z "$cargo_v" ] || [ -z "$rest_v" ]; then
		echo "error: could not read versions:" >&2
		[ -n "$cargo_v" ] || echo "  $CARGO_TOML: not found" >&2
		[ -n "$rest_v" ] || echo "  $REST_RS: not found" >&2
		exit 1
	fi

	if [ "$cargo_v" != "$rest_v" ]; then
		echo "error: version mismatch" >&2
		echo "  $CARGO_TOML: $cargo_v" >&2
		echo "  $REST_RS:   $rest_v" >&2
		exit 1
	fi

	if [ -n "$expected" ]; then
		expected="${expected#v}"
		if ! is_semver "$expected"; then
			die "$expected is not a valid semver string"
		fi
		if [ "$cargo_v" != "$expected" ]; then
			echo "error: version mismatch with expected value" >&2
			echo "  expected:  $expected" >&2
			echo "  $CARGO_TOML: $cargo_v" >&2
			echo "  $REST_RS:   $rest_v" >&2
			exit 1
		fi
	fi

	echo "ok"
}

main() {
	if [ $# -lt 1 ]; then
		usage >&2
		exit 64
	fi

	local cmd="$1"
	shift

	case "$cmd" in
	set)
		[ $# -eq 1 ] || {
			usage >&2
			exit 64
		}
		cmd_set "$1"
		;;
	check)
		[ $# -le 1 ] || {
			usage >&2
			exit 64
		}
		cmd_check "${1:-}"
		;;
	-h | --help | help)
		usage
		;;
	*)
		usage >&2
		exit 64
		;;
	esac
}

main "$@"
