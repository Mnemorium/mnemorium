# Testing

This page owns the black-box performance suite. The testing strategy and the unit, integration and system conventions
live in [Technical Design](TechnicalDesign.md) § 5; this page is the one home for how the server is measured under load
and what a failing measurement means.

## System test coverage

`docs/development/artifacts/SystemTestCoverage.md` is the generated map of the specification tree's requirements and
scenarios to the tests that prove them. It is committed, and `script/investigation.sh` (or
`devenv tasks run system:investigation`) regenerates it from `openspec/specs/**/spec.md`, the `@scenario` decorators in
`test/system/*.py` and the `// proves:` comments in `src/` — no pytest run and no container.

Status is presence-based: `Tested` means a committed test claims the scenario and CI is green, not that a run passed. In
a requirement row the eight type columns count the scenario-type pairs claimed, so `0` means a type has no proof yet;
`N/A` is a hand judgement that the type does not apply, and the generator warns in `## Contradictions` when a proof
appears behind it. `Unclassified` counts claimed scenarios that declare no type. In a scenario row, `Cause` is the one
hand-authored place for "another layer owns this" or "nobody wrote it yet"; a `Not tested` scenario with an empty
`Cause` is the worklist, and one that a Rust test proves is filled with the test's path and function automatically.

The generator never overwrites authored text: a type cell survives unless it is empty or a bare integer, a `Cause`
survives while it is non-empty, and a row whose identifier leaves the specification moves to `## Detached notes` rather
than disappearing. A regeneration with nothing changed produces no diff.

## Performance suite

The suite treats the server as a black box: k6 drives the public HTTP API of a disposable, seeded container, and no test
imports Rust code. It measures three things:

1. **Throughput** — completed flow iterations per second and HTTP requests per second.
2. **Tail latency** — p95 and p99 of `http_req_duration`, per flow. p50 is recorded but never headlined.
3. **Error rate** — HTTP failures (`http_req_failed`) and business-check failures (`checks`), reported separately.

The committed baseline is `test/performance/Baseline.md`. Run artifacts are never committed: they are written to an
ignored run directory, and CI publishes one zip of the whole directory.

### Entry points

| Task                               | What it does                                                                    |
| ---------------------------------- | ------------------------------------------------------------------------------- |
| `devenv tasks run setup:perf`      | `npm ci` for the suite's TypeScript toolchain                                   |
| `devenv tasks run perf:client`     | Build, boot and dataset-gate the SUT, then regenerate `generated/client.gen.ts` |
| `devenv tasks run perf:smoke`      | 30 s per flow, one VU, both profiles                                            |
| `devenv tasks run perf:load`       | 2 min per flow at its promoted rate, both profiles                              |
| `devenv tasks run perf:stress`     | VU ladder to 100 plus one upload worker                                         |
| `devenv tasks run perf:soak`       | 15 min read mix at the target rate                                              |
| `devenv tasks run perf:baseline`   | Smoke and load for both profiles into a candidate report                        |
| `devenv tasks run perf:flamegraph` | Local read and write flamegraphs                                                |
| `devenv tasks run lint:typescript` | `tsc --noEmit`, ESLint and Prettier                                             |

The same commands are available as `script/performance.sh run`, `... baseline`, `... flamegraph`, `... client` and
`... clean`; the script is the only implementation and the tasks are thin wrappers. The full flag set is in the script's
header (`--profile`, `--flow`, `--repeat`, `--strict`, `--keep`).

### Method

1. `docker build --build-arg CARGO_FEATURES="api-docs test-seed"` builds the SUT from the working tree. The image is
   never pulled from a registry.
2. A disk-backed temporary directory becomes `/app`: it owns `mnemorium.db`, `state.yaml`, `media/` and the SUT's
   `config.yaml`, so the caller's development datastore is never touched. The container is capped at
   `--cpus 4 --memory 4g --memory-swap 4g`, bound to a random loopback port, and runs as the invoking user so the bind
   mount is writable.
3. The script waits for `/health`, then for the startup scan's `reconciled the library root` line. The line must report
   `registered=0`, `changed=0` and `removed=0`; anything else means the disk and the rows disagree and the run stops.
4. The dataset gate reads one page per collection and the stream asset. It asserts the seeded identifier range and the
   page shapes. A run whose library was emptied stops with `dataset not grounded` instead of reporting latencies on
   404s.
