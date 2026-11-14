#!/usr/bin/env bash
# Drive the k6 performance suite against a disposable, seeded server.
#
# Modes:
#   run <smoke|load|stress|soak> [--profile read|write|all] [--flow NAME]
#       [--repeat N] [--strict] [--keep]
#   baseline [--environment local|ubuntu-latest]
#   flamegraph [--profile read|write|both]
#   client
#   clean
#
# The SUT is the repository Docker image built with `api-docs test-seed`,
# capped at four CPUs and 4 GiB, and bound to a temporary working directory on a
# random loopback port. The container reads `test/performance/server/config.yaml`
# as its `config.yaml`, so the rate limiter and debug log cannot make a load run
# meaningless.
#
# Environment:
#   PERF_IMAGE            image tag to build/run (default mnemorium-perf:local)
#   PERF_SKIP_BUILD=1     reuse the image instead of building it
#   PERF_ENVIRONMENT      baseline section to report against (local|ubuntu-latest)
#   K6                    k6 binary (default: k6 on PATH)
#   KEEP=1                keep the disposable working directory and container logs
#   PERF_CPUS             CPU cap (default 4)
#   PERF_MEMORY           memory cap (default 4g)

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly REPO_ROOT
readonly PERF_DIR="${REPO_ROOT}/test/performance"
readonly ARTIFACTS_DIR="${REPO_ROOT}/.artifacts/performance"
readonly IMAGE="${PERF_IMAGE:-mnemorium-perf:local}"
readonly K6="${K6:-k6}"
readonly CPUS="${PERF_CPUS:-4}"
readonly MEMORY="${PERF_MEMORY:-4g}"
readonly SERIES_ID=6000000
readonly STREAM_FILE_ID=1200000
readonly STREAM_BYTES=4194304
# Free space a capture needs, in KiB. `perf record` writes its sample file into
# the profiled process's working directory; at 99 Hz with the call-graph dwarf
# dump cargo-flamegraph hardcodes, a 90 s read capture on four cores runs to
# tens of GiB, because the sample volume follows the threads the SUT schedules.
readonly FLAMEGRAPH_FREE_KIB=$((32 * 1024 * 1024))

CONTAINER_NAME=""
WORK_DIR=""
PORT=""
RUN_DIR=""
LAST_RUN_DIR=""
STATS_PID=""
FLAME_PID=""
SUT_LOG=""
BASE_URL=""
ROOT_URL=""
FINGERPRINT=""
STRICT=0
KEEP="${KEEP:-}"

log() {
	printf '==> %s\n' "$*" >&2
}

die() {
	printf 'error: %s\n' "$*" >&2
	exit 1
}

cleanup() {
	local status=$?
	if [[ -n ${STATS_PID} ]]; then
		kill "${STATS_PID}" 2>/dev/null || true
	fi
	if [[ -n ${CONTAINER_NAME} ]]; then
		docker rm -f "${CONTAINER_NAME}" >/dev/null 2>&1 || true
	fi
	# Stop a profiled server before its `cargo flamegraph` wrapper, so perf can
	# still flush the capture (see `stop_flamegraph_sut`). Only when a capture
	# is running: another mode must not touch an unrelated server.
	if [[ -n ${FLAME_PID} ]]; then
		pkill -INT -f 'target/release/server' 2>/dev/null || true
		kill -INT "${FLAME_PID}" 2>/dev/null || true
		wait "${FLAME_PID}" 2>/dev/null || true
	fi
	if [[ -n ${WORK_DIR} ]]; then
		if [[ -n ${KEEP} ]]; then
			echo "kept the disposable working directory at ${WORK_DIR}" >&2
		else
			rm -rf "${WORK_DIR}"
		fi
	fi
	exit "${status}"
}
trap cleanup EXIT

