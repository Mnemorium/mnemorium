# Tasks

Schemathesis already proves a requirement's declarative contract against the served document: every declared status,
media type, response header, envelope shape and `_links`/`_embedded` field belongs to the API fuzz test
(`script/api_fuzz.sh`). Do not add a system case that restates it — a system case proves only the effect the fuzzer
cannot observe, and the task names the fuzz operation that covers the rest.

Do not run the E2E suite (`script/e2e.sh` or `pytest test/e2e`), the configuration suite (`script/configuration.sh`) or
`devenv test` in this change, with no exceptions.

## 1. Validate the specification deltas

- [x] 1.1 Run `openspec validate specify-upload --strict` and verify it passes; fix any requirement-description length
      or scenario-format finding in the delta files without changing the stated behavior.
- [x] 1.2 Verify `specs/upload/spec.md`: 17 `REQ-UPLOAD-*` requirements, each with at least one `SCN-UPLOAD-*` scenario,
      no repeated identifier, and every scenario using four heading hashes; count the scenarios and record the totals.
- [x] 1.3 Verify `specs/library/assets/spec.md` removes exactly 16 requirements, each with a **Reason** and
      **Migration**, and every removed header matches the main spec's header text exactly; `openspec validate` reports
      no unmatched requirement.
- [x] 1.4 Verify the mapping is complete: each of the 16 removed intake requirements names its `REQ-UPLOAD-*` successor,
      except "An upload session belongs to one caller", whose migration names the existing
      `REQ-AUTHORIZATION-018`/`SCN-AUTHORIZATION-002-E`/`SCN-AUTHORIZATION-015-B`; no scenario is lost without a named
      owner.

## 2. System suite: scaffold and session lifecycle

- [x] 2.1 Add `test/system/upload.py` with the shared fixture setup, `@allure.epic("Upload")` on every case and the
      `feature`/`story` labels for the requirement and scenario it proves; verify
      `pytest --collect-only -q test/system/upload.py` lists it without an entry in `pytest.ini`.
- [x] 2.2 Write the `REQ-UPLOAD-001` cases first — a valid begin, an unsafe name, an unsupported content type, a zero
      size, an oversized file under a configured maximum, and a malformed hash — each asserting the observable effect
      (no session readable, no staged content) rather than a status, which Schemathesis owns; run them against the
      current image and verify they pass, recording any divergence per design Decision 7 before changing a test.
- [x] 2.3 Prove `REQ-UPLOAD-002`: a failed begin leaves neither a session nor staged content, and a begin reclaims a
      staging file planted for the identifier it is about to use. Verify in the container's working directory; if the
      next identifier cannot be predicted reliably, move the reclaim case to the application test that owns it and
      record the move in `design.md`.
- [x] 2.4 Prove `REQ-UPLOAD-006`: a session read returns the bitmap, total, expiry, finished flag, file identifier and
      links; an unknown session answers `404` with no body leak; no collection route lists sessions.
- [x] 2.5 Prove `REQ-UPLOAD-013`: a pending session is cancelled with `204` and its staged content is gone; a finished
      session answers `409` and its file stays readable; an unknown session answers `404`.
- [x] 2.6 Prove `REQ-UPLOAD-014`: completing a finished session returns the same file, its bitmap is complete, and a
      later session never reuses its identifier.

## 3. System suite: chunk protocol and completion checks

- [x] 3.1 Prove `REQ-UPLOAD-004` cases first: a valid chunk and its bitmap, an idempotent re-send, a digest mismatch, a
      range mismatch and the malformed/missing header cases. Assert that no bytes and no bitmap change result; leave the
      declared `400`/`422` to Schemathesis. Verify the module passes and each case carries its story label.
- [x] 3.2 Prove `REQ-UPLOAD-005` for a finished session (`409`, no bytes stored); the expired case moves to task 6.3 and
      the sealed case to task 4.1, so no case waits on a race the container cannot stage.
- [x] 3.3 Prove `REQ-UPLOAD-007`: a matching hash proceeds, a mismatch and an incomplete session each register nothing
      and leave the session retryable; verify the file link is absent and the session reports no file identifier.

## 4. Application tests: sealing and reaper mechanisms

- [x] 4.1 Extend the `complete_upload` application test with the `REQ-UPLOAD-008` race: two concurrent completions leave
      one success and one `409`, and a failure after sealing unseals the session so a retry succeeds; verify
      `cargo test complete_upload` passes and the test carries the requirement and scenario labels.
- [x] 4.2 Extend the `reap_uploads` application test for the clock-bound half of `REQ-UPLOAD-015`: a stale verification
      is unsealed and a reaping run does not delay a request; verify `cargo test reap_uploads` passes and the
      aggregate-count record carries no file name or path.

## 5. System suite: completion and attribution

- [x] 5.1 Prove `REQ-UPLOAD-009` with `test/seed/media` fixtures: an image, a video and an audio file each register
      their typed row with the probed values; an audio file reporting no bit depth or block size records them absent;
      content of the wrong family and unprobeable content register nothing. Verify the module passes.
- [x] 5.2 Prove `REQ-UPLOAD-010`: a declared audio type over content that also carries a video track is stored as
      declared, and content with no stream of the declared family is rejected with nothing stored.
- [x] 5.3 Prove `REQ-UPLOAD-011`: completing the same bytes twice for one caller returns the first file, stores no
      second copy and leaves the second session unfinished; a second caller receives its own copy.