5. `GET /doc/openapi.json` is fetched and `k6 x openapi` regenerates `test/performance/generated/client.gen.ts`. The
   generated file is never committed; a handler change changes the client the next time it is generated.
6. `k6 run test/performance/main.ts` runs the selected scenario. `SCENARIO` selects the shape and
   `PERF_PROFILE`/`PERF_FLOWS` the flows.
7. `docker stats` is sampled every two seconds for the measurement window.
8. The container is removed and `report.md`, `summary.json`, `k6.log`, `server.log` and `docker-stats.csv` are left
   under the run directory.

### Flows and use cases

| Flow               | Use cases              | Endpoints                                                                                                   | Correctness under load                                                                                                                                   |
| ------------------ | ---------------------- | ----------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `login`            | UC-002                 | `POST /identity/login`, `GET /user/me`                                                                      | 200; `access_token` non-empty; `/me` reports the same username                                                                                           |
| `browse-galleries` | UC-008                 | `GET /library/gallery`, `GET /library/gallery/{id}`, `GET /library/gallery/{id}/item`                       | page length and `next` presence match the remaining collection; every row is in the seeded range; the item route is byte-identical to the gallery detail |
| `browse-music`     | UC-012, UC-014         | `GET /library/music/album`, `GET /library/music/playlist`, `GET /library/music/album/{id}/medium/{n}/track` | page shape; ten album tracks with indices `0..9`; a second read returns the same order; the playlist detail's `track_count` matches its five tracks      |
| `browse-video`     | UC-022, UC-023         | `GET /library/movie`, `GET /movie/{id}`, `GET /series`, `GET /series/{id}/season/{n}/episode`               | `video.file_id` present; ten episodes per season; a second read returns the same order                                                                   |
| `read-asset`       | UC-031                 | `GET /asset/file/{id}`, `GET /asset/file/{id}/content` (whole and `Range`), `GET /asset/image/{id}/content` | 200/206; the body length matches `Content-Length`; `Content-Range` names the seeded stream                                                               |
| `upload-asset`     | UC-006                 | `POST /asset/upload`, `PUT /asset/upload/{id}/chunk/{n}`, `POST /asset/upload/{id}/complete`                | session created once; a chunk replay keeps the same bitmap; completion replay returns the same file; the file is readable once                           |
| `create-gallery`   | UC-007, UC-009         | `POST /library/gallery`, `POST /library/gallery/{id}/item`, `GET`                                           | 201 with `Location`; the unique name resolves to one row; the attached item appears once                                                                 |
| `curate-playlist`  | UC-013, UC-016, UC-018 | `POST /library/music/playlist`, `PATCH .../{id}`, `PUT .../{id}/track`, `GET`                               | append raises `track_count`; reorder is a permutation; no duplicate track rows                                                                           |

