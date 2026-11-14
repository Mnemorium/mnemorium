# Design

## Context

See `proposal.md` - Why. The constraints that shape the approach:

- `openspec/specs/instance/state/spec.md` holds six requirements without `REQ-*`/`SCN-*` identifiers. Five describe the
  state; the sixth describes creation events and redaction and is already being removed by the in-flight
  `specify-logging` change.
- The code already enforces most of the state contract: `FileInstanceStateStore` writes `state.yaml` atomically at
  `0600`, checks the format version, caps the document at 4 KiB and rejects unknown fields; `LoadState` probes the
  datastore with `try_exists`, validates both seeds and refuses a missing state beside an existing datastore.
- The proof lives in `test/configuration/instance_state.py`, with `test/configuration/test_scenarios.py` as its gate.
  `pytest.ini` registers `state_scenario`; `TEST-053`, `TEST-054` and `TEST-055` describe the configuration suite's
  remit.
- `specify-logging` has a spec delta that removes **Identity creation is reported and secrets never leak** from
  `instance/state` and moves it to `REQ-LOGGING-015` and `REQ-LOGGING-008`.
- `specify-configuration` retires `configuration/loading` and creates `configuration`. Its design states that the state
  gate stays in the configuration suite and that the instance identity's ordering against configuration belongs to
  `instance/state`. This change moves both; the two changes must be reconciled.
- `specify-upload` does not reference the state capability.

## Goals / Non-Goals

**Goals:**

- One top-level `state` capability, with stable identifiers and one behaviour per requirement.
- A state specification that owns no configuration ordering and no logging obligation.
- A proof in the system suite, where a capability scenario is observable black box and carries Allure labels, so the
  report groups the scenarios by requirement and scenario without a separate map or meta-test.
- A fail-closed rule for a restored `state.yaml` that another account can read or write.

**Non-Goals:**

- Changing `configuration` or `logging`; the logging requirement is handed off, not restated.
- Re-adding the removed concurrency promise.
- Encrypting `state.yaml`, detecting a datastore/state pair restored from two different instances, or rotating either
  secret.
- Renaming the Rust types or files (`InstanceState`, `instance_state_store.rs`, `load_state.rs`). The capability path
  changes; the code vocabulary stays.
- Changing the `state.yaml` document format, the datastore schema, the HTTP API or the OpenAPI document.

## Decisions

### D1. Retire the old capability and create a new one

`instance/state` is retired with a `REMOVED Requirements` delta for its five state requirements. The new `state`
capability is created at `openspec/specs/state/spec.md`. The change sets `retire_capabilities: true` in
`.openspec.yaml`, so `openspec archive` deletes the emptied `instance/state` spec instead of leaving a requirement-less
file.

**Alternative considered:** keep `instance/state` and rewrite it in place. Rejected because the capability is renamed
(`state`), the proof moves to the system suite, and the archive path for a renamed capability is the same retirement
pattern `specify-configuration` uses for `configuration/loading`.

**Why the sixth requirement is not in this delta:** `specify-logging` already owns its removal. This change assumes that
change archives first. If it does not, its `instance/state` delta and this retirement overlap and neither may archive;
the pre-flight task stops and the two changes are rebased. The retirement delta therefore lists exactly the five state
requirements.

### D2. The state spec owns no configuration and no logging

The new spec states the state-before-datastore rule; it does not state the ordering against configuration. That ordering
remains a startup statement in `TechnicalDesign.md` § 10.2. The state spec also does not state events or redaction; the
logging capability owns both.

**Consequence:** `specify-configuration` currently records the ordering and the configuration-suite gate as owned by
`instance/state`. That statement becomes stale. The configuration owner must update its design/tasks in the same
integration window; this change does not edit another change's artifacts.

**Alternative considered:** keep the before-configuration rule in `state` and add a `configuration` delta. Rejected
because it reintroduces configuration into the state spec and expands the change into a second capability, which the
exploration deliberately excluded.

### D3. Eight requirements, split for the one-behaviour rule

The exploration sketched four requirements. The schema's strict validation caps a requirement description at 500
characters and asks for one behaviour per requirement, so the model splits into three values and the four become eight:

