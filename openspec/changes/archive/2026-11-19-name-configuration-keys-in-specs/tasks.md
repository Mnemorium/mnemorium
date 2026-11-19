# Tasks

## 1. Upload delta

- [x] 1.1 Confirm `specs/upload/spec.md` names `asset.upload.max_file_size_bytes` in `REQ-UPLOAD-001`'s description and
      in `SCN-UPLOAD-001-A`/`-E`, verified by `grep -c 'asset.upload.max_file_size_bytes' specs/upload/spec.md`
      returning 3.
- [x] 1.2 Confirm `REQ-UPLOAD-012` names `asset.upload.expiry_seconds` in its description and keeps
      `SCN-UPLOAD-012-A`–`-D` unchanged, verified by `grep -c 'asset.upload.expiry_seconds' specs/upload/spec.md`
      returning 1 and `grep -c 'SCN-UPLOAD-012-' specs/upload/spec.md` returning 4.
- [x] 1.3 Confirm `REQ-UPLOAD-015` names `asset.upload.reap_interval_seconds` once and `asset.upload.reap_batch_size`
      once, states the per-phase bound, and keeps its five scenarios, verified by those greps returning 1 each and
      `grep -c 'SCN-UPLOAD-015-'` returning 5.

## 2. media-ingest delta

- [x] 2.1 Confirm `REQ-MEDIA-INGEST-002` and `REQ-MEDIA-INGEST-003` name the two `library.startup` settings in the
      description and in the scenarios that decide on them, verified by
      `grep -c 'library.startup.media_root_wait_seconds' specs/media-ingest/spec.md` returning 2 and
      `grep -c 'library.startup.fail_without_media_root'` returning 6.
- [x] 2.2 Confirm the block keeps `SCN-MEDIA-INGEST-002-A`–`-D` and `SCN-MEDIA-INGEST-003-A`–`-C` with their outcomes
      intact, verified by the scenario identifiers and `THEN` lines matching the main spec apart from the named keys.

## 3. logging delta

- [x] 3.1 Confirm `REQ-LOGGING-001` names `logging.ansi`, verified by `grep -c 'logging.ansi' specs/logging/spec.md`
      returning 3.
- [x] 3.2 Confirm `REQ-LOGGING-002` names `logging.rotation` (2) and `logging.max_files` (4), verified by the greps
      returning those counts.
- [x] 3.3 Confirm `REQ-LOGGING-003` names `logging.level` (5) and keeps `SCN-LOGGING-003-A`–`-D`, verified by the greps
      returning those counts and 4.

## 4. authentication delta

- [x] 4.1 Confirm `REQ-AUTHENTICATION-008` names `security.rate_limit.burst_size`, `security.rate_limit.period_seconds`,
      `security.rate_limit.trusted_proxies` and `security.rate_limit.client_ip_header` in the description and the
      scenario that varies each, verified by `grep -c` returning 2 for every one of the four paths.
- [x] 4.2 Confirm the block keeps `SCN-AUTHENTICATION-008-A`–`-E` with unchanged outcomes, verified by
      `grep -c 'SCN-AUTHENTICATION-008-'` returning 5.

## 5. Contract integrity

- [x] 5.1 Confirm each `MODIFIED` block reproduces its main-spec counterpart with no change beyond the intended path
      insertions, verified by diffing every delta block against `openspec/specs/<capability>/spec.md` and finding only
      the setting-name substitutions.
- [x] 5.2 Confirm no scenario identifier was added or removed, verified by comparing the `SCN-` identifier list in each
      delta with the main spec (upload 3 blocks / 15 scenarios; media-ingest 2 / 7; logging 3 / 11; authentication 1 /
      5).
- [x] 5.3 Confirm the change's specification artifacts validate, verified by
      `openspec validate name-configuration-keys-in-specs --strict` reporting the change valid.
- [x] 5.4 Confirm the two deliberate exclusions hold, verified by
      `grep -R 'persistence.sqlite3.max_connections' openspec/changes/name-configuration-keys-in-specs/specs` finding
      nothing and `git diff --exit-code -- openspec/specs/configuration/spec.md` exiting 0.
- [x] 5.5 Confirm the change touched no project file, verified by `git status --short` listing no change outside
      `openspec/changes/name-configuration-keys-in-specs/`.

## Workflow follow-up

- Archive the change once review is complete; archive merges the four deltas into their main specs.
- Verify the archive by checking that every one of the 19 in-scope setting paths resolves to exactly one owning spec
  (`upload` 5, `metadata-enrichment` 2, `media-ingest` 2, `logging` 4, `authentication` 6) and that
  `persistence.sqlite3.max_connections` still appears in none.