- [x] 5.4 Prove `REQ-UPLOAD-017` from the file representation: owner link names the uploader, the public flag is false,
      origin is `UPLOADED`, size and modification instant are present, and the path is root-relative and unique; verify
      the path resolves to the served content under the root.

## 6. System suite: configured limits

- [x] 6.1 Prove `SCN-UPLOAD-001-E` by starting a container with a small `asset.upload.max_file_size_bytes`; verify the
      oversized begin creates no session and the staged directory stays empty.
- [x] 6.2 Prove `REQ-UPLOAD-003`'s configured effect: with a small `asset.upload.chunk_size_bytes`, a chunk at that size
      is accepted and a larger one is refused; prove the mid-session/restart rule in the application test that snapshots
      the value and record where each half is proved.
- [x] 6.3 Prove `REQ-UPLOAD-012` with `expiry_seconds = 1`: a chunk after expiry answers `410`, completion answers
      `410`, a status read answers `404`, staged content is removed, and chunks sent before expiry do not extend it.
      Keep the intentional wait within the 3 s bound of `TEST-056`.
- [x] 6.4 Prove the observable half of `REQ-UPLOAD-015` with `reap_interval_seconds = 1`: an expired session and its
      staged content are removed, orphaned staged content is removed, and one aggregate record reports counts only. If
      the run is flaky inside the 3 s bound, keep the request-visible expiry case here and leave the clock case with
      task 4.2, recording the move in `design.md`.
- [x] 6.5 Delete `test/configuration/asset_settings.py` once tasks 6.1–6.4 prove the same four effects here, so one
      behavior is never proved twice; verify no remaining file or case references the deleted module, and keep the
      shared `begin_upload`/`put_chunk`/`complete_upload` helpers in `test/configuration/harness.py` because the system
      conftest imports them.

## 7. System suite: absent library root

- [x] 7.1 Prove `REQ-UPLOAD-016`: with the library root absent, a begin answers `503` and creates no session, a
      completion answers `503` and registers nothing, and a non-upload route still answers; verify the case starts its
      container in the state the harness supports and record the constraint if the root cannot be removed after start.

## 8. Replace the E2E and configuration proofs

- [x] 8.1 Delete `test/e2e/asset/uc006_upload_media_file.py` and verify every case it proved has a system story with the
      same requirement and scenario; verify no remaining file references the deleted module.
- [x] 8.2 Verify no helper or constant is left unused after the `asset_settings.py` deletion in task 6.5; keep the
      shared `begin_upload`/`put_chunk`/`complete_upload` helpers in `test/configuration/harness.py` because the system
      conftest imports them.
- [x] 8.3 Update `TEST-053` and the § 5 configuration/system text in `docs/development/TechnicalDesign.md`: drop
      `asset_settings.py` from the module list, state that the `asset.upload.*` effects are proven by
      `test/system/upload.py`, and verify the § 5 module sentence names the suite as it now exists.
- [x] 8.4 Verify no scenario is proved twice: each `SCN-UPLOAD-*` appears as a `story` label once in the Python suites,
      and every deleted E2E or configuration case maps to one of them or to a Rust mechanism named in `design.md`.

## 9. Documentation and vocabulary

- [x] 9.1 Add **Upload session**, **Chunk**, **Staged content**, **Library root** and **Integrity hash** to
      `docs/development/Glossary.md` beside the terms they relate to; verify each is used by the new spec and that
      markdown lint passes.
- [x] 9.2 Update the system-suite description in `TechnicalDesign.md` § 5 and the `test/system/conftest.py` docstring to
      name `upload.py` as a feature module; verify the text matches the suite and markdown lint passes.
- [x] 9.3 Read UC-006 in `docs/development/UseCases.md` against `specs/upload/spec.md` and correct any contradiction in
      the use case's postconditions or alternative flows; record that the `## Asset` bounded context is intentionally
      unchanged.

## 10. Integration checks

- [x] 10.1 Run `cargo test` and both clippy commands; verify the Rust suite is green, including the two application
      cases from task group 4 and the tests for the lowercase-hash and resource-deletion fixes.
- [x] 10.2 Run `pytest test/system` against a built image and verify it passes, with the new upload module proving the
      moved scenarios.
- [x] 10.3 Run `script/rule_enforcer.sh check`, the Markdown gates (`lint:md`, `fmt:md`) and
      `openspec validate specify-upload --strict`; verify every check passes.
- [x] 10.4 Verify the served document still declares the outcomes the new spec states: read the upload handlers'
      `utoipa` status rows and compare them with `REQ-UPLOAD-001`/`004`/`007`/`013`/`016`; if a declared status is
      missing, record the divergence and fix the declaration, not the route. Run `script/api_fuzz.sh` if a declaration
      changed.

## Workflow follow-up

- Archive the change after the project's review requirements are satisfied; the archive creates `upload` and applies the
  sixteen removals to `library/assets`.
- After the archive, rewrite the `## Purpose` of `openspec/specs/library/assets/spec.md` to the serving-only sentence;
  the archive preserves an existing Purpose and a delta cannot replace it.
- Verify the archived result: `openspec show upload --type spec` carries 17 requirements and the new Purpose, and
  `openspec show library/assets --type spec` carries 11 requirements and no longer describes accepting an upload.