usage() {
	sed -n '2,16p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

random_port() {
	python3 - <<'PY'
import socket
with socket.socket() as sock:
    sock.bind(("127.0.0.1", 0))
    print(sock.getsockname()[1])
PY
}

wait_for_health() {
	local base_url=$1 timeout=${2:-120}
	local deadline=$((SECONDS + timeout))
	while ((SECONDS < deadline)); do
		if curl -fsS --max-time 1 "${base_url}/health" >/dev/null 2>&1; then
			return 0
		fi
		sleep 1
	done
	return 1
}

wait_for_scan() {
	local logs=$1 timeout=${2:-120}
	local deadline=$((SECONDS + timeout))
	while ((SECONDS < deadline)); do
		if [[ -f ${logs} ]] && grep -q 'reconciled the library root' "${logs}"; then
			return 0
		fi
		sleep 1
	done
	return 1
}

assert_scan_clean() {
	local logs=$1 line
	line="$(grep 'reconciled the library root' "${logs}" | tail -n 1)"
	[[ ${line} == *registered=0* ]] || die "the startup scan registered files: ${line}"
	[[ ${line} == *changed=0* ]] || die "the startup scan re-probed files: ${line}"
	[[ ${line} == *removed=0* ]] || die "the startup scan removed rows: ${line}"
	[[ ${line} == *missing=0* ]] || die "the startup scan reported missing files: ${line}"
	log "startup scan: ${line/*reconciled/reconciled}"
}

login_token() {
	local base_url=$1
	curl -fsS -X POST "${base_url}/identity/login" \
		-H 'Content-Type: application/json' \
		--data '{"username":"alice","password":"C0rrect!Horse"}' |
		jq -r '.access_token'
}

# Fail before the capture starts rather than after `perf` fills the volume: a
# sample file that cannot be written aborts perf, which kills the profiled
# server and leaves k6 reporting only connection refused.
require_free_space() {
	local path=$1 needed_kib=$2 avail_kib
	avail_kib="$(df -Pk "${path}" | awk 'NR == 2 { print $4 }')"
	[[ -n ${avail_kib} ]] || die "cannot read the free space of ${path}"
	((avail_kib >= needed_kib)) ||
		die "${path} has $((avail_kib / 1024)) MiB free; a flamegraph capture needs about $((needed_kib / 1024)) MiB"
}

# Refuse to measure latencies on 404s: every list has the seeded shape.
assert_dataset() {
	local base_url=$1 token page
	token="$(login_token "${base_url}")"
	[[ -n ${token} && ${token} != null ]] || die "dataset not grounded: login failed"

	page="$(curl -fsS "${base_url}/library/gallery?limit=100&offset=1" \
		-H "Authorization: Bearer ${token}")"
	[[ $(jq '.items | length' <<<"${page}") -eq 100 ]] ||
		die "dataset not grounded: the first gallery page is not full"
	[[ $(jq '.items[0].id' <<<"${page}") -ge 1000000 ]] ||
		die "dataset not grounded: the gallery range is absent"

	page="$(curl -fsS "${base_url}/library/music/album?limit=1&offset=1" \
		-H "Authorization: Bearer ${token}")"
	[[ $(jq '._embedded.items[0].id' <<<"${page}") -ge 4000000 ]] ||
		die "dataset not grounded: the album range is absent"

	page="$(curl -fsS "${base_url}/library/movie?limit=1" \
		-H "Authorization: Bearer ${token}")"
	[[ $(jq '._embedded.items[0].id' <<<"${page}") -ge 5000000 ]] ||
		die "dataset not grounded: the movie range is absent"

	page="$(curl -fsS "${base_url}/library/series?limit=1" \
		-H "Authorization: Bearer ${token}")"
	[[ $(jq '._embedded.items[0].id' <<<"${page}") -ge 6000000 ]] ||
		die "dataset not grounded: the series range is absent"

	page="$(curl -fsS "${base_url}/library/series/${SERIES_ID}/season/1/episode" \
		-H "Authorization: Bearer ${token}")"
	[[ $(jq '.items | length' <<<"${page}") -eq 10 ]] ||
		die "dataset not grounded: the episode range is absent"

	page="$(curl -fsS "${base_url}/asset/file/${STREAM_FILE_ID}" \
		-H "Authorization: Bearer ${token}")"
	[[ $(jq '.size_bytes' <<<"${page}") -eq ${STREAM_BYTES} ]] ||
		die "dataset not grounded: the stream asset is absent"

	log "dataset gate passed"
}

build_image() {
	if [[ -n ${PERF_SKIP_BUILD:-} ]]; then
		docker image inspect "${IMAGE}" >/dev/null 2>&1 ||
			die "PERF_SKIP_BUILD=1 but ${IMAGE} is absent"
		log "reusing image ${IMAGE}"
		return
	fi
	log "building ${IMAGE} with api-docs and test-seed"
	docker build \
		--build-arg CARGO_FEATURES="api-docs test-seed" \
		-t "${IMAGE}" \
		"${REPO_ROOT}" >&2
}

start_sut() {
	build_image
	WORK_DIR="$(mktemp -d)"
	local log_dir="${WORK_DIR}"
	mkdir -p "${log_dir}/media"
	cp "${PERF_DIR}/server/config.yaml" "${log_dir}/config.yaml"
	PORT="$(random_port)"
	ROOT_URL="http://127.0.0.1:${PORT}"
	CONTAINER_NAME="mnemorium-perf-$$-$(date +%s)"

	log "starting ${IMAGE} on 127.0.0.1:${PORT} (cpus ${CPUS}, memory ${MEMORY})"
	docker run -d \
		--name "${CONTAINER_NAME}" \
		--cpus "${CPUS}" \
		--memory "${MEMORY}" \
		--memory-swap "${MEMORY}" \
		--user "$(id -u):$(id -g)" \
		-p "127.0.0.1:${PORT}:4080" \
		-v "${log_dir}:/app" \
		"${IMAGE}" >/dev/null

	if ! wait_for_health "${ROOT_URL}" 180; then
		docker logs "${CONTAINER_NAME}" >&2 || true
		die "the server did not become healthy"
	fi
	local logs="${ARTIFACTS_DIR}/server-${PORT}.log"
	mkdir -p "${ARTIFACTS_DIR}"
	# Stream logs to a file the scan waiter can read; `docker logs -f` would
	# block, so poll.
	local deadline=$((SECONDS + 180))
	while ((SECONDS < deadline)); do
		docker logs "${CONTAINER_NAME}" >"${logs}" 2>&1 || true
		if grep -q 'reconciled the library root' "${logs}"; then
			break
		fi
		sleep 1
	done
	wait_for_scan "${logs}" 180 || die "the startup scan did not complete"
	assert_scan_clean "${logs}"
	SUT_LOG="${logs}"
	BASE_URL="${ROOT_URL}/api/v1"
	assert_dataset "${BASE_URL}"
}

stop_sut() {
	if [[ -n ${CONTAINER_NAME} ]]; then
		docker logs "${CONTAINER_NAME}" >>"${SUT_LOG:-/dev/null}" 2>&1 || true
		docker rm -f "${CONTAINER_NAME}" >/dev/null 2>&1 || true
		CONTAINER_NAME=""
	fi
	if [[ -n ${WORK_DIR} && -z ${KEEP} ]]; then
		rm -rf "${WORK_DIR}"
		WORK_DIR=""
	fi
}

generate_client() {
	log "generating the typed client from ${ROOT_URL}/doc/openapi.json"
	mkdir -p "${PERF_DIR}/generated"
	local descriptor="${ARTIFACTS_DIR}/openapi.json"
	mkdir -p "${ARTIFACTS_DIR}"
	curl -fsS "${ROOT_URL}/doc/openapi.json" -o "${descriptor}"
	"${K6}" x openapi "${descriptor}" -o "${PERF_DIR}/generated/client.gen.ts" >/dev/null
}

fingerprint() {
	local date commit cpu cores ram k6 image_sha config_sha
	date="$(date -u --iso-8601=seconds)"
	commit="$(git -C "${REPO_ROOT}" rev-parse HEAD 2>/dev/null || echo unknown)"
	cpu="$(awk -F': ' '/model name/ { print $2; exit }' /proc/cpuinfo 2>/dev/null || uname -m)"
	cores="$(nproc 2>/dev/null || echo unknown)"
	ram="$(awk '/MemTotal/ { printf "%.0f GiB", $2 / 1024 / 1024 }' /proc/meminfo 2>/dev/null || echo unknown)"
	k6="$("${K6}" version 2>/dev/null | head -n 1 || echo unknown)"
	image_sha="$(docker image inspect --format '{{.Id}}' "${IMAGE}" 2>/dev/null || echo unknown)"
	config_sha="$(sha256sum "${PERF_DIR}/server/config.yaml" | cut -d' ' -f1)"
	jq -cn \
		--arg date "${date}" \
		--arg commit "${commit}" \
		--arg cpu "${cpu}" \
		--arg cores "${cores}" \
		--arg ram "${ram}" \
		--arg k6 "${k6}" \
		--arg image "${image_sha}" \
		--arg config "${config_sha}" \
		--arg cpus "${CPUS}" \
		--arg memory "${MEMORY}" \
		--arg dataset "local-reference-v1" \
		--arg volumes "galleries=1000 items=5 albums=500 tracks=10 playlists=100 movies=1000 series=200 seasons=3 episodes=10 files=19048" \
		--arg seed_cost "sql=8.5MB boot=1s scan=7s (debug reference)" \
		--arg environment "${PERF_ENVIRONMENT:-local}" \
		'{
		    date: $date,
		    commit: $commit,
		    cpu: $cpu,
		    cores: $cores,
		    ram: $ram,
		    k6: $k6,
		    image: $image,
		    config_sha256: $config,
		    cpus: $cpus,
		    memory: $memory,
		    dataset: $dataset,
		    volumes: $volumes,
		    seed_cost: $seed_cost,
		    environment: $environment
		}'
}

