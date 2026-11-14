# Design

## Context

See `proposal.md` — Why. The serving path today is one application use case, `GetAssetContent`, reached through four
HTTP handlers in `src/lib/infrastructure/inbound/rest/handler/asset/` and registered by `content_asset_router()` beside
the authenticated asset routes. It resolves the addressed typed asset to its file, authorizes `Download` on the file
**Resource**, opens the stored bytes through `FileStorage::file_size` and `open_range`, and answers the range, validator
and `HEAD` semantics. The `ContentAccess` extractor accepts either a session bearer token or a file-scoped credential
minted by `JwtContentLinkProvider`; a credentialed request skips the decision entirely. The four typed representation
use cases mint that credential and embed it in `_links.content` through `content_href`, and each of their responses sets
`Cache-Control: no-store`.

Constraints that shape the approach:

- The layer rule is unchanged: the decision runs in the application layer, HTTP stays in the handler tree.
- The `FileStorage` port already exposes bounded range reads; nothing in the storage seam changes.
- `identity/authorization` owns who may hold an **Action**; this capability only names the action its route decides.
- The upload context already proves a peer capability can leave `library/assets` without changing a path.

## Goals / Non-Goals

**Goals:**

- One owner for byte delivery: routes, router registration, decision, streaming, validators, outcomes and the save
  surface's name.
- One authentication rule for content: the session bearer token, decided on every request.
- One **Action** per content route, mapped to the catalog's own definitions.
- Delete the minted credential and everything that exists only to serve it.

**Non-Goals:**

- Transcoding, repackaging or any transformed variant of the bytes; the stored file is served as stored.
- New routes, new query parameters or a client-facing download collection.
- Any change to the range, validator or chunking algorithms; only their wrapping moves.
- One-time or signed download links. They are the mechanism this change removes, and they return only with a stated
  reason.
- Quotas, transfer accounting or bandwidth limits.

## Decisions

**D1 — Session bearer only; no minted credential.** The content routes use the ordinary authenticated path, and the
`ContentAccess` extractor collapses to it. Alternatives rejected: keeping the credential with a lifetime bound to the
token's expiry (still a pre-decided capability, still a second authentication mechanism to document and test); accepting
the session token in the query string (it puts the account's most privileged secret into browser history, `Referer`
headers and any reverse proxy's log, contradicts `API-043` and `OBS-003`, and rotation does not revoke it, so a leak is
not recoverable).

**D2 — One action per route, by route purpose.** `/image/{id}/content` decides `View`, `/audio/{id}/content` and
`/video/{id}/content` decide `Stream`, and `/file/{id}/content` decides `Download`. This matches the catalog's own
wording — `View` reads rendered content, `Stream` plays audio or video bytes, `Download` retrieves the original bytes —
and keeps `SCN-AUTHORIZATION-007-B` true as written: a holder of `View` without `Download` still renders an image and
cannot take the file. Alternative rejected: `Download` for all four kinds, which leaves `Stream` with no byte surface
and blanks every cover and gallery photo for a standard user, since the seeded `STANDARD` role holds `VIEWER` on the
catalogue and no `Download` anywhere.

**D3 — Decide per request; cache nothing.** Every content request is decided with the state at that moment, so a
**Grant** change, a new deny or a deleted share applies to the next request. Alternatives rejected: deciding once and
publishing a capability (the defect being removed); caching a decision per caller and resource (a cache-invalidation
problem for a query that already runs in microseconds against local SQLite). The cost is one authorization read per
range request. It is bounded by the existing rule that the resource and its chain are read once per request, and by
streaming a chunk per request rather than a byte; a player issuing many ranges costs many cheap reads, not a full
content read.

**D4 — Denial distinguishes concealment from refusal.** A denied content request answers `403 Forbidden` when the
**Caller** can discover the addressed asset and `404 Not Found` when it cannot, per `REQ-AUTHORIZATION-027`.
Implementation: on a refusal, the use case evaluates `Discover` against the same **Resource** and maps the pair; denials
are rare enough that the second evaluation costs nothing worth designing around. Alternative rejected: `404` for both,
which hides a file the caller can plainly see in the asset representation and makes a permission mistake
indistinguishable from a missing file.

