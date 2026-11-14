# Tasks

## 1. Migrate the missing proofs into the system suite

- [x] 1.1 Add `SCN-API-015-C` to `test/system/api.py`: walk the **API index** links unauthenticated, treat `self`,
      `login`, `register`, `token` and `health` as public, and assert every other link answers the `401` envelope;
      verify `pytest test/system/api.py` passes.
- [x] 1.2 Add `REQ-AUTHORIZATION-019` `-A` and `-B` to `test/system/authorization.py`: an **Admin** creates a **Standard
      User**, and an Admin creating an Admin answers `403` and stores nothing; verify the module passes.
- [x] 1.3 Add `REQ-AUTHORIZATION-019` `-C`, `-D` and `-E` to `test/system/authorization.py`: an Admin deleting an Admin
      peer or itself answers `403`, the **Root Admin** deletes an Admin, and deleting or updating account `0` answers
      `403`; verify the module passes.
- [x] 1.4 Add `REQ-AUTHORIZATION-020` `-A` to `test/system/authorization.py`: an Admin replaces a Standard user's
      resource grants and the replacement is readable; verify the module passes.
- [x] 1.5 Add `SCN-AUTHORIZATION-017-A` to `test/system/authorization.py`: the Root Admin reads, appends media to and
      deletes another caller's private **Gallery**; verify the module passes.
- [x] 1.6 Add `SCN-GALLERY-006-C` to `test/system/authorization.py`: an Admin deletes a readable public Gallery and
      receives `403`, and the Gallery survives; note in the module docstring that the labels stay the gallery
      capability's; verify the module passes.
- [x] 1.7 Add `SCN-AUTHORIZATION-038-A` to `test/system/gallery.py`: adding to and removing from a Gallery the caller
      cannot read answer `404` and change nothing; verify the module passes.
- [x] 1.8 Run the complete system suite against a freshly built image (`script/system.sh`) and confirm the new cases
      pass before any file is deleted; verify the seven new cases appear green in the report.

## 2. Retire the suite, the harness and their references

- [x] 2.1 Delete `test/e2e/` including `conftest.py`, `fixture/` and stale bytecode; verify
      `git grep -l "test/e2e" --     '*.py' '*.ini'` returns nothing and `pytest --collect-only -q` collects no E2E
      module.
- [x] 2.2 Delete `script/e2e.sh` and retarget the build hint in `script/configuration.sh` and `script/system.sh` to
      `docker build -t mnemorium:latest .`; verify `shellcheck script/*.sh` and `shfmt -d script/*.sh` pass and no
      script names `e2e.sh`.
- [x] 2.3 Update `pytest.ini`: drop `test/e2e` from `testpaths` and rewrite the header comment that describes the
      use-case-named modules; verify a bare `pytest --collect-only -q` succeeds.
- [x] 2.4 Remove the `test/e2e/` entry from `.dockerignore`; verify `docker build` still succeeds and the file has no
      `e2e` line.
- [x] 2.5 Update `docs/development/TechnicalDesign.md` § 5: retire the E2E subsection, the handler-strategy and
      native-binary references, `TEST-003`, `TEST-004`, `TEST-052` and `TEST-058`, and the testing introduction; verify
      the remaining text names only the system and configuration suites and the Markdown gates pass.
- [x] 2.6 Update the command tables in `AGENTS.md` and `README.md`: remove the E2E row and the E2E wording in the
      coverage row; verify the tables and their alignment still render.
- [x] 2.7 Update `.opencode/agents/test-specialist.md`, `.opencode/agents/execution-governor.md` and
      `.opencode/agents/lead-reviewer.md`: remove the E2E layer, its report and its gate row; verify no prompt names
      `test/e2e` or `script/e2e.sh`.

## 3. Correct the documentation that describes the retired model

- [x] 3.1 Rewrite `UC-024` in `docs/development/UseCases.md` for the resource-and-grant model — the permission summary,
      the resource permission read/replacement and the share — with no per-user override, no scope value and no route
      the server no longer serves; verify the entry agrees with `docs/Permissions.md` and contains none of `ALL`,
      `PUBLIC_OR_OWNED`, `OWNED`, `LISTED`, `NONE`.
- [x] 3.2 Correct UC-009 and UC-020 alternative flow 4a so that only the Root Admin may use every file; verify no
      remaining sentence grants an Admin every file.
- [x] 3.3 Verify `git grep -n "scope" docs/development/UseCases.md` finds no permission-scope reference.

## 4. Re-wire CI

- [x] 4.1 Remove the E2E step, the `e2e-report` artifact upload and the stop-container step from the `system` job in
      `.github/workflows/ci.yml`, keeping image build, configuration and system runs; rename the buildx cache keys from
      `e2e` to `system`; verify `yamllint -c .yamllint .github/workflows/ci.yml` passes and the workflow names no E2E
      command.
- [x] 4.2 Leave the `test-e2e` commit scope in place as recorded in the proposal; verify no other workflow or scope
      check now references the deleted suite.

## 5. Integration verification

- [ ] 5.1 Run `script/rule_enforcer.sh check` and confirm every rule passes.
- [x] 5.2 Run `script/configuration.sh` and `script/system.sh` against the same image and confirm both suites pass.
- [ ] 5.3 Run `markdownlint-cli2 '**/*.md'` and `prettier --check "**/*.md"` and confirm both pass.
- [x] 5.4 Run `git grep -n -i "e2e"` and confirm the only hits are the parked `test-e2e` commit scope, archived changes
      and historical vendor paths.
- [x] 5.5 Run `openspec validate remove-e2e-test-suite --strict` and confirm the change validates with `specs` skipped.

## Workflow follow-up

- Archive the change after the project's review requirements are satisfied.
- Verify the archived change carries no `specs/` directory and keeps its glossary references.
