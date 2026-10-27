# Tasks

## 1. Remove the requirement

- [x] 1.1 Delete the requirement **Concurrent first starts create at most one identity** and its **Concurrent creation**
      scenario from the instance-state spec; verify `openspec validate --specs` passes and the spec holds six
      requirements.

## 2. Delete the configuration test

- [x] 2.1 Delete the concurrent test, its two markers, the `CONCURRENT` constant, its `STATE_SCENARIOS` entry and the
      imports only it used in `test/configuration/instance_state.py`; verify `test_scenarios.py` passes inside a full
      suite run and `ruff check test/configuration` reports no unused import.

## 3. Delete the adapter test

- [x] 3.1 Delete the concurrent-creation adapter test and the imports only it used in
      `src/lib/infrastructure/outbound/file_system/instance_state_store.rs`; verify
      `cargo test --lib instance_state_store` still covers load, save, overwrite refusal and rejection, and
      `cargo clippy` passes with `-D warnings`.

## 4. Make the harness report a startup it cannot observe

- [x] 4.1 In `test/configuration/harness.py`, treat only `exited`, `dead` and `removing` as terminal so a container
      observed as `created` or `restarting` is polled instead of reported as a startup failure; verify a full run of
      `test/configuration` still starts and stops every server.
- [x] 4.2 Make `StartupFailed` name the failure it carries, so an exited container reports its exit code and a container
      that never published a port reports the timeout; verify the module's `exit_code` assertions still hold.

## Workflow follow-up

- Archive the change after the project's review requirements are satisfied.
- Comment on and close issue #340, naming the removal and the harness fix.