**D5 — The attachment name comes from the typed media row, falling back to the stored path.** The typed media row
already carries the uploaded original name (`library/assets` keeps that rule), and the unit of work already searches
those repositories by `file_id` for the file representation. The file content use case does the same, only when the
addressed target is a bare file or the row is absent, and otherwise takes the final segment of the stored path.
Alternative rejected: always using the path segment, which hands the client `1024.png` instead of `holiday-photo.png`.
The name is user-supplied, so it is emitted with `filename*=` (RFC 6266) whenever it is not a plain quoted token, with a
sanitized ASCII `filename=` fallback — the header is a CR/LF injection surface otherwise.

**D6 — Representations are cacheable; bytes are not pre-authorized.** `Cache-Control: no-store` existed because the
representation embedded a credential. It becomes `private, max-age=60`: `private` keeps a shared proxy out, and the
window is short enough that a stale representation is harmless — its `_links.content` is now a plain link whose request
is decided afresh, so a stale cache cannot widen access. Content responses keep their validators and gain no cache
directive of their own.

**D7 — The handlers stay in the Asset context; the use case keeps its name.** The four handlers and the shared helpers
(`content_command`, `asset_content_response`, `content_disposition`) stay in `handler/asset/`, and
`content_asset_router()` keeps registering the four content routes under the asset routes. The application use case
stays `GetAssetContent` in `application/use_case/get_asset_content.rs`. Download is a capability inside the Asset
bounded context, not a second context: the folder already owns every asset route, and moving the handlers would split
one context's route table for no behavioral gain. Alternatives rejected: a `handler/download/` folder (it makes a
capability look like a bounded context); renaming the use case to `DownloadContent` (churn without a behavioral
difference).

**D8 — The credential leaves at the adapter edge.** `ContentLinkProvider`, `JwtContentLinkProvider`,
`CONTENT_LINK_TTL_SECONDS`, the `AppState` `FromRef` impl, the composition-root wiring and the four representations'
`mint` calls are deleted; `content_href` becomes `Link::new`, and the representation use cases lose the provider type
parameter. `JwtTokenProvider` is untouched.

## Risks / Trade-offs

- **[Per-request authorization on a media read path]** → The read is one local SQLite query over the chain; the
  alternative was stale permissions, which is worse. If it ever shows up in a profile, the decision is memoized per
  request, not cached across requests.
- **[Browser clients that relied on the URL]** → **BREAKING**: a client must send the header. Documented in the proposal
  and in `TechnicalDesign.md`; `API-043` already treats a token in a URL as an accident, not a mechanism.
- **[`403` names a resource the caller can see]** → Intended by `REQ-AUTHORIZATION-027`; a caller unable to discover the
  asset still receives `404`, so nothing new is disclosed.
- **[Header injection through a user-supplied filename]** → RFC 6266 encoding with a sanitized fallback, plus a Rust
  test covering a quote, a separator, a non-ASCII name and a CR/LF attempt.
- **[Stale cached representation after a permission change]** → Bounded to 60 s, and it can only affect metadata and a
  plain link; every byte request is decided on its own.
- **[Deleting the credential removes test coverage that proved it]** → The retired E2E case is replaced by
  `test/system/download.py`, which proves the scenarios end to end; the Rust tests that proved the credential are
  deleted with it rather than muted.

## Migration Plan

No persistence changes and no configuration changes, so startup order is untouched. Deployment is atomic: the container
carries the new handlers, the removed adapter and the representation change together. Rolling back is reverting the
commit; nothing is written that a previous version cannot read. Clients adopt the change by sending
`Authorization: Bearer` on content requests; a client that cannot is out of contract until it does.

## Open Questions

- Whether a representation should ever publish a `_links.download` of its own, so a client saves the original in one hop
  instead of typed asset → file → content. Deferrable: the file hop exists and this change does not depend on the
  answer.
