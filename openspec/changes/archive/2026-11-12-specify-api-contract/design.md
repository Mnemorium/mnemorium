# Design

## Context

See `proposal.md` — Why. The constraints that shape the approach:

- `src/lib/infrastructure/inbound/rest/hal.rs` already exposes `Link`, `SelfLinks` and `hal_json`; every handler builds
  its own `_links` struct and `hal_errors` wraps every `4xx`/`5xx` into one `ErrorBody`.
- `rule_enforcer.yaml` `API-031` allowlists the non-HAL bodies, and `check_api_032` requires every body declared
  `application/hal+json` to expose a `_links` field whose type declares `self`. Both run in `script/rule_enforcer.sh`.
- `openapi()` assembles the document from the same `utoipa-axum` registration the runtime router uses; `api-docs` is off
  by default and not enabled by the container build.
- The system suite and the configuration suite share `test/configuration/harness.py`. System modules import `Sut`,
  `register` and the upload helpers; the E2E suite has its own `conftest.py` and is not part of this change.

## Goals / Non-Goals

**Goals:**

- One normative owner for the shared HTTP contract, with the six capability specs reduced to their own relations and
  outcomes.
- Every JSON success is a HAL representation with a self link, enforced mechanically by API-031 and API-032.
- A public, caller-independent index at `/api/v1` that makes the surface reachable from one known URI.
- A catalog page that renders from one response: the film, show, album and music-group listings embed their page's
  partial item representations instead of forcing a request per item.
- A system suite that proves navigation: it holds `/api/v1` as its only base literal and follows links from the index
  and from representations.

**Non-Goals:**

- Permission-aware action links (`edit`, `delete`, `share`) and a `/` redirect to the index.
- `Accept` negotiation, an `api-off` variant, or serving the OpenAPI document in a default build.
- Paginating the user collection.
- Linking the `file_id` carried by a gallery item, album track, playlist track, film or episode. The
  `TechnicalDesign.md` file-link guideline still applies to those representations, but galleries have no owning
  capability yet and the gap is a separate change.
- Converting galleries, playlists, user or track collections to `_embedded`; their root `items` stays until a separate
  proven need.
- Refactoring the E2E suite's URLs; it keeps its own route constants.

## Decisions

### 1. One `api` capability owns the shape, not the endpoints

The `api` spec states the versioned base, request contract, media types, representation shape, self-link rule, relation
form, templated search, page model, error envelope and status taxonomy, the generated document, and the index. Each
feature capability keeps its routes, fields, named relations and authorization. A single capability rather than
`api/hal`, `api/errors` and `api/openapi` keeps one requirement sequence (`REQ-API-*`) and one place to read the
contract; splitting would create three owners of one response.

**Alternative considered:** keep the rules only in `TechnicalDesign.md` and let each feature spec restate them.
Rejected: that is the current state, and six copies already exist.

### 2. Uniform HAL for every JSON success

Every successful JSON body is `application/hal+json` with `_links.self`; the token issue, the token regeneration and the
liveness probe stop being exceptions. `API-031.non_hal_bodies` becomes `[Vec]` (asset bytes), and `check_api_032` then
requires `_links` and `self` on `HealthResponse`, `LoginResponse`, `TokenResponse` and the index response.

**Alternative considered:** the nature-based rule (only responses about an addressable resource are HAL). Rejected by
the product decision: uniformity is worth more than avoiding a `self` on a token issue.

**Consequence:** a client that required the token payload to contain exactly three members breaks; this is the one
**BREAKING** point in the change.

### 3. One self rule, and errors stop being special

`_links.self` names the canonical URI of the resource the representation is about. An action representation names the
resource the request addressed; an operation with no addressed resource — login, token regeneration, the probe — names
the operation's request path; a failure does the same. This is the existing `API-042` rule extended, so `_links.self` on
an error envelope is no longer an exception to the representation rule but an instance of it. `API-043` still strips the
query from the failure path.

**Alternative considered:** set `self` to the authenticated account on a login response. Rejected: the token is not a
property of the account, and the payload would claim a resource relation that does not exist.

### 4. A public, static index at the API base

