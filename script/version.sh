#!/bin/bash
set -u -o pipefail

readonly CARGO_TOML="Cargo.toml"
readonly REST_RS="src/lib/infrastructure/inbound/rest.rs"
readonly OPENAPI_JSON="docs/development/api/openapi.json"
readonly CARGO_VERSION_ENV='version = env!("CARGO_PKG_VERSION")'
readonly SEMVER_REGEX='^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.]+)?$'

usage() {
	echo "Usage:"
	echo "  script/version.sh set <version>"
	echo "  script/version.sh check [expected]"
	echo ""
	echo "  set    Update the version in Cargo.toml and regenerate openapi.json"
	echo "  check  Verify Cargo.toml and openapi.json agree; with an argument,"
	echo "         verify they equal it"
	echo ""
	echo "Examples:"
	echo "  script/version.sh set 0.2.0"
	echo "  script/version.sh check          # Cargo.toml and openapi.json agree"
	echo "  script/version.sh check v0.2.0   # and equal the tag"
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

openapi_version() {
	local line
	line=$(
		sed -n '/^  "info": {/,/^  },/p' "$OPENAPI_JSON" |
			grep -m1 '"version"' |
			sed -E 's/.*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/'
	)
	if [ -z "$line" ]; then
		die "could not find info.version in $OPENAPI_JSON"
	fi
	echo "$line"
}

assert_rest_uses_cargo_version() {
	if ! grep -Fq "$CARGO_VERSION_ENV" "$REST_RS"; then
		die "$REST_RS must declare '$CARGO_VERSION_ENV' so the spec version comes from $CARGO_TOML"
	fi
}

regenerate_openapi() {
	echo "regenerating $OPENAPI_JSON"
	cargo run --bin openapi_gen >/dev/null 2>&1 ||
		die "failed to regenerate $OPENAPI_JSON — run 'cargo run --bin openapi_gen' manually"
}

cmd_set() {
	local version="$1"

	if ! is_semver "$version"; then
		die "$version is not a valid semver string (expected e.g. 0.2.0 or 0.2.0-rc.1)"
	fi

	assert_rest_uses_cargo_version

	local current
	current=$(cargo_version)

	if [ "$current" = "$version" ]; then
		echo "$CARGO_TOML is already at $version"
	else
		if ! sed -i "s/^version = \"${current}\"$/version = \"${version}\"/" "$CARGO_TOML"; then
			die "failed to rewrite the version in $CARGO_TOML"
		fi
		echo "version set to ${version} in $CARGO_TOML"
	fi

	regenerate_openapi

	if [ "$(openapi_version)" != "$version" ]; then
		die "$OPENAPI_JSON reports $(openapi_version) but expected $version"
	fi

	echo "done: $CARGO_TOML and $OPENAPI_JSON are at $version"
}

cmd_check() {
	local expected="${1:-}"
	local cargo_v openapi_v

	cargo_v=$(cargo_version 2>/dev/null) || cargo_v=""
	if [ -z "$cargo_v" ]; then
		die "could not read the version from $CARGO_TOML"
	fi

	assert_rest_uses_cargo_version

	openapi_v=$(openapi_version 2>/dev/null) || openapi_v=""
	if [ -z "$openapi_v" ]; then
		die "could not read info.version from $OPENAPI_JSON"
	fi

	if [ "$cargo_v" != "$openapi_v" ]; then
		echo "error: version mismatch" >&2
		echo "  $CARGO_TOML:   $cargo_v" >&2
		echo "  $OPENAPI_JSON: $openapi_v" >&2
		exit 1
	fi

	if [ -n "$expected" ]; then
		expected="${expected#v}"
		if ! is_semver "$expected"; then
			die "$expected is not a valid semver string"
		fi
		if [ "$cargo_v" != "$expected" ]; then
			echo "error: version mismatch with expected value" >&2
			echo "  expected:      $expected" >&2
			echo "  $CARGO_TOML:   $cargo_v" >&2
			echo "  $OPENAPI_JSON: $openapi_v" >&2
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