check_strict() {
	local current=$1 previous="${ARTIFACTS_DIR}/fingerprint.json"
	[[ ${STRICT:-0} -eq 1 ]] || return 0
	[[ -f ${previous} ]] || {
		log "no previous fingerprint; writing ${previous}"
		return 0
	}
	if ! diff -u \
		<(jq -S 'del(.date)' "${previous}") \
		<(jq -S 'del(.date)' <<<"${current}") >&2; then
		die "fingerprint mismatch (rerun without --strict to record a new one)"
	fi
}

sample_stats() {
	local output=$1
	printf 'timestamp,cpu_percent,memory\n' >"${output}"
	while true; do
		docker stats --no-stream --format '{{.CPUPerc}},{{.MemUsage}}' "${CONTAINER_NAME}" 2>/dev/null |
			awk -v stamp="$(date -u +%s)" -F',' 'NF == 2 { print stamp "," $1 "," $2 }' >>"${output}" || true
		sleep 2
	done
}

run_k6() {
	local scenario=$1 profile=$2 flow=$3 output_dir=$4
	local -a env_args=(
		--env "SCENARIO=${scenario}"
		--env "PERF_PROFILE=${profile}"
		--env "PERF_BASE_URL=${BASE_URL}"
		--env "PERF_ENVIRONMENT=${PERF_ENVIRONMENT:-local}"
		--env "PERF_REPORT_DIR=${output_dir}"
		--env "PERF_FINGERPRINT=${FINGERPRINT}"
	)
	if [[ -n ${flow} ]]; then
		env_args+=(--env "PERF_FLOWS=${flow}")
	fi
	mkdir -p "${output_dir}"
	log "k6 ${scenario} (${profile}${flow:+, flow ${flow}})"
	local status=0
	(
		cd "${REPO_ROOT}"
		"${K6}" run --quiet "${env_args[@]}" "${PERF_DIR}/main.ts"
	) 2>&1 | tee "${output_dir}/k6.log" || status=$?
	return "${status}"
}