```
GET /api/v1
{
  "_links": {
    "self":        {"href": "/api/v1"},
    "login":       {"href": "/api/v1/identity/login"},
    "register":    {"href": "/api/v1/identity/register"},
    "token":       {"href": "/api/v1/identity/token"},
    "credential":  {"href": "/api/v1/identity/credential/{id}", "templated": true},
    "user":        {"href": "/api/v1/user"},
    "current_user":{"href": "/api/v1/user/me"},
    "gallery":     {"href": "/api/v1/library/gallery"},
    "playlist":    {"href": "/api/v1/library/music/playlist"},
    "movie":       {"href": "/api/v1/library/movie"},
    "series":      {"href": "/api/v1/library/series"},
    "music_group": {"href": "/api/v1/library/music/group"},
    "music_album": {"href": "/api/v1/library/music/album"},
    "upload":      {"href": "/api/v1/asset/upload"},
    "health":      {"href": "/health"}
  }
}
```

- **Public and static.** It cannot require a token: the login operation is discovered from it. It is not filtered by
  permission, so it is cacheable and identical for every caller; following a protected link without a token answers the
  normal `401` envelope. Authorization still decides every request that follows.
- **Entry points only.** One link per entry collection and per authentication operation, plus the credential URI
  template. Item routes are reached from the collection's `items` or the representation's named links; the index does
  not enumerate them.
- **URI templates for parameterized operations.** The credential operation is the one route the suite addresses by
  identifier without holding a representation first, so it is published as an RFC 6570 template. The test helper expands
  it; the suite never writes the identifier into a path.
- **Methods are not in HAL.** A link carries a URI, not a method. `POST /api/v1/identity/login` is known from the
  published OpenAPI document, or from the client's knowledge of the operation. Extending `Link` with a non-standard
  method member is not justified for one consumer.

