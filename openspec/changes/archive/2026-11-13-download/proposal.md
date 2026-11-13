# Proposal

## Why

Byte delivery for stored assets lives inside `library/assets`, beside the asset's typed representation, and that area is
already mid-split: `specify-upload` moves the intake requirements to `upload` and leaves `library/assets` with ten
serving requirements. Those ten hold two unrelated contracts — what a stored asset _is_ (its typed representation, its
aliases, the name its media row carries) and how its bytes come _out_ (the content routes, their ranges, validators and
outcomes). No capability owns the byte contract on its own, so the five requirements that describe it sit beside ones
that never mention bytes.

The serving half also carries a second authentication mechanism. `_links.content` publishes a twelve-hour, file-scoped
credential signed with the instance signing secret; a content route accepts it in place of the session token and then
skips authorization, because the decision ran once, when the link was minted. That mechanism disagrees with the contract
it enforces. `identity/authorization` says `View` reads content and never download (`SCN-AUTHORIZATION-007-B`) and that
`Stream` serves audio and video bytes (`SCN-AUTHORIZATION-007-C`); the content routes instead decide `Download` for all
four kinds, a minted link spends as `Download` after a `View` or `Stream` check, and no **Grant** change and no new deny
re-checks for twelve hours — twelve times the session lifetime, since `security.jwt.ttl` is one hour. A capability that
owns byte delivery can state one authentication rule, one **Action** per route, and no permission decided ahead of the
request.

## What Changes

- Add a top-level `download` capability that owns byte delivery for stored assets: the four content routes
  (`/file/{id}/content`, `/image/{id}/content`, `/audio/{id}/content`, `/video/{id}/content`), byte ranges, the entity
  validators and their conditional requests, `HEAD`, the stored **Content Type** served and the distinguishable outcome
  taxonomy.
- Authenticate every content request with the session bearer token only. Remove the minted content credential: no
  `exp`/`sig` query values, no `media-link` token audience, no ambiguous-credential rule, and no window that outlives
  the session that minted it.
- Decide one **Action** per request, one per route: `View` for image content, `Stream` for audio and video content, and
  `Download` for file content. Its requirements name the action and never restate who holds it, as the **Who-may rule**
  requires.
- Serve file content with `Content-Disposition: attachment`, naming the media row's original name where the addressed
  asset has one, so the single save surface is distinguishable from the three display surfaces.
- Stop publishing a credential in `_links.content`; it becomes a plain route link, and the representations move from
  `Cache-Control: no-store` to `Cache-Control: private, max-age=60`.
- Remove the serving requirements from `library/assets`, which keeps the typed file, image, audio and video
  representations, their file aliases and the media-row naming rule.
- **BREAKING** (wire): a content link copied out of a representation and requested without a bearer header no longer
  returns bytes. No route is added, moved or removed; the four content paths are unchanged.

## Capabilities

### New Capabilities

- `download`: byte delivery for a stored asset — the content routes and the media type they serve, byte ranges and their
  refusals, entity validators and conditional requests, `HEAD`, the save surface's attachment name, and the outcome
  taxonomy that separates an unknown or unreadable asset, an unavailable library root and absent bytes. Session bearer
  authentication and per-request **Action** decisions belong to this capability; who holds an action remains
  `identity/authorization`'s.

### Modified Capabilities

- `library/assets`: the five serving requirements (content, byte ranges, validators, the content credential and the
  outcome taxonomy) leave for `download`; the typed reads, the file aliases and the media-row naming rule stay, and
  `_links.content` becomes a plain link.

## Impact

- **Specs**: `openspec/specs/download/spec.md` is created; `library/assets` falls from 11 to 6 requirements. No delta
  for `api`, `identity/authorization`, `upload`, `library/cover-art` or `configuration/loading` — the action catalog,
  the resource tree, the cover link and the settings are unchanged.
- **API**: no path changes. The four content routes become bearer-only (the OpenAPI security scheme is already
  `bearer_auth`), file content gains a `Content-Disposition` header, and the four asset representations change their
  `Cache-Control` value.
- **Code**: the `jwt` content-link adapter (`ContentLinkProvider`, `JwtContentLinkProvider`,
  `CONTENT_LINK_TTL_SECONDS`), `ContentAccess::Link`, the `?exp=&sig` parsing and the ambiguous-credential branch are
  deleted; `get_asset_content` and the four content handlers and routes stay in the asset context; the typed
  representation use cases stop minting a link and the `ContentLinkProvider` port loses its last caller.
- **Tests**: a new `test/system/download.py` proves the scenarios black box;
  `test/e2e/asset/uc031_read_asset_content.py` is retired; Rust keeps the storage seams, the range arithmetic and the
  validator edge cases.
- **Docs**: the asset section of `docs/development/TechnicalDesign.md` loses the `exp`/`sig` paragraph and its
  query-string justification; UC-031 in `docs/development/UseCases.md` loses its playback-link sentence. No glossary
  term is added.
- **Configuration, dependencies, persistence**: none.
