# Design

## Context

See `proposal.md` — Why. The constraints that shape the approach:

- After `specify-metadata-enrichment`, `library/movies` holds seven requirements: authenticated reads, list,
  filter/page, read one, order, create from a video and partial update. The apply requirement already moved.
- The film use cases are `list_movies`, `get_movie`, `create_movie` and `update_movie`; the handler embeds a
  `MovieSummary` partial item in the collection and a full `MovieResponse` on the detail read. The full representation
  embeds a `VideoResponse` summary.
- Authorization actions in the code: `Discover` per film when listing, `View` to read, `CreateChild` on the catalogue
  with `ChildKind::Movie` to create, and `View` then `EditMetadata` to update.
- `cover-art` (created by `specify-music`) owns the `cover_image_id` and `_links.cover` fields every representation that
  carries art reports. `video` (created by `specify-typed-assets`) owns the summary shape.
- `test/e2e/library/uc022_browse_movies.py` says no endpoint creates a film; `uc026_write_movie_metadata.py` creates the
  film it patches. The seed SQL has one film for the Schemathesis fixture, not for the system suite.
- The system suite starts a fresh instance per case and navigates by links; no movie helper exists yet in its
  `conftest`.

## Goals / Non-Goals

**Goals:**

- One capability owning the film catalogue contract with stable identifiers.
- The film representation references the artwork and video contracts instead of copying their fields.
- The black-box proof covers create, list, read, patch and the partial listing shape.

**Non-Goals:**

- Identification and provider application, owned by `metadata-enrichment`.
- A film `PUT` or delete route.
- Changing the film route, payload, status or DTO shape.
- Re-authoring the series area in this change.

## Decisions

### 1. Top-level `movies`, retiring `library/movies`

The new capability owns the film contract end to end. The old path is retired through a `REMOVED` delta, following
`music`, which retired four `library/music-*` capabilities. The change name says `movies`, while the capabilities are
`movies` and (in the sibling change) `series`.

**Alternative considered:** keep `library/movies` and add identifiers through deltas. Rejected: it would require
retiring and re-adding every requirement to renumber it, which is the same churn as a clean creation with a less
truthful path.

### 2. Actions replace permission prose; no authorization delta

The old requirements say `library.read` and `library.manage`. The new ones name `Discover`, `View`, `CreateChild` and
`EditMetadata` and leave the rule to `authorization`, which already owns it. No authorization requirement changes.

### 3. Artwork and video are referenced, not restated

The film representation carries the cover `cover-art` defines and the summary `video` defines. `movies` does not list
their fields. This removes the two most likely sources of drift: a changed video summary or a changed cover rule would
otherwise need three edits.

**Alternative considered:** keep the field list in `movies` for readability. Rejected: the old spec's cover and video
clauses are exactly the duplication this re-authoring removes.

### 4. The partial listing representation stays

A listed film carries only its identifier, name, cover and links; the detail read carries plot, genres and the video
summary. That is the shipped `MovieSummary`, and the spec states it rather than treating the listing as a truncated
detail.

### 5. Code changes are limited to served doc comments

The use cases and DTOs already implement the stated behavior. The handler doc comments still name `library.manage` and
`library.read`; they are corrected to the actions. No route, payload or status changes, and the OpenAPI document should
regenerate unchanged.

## Risks / Trade-offs

- **The capability references two in-flight capabilities** → author it after `specify-music` and `specify-typed-assets`;
  the design and proposal name the sequence, and the deltas apply with a warning if the referenced capability is not yet
  archived.
- **The apply requirement may already be gone** → the removal lists only the seven requirements
  `specify-metadata-enrichment` leaves; removing an already-absent apply requirement would be an unmatched delta.
- **The E2E docstring claims no creation route** → the E2E reduction corrects the docstring while keeping the flow.
- **The system cases need a film before they can list one** → each case creates the film it reads through the current
  create route; no seed fixture is assumed.

## Migration Plan

No data migration. Apply the change after `metadata-enrichment`, `music` and `typed-assets`; `openspec archive` deletes
`library/movies` and creates `movies`. Rollback is the reverse archive.

## Open Questions

None.