**Alternatives considered:** a `/` root or `/api` index (outside the versioned base, and `/` is the deployment's space);
an authorization-filtered index (needs a token to find login, and personalizes a cacheable document); the OpenAPI
document as the sole bootstrap (feature-gated off by default and still requires path construction).

### 5. The index is an ordinary declared operation

`get_api_index` is declared with `path = "/"`, tag `system`, `security(())`, a `200` response of `ApiIndexResponse` and
`application/hal+json`. The runtime `setup_routes` nests it under `/api/v1` beside the contexts; `openapi()` registers
it at the document root, which resolves to the `/api/v1` server entry. It reuses `Link` and `hal_json`, and declares a
typed `_links` struct like every other representation, so API-032 checks it.

### 6. Shape moves to `api`, relations stay with the capability

The delta specs delete or trim only cross-cutting text:

| Capability                | Removed or trimmed                                                                                                                                                                |
| ------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `library/movies`          | the HAL-representation requirement and both scenarios; "HAL" from the collection scenario                                                                                         |
| `library/series`          | the HAL-representation requirement and both scenarios; "HAL" and the self clause from the listing scenarios                                                                       |
| `library/music-albums`    | the HAL-representation requirement and both scenarios; "HAL" from the two collection scenarios; the limit bounds and self-preserves-filter clause                                 |
| `library/music-playlists` | the HAL-representation requirement and all five scenarios; "HAL" from the listing scenario; the self clauses in the playlist content, alias and track scenarios; the limit bounds |
| `library/music-groups`    | the inline `_links.self` and "HAL collection"                                                                                                                                     |
| `library/assets`          | the inline `_links.self` from the four typed reads                                                                                                                                |
| `authentication`          | the media type and `_links.self` on the login and token-regeneration responses                                                                                                    |

The resource-specific relations stay: `_links.cover`, `_links.group`, `_links.tracks`, `_links.playlist`, `_links.file`,
`_links.content`, and the album/medium/track relations. The `Out-of-range limit` scenario stays in every paginated
collection because it is the endpoint's outcome; the bound itself is stated once in `REQ-API-007`.

**Alternative considered:** keep the per-endpoint HAL scenarios as extra proof. Rejected: they are prose duplicates that
drift, and the declarative `_links` field plus Schemathesis already validates the media type and required self link on
every route.

### 7. Catalog collections embed partial items

The catalog collections return:

```json
{
  "_links": {
    "self": { "href": "/api/v1/library/movie" },
    "search": { "href": "/api/v1/library/movie{?name,limit,offset}", "templated": true }
  },
  "_embedded": {
    "items": [
      {
        "_links": {
          "self": { "href": "/api/v1/library/movie/1" },
          "cover": { "href": "/api/v1/asset/image/7" }
        },
        "id": 1,
        "name": "Arrival",
        "cover_image_id": 7
      }
    ]
  }
}
```

The partial item carries only the browse fields — identifier, name, cover image identifier — plus its `self` and `cover`
links; the detail route returns the full film, show, album or group. Root `items` disappears for these four collections.

Why: the catalog view needs a poster grid, not detail. A links-only page forces one request per item (N+1); a
full-detail page carries plots, genres, video summaries and mediums the view never renders. The embedded partial is one
request and no per-item fetch. The `cover` link stays because a client that holds only `cover_image_id` would have to
construct the image URI, which `API-037` forbids.

Scope: the four catalog listings only. Galleries, playlists, user and track collections keep root `items` until a
separate decision. The rulebook records the proven need: API-033 stops treating `_embedded` as a wrapper key (API-035
alone gates it), and API-035 records the four list response structs in `allowed_locations`.

**Alternatives considered:** keep root `items` with full representations (current; page bloat and detail fields in a
browse view); return links only (N+1; rejected); embed the full detail representations (one request, but the page
carries data the catalog never renders and grows with the detail schema).

**Trade-off:** **BREAKING** for a client reading root `items` or detail fields from the listing; it reads
`_embedded.items` and follows `self` for the detail.

### 8. The system suite navigates; it does not address

Add to `test/configuration/harness.py`:

- `API_PREFIX = "/api/v1"` stays the only base literal.
- `Sut.index()` — `GET API_PREFIX` and return the parsed document; no cache, so a test can always see the current
  document.
- `Sut.href(rel, **values)` — read the index, take `_links[rel].href`, and expand `{name}` or `{?a,b}` with `values`. A
  missing value or relation raises an `AssertionError` naming the relation.
- `expand_template(template, **values)` — the small RFC 6570 subset the API publishes: `{name}` simple expansion and
  `{?a,b,c}` query expansion with percent-encoding. Only the credential and upload-chunk templates use simple expansion;
  the collection `search` templates stay unpublished to the suite because a test appends its filter query to the
  collection's linked URI.
- `link_of(representation, rel, *, templated=False)` — a pure helper that returns a named link and asserts its presence.
- `Sut.follow(representation, rel, token=None)` — resolve `link_of` and `GET` it.
- `begin_upload` returns an `UploadSession` value (`id`, `chunk_template`, `complete_href`) read from the `201`
  representation; `put_chunk` expands `chunk_template` with `{chunk_number}`; `complete_upload` follows `complete_href`.
  The three `test/configuration/asset_settings.py` call sites follow the new shape; the configuration suite keeps its
  behavior and gains the same navigation.

`test/system/conftest.py` loses `GALLERY_PATH`, `PLAYLIST_PATH`, `MOVIE_PATH`, `ASSET_FILE_PATH`, `ASSET_IMAGE_PATH` and
`ASSET_UPLOAD_PATH`; `test/system/user.py` and `test/system/authentication.py` lose `USER_PATH`, `TOKEN_PATH`,
`ME_PATH`, `REGISTER_PATH` and the `identity/credential` literals. A test resolves the collection through
`server.href("gallery")`, the item collection through the gallery representation's `_links.items`, the track collection
through the playlist's `_links.tracks`, the file through the upload completion's `_links.file`, the typed image through
the file's `_links.image`, content through `_links.content`, and the credential through
`server.href("credential", id=...)`. `Location` assertions compare against the response's own `_links.self.href`.

The permission route (`/user/{id}/permission`) is owned by the in-flight `specify-authorization` change; this change
leaves that one case alone until its owning change publishes a link.

**Alternative considered:** keep the constants and only add the index test. Rejected: the suite would pass even if every
link were wrong, which is the gap this change exists to close.

### 9. Test placement

- Schemathesis owns the declarative contract: every response media type, status, envelope, required `_links`/`_embedded`
  field and pagination bound the OpenAPI document declares, including the three uniform bodies and the four catalog
  pages. No system test restates it.
- `test/system/api.py` is deliberately narrow: it proves only the semantics the schema cannot — every index link
  resolves, the index is caller-independent, an alias reports its canonical `self`, a catalog page's embedded item
  reaches its full detail, and a collection's navigation links preserve their view. It starts from the index and follows
  links.
- `test/system/authentication.py` and `test/system/user.py` keep proving their own capabilities; they are refactored to
  navigate and to read catalog items from `_embedded.items`, and they do not re-assert the API's declarative rules.
- Rust handler tests cover the index's mounted path and the document registration; `rest.rs` tests extend the document
  assertions to the index path. Schemathesis validates the declared media types, the required `_links` on every route
  and the catalog list schemas.
- The deleted capability scenarios were prose, not test cases; no test is deleted for them. The API assertions that
  replace them are in `test/system/api.py`.

### 10. Rulebook and documentation move with the behavior

- `rule_enforcer.yaml` API-031: `non_hal_bodies` becomes `[Vec]` and the comment names the three bodies that are now
  HAL. The gate fails until the handlers and the index declare the media type and `self`; that ordering is intentional.
- `rule_enforcer.yaml` API-033 drops `_embedded` from `wrapper_keys`, so the rule keeps flagging `data`, `payload`,
  `body`, `result` and `resource` while API-035 alone decides whether an `_embedded` is permitted. API-035 adds the four
  catalog list response structs to `allowed_locations`.
- `TechnicalDesign.md`: `API-032` and `API-042` lose the "token issues, probes and `204` are not representations"
  wording; `API-033` and `API-035` record the catalog need and the API-035 gate; the HAL introduction describes the
  uniform rule, the operation self and the catalog embedding; § 2.5 notes that the error envelope follows the same self
  rule; § 5 states the system suite's single-literal rule.
- `Overview.md` gains `handler/get_api_index.rs`; `Glossary.md` records **API index**, **Resource representation**,
  **Catalog collection** and **Partial representation**, and the change verifies each against the served behavior. The
  change's spec companions record them.

## Risks / Trade-offs

- **The token payload grows a `_links` member** → Marked **BREAKING** in the proposal; clients that parse a JSON object
  tolerate it, strict schema consumers do not. No compatibility shim: the uniform rule is the point.
- **Every system case now depends on the index** → Intended: a broken index fails the suite at the first case. The API
  module names the index failure before any feature case obscures it.
- **The index can rot as routes move** → `test/system/api.py` follows every link and fails on a non-resolving one; the
  link set is small and reviewed beside the route registration.
- **The test template expander is a subset of RFC 6570** → It supports only the two published forms and has its own unit
  test; a new template form must extend the expander in the same change.
- **`self` on a token response has little navigational value** → Accepted for uniformity; the alternative is a mixed
  media type and a special case in every client.
- **The shared harness change reaches the configuration suite** → Its three upload call sites are updated in the same
  change; the configuration suite runs green before the system refactor is finished.
- **The in-flight authorization change owns a system test route** → The permission case is left to that change; no
  second owner is created here.
- **The catalog listing shape changes** → Marked **BREAKING**: a client reads `_embedded.items` instead of root `items`
  and follows the item's `self` for detail. The detail routes are unchanged, so the break is confined to the four
  listings.
- **A partial item lacks a field a client wants** → The partial carries only the browse fields; a client needing more
  follows `self`, and adding a browse field later is additive.
- **Relaxing API-033 could let business data nest under `_embedded`** → API-035 still requires an explicit recorded
  location for every `_embedded` use; only the four catalog list structs are allowed.
- **Catalog and non-catalog collections differ in shape** → SCN-API-016-E makes the boundary explicit; galleries,
  playlists, user and track collections keep root `items`.

## Migration Plan

No datastore change. Deploy is a code change only; rollback is a revert. The suite and its harness must ship in the same
commit as the handlers, because the old constants disappear. Order inside the change:

1. Add the harness helpers and the narrow `test/system/api.py`; run it and record the failures (no index, links
   unresolved). The declarative failures — plain token/probe and root `items` — belong to Schemathesis, not a system
   test.
2. Implement the index and the uniform HAL, then make `test/system/api.py` green.
3. Implement the four catalog collections' partial items and `_embedded.items`; make the catalog scenarios green.
4. Refactor the two feature modules and the shared upload helpers, then run the system and configuration suites.
5. Apply the rulebook and documentation edits; run `script/rule_enforcer.sh check API-031`, `API-032`, `API-033`,
   `API-035`, the full gate and Schemathesis.

## Open Questions

None. The deferrable items — action links, the file-link gap, `api-docs` by default — are recorded as non-goals rather
than unknowns that could change the approach.
