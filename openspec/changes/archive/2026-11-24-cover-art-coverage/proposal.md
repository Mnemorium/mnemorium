# Proposal

## Why

The generated coverage matrix reports 18 of the 24 `cover-art` scenarios proven. Of the six unclaimed rows, one states a
premise the datastore forbids, one restates a rule the implementation deliberately does not enforce, and four are
ordinary black-box work the suite never wrote. The same reading of the attach path exposed a missing target-side check:
a caller may attach any image identifier as a cover without being able to read the image, while the gallery capability
already requires `View` on the media it references.

`SCN-COVER-ART-004-C` — the link precedes the route — asks for a representation whose cover link names an image route
that is not served. The foreign key guarantees the image row exists, so the only reachable "not served" caller is one
that may not view the image; that is the behaviour `REQ-AUTHORIZATION-025-A` says must not happen. Keeping the scenario
either encodes the defect or duplicates a rule the authorization capability owns.

`REQ-COVER-ART-003` says an image has at most one referrer, but a committed case deliberately shares one `LIBRARY` image
between an album and a group (`test/system/music.py:781`, `:957`), and the authorization backfill expects a `LIBRARY`
image to be reachable through one container. Only a `USER` image has a meaningful one-referrer rule, and the schema
already enforces it.

## What Changes

- **BREAKING (spec only)**: remove `SCN-COVER-ART-004-C` from `REQ-COVER-ART-004` in the main specification. The
  link-exposed half stays proven by `004-A`/`004-B`; the "route not served" premise is unobservable once the foreign key
  and the artwork-read rule hold, and the rule it would test belongs to `REQ-AUTHORIZATION-025-A`. OpenSpec cannot
  express a scenario-level removal (a `MODIFIED` block may not omit a scenario and there is no `REMOVED Scenarios`
  section), so this one edit lands on `openspec/specs/cover-art/spec.md` directly, recorded here and in the design.
- Reword `REQ-COVER-ART-003`: a `USER` image has at most one referrer, a `LIBRARY` image may back several catalogue
  covers, and the scope still fixes on the first attachment. Add `SCN-COVER-ART-003-E` proving a second playlist cannot
  name an image another playlist already backs.
- Extend `REQ-AUTHORIZATION-026` with the cover case: setting, replacing or keeping a cover needs the container's
  metadata write and `View` on the image, both all-of, applying nothing when either refuses. Add
  `SCN-AUTHORIZATION-026-D`. An unreadable image is refused as a conflict (`409`), indistinguishable from an unknown
  cover (`REQ-API-013`), so the refusal cannot probe image existence.
- Implement the check once in the application layer and call it from every cover write path: music group, album, film,
  show and season creation and update, season put, and playlist creation and update.
- Add the six missing system cases and an authorization case that drives the new check across two callers.
- Tighten the existing cover cases where a weaker assertion passed: a cleared cover asserts the `_links.cover` key is
  absent rather than `null`, and an unknown image pins `409`.
- Update the `Cover art` glossary definition so it stops claiming a universal one-referrer rule.

## Impact

- Specification: `openspec/specs/cover-art/spec.md`, `openspec/specs/authorization/spec.md`.
- Code: the application-layer cover check and every use case that writes a cover; handler documentation for the twelve
  cover endpoints.
- Tests: `test/system/cover_art.py`, `test/system/authorization.py`, and the unit modules of the affected use cases.
- Docs: `docs/development/Glossary.md`; the coverage matrix regenerates with no hand edit.
- Operator-visible behaviour: a caller who may manage a container can no longer attach an image it cannot view. No
  route, method or success payload changes.