run_once() {
	local scenario=$1 profile=$2 flow=$3
	local stamp
	stamp="$(date -u +%Y%m%dT%H%M%SZ)"
	RUN_DIR="${ARTIFACTS_DIR}/${scenario}-${profile}-${stamp}"
	mkdir -p "${RUN_DIR}"
	# Build first, so the fingerprint names the image the run actually uses.
	build_image
	FINGERPRINT="$(fingerprint)"
	check_strict "${FINGERPRINT}"
	printf '%s\n' "${FINGERPRINT}" >"${ARTIFACTS_DIR}/fingerprint.json"

	start_sut
	generate_client
	sample_stats "${RUN_DIR}/docker-stats.csv" &
	STATS_PID=$!

	local status=0
	run_k6 "${scenario}" "${profile}" "${flow}" "${RUN_DIR}" || status=$?

	kill "${STATS_PID}" 2>/dev/null || true
	STATS_PID=""
	stop_sut
	cp "${SUT_LOG}" "${RUN_DIR}/server.log" 2>/dev/null || true
	LAST_RUN_DIR="${RUN_DIR}"
	[[ -f ${RUN_DIR}/report.md ]] || die "k6 wrote no report"
	log "report: ${RUN_DIR}/report.md"
	return "${status}"
}

cmd_run() {
	local scenario=$1 profile=$2 flow=$3 repeat=$4
	local status=0 attempt
	for ((attempt = 1; attempt <= repeat; attempt++)); do
		if [[ ${repeat} -gt 1 ]]; then
			log "run ${attempt}/${repeat}"
		fi
		run_once "${scenario}" "${profile}" "${flow}" || status=$?
	done
	return "${status}"
}