| New requirement | Owns                                                                                            | Old requirement                                                           |
| --------------- | ----------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| `REQ-STATE-001` | the site pepper as 64 hexadecimal characters                                                    | Identity holds only non-derivable secrets                                 |
| `REQ-STATE-002` | the signing secret as 64 hexadecimal characters                                                 | Identity holds only non-derivable secrets                                 |
| `REQ-STATE-003` | the creation instant as Unix seconds                                                            | Identity holds only non-derivable secrets                                 |
| `REQ-STATE-004` | creation seeds: names, generation, authority, differing and malformed seeds                     | Environment seeds apply only at creation                                  |
| `REQ-STATE-005` | state before the datastore; unreadable state stops startup early                                | first half of Identity is resolved before configuration and the datastore |
| `REQ-STATE-006` | missing state beside a datastore, fresh instance, state without datastore, undecidable presence | A missing identity for an existing datastore fails startup                |
| `REQ-STATE-007` | fixed path, atomic write, version, unknown field, size cap, unsupported version                 | first half of Identity persistence is atomic, versioned and private       |
| `REQ-STATE-008` | no other account may read or write the file, including a restored file                          | second half of Identity persistence is atomic, versioned and private      |

The split is not a scope increase: every behaviour was in the old capability or is already enforced by the code. The
model's three values each gain a creation and a restart scenario; the other new scenarios are the missing ones:
malformed seed, undecidable datastore, fixed path, unsupported older version, oversized document, unknown field, failed
atomic write, restored-file privacy.

### D4. The system suite owns the proof

`test/system/state.py` takes the cases from `test/configuration/instance_state.py`. The module uses the shared `sut`
fixture directly because it starts, stops and restarts its own container; it does not use the `server` fixture. It
resolves `/health` and the current-user route through `sut.index()` and representation links, so the system suite's
single-base-literal rule holds.

Every case carries `epic("State")`, `feature("REQ-STATE-00n")` and `story("SCN-STATE-00n-x")`; the Allure report groups
the proof by requirement and scenario. The suite keeps no separate scenario map or meta-test, and `TEST-054` states that
label discipline. The configuration suite loses both files.

**Decode-level scenarios stay in Rust.** A scenario the container cannot reach is proven by the Rust adapter test that
owns it: unsupported version, oversized document, unknown field, failed atomic write, and the undecidable datastore
probe. The system module proves the operator-visible startup and filesystem outcomes.

**Alternative considered:** leave the proof in the configuration suite and add Allure labels there. Rejected: the
configuration suite has no label discipline, its remit is one question per setting, and the state is a lifecycle, not a
setting.

### D5. A restored state file is repaired or refused on the opened descriptor

`FileInstanceStateStore::load` opens the file, then inspects the opened descriptor before reading:

```text
is_file()
owner uid == working-directory owner uid
mode & 0o077 != 0 -> file.set_permissions(0600)
```

The repair uses `std::fs::File::set_permissions` on the descriptor, not a path lookup, so a symlink or file swap between
check and repair cannot bypass it. A regular file owned by the server account with a wider mode is tightened to `0600`
and read; a non-regular file, a file owned by another account, or a file whose mode cannot be changed returns
`InstanceStateStoreError::Insecure` and startup fails. The server account is the account that owns the working directory
the state is written into: the image chowns `/app` to `mnemorium`, and the test harness runs as the uid that owns the
temporary directory. No process-UID crate is needed; the owner comparison uses only `std::os::unix::fs::MetadataExt`.

The failure maps to a `LoadStateError::StateInsecure` whose message names `state.yaml` and the remedy — `chmod 600` for
a mode that cannot be repaired, or `chown` to the working directory's owner for a foreign file — and never a secret. The
file is left unchanged on failure. The pure predicate is unit-tested with 0600, 0644, a directory and a file owner that
differs from the directory owner; the system module proves a 0644 file becomes `0600` and starts, and a 0600 file starts
unchanged.

**Alternative considered:** fail closed on any wider mode. Rejected because a restored backup with a wider mode is a
common operator path and the server can repair the mode it owns; the foreign-owner and failed-repair paths still fail
closed. The trade-off is that the file was readable before startup, so the repair prevents future reads but cannot undo
exposure that already happened.

### D6. The datastore presence probe becomes a port

`LoadState` currently calls `Path::new(DATABASE_PATH).try_exists()` directly, so the `try_exists` error path cannot be
tested without a live permission failure. The design adds a small outbound port:

```text
DatastorePresence::exists() -> Result<bool, DatastorePresenceError>
```

