# Proposal

## Why

`library/movies` is one of the last capabilities whose specification predates the `REQ-*`/`SCN-*` convention. Its eight
requirements restate three contracts that now have their own owners: the obsolete `library.read`/`library.manage`
permission model, the artwork field `cover-art` owns, and the video summary `video` owns. `specify-metadata-enrichment`
already removes its apply requirement; what remains is the film catalogue contract, buried under restatements a reader
cannot tell from behaviour.

The redesign gives each functional area one owner with stable identifiers. The film area is the next one to re-author,
with its read and write rules stated once and its permission, artwork and video fields referenced rather than copied.

## What Changes

- Add a top-level `movies` capability with `REQ-MOVIES-*`/`SCN-MOVIES-*` identifiers: list, read, create from a video
  and partial metadata update.
- Retire `library/movies`. Its read and write requirements move to `movies`; its apply requirement already moved to
  `metadata-enrichment` and is not restated.
- Replace every who-may restatement with the **action** the route requires — `Discover` to list, `View` to read,
  `CreateChild` on the catalogue to create, `View` plus `EditMetadata` to update — leaving the rule to `authorization`.
- Replace the artwork clauses with a reference to `cover_image_id` and `_links.cover` as `cover-art` defines, and the
  video clauses with a reference to the summary `video` defines.
- Keep the partial listing representation, the ascending identifier order, the upsert by video and the on-demand genre
  storage as the film contract's own behaviour.
- No route, payload or status changes. **BREAKING**: none.

## Capabilities

### New Capabilities

- `movies`: the film catalogue contract — listing films with their partial representation, reading one film, creating a
  film from an unused video, and partially updating its metadata.

### Modified Capabilities

- `library/movies`: retired. Its seven remaining requirements move to `movies`; the external-metadata apply requirement
  is already owned by `metadata-enrichment` and is not restated.

## Impact

- **Specs**: one capability is created and one retired.
- **API**: none. No route, payload, status or served OpenAPI document changes.
- **Code**: none. The use cases, repository port, handler and DTOs are unchanged.
- **Tests**: `test/system/movies.py` becomes the black-box proof; `test/e2e/library/uc022_browse_movies.py` and
  `uc026_write_movie_metadata.py` keep only their business flow; the `list_movies`, `get_movie`, `create_movie` and
  `update_movie` application tests gain the three Allure labels.
- **Docs**: `TechnicalDesign.md` § 5 names `movies.py`; the glossary gains **Movie** with its film synonym resolved;
  `docs/development/UseCases.md` is unchanged.
- **Sequencing**: authored against the tree after `specify-metadata-enrichment` (which removes the apply requirement)
  and after `specify-music` (which creates `cover-art`). It also references the `video` summary from
  `specify-typed-assets`.

## Out of Scope

- Identification and provider application; `metadata-enrichment` owns both.
- Cast, crew, runtime, ratings and other provider fields the catalogue does not store.
- A `PUT` or delete route for a film; the server exposes create, read and patch only.
