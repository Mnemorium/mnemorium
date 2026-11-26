# Tasks

## 1. Validate the contract change

- [x] 1.1 Run `openspec validate --specs` and `openspec validate capability-owned-authorization --strict`; every
      modified capability delta parses, every requirement keeps a scenario, and no removed identifier is reused. Verify:
      both commands pass.
- [x] 1.2 Compare the removed and added identifiers against the delta files and the design; confirm
      `REQ-AUTHORIZATION-010`, `-018` to `-022`, `-024` and `-026` have no replacement inside `authorization`, and that
      each migration names its owning requirement. Verify: the mapping in `design.md` matches the files.
- [x] 1.3 Confirm no production behavior changes: search `src/` for references to a removed authorization identifier and
      update only comments or `// proves:` annotations. Verify: `git diff -- src/` contains no executable change.

## 2. Make the ownership boundary canonical

- [x] 2.1 Redefine the glossary's **Who-may rule** in `docs/development/Glossary.md` as the binding between an operation
      and the action it requires on a resource, owned by the capability that owns the operation; keep `authorization` as
      the owner of what holding the action means. Verify: the definition is one sentence and cites no rule identifier.
- [x] 2.2 Update the `user` capability Purpose in `openspec/specs/user/spec.md` so it no longer delegates authorization
      wholesale; state that authentication stays with its own capability and this capability owns account administration
      and reads. Verify: the Purpose names both boundaries.
- [x] 2.3 Update the `upload` capability Purpose in `openspec/specs/upload/spec.md` so serving stored content stays with
      `download` and the session's owner-only access belongs to `upload`. Verify: the Purpose names both boundaries.
- [x] 2.4 Update the introduction of `docs/Permissions.md` to say the capability owns the binding and `authorization`
      owns the engine and meaning; keep the PlantUML tree as the canonical picture. Verify: the guide and the glossary
      use the same boundary.
- [x] 2.5 Update the proof section of `docs/development/TechnicalDesign.md` § System test so
      `test/system/authorization.py` proves the engine and each capability suite proves its own bindings. Verify: the
      section names the new ownership and still names `authorization.py` as the engine's primary proof.

## 3. Move the gallery, upload and user proofs

- [x] 3.1 In `test/system/gallery.py`, annotate the existing create case with `SCN-GALLERY-014-A`; add the standard-user
      and admin creation cases for `SCN-GALLERY-014-B` and the explicit-deny case for `SCN-GALLERY-014-C`. Verify: the
      cases pass and every new gallery scenario has a proof.
- [x] 3.2 In `test/system/gallery.py`, prove `SCN-GALLERY-012-A` to `-C` for the library parent, the item parent and the
      surface-not-parent rule; reuse the existing read and item-add cases where they already observe the parent. Verify:
      the new scenarios pass.
- [x] 3.3 In `test/system/gallery.py`, prove `SCN-GALLERY-013-A` to `-C` for the `is_public` mapping and the limit of
      `Public`; merge the existing public-gallery read and the denied-edit case rather than duplicating them. Verify:
      the cases pass and the public gallery still grants no edit, delete or share.
- [x] 3.4 Move `SCN-AUTHORIZATION-018-A` and `-B` from `test/system/authorization.py` to `test/system/upload.py`; merge
      the root case into the existing cancel or read case where it already asserts the `404`. Verify: the upload suite
      passes and the authorization cases are deleted.
- [x] 3.5 Move `SCN-AUTHORIZATION-019-A` to `-E`, `-020-A`, `-021-A` and `-021-B` from `test/system/authorization.py` to
      `test/system/user.py`; merge `-021-A` into the existing own-account case. Verify: the user suite passes and the
      replaced authorization cases are deleted.
- [x] 3.6 Move `SCN-AUTHORIZATION-010-A` to `test/system/user.py` as `SCN-USER-024-A` and update the
      `// proves: SCN-AUTHORIZATION-010-A` comment in `src/lib/application/use_case/update_user.rs`. Verify: the user
      suite passes and the comment names the new identifier.
- [x] 3.7 In the user application tests, prove `SCN-USER-025-A` that the new account resource's authorization parent is
      the instance root. Verify: the test fails if the parent is absent or names another resource.