The file adapter maps `NotFound` to `Ok(false)` and every other `try_exists` failure to `Err`. `LoadState` takes the
port as a fourth dependency; `app.rs` wires the file adapter. A mock returns the error in the application test and
proves `LoadStateError::DatastoreUnreadable`; the system suite does not need a permission failure it cannot create
reliably.

**Alternative considered:** a test-only static override of `DATABASE_PATH`. Rejected: it hides a real dependency behind
a test seam and leaves the use case untestable in isolation.

### D7. The atomic write gets an injection seam

`write_atomic` keeps its production behavior but delegates the final rename to a private `write_atomic_with` function
whose rename is a parameter. `save` calls it with `std::fs::rename`. The adapter test calls `write_atomic_with` with a
rename that returns an error, after a valid previous state exists, and asserts that `load` still returns the previous
state and no partial `state.yaml` was written. This proves `SCN-STATE-007-F` without a static or a process-level race.

**Alternative considered:** keep the current scenario mapping to the conflict test and reword the scenario. Rejected:
the conflict test proves that a second writer is refused, not that a crash cannot leave a partial file.

### D8. The in-flight changes are sequenced, not silently overlapped

The preferred order is `specify-logging` first: it removes the sixth requirement and its `instance/state` delta is
applied to a live spec. This change then removes the remaining five and retires the path.

If `specify-logging` cannot land first, this change stops at the pre-flight task and the logging owner rebases its
`instance/state` delta onto `state` (where the requirement is already absent, so the delta is dropped). The two changes
never archive with overlapping deltas.

`specify-configuration` may land independently. Its `configuration` spec is unaffected; only its design/tasks prose
about the state gate and the configuration ordering becomes stale. Its owner updates that prose when the two changes
meet.

### D9. No code rename

The domain type stays `InstanceState`, the port stays `InstanceStateStore`, and the adapter file stays
`instance_state_store.rs`. The capability path and the spec vocabulary become `state`; the code vocabulary is already
consistent with the file and the domain concept. A rename would touch every factory, test helper and rule citation
without changing behavior.

### D10. Documentation follows the spec

`TechnicalDesign.md` § 10.2 points at `state/spec.md`, records the fail-closed privacy rule and states that the
configuration ordering is design, not a state requirement. § 5 updates `TEST-053` through `TEST-055` and the
configuration/system narratives. `Overview.md` § Startup keeps the order but uses "state" consistently. `Glossary.md`
adds instance state, site pepper, token signing secret and instance root (to distinguish the authorization resource
kind). `README.md` tells an operator to restore `state.yaml` at mode `0600` and to `chown` it to the server account.
`pytest.ini` describes the marker as the state scenario.

## Risks / Trade-offs

- **A wider mode is repaired, not refused.** The server sets `0600` before reading, so the file is private from then on;
  exposure before startup is not undone. `README.md` records the backup mode.
- **A file the server cannot repair refuses startup.** A foreign owner or an unwritable file leaves the state
  unreadable; the message names the `chown` or `chmod` remedy. Dropping the owner check would accept a file another
  account owns.
- **The owner check treats the working-directory owner as the server account.** The image chowns `/app` to `mnemorium`
  and the test harness runs as the directory's uid, so the invariant holds there. A deployment that runs from a
  directory owned by another account must chown the state file or the directory; the error names the remedy.
- **The system suite gains startup cases.** They already existed in the configuration suite; the move changes the suite,
  not the number of containers materially. The system runner's timeout and per-case container rule still apply.
- **The Allure labels are the only link to a scenario.** A renamed or duplicate `SCN-*` label is caught by review, not a
  meta-test; the spec remains the source of the identifiers.
- **`specify-logging` and `specify-configuration` overlap.** D8 and the pre-flight task make the overlap explicit; no
  archive happens with two deltas touching the same retired capability.

## Migration Plan

1. Land `specify-logging` first and archive it; confirm `instance/state` no longer carries the logging requirement.
2. Apply `specify-state`: move the proof, add the privacy check and the two test seams, update the docs and archive with
   `retire_capabilities: true`.
3. Rollback: revert the change. The old `instance/state` spec and the configuration-suite cases return; there is no
   schema or persisted-data migration.
4. If `specify-configuration` lands after this change, its owner removes the stale `instance/state` gate and ordering
   statements from its design/tasks; its `configuration` spec is unchanged.