cmd_baseline() {
	local environment=${PERF_ENVIRONMENT:-local}
	local output="${ARTIFACTS_DIR}/baseline-${environment}.md"
	mkdir -p "${ARTIFACTS_DIR}"
	: >"${output}"
	local scenario profile status=0
	for scenario in smoke load; do
		for profile in read write; do
			log "baseline: ${scenario} (${profile})"
			run_once "${scenario}" "${profile}" "" || status=$?
			{
				printf '\n<!-- %s / %s -->\n\n' "${scenario}" "${profile}"
				cat "${LAST_RUN_DIR}/report.md"
			} >>"${output}"
		done
	done
	if [[ ${status} -ne 0 ]]; then
		die "a baseline run crossed a threshold; see ${output}"
	fi
	log "baseline written to ${output}; promote its numbers into config.ts and Baseline.md by hand"
}

start_flamegraph_sut() {
	local output=$1 profile=$2
	command -v cargo >/dev/null 2>&1 || die "cargo-flamegraph is not on PATH"
	command -v perf >/dev/null 2>&1 || die "perf is not on PATH"
	local paranoid
	paranoid="$(cat /proc/sys/kernel/perf_event_paranoid 2>/dev/null || echo 2)"
	((paranoid <= 1)) ||
		die "kernel.perf_event_paranoid is ${paranoid}; set it to 1 or lower to capture a flamegraph"
	if curl -fsS --max-time 1 http://127.0.0.1:4080/health >/dev/null 2>&1; then
		die "something already answers on 127.0.0.1:4080; stop it first"
	fi

	# `perf record` writes its sample file into the profiled process's working
	# directory and the seeded server writes its database beside it, so both
	# live in the ignored run directory instead of a small /tmp partition.
	WORK_DIR="${ARTIFACTS_DIR}/flamegraph-${profile}-work"
	mkdir -p "${ARTIFACTS_DIR}"
	rm -rf "${WORK_DIR}"
	mkdir -p "${WORK_DIR}/media"
	cp "${PERF_DIR}/server/config.yaml" "${WORK_DIR}/config.yaml"
	require_free_space "${WORK_DIR}" "${FLAMEGRAPH_FREE_KIB}"
	ROOT_URL="http://127.0.0.1:4080"
	BASE_URL="${ROOT_URL}/api/v1"
	log "capturing ${output} from ${WORK_DIR}"
	(
		cd "${WORK_DIR}"
		# 99 Hz: cargo-flamegraph hardcodes `--call-graph dwarf,64000`, and at its
		# 997 Hz default a busy 90 s read capture writes about 90 GiB.
		CARGO_PROFILE_RELEASE_DEBUG=1 cargo flamegraph \
			--freq 99 \
			--manifest-path "${REPO_ROOT}/Cargo.toml" \
			--bin server \
			--features "api-docs test-seed" \
			--output "${output}" >"${ARTIFACTS_DIR}/flamegraph.log" 2>&1
	) &
	FLAME_PID=$!

	wait_for_health "http://127.0.0.1:4080" 300 || die "the profiled server did not become healthy"
	local logs="${ARTIFACTS_DIR}/flamegraph-server.log"
	local deadline=$((SECONDS + 180))
	while ((SECONDS < deadline)); do
		pgrep -f 'target/release/server' >/dev/null 2>&1 && break
		sleep 1
	done
	curl -fsS "http://127.0.0.1:4080/health" >/dev/null
	# The server writes its scan line to the flamegraph log.
	while ((SECONDS < deadline)); do
		if grep -q 'reconciled the library root' "${ARTIFACTS_DIR}/flamegraph.log" 2>/dev/null; then
			break
		fi
		sleep 1
	done
	cp "${ARTIFACTS_DIR}/flamegraph.log" "${logs}" 2>/dev/null || true
	grep -q 'reconciled the library root' "${logs}" || die "the profiled scan did not complete"
	assert_dataset "${BASE_URL}"
}

