# Design

## Context

`openspec/specs/instance/state/spec.md` declares one requirement about concurrency: _Concurrent first starts create at
most one identity_, with a single **Concurrent creation** scenario. `TEST-056` maps every scenario of that spec to a
test, so the requirement carries a two-container configuration test
(`test/configuration/instance_state.py::test_concurrent_first_starts_persist_one_identity`) and an in-process adapter
test (`file_instance_state_store_concurrent_creation_persists_once`).

The requirement is not a product promise. The server is a single-instance, self-hosted backend: a second process on one
working directory is an operator error, and nothing in the product's surface depends on two processes racing. What the
product does owe is that a persisted identity is never replaced — a requirement that stands on its own (**Identity
persistence is atomic, versioned and private**, **No partially written identity**) and that the same mechanism already
satisfies.

The configuration test also cannot attribute its own failures: when both containers fail to become healthy it asserts an
empty list, discarding the `StartupFailed` objects that hold the exit codes and logs.

## Goals

- Remove the requirement and the two tests that exist only for it, without changing production behaviour.
- Leave the configuration harness able to report a startup it could not observe.

## Non-Goals

- Changing how the identity is loaded, created, persisted or logged.
- Inviting or supporting multi-process operation.
- Adding a rule for configuration-test failure evidence: after this change, every remaining failure path already carries
  the container's exit code and logs through `StartupFailed`.

## Decisions

### D1 — Remove the requirement, keep the mechanism

The exclusive lock on `state.yaml.lock` plus the create-new write stay. They are what makes a _lost_ update impossible
and what the surviving atomicity requirement asserts; the concurrency promise was a property of that mechanism, not its
reason. `save_locked` remains reachable through `save`, and
`file_instance_state_store_never_replaces_an_existing_identity` still covers the `Conflict` branch.

### D2 — Delete both tests rather than one

The adapter test races four threads to assert that exactly one wins. With no requirement to hold it, it pins scheduling
behaviour the product does not promise, and the surviving single-writer tests cover the conflict path. The scenario
mapping in `STATE_SCENARIOS` goes with the configuration test; `test_scenarios.py` needs no edit because its forward and
reverse gates only consider what remains.

### D3 — Poll non-terminal container states

`Sut._run()` returns as soon as a single `docker inspect` read is not `running`, so a container caught in `created` (or
an empty read while `docker inspect` itself fails) surfaces as a startup failure for a server that never failed. Only
`exited`, `dead` and `removing` end the poll; everything else keeps polling until the port appears or the deadline
expires.

### D4 — `StartupFailed` names its cause

The class currently says "server exited with code N" for every failure, including a container that is still running. It
gains an optional reason so the message distinguishes an exit from a publication timeout, while the `exit_code`
attribute its callers assert on is unchanged.

## Risks

- A container stuck in `created` now costs the full `STARTUP_TIMEOUT_S` (20 s) before failing. That is a poll, not a
  deliberate sleep, so `TEST-058` is unaffected.
- Removing a scenario lowers the spec's coverage of the identity lifecycle to what the product actually promises; the
  remaining six requirements and their scenarios are untouched.
