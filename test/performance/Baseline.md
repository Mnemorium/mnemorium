# Performance baseline

This file is the human record of the promoted performance baseline. k6 reads its thresholds from
`test/performance/config.ts`; the two must agree. A row is promoted by a pull request that states the fingerprint it was
measured on, not by CI and not by the run script.

## Method

- The SUT is the repository image built with `api-docs test-seed`, run in a disposable working directory, capped at four
  CPUs and 4 GiB.
- Three metrics are recorded per flow: completed iterations per second, the p50/p95/p99 of `http_req_duration`, and the
  HTTP and business-check failure rates. p50 is recorded and never headlined.
- One measured run per scenario. A number that looks wrong is repeated before it is promoted; a median of runs is not
  taken.
- Runs that fail the dataset gate, or whose startup scan reports anything other than `registered=0 changed=0 removed=0`,
  are not eligible.

## Environment: ubuntu-latest

No baseline row is promoted yet. A scheduled smoke run is report-only until two consecutive runs agree on the same
fingerprint.

| Fingerprint  | Value                                                |
| ------------ | ---------------------------------------------------- |
| Date         | not recorded                                         |
| Commit       | not recorded                                         |
| CPU          | GitHub-hosted runner, 4 vCPU                         |
| RAM          | 16 GiB                                               |
| SUT caps     | 4 CPUs, 4 GiB                                        |
| k6           | not recorded                                         |
| Image digest | not recorded                                         |
| Dataset      | local-reference-v1 (19 048 media rows, 19 048 files) |
| Config hash  | not recorded                                         |

| Flow             | iters/s | p50 ms | p95 ms | p99 ms | http err | check err | Verdict |
| ---------------- | ------: | -----: | -----: | -----: | -------: | --------: | ------- |
| login            |       — |      — |      — |      — |        — |         — | n/a     |
| browse-galleries |       — |      — |      — |      — |        — |         — | n/a     |
| browse-music     |       — |      — |      — |      — |        — |         — | n/a     |
| browse-video     |       — |      — |      — |      — |        — |         — | n/a     |
| read-asset       |       — |      — |      — |      — |        — |         — | n/a     |
| upload-asset     |       — |      — |      — |      — |        — |         — | n/a     |
| create-gallery   |       — |      — |      — |      — |        — |         — | n/a     |
| curate-playlist  |       — |      — |      — |      — |        — |         — | n/a     |

### Resources

Peak CPU and peak RSS are recorded by the run's `docker-stats.csv` once a baseline is promoted.

## Environment: local reference

No baseline row is promoted yet. The reference is the development workstation; its fingerprint is recorded by each run
and compared with `--strict`.

| Fingerprint  | Value                                                |
| ------------ | ---------------------------------------------------- |
| Date         | not recorded                                         |
| Commit       | not recorded                                         |
| CPU          | not recorded                                         |
| RAM          | not recorded                                         |
| SUT caps     | 4 CPUs, 4 GiB                                        |
| k6           | not recorded                                         |
| Image digest | not recorded                                         |
| Dataset      | local-reference-v1 (19 048 media rows, 19 048 files) |
| Config hash  | not recorded                                         |

| Flow             | iters/s | p50 ms | p95 ms | p99 ms | http err | check err | Verdict |
| ---------------- | ------: | -----: | -----: | -----: | -------: | --------: | ------- |
| login            |       — |      — |      — |      — |        — |         — | n/a     |
| browse-galleries |       — |      — |      — |      — |        — |         — | n/a     |
| browse-music     |       — |      — |      — |      — |        — |         — | n/a     |
| browse-video     |       — |      — |      — |      — |        — |         — | n/a     |
| read-asset       |       — |      — |      — |      — |        — |         — | n/a     |
| upload-asset     |       — |      — |      — |      — |        — |         — | n/a     |
| create-gallery   |       — |      — |      — |      — |        — |         — | n/a     |
| curate-playlist  |       — |      — |      — |      — |        — |         — | n/a     |

### Resources

Peak CPU and peak RSS are recorded by the run's `docker-stats.csv` once a baseline is promoted.

## Threshold policy

| Metric                      | Rule                        |
| --------------------------- | --------------------------- |
| p95                         | at most 1.20× the baseline  |
| p99                         | at most 1.30× the baseline  |
| throughput                  | at least 0.90× the baseline |
| HTTP error rate             | at most 0.1%                |
| business-check failure rate | at most 0.1%                |

An environment without a promoted row reports its values with `n/a` verdicts and enforces only the error-rate
thresholds. A row is promoted with the exact fingerprint it was measured on, and a run on a different fingerprint is
marked as such by `--strict`.

## Known-unstable

Nothing is promoted yet, so no metric has a measured spread. A metric whose spread exceeds 10% on a repeat is recorded
here and is not promoted until it settles.

## Non-goals

- Response-schema validation.
- Event or message publication.
- Configuration, administration and provider-backed metadata flows.
- Destructive use cases as measured traffic.