stop_flamegraph_sut() {
	# Stop the profiled server first: `perf record` ends when its child exits,
	# and `cargo flamegraph` renders the SVG only after perf returns. Signalling
	# the wrapper first would leave the capture unwritten.
	pkill -INT -f 'target/release/server' 2>/dev/null || true
	if [[ -n ${FLAME_PID} ]]; then
		local deadline=$((SECONDS + 120))
		while ((SECONDS < deadline)) && kill -0 "${FLAME_PID}" 2>/dev/null; do
			sleep 1
		done
		if kill -0 "${FLAME_PID}" 2>/dev/null; then
			kill -INT "${FLAME_PID}" 2>/dev/null || true
		fi
		wait "${FLAME_PID}" 2>/dev/null || true
	fi
	FLAME_PID=""
}

# Release a profile's capture as soon as its SVG exists: the sample file dwarfs
# the output, and the next profile must not compete with it for space.
release_flamegraph_work() {
	if [[ -n ${WORK_DIR} ]]; then
		if [[ -n ${KEEP} ]]; then
			echo "kept the capture directory at ${WORK_DIR}" >&2
		else
			rm -rf "${WORK_DIR}"
		fi
		WORK_DIR=""
	fi
}

cmd_flamegraph() {
	local profile=$1
	local -a profiles
	case ${profile} in
	both) profiles=(read write) ;;
	read | write) profiles=("${profile}") ;;
	*) die "unknown profile '${profile}' (read|write|both)" ;;
	esac
	mkdir -p "${ARTIFACTS_DIR}"
	for selected in "${profiles[@]}"; do
		local output="${ARTIFACTS_DIR}/flamegraph-${selected}.svg"
		start_flamegraph_sut "${output}" "${selected}"
		generate_client
		RUN_DIR="${ARTIFACTS_DIR}/flamegraph-${selected}"
		mkdir -p "${RUN_DIR}"
		FINGERPRINT="$(fingerprint)"
		run_k6 "flamegraph" "${selected}" "" "${RUN_DIR}" || true
		stop_flamegraph_sut
		[[ -s ${output} ]] || die "cargo flamegraph wrote no ${output}"
		log "wrote ${output}"
		release_flamegraph_work
	done
}

cmd_client() {
	mkdir -p "${ARTIFACTS_DIR}"
	start_sut
	generate_client
	stop_sut
	log "client written to ${PERF_DIR}/generated/client.gen.ts"
}

cmd_clean() {
	rm -rf "${ARTIFACTS_DIR}" "${PERF_DIR}/generated"
	log "removed ${ARTIFACTS_DIR} and ${PERF_DIR}/generated"
}

MODE="${1:-}"
[[ -n ${MODE} ]] || {
	usage
	exit 1
}
shift

SCENARIO=""
PROFILE="all"
FLOW=""
REPEAT=1

if [[ ${MODE} == run ]]; then
	SCENARIO="${1:-}"
	[[ -n ${SCENARIO} ]] || die "usage: performance.sh run <smoke|load|stress|soak> [options]"
	shift
fi

while (($#)); do
	case $1 in
	--profile)
		PROFILE="${2:-}"
		shift 2
		;;
	--flow)
		FLOW="${2:-}"
		shift 2
		;;
	--repeat)
		REPEAT="${2:-1}"
		shift 2
		;;
	--environment)
		PERF_ENVIRONMENT="${2:-local}"
		shift 2
		;;
	--strict)
		STRICT=1
		shift
		;;
	--keep)
		KEEP=1
		shift
		;;
	*)
		die "unknown argument '$1'"
		;;
	esac
done

case ${PERF_ENVIRONMENT:-local} in
local | ubuntu-latest) ;;
*) die "unknown environment '${PERF_ENVIRONMENT}' (local|ubuntu-latest)" ;;
esac

case ${MODE} in
run)
	case ${PROFILE} in
	read | write | all) ;;
	*) die "unknown profile '${PROFILE}' (read|write|all)" ;;
	esac
	case ${SCENARIO} in
	smoke | load | stress | soak) ;;
	*) die "unknown scenario '${SCENARIO}' (smoke|load|stress|soak)" ;;
	esac
	cmd_run "${SCENARIO}" "${PROFILE}" "${FLOW}" "${REPEAT}"
	;;
baseline)
	cmd_baseline
	;;
flamegraph)
	case ${PROFILE} in
	all) PROFILE="both" ;;
	read | write | both) ;;
	*) die "unknown profile '${PROFILE}' (read|write|both)" ;;
	esac
	cmd_flamegraph "${PROFILE}"
	;;
client)
	cmd_client
	;;
clean)
	cmd_clean
	;;
*)
	usage
	die "unknown mode '${MODE}'"
	;;
esac
