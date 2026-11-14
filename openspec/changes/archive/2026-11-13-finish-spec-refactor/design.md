# Design

## Context

See [proposal.md](proposal.md) - Why. What shapes the approach is the archive's own contract, established by reading the
OpenSpec CLI and by running it against a scratch copy of this repository:

- `openspec/specs/` is the source of truth. `openspec archive` is the only writer, and it rebuilds a main spec from a
  change's delta for every capability the delta names.
- A capability whose last requirement is removed is **retired**: the archive deletes `spec.md` and prunes the directory
  it leaves empty. It does that only when the change declares `retire_capabilities: true` in `.openspec.yaml`. Without
  the marker the rebuilt spec fails validation ("must have at least one requirement") and the whole archive aborts.
- The five archived changes that specified these retirements carry complete `REMOVED Requirements` blocks but no marker,
  so they never ran. Their `ADDED` halves did run, which is why `movies`, `series`, `music` and `cover-art` exist beside
  the specs that duplicate them.
- `openspec validate` judges one capability at a time, so a duplicate pair passes; there is no cross-capability check.

## Goals / Non-Goals

**Goals:**

- Remove `openspec/specs/library/` without losing a requirement that has no other statement.
- Leave every capability in the tree carrying `REQ-*`/`SCN-*` identifiers, with no nesting.
- Make the two new capabilities testable the way every other capability in this refactor is.

**Non-Goals:**

- Changing any behaviour, route, payload, status, schema or migration.
- Re-authoring `music`, `series`, `movies`, `cover-art` or `download` beyond the one addition `music` needs.
- Reviewing the wording of the retired requirements themselves; they are removed, not repaired.

## Decisions

**Retire through deltas, never by deleting directories.** `git rm -r openspec/specs/library` would leave no reason, no
migration pointer and no snapshot; the archive records all three and restores every target if a later mutation fails.
The cost is writing eight delta files that say nothing new — accepted, because they are the retirement's only record.

**One change, declaring `retire_capabilities: true`.** The retirements and the two relocations produce one consistent
tree, so splitting them leaves an intermediate state where `library/` is half-deleted. The marker is a mechanical
requirement of the archive, verified against a scratch copy before this artifact was written. Alternative considered:
three changes in the shape of the earlier `specify-*` changes; rejected because the deletions would then be a fourth
change whose only content is other changes' decisions.

**Copy the archived `REMOVED` blocks instead of re-deriving them.** The retirement decision has already been reviewed
and recorded; a fresh delta would state the same intent a third time. One deliberate difference: `library/assets` is
retired in full, with all six live requirements removed, where `2026-11-13-download` removed five and left the
stored-file read behind. `file` re-authors that survivor, so keeping it in `assets` would leave a one-requirement
capability holding a contract `file` now owns. `library/media-ingest` is the one retirement with no archived
counterpart: its requirements move into a new capability rather than into an existing one, so this change decides it.

**`media-ingest` re-authors identifiers, not behaviour.** The 16 requirements keep their titles, their scenarios and
their boundaries; only the identifiers are new, and the single who-may clause in `REQ-MEDIA-INGEST-008` is replaced by
attribution alone, because `authorization` owns who holds an action. A rename that also edits behaviour could not be
reviewed by diffing titles, which is exactly the check this change needs.

**`file` states three requirements rather than one.** The survivor is a five-scenario blob covering the record's shape,
the read and the links. Split, it matches the granularity of `image`, `audio` and `video`; unsplit it would be the
thinnest capability in the tree at one requirement, against a floor of four everywhere else.

**`music` gains `REQ-MUSIC-029` for the partial album update.** `PATCH /music/album/{album_id}` is served by
`patch_album.rs`, which requires `View` and then `EditMetadata`, and `music` never states it — the re-authoring dropped
it while `library/music-albums` still carries it. Retiring the old spec without this would delete the only statement of
a live contract. Alternative considered: retire anyway and file an issue; rejected, because the deletion is what creates
the gap.

**The glossary defines `Stored file` and stops citing a capability path.** `Typed asset` is the file's only entry that
names a capability, and it names one being retired. A definition that points at a spec is a citation, and a citation to
a retired capability is broken by this very change; `Stored file` is needed anyway, since `image`, `audio`, `video`,
`authorization` and the glossary itself all use the term undefined. Rewritten, `Typed asset` contrasts two defined terms
instead of routing the reader to a document.

**Two system suites, not one.** Every capability this refactor created ships `test/system/<capability>.py`, and nothing
under `test/system/` touches `/asset/file` or the ingest at all today. Without `media_ingest.py` and `file.py` the two
new capabilities would be the only ones with no black-box proof and no Allure label tying a test to a `REQ-*`/`SCN-*`.

## Risks / Trade-offs

- **The archive may refuse the whole change if any delta is wrong** → every delta names exactly the requirements its
  live spec holds, counted against the spec before writing (7, 15, 15, 5, 17, 4, 6). A `REMOVED` that names a missing
  requirement only warns and is treated as already removed, so a stale name cannot block the archive.
- **A retirement deletes files** → the archive writes every target only after all deltas validate, snapshots them first,
  and prints the exact `git checkout` command for each deleted path. Rollback is that command.
- **Re-authoring 19 requirements can drift from what the server does** → the ingest mapping is carried title-by-title
  with scenarios unchanged, and the two new suites are written against the running server: a spec that disagrees with it
  fails the suite, not the review.
- **`REQ-FILE-003` and `download` both touch `_links.content`** → `file` states that the link exists and names the
  route; `download` owns what a request to it authenticates, decides and returns. No behaviour is stated twice.
- **The new suites assert behaviour this change does not write** → the tests are written first and must pass on the
  current server. A failure means the re-authored spec is wrong, which is the signal the change exists to produce.

## Migration Plan

1. Land `specs/media-ingest/`, `specs/file/` and the `music` addition. Nothing the server serves changes.
2. Land the eight retirement deltas.
3. Update `docs/development/Glossary.md`: add `Stored file`, rewrite `Typed asset`.
4. Add `test/system/media_ingest.py` and `test/system/file.py`.
5. Archive the change. The archive deletes the eight `library/*` specs and prunes `openspec/specs/library/`.

Rollback: `git checkout -- openspec/specs` restores every retired spec; the archive names each deleted path in its own
output for a targeted restore.

## Open Questions

None. The two decisions that could have changed the specs — where `media-ingest` and the stored-file read land — were
settled before this artifact: `specs/media-ingest` and `specs/file`.
