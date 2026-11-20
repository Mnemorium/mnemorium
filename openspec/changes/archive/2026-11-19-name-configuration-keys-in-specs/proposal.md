# Proposal

## Why

The `configuration` capability states its purpose as leaving each setting's meaning to the capability that consumes it,
but only 5 of the 20 settings a server accepts are named by path in a durable spec. The other 14 are described only as
"the configured maximum", "the configured lifetime", "the configured window" or "the configured burst", so a reader
cannot go from a key in `config.yaml` to the requirement that owns it, and a key renamed or dropped in code would pass
spec review without a trace. The gap is inconsistent even with the spec redesign that already names
`asset.upload.chunk_size_bytes`, `security.jwt.ttl` and the provider lists explicitly.

One setting has no stated effect anywhere: `asset.upload.reap_batch_size`. `REQ-UPLOAD-015` says the reaper removes
expired **Upload session**s "periodically" and never mentions batching, so the bound that stops one phase from running
past the next tick exists only in code.

## What Changes

- Name every in-scope setting in the requirement that already owns its behaviour:
  - `upload`: `asset.upload.max_file_size_bytes` (`REQ-UPLOAD-001`), `asset.upload.expiry_seconds` (`REQ-UPLOAD-012`),
    and `asset.upload.reap_interval_seconds` with `asset.upload.reap_batch_size` (`REQ-UPLOAD-015`).
  - `media-ingest`: `library.startup.media_root_wait_seconds` and `library.startup.fail_without_media_root`
    (`REQ-MEDIA-INGEST-002`, `REQ-MEDIA-INGEST-003`).
  - `logging`: `logging.ansi` (`REQ-LOGGING-001`), `logging.rotation` and `logging.max_files` (`REQ-LOGGING-002`), and
    `logging.level` (`REQ-LOGGING-003`).
  - `authentication`: `security.rate_limit.burst_size`, `security.rate_limit.period_seconds`,
    `security.rate_limit.trusted_proxies` and `security.rate_limit.client_ip_header` (`REQ-AUTHENTICATION-008`).
- Add the bounded-batch clause `REQ-UPLOAD-015` is missing: each reaper phase handles at most
  `asset.upload.reap_batch_size` items, which gives that setting a stated effect without changing what the reaper does.
- Leave `persistence.sqlite3.max_connections` unnamed by any spec. It sizes the SQLite pool in `init_db`, has no
  consuming capability, and stays a TechnicalDesign § 10.1 startup-only fact.
- Leave `openspec/specs/configuration/spec.md` untouched: its layering, refusal, reporting and immutability requirements
  already own configuration as such, and they are not where a setting's meaning lives.

**BREAKING**: none. No requirement is added or removed, no identifier changes, and no setting changes its accepted
values, default or effect.

## Capabilities

### New Capabilities

None. This change introduces no capability; it names existing settings inside requirements that already exist.

### Modified Capabilities

- `upload`: `REQ-UPLOAD-001`, `REQ-UPLOAD-012` and `REQ-UPLOAD-015` name the settings that already govern them, and
  `REQ-UPLOAD-015` gains the bounded-batch clause that gives `asset.upload.reap_batch_size` its stated effect.
- `media-ingest`: `REQ-MEDIA-INGEST-002` and `REQ-MEDIA-INGEST-003` name the two `library.startup` settings behind the
  absent **Library root** wait and the refuse-or-reconcile decision.
- `logging`: `REQ-LOGGING-001`, `REQ-LOGGING-002` and `REQ-LOGGING-003` name `logging.ansi`, `logging.rotation`,
  `logging.max_files` and `logging.level`.
- `authentication`: `REQ-AUTHENTICATION-008` names the four `security.rate_limit` settings behind the two login limiters
  and the client-address resolution.

## Impact

- **Specs**: four delta specs modify five existing requirements in place; every `REQ-*` and `SCN-*` identifier is
  preserved. No capability is created, split or retired.
- **Code, tests and docs**: none. Every key named here already exists with the same default and effect, and the
  observable behaviour is already proved by the configuration suite (`TEST-053`) and by the capability suites; this
  change makes the owning requirement say which key it means.
- **Deliberately out of scope**: `persistence.sqlite3.max_connections` and `openspec/specs/configuration/spec.md`.
