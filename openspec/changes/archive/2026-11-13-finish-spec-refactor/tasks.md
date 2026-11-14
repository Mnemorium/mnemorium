# Tasks

## 1. The stored-file capability

- [x] 1.1 Author `specs/file/spec.md` with `REQ-FILE-001`–`REQ-FILE-003` and their `SCN-FILE-*` scenarios, stating the
      record, the read and the links as the three separate behaviours `specs/image/spec.md` splits its own into. Verify:
      `openspec validate finish-spec-refactor` passes and every requirement carries at least one scenario.
- [x] 1.2 Write `test/system/file.py` covering every scenario of `REQ-FILE-001`–`003` against the seeded server, each
      test labelled `epic` `File`, `feature` the requirement and `story` the scenario. Verify: the suite passes. This
      change adds no code, so a failure means the delta disagrees with the server — correct `specs/file/spec.md`, not
      the server.
- [x] 1.3 Confirm the representation the suite asserts matches `FileResponse` field for field — identifier,
      root-relative path, media type, size, registration date, modification instant, origin and public flag, plus the
      four optional typed links. Verify: every field the delta names is asserted by a test and every field the handler
      serialises is named by the delta.

## 2. The media-ingest capability

- [x] 2.1 Author `specs/media-ingest/spec.md`, re-authoring the 16 requirements of
      `openspec/specs/library/media-ingest/` with `REQ-MEDIA-INGEST-*`/`SCN-MEDIA-INGEST-*` identifiers and its
      `## Purpose` carried into the delta. Verify: the 16 requirement titles and their scenarios match the retired spec
      one for one, except the who-may clause in `REQ-MEDIA-INGEST-008` that `authorization` owns — a title-by-title diff
      shows no other difference.
- [x] 2.2 Write `test/system/media_ingest.py` for the scenarios a black-box client can observe through the container:
      the startup window with the root present, absent and late; an accepted file, an unsupported one and a non-media
      one; a file added, renamed and removed while the server runs; and content left byte-identical. Label each test
      `epic` `Media ingest`, `feature` the requirement and `story` the scenario. Verify: the suite passes against a
      container with a temporary library mount.
- [x] 2.3 Record inside `test/system/media_ingest.py`, as a module docstring, which scenarios the black-box suite cannot
      observe — probe-failure atomicity, the in-place modification case and the cascade in `REQ-MEDIA-INGEST-016` — and
      why the layer that owns them covers them instead. Verify: every one of the 16 requirements is either covered by a
      test or named in that docstring.

## 3. The album partial update

- [x] 3.1 Author `specs/music/spec.md` with `## ADDED Requirements` holding `REQ-MUSIC-029` and its `SCN-MUSIC-029-*`
      scenarios, naming `View` and `EditMetadata` as `patch_album.rs` decides them. Verify:
      `openspec validate finish-spec-refactor` passes and the fields the requirement names match `PatchAlbumRequest` —
      name, genres, group, production type, release date and cover.
- [x] 3.2 Extend `test/system/music.py` with the five `REQ-MUSIC-029` scenarios, labelled `epic` `Music`, `feature`
      `REQ-MUSIC-029` and `story` the scenario. Verify: the extended suite passes, including the `409 Conflict` on a
      `(name, group)` collision and the `422 Unprocessable Content` on an invalid value.

## 4. The retirement deltas

- [x] 4.1 Author the eight `specs/library/<capability>/spec.md` deltas, each listing under `## REMOVED Requirements`
      every requirement its live spec holds, with a `**Reason**` and a `**Migration**` naming the requirements that now
      own it: `movies` 7, `series` 15, `music-albums` 15, `music-groups` 5, `music-playlists` 17, `cover-art` 4,
      `assets` 6 and `media-ingest` 16. Verify: for each delta, the count of `### Requirement:` headers under the
      REMOVED section equals the count of `### Requirement:` headers in `openspec/specs/library/<capability>/spec.md`.
- [x] 4.2 Copy each `Reason` and `Migration` from the archived delta that already decided that retirement, adapting only
      the `REQ-*` pointers, so the retirement is recorded once rather than re-argued. The two exceptions are
      `library/assets`, which removes all six where its archived counterpart removed five and left the stored-file read
      behind, and `library/media-ingest`, whose relocation is decided here for the first time. Verify: every `REQ-*` a
      `Migration` names exists in `openspec/specs/`.
- [x] 4.3 Confirm no surviving artifact cites a retired capability. Verify: a search of `src/`, `test/`, `docs/` and
      `.agents/` for `library/movies`, `library/series`, `library/music-`, `library/cover-art` and `library/assets`
      returns only the two spots group 5 fixes, and every `REQ-*` cited in `src/` and `test/` names a surviving
      capability.

## 5. Glossary and documentation

- [x] 5.1 Add the `Stored file` term to `docs/development/Glossary.md`, beside `Typed asset`, defining the record a
      stored file is and stating that it is not the bytes it names. Verify: the entry is one sentence, and the table's
      feature-area order puts it next to the terms it relates to.
- [x] 5.2 Rewrite the `Typed asset` definition so it states the concept and no longer names a capability path: keep the
      contrast with the bytes, drop the pointer to `library/assets`. Verify: no definition in the file names an
      `openspec/specs/` path or a capability path, and the term still disambiguates a typed asset from a stored file.
- [x] 5.3 Replace the `library/assets` example path in `.agents/skills/glossary/SKILL.md` with a capability that still
      exists. Verify: the path the example names resolves under `openspec/specs/`.

## 6. Integration check

- [x] 6.1 Confirm nothing this change deletes was contract-bearing. Verify: for each of the eight retirement deltas,
      read the `Migration` lines, and each named `REQ-*` exists in a main spec and states the behaviour the removed
      requirement stated.
- [ ] 6.2 Confirm the tree is consistent after the specs land. Verify: `openspec validate --specs` passes, every
      capability under `openspec/specs/` carries `REQ-*` and `SCN-*` identifiers, and no capability path is nested.

## Workflow follow-up

- Archive the change once review passes. The archive needs `retire_capabilities: true`, which `.openspec.yaml` already
  declares, and it deletes the eight `library/*` specs and prunes `openspec/specs/library/`.
- After archiving, confirm `openspec/specs/library/` is absent and `openspec validate --specs` still passes.
- Revisit `REQ-MEDIA-INGEST-002`'s refusal wording: `SCN-MEDIA-INGEST-002-C` promises a message naming the absent root,
  not the `fail_without_media_root` setting name, so `remove-configuration-test-suite` dropped that assertion. Decide
  whether the requirement should name the setting.