## 4. Move the catalogue proofs

- [x] 4.1 In `test/system/movies.py`, prove `SCN-MOVIES-006-A` to `-C` and `SCN-MOVIES-007-A` to `-C`; move the film
      cases from `test/system/authorization.py` where they already exercise `SCN-AUTHORIZATION-022-A`, `-022-B` and
      `-023-B`. Keep the retained `SCN-AUTHORIZATION-023-B` example on the same case as `SCN-MOVIES-007-A`. Verify: the
      movies suite passes and the removed `022` cases are deleted.
- [x] 4.2 In `test/system/series.py`, prove `SCN-SERIES-014-A` to `-C` and `SCN-SERIES-015-A` to `-C` with a
      standard-user read, a refused show rename, an admin season edit, a refused show creation, an admin creation and a
      denied catalogue. Verify: the series suite passes and the cases observe the seeded roles.
- [x] 4.3 In `test/system/music.py`, prove `SCN-MUSIC-031-A` to `-C` and `SCN-MUSIC-032-A` to `-D`; move the album,
      group and playlist cases from `test/system/authorization.py` where they already exercise
      `SCN-AUTHORIZATION-022-B`, `-022-C`, `-023-C`, `-023-F`, `-023-G` and `-024-B`. Keep the retained `023` examples
      on the same cases as the new music scenarios, and prove the playlist's library parent and the catalogue resource's
      catalogue parent through their decisions. Verify: the music suite passes and the removed `022` and `024` cases are
      deleted.
- [x] 4.4 Confirm `REQ-MUSIC-025` already proves the playlist-entry pair (`SCN-MUSIC-025-A` to `-C`); no new music
      requirement is added, and the removed `REQ-AUTHORIZATION-026-B` has no remaining proof obligation. Verify: the
      traceability report maps the pair to `SCN-MUSIC-025-B`.
- [x] 4.5 Keep `SCN-AUTHORIZATION-012-B` and the retained `023` examples only where they prove the engine's ownership or
      creation rule; delete the cases that move to a capability suite so no failure is reported twice. Verify:
      `test/system/authorization.py` still proves the general ownership rule and no moved case remains.

## 5. Move the cover proof and sweep the citations

- [x] 5.1 In `test/system/cover_art.py`, annotate the existing unreadable-cover case with `SCN-COVER-ART-008-A`; add or
      merge `SCN-COVER-ART-008-B` for the both-allow outcome. Verify: the cover-art suite passes and the cover write
      still answers `409` for an unviewable image.
- [x] 5.2 Search `src/` for the removed identifiers and update only the citations that name a moved requirement. Verify:
      `rg "REQ-AUTHORIZATION-(010|018|019|020|021|022|024|026)|SCN-AUTHORIZATION-(010|018|019|020|021|022|024|026)"     src/`
      returns no stale citation; `REQ-AUTHORIZATION-025` and its scenarios stay untouched.

## 6. Regenerate and verify the contract

- [x] 6.1 Regenerate `docs/development/artifacts/RequirementsTraceabilityCoverage.html` with `script/investigation.sh`.
      Verify: every removed scenario is gone, every added scenario is covered, and no stale identifier is reported.
- [x] 6.2 Run `script/investigation.sh --check`. Verify: the report is current and the command passes.
- [x] 6.3 Run the system suites touched by the change:
      `pytest test/system/authorization.py test/system/gallery.py test/system/upload.py test/system/user.py test/system/movies.py test/system/series.py test/system/music.py test/system/cover_art.py`.
      Verify: all pass against the built image.
- [x] 6.4 Run the full gate: `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`,
      `cargo fmt --all -- --check`, `script/rule_enforcer.sh check`, `markdownlint-cli2 '**/*.md'` and
      `prettier --check '**/*.md'`. Verify: all pass, and `git diff` shows only the spec, documentation, test-annotation
      and comment changes.

## Workflow follow-up

- Archive the change after the review requirements are satisfied.
- Verify the archived main specs no longer contain the removed authorization requirements and that each capability spec
  carries its new rule.
