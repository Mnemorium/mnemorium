# Proposal

## Why

`library/series` is the largest remaining capability whose specification predates the `REQ-*`/`SCN-*` convention: 17
requirements, no identifiers, and every read and write restates the obsolete `library.read`/`library.manage` model. It
also copies the artwork field `cover-art` owns and the video summary `video` owns, and its two external-apply
requirements are already moving to `metadata-enrichment`.

What remains is the show → season → episode hierarchy: identity by parent-scoped index, partial show summaries, the
season and episode representations, create-or-replace writes and partial updates. That contract deserves one owner with
stable identifiers, stated once.

## What Changes

- Add a top-level `series` capability with `REQ-SERIES-*`/`SCN-SERIES-*` identifiers covering the show, season and
  episode hierarchy.
- Retire `library/series`. Its fifteen remaining requirements move to `series`; the two apply requirements are already
  owned by `metadata-enrichment` and are not restated.
- Replace every who-may restatement with the **action** the route requires — `Discover` to list, `View` to read,
  `CreateChild` on the catalogue or show, and `View` plus `EditMetadata` to update — leaving the rule to
  `authorization`.
- Replace the artwork clauses with `cover_image_id` and `_links.cover` as `cover-art` defines, and the video clauses
  with the summary `video` defines.
- State the code's actual authorization scope: creating a season or an episode checks `CreateChild` on the **show**,
  with the season or episode child kind, not on the season.
- Keep the index identity, the ascending order, the create-or-replace `PUT` semantics and the partial `PATCH` semantics
  as the hierarchy's own behaviour.
- No route, payload or status changes. **BREAKING**: none.

## Capabilities

### New Capabilities

- `series`: the show, season and episode catalogue contract — listing and reading shows, the season and episode identity
  and order, creating a show, creating or replacing a season and an episode, and partially updating each level.

### Modified Capabilities

- `library/series`: retired. Its fifteen remaining requirements move to `series`; the two external-metadata apply
  requirements are already owned by `metadata-enrichment` and are not restated.

## Impact

- **Specs**: one capability is created and one retired.
- **API**: none. No route, payload, status or served OpenAPI document changes.
- **Code**: none beyond the handler doc comments that still name `library.manage`/`library.read`.
- **Tests**: `test/system/series.py` becomes the black-box proof; `test/e2e/library/uc023_browse_series.py`,
  `uc027_write_series_metadata.py`, `uc028_write_season_metadata.py` and `uc029_write_episode_metadata.py` keep only
  their business flow; the series, season and episode application tests gain the three Allure labels.
- **Docs**: `TechnicalDesign.md` § 5 names `series.py`; the glossary gains **Series**, **Season** and **Episode**;
  `docs/development/UseCases.md` is unchanged.
- **Sequencing**: authored against the tree after `specify-metadata-enrichment` (which removes the apply requirements),
  `specify-music` (which creates `cover-art`) and `specify-typed-assets` (which creates `video`). It is independent of
  `specify-movies`, with which it shares the catalogue dependencies only.

## Out of Scope

- Identification and provider application; `metadata-enrichment` owns both.
- A show, season or episode delete route; the server exposes create, read, replace and patch only.
- Moving the `/api/v1/library/series` routes or renaming the `library` handler module.