Teardown is not measured. Deletes are covered by the system suite; see [Known limitations](#known-limitations).

### Endpoint tags

Every measured request carries an `endpoint` tag, added by `endpointTag()` in `config.ts` and named in the closed
`Endpoint` union beside it. An endpoint name belongs to exactly one flow: the per-endpoint submetric is single-tag
(`http_req_duration{endpoint:list-galleries}`), so a name two flows shared would merge their traffic into one row. A
flow that drives a route another flow already owns names its own call for what it does — `list-galleries-by-name`,
`get-created-gallery`, `get-curated-playlist`, `get-uploaded-file`.

`thresholdsFor` materialises one `http_req_duration` and one `http_reqs` submetric per selected endpoint, which lets
`handleSummary` print the report's `## Endpoints` table: request rate, p50, p95 and p99 per flow and endpoint. The
endpoint thresholds are a 60 s hang guard, never a budget: no verdict reads them, and a promoted baseline stays per
flow. A request that went out untagged prints em dashes rather than dropping its row.

### Dataset

The volume library lives in `test/performance/seed` and is applied only when `MNEMORIUM_SEED_PROFILE=perf`, which
`script/performance.sh` sets on the container it measures. `MNEMORIUM_SEED_PROFILE=api` applies `test/api/seed` instead:
the small demo library Schemathesis addresses. The fixtures are data rather than part of the image, so every launcher
mounts or links the checkout's `test/` tree beside the server's working directory: `script/performance.sh` and
`script/system.sh` mount it read-only (the system profile's media is mounted at `media/system`, the directory its `file`
rows name), and the host-binary launchers link it into their scratch directory. A `test-seed` boot that names no profile
applies no fixture at all, and an unknown profile fails startup naming `seed.profile`. Fuzzing the volume library costs
about ten times the wall clock, so it is never the default. The system suite's image is built with `test-seed` and
`api-docs` — the second serves the document the suite's typed client is generated from — and names its own profile. The
SQL is generated by `script/generate_perf_seed.sh` from one preset:

| Object                             | Count | Serves                            |
| ---------------------------------- | ----: | --------------------------------- |
| gallery + 5 items                  | 1 000 | gallery pagination and item reads |
| unattached image                   | 2 048 | the write flows' spare media      |
| album + medium + 10 tracks + audio |   500 | album pages, track reads          |
| playlist + 5 entries               |   100 | playlist pages, track reads       |
| movie                              | 1 000 | film reads                        |
| series × 3 seasons × 10 episodes   |   200 | episode reads                     |

Totals: 19 048 bulk media rows and 19 048 `file` rows, plus 100 playlists with 500 entries, authorization resources and
relations. Identifiers are arithmetic, owned by user 9001 (`alice`), and start at `1_000_000` for files, `2_000_000` for
media, `3_000_000` for galleries and so on.

The seeder writes one hidden hardlink base per media type under `media/perf/.base/` and links every bulk path to it. The
base's size and modification instant are the values the generated `file` rows declare, so the startup scan reports every
file `Untouched`: no probe, no registration, no removal. The base directory is hidden, so the walk never registers it.
One base is a 4 MiB stream, read by `read-asset`.

Measured cost on the reference workstation (debug build, 2026-11-14):

| Measurement                     | Budget  | Measured             |
| ------------------------------- | ------- | -------------------- |
| committed SQL                   | ≤ 25 MB | 8.5 MB               |
| added time per `test-seed` boot | ≤ 5 s   | ≈ 1 s                |
| startup scan with the dataset   | ≤ 10 s  | ≈ 7 s (19 055 files) |

The preset lives in `script/generate_perf_seed.sh`; the numbers are the measured compromise after the first proposal
(165 000 files) missed the SQL and container-start budgets. Shrinking is one preset change and a regenerated SQL file,
with no code change.

### System-suite fixture

The system suite boots the `system` profile from `test/system/seed`: a small browsable catalogue whose SQL scripts and
committed media the suite addresses by fixed identifiers. The three seed trees share one mechanism: `test/api/seed` is
the demo library Schemathesis fuzzes, `test/performance/seed` the volume library above, and `test/system/seed` the
system catalogue; each carries `sql/` scripts and `media/` bytes, resolves from the checkout beside the working
directory, and is mounted read-only by its launcher. A `test-seed` build that names no profile starts unseeded.

A pytest worker boots one shared instance lazily, and every case that does not mark itself
`@pytest.mark.own_instance(reason=...)` drives it. That instance runs the system profile, owns the worker's one writable
working directory and a random loopback port, raises the login quota so the suite's own volume never throttles it, and
hands out cached tokens for the Root Admin and the profile's seeded identity. An own container stays unseeded unless the
case names a profile. The six reasons are `first_boot` (the boot's own secrets or generated files are the subject),
`restart` (the case restarts the same directory), `global_filesystem` (the case reads or changes the working tree or the
library root), `log_sink` (the case installs or reads a sink), `exact_collection` (the case needs an exact, unseeded
collection) and `process_state` (the case needs fixed principals, a login quota or minted tokens).

### Scenarios

| Scenario     | Profiles                 | Shape                                   | Where                     |
| ------------ | ------------------------ | --------------------------------------- | ------------------------- |
| `smoke`      | read + write             | 1 VU, 30 s per flow                     | scheduled CI and dispatch |
| `load`       | read, write              | 2 min at the promoted rate per flow     | dispatch and local        |
| `stress`     | read + one upload worker | VU ladder to 100, abort on HTTP failure | dispatch and local        |
| `soak`       | read                     | 15 min at the promoted rate             | manual only               |
| `flamegraph` | read and write           | 5 rps read, 1 rps write, 90 s each      | local only                |

### Thresholds and the baseline

The policy is per environment, scenario and flow:

| Metric                      | Rule                           |
| --------------------------- | ------------------------------ |
| p95                         | at most 20% above the baseline |
| p99                         | at most 30% above the baseline |
| throughput                  | at least 90% of the baseline   |
| HTTP error rate             | below 0.1%                     |
| business-check failure rate | below 0.1%                     |

`config.ts` carries the promoted rows; k6 enforces them as thresholds and the report prints `PASS`/`FAIL`. An
environment without a promoted row still enforces the error rates, prints every value, and reports `n/a` for the latency
and throughput verdicts. `test/performance/Baseline.md` is the human record of what was promoted and on which
fingerprint.

`script/performance.sh baseline` runs smoke and load and writes a candidate report. It never edits the repository: a
human promotes the numbers into `config.ts` and `Baseline.md` in a pull request that states the fingerprint it was
measured on. `--strict` turns a fingerprint mismatch against the last run into a failure.

### Continuous integration

`performance.yml` runs on `workflow_dispatch` and twice a week (`0 2 * * 1,4`), with the scheduled run restricted to
smoke. Jobs split smoke and load by profile for wall-clock, and stress runs in a third job; soak is never scheduled and
no performance job gates a pull request. Each job writes the job summary and mails `report.md` through the SMTP action
when the mail secret is configured; the zip artifact carries the rest. CI never commits a baseline.

`typescript.yml` runs `npm ci`, `tsc --noEmit`, `eslint .` and `prettier --check` for pull requests that touch the suite
or the dev environment. The type-aware rules that matter are `no-floating-promises` (an un-awaited generated-client call
silently drops a request) and `no-misused-promises`.

### Flamegraph

`devenv tasks run perf:flamegraph` builds a release binary with debug symbols, runs it against a temporary library, and
drives it at a fixed 5 rps read and 1 rps write rate. It writes `flamegraph-read.svg` and `flamegraph-write.svg` into
the run directory.

The rate is deliberately below what the server can serve unsampled: `perf record` writes its sample file at close to a
gigabyte per second when the server is busy, so pushing harder starves the arrival rate and biases the stacks toward the
pages that happened to run. `--freq 99` keeps a 90 s capture to a few GiB and a fold under a minute; the timings it
reports are distorted by the profiler and are never a measurement.

It is local only, never CI, and the host binary only. It needs `kernel.perf_event_paranoid` at 1 or lower; this
workstation defaults to 2, and the task stops with that message rather than producing an empty capture. The profiled
server runs from a scratch directory inside the ignored run directory, because `perf record` writes its sample file into
the profiled process's working directory; the task refuses to start unless that volume has 16 GiB free, and releases the
capture as soon as the SVG exists. A full volume is worth checking first when a capture dies with `connection refused`:
perf aborts on a failed write and takes the server with it. Read the result from the widest server frame down: the top
of the stack is k6's client, and only frames under `mnemorium` are the server.

### Non-goals

- **Response-schema validation** — owned by `test/system/**` and Schemathesis.
- **Correctness for its own sake** — the suite only asserts loss of exactness under load.
- **Event or message publication** — the server has no broker, so there is no event to check.
- **Configuration and administration** — UC-001, UC-003, UC-004, UC-005 and UC-024 are not measured.
- **Provider-backed metadata** — UC-025–UC-030 need a configured provider, and the fixture configures none.
- **Destructive use cases** — deletes are not measured traffic.
- **Speeding up the existing suites** — the cost the volume library adds is measured, not optimised.

### Known limitations

- **A delete frees its identifier.** `delete_playlist`, `delete_user`, `delete_gallery` and `delete_gallery_item` remove
  their entity row and its resource in the same unit of work, so a later create may reuse the freed identifier safely.
  The write flows still leave their created gallery and playlist containers in place: they measure creation and reads,
  not teardown, and every run starts from a fresh database.
- **The baseline sections are candidates.** The local and `ubuntu-latest` sections in `Baseline.md` are populated by a
  human promotion, not by CI or by this implementation. Until a row is promoted, its verdict is `n/a`.
- **The system suite carries its own fixture.** `script/system.sh` builds the image with `test-seed` and `api-docs` and
  the suite names the `system` profile, so the volume library is absent there. Running that suite against a `perf`
  profile fails its list assertions by design; the measurements in this page therefore cover the performance and
  Schemathesis images only.
