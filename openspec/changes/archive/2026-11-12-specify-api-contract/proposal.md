# Proposal

## Why

The backend has no `api` capability. The contract every endpoint shares — media types, HAL payload shape, self and
relation links, collection navigation, the error envelope, status meanings and the published document — exists only as
implementation rules in `docs/development/TechnicalDesign.md` § API and is restated in six capability specs. Each
restatement can drift, and a reader cannot tell which capability owns the rule. The archived `specify-user-account`
design already decided that hypermedia belongs to the API functional area; this change creates that area.

Two consequences are concrete. First, `rule_enforcer.yaml` API-031 still exempts the login, token-regeneration and probe
bodies from HAL, so success responses are split across two media types and the error envelope is the only uniformly HAL
shape. Second, there is no entry point: a client must know every route by heart, and the generated OpenAPI document sits
behind a feature that is off by default, so a deployed server offers no discovery. A catalog listing that returns only
links forces one request per item — the N+1 pattern — while returning every detail field makes the page carry data the
catalog view never renders. The system suite mirrors the discovery gap — it hardcodes every route, so it proves
individual endpoint outcomes but never proves the API is navigable.

## What Changes

- Add an `api` capability that owns the shared HTTP contract: the versioned base, the request contract, the HAL
  representation shape and `_links.self`, named relation links, templated search links, the collection page model, one
  error envelope with a status taxonomy and existence concealment, the generated published document, and a public API
  index.
- Make every JSON success a HAL representation. Login, token regeneration and `/health` gain `_links.self` and answer
  `application/hal+json`; the three corresponding `non_hal_bodies` entries are removed so API-031 enforces the rule.
  **BREAKING** for a client that requires the token payload to contain exactly `access_token`, `expires_in` and
  `token_type`.
- Add a public `GET /api/v1` HAL index that links each context's entry collections and operations, so a client that
  knows only the API base reaches the rest of the surface by following links. It is static and caller-independent, and
  it publishes the credential operation as a URI template so even a parameterized route is discovered rather than built.
- Return the film, show, album and music-group listings as catalog collections: their page items are partial
  representations — identifier, name, cover and the item's `self` and `cover` links — embedded under `_embedded.items`,
  so the catalog view renders from one response and no per-item request is needed. **BREAKING** for a client that reads
  list items from the root `items` or expects detail-only fields in a listing.
- Remove the restated HAL, self-link, `search` and pagination clauses from `library/movies`, `library/series`,
  `library/music-albums`, `library/music-playlists`, `library/music-groups` and `library/assets`; each keeps the
  relations and outcomes it owns. `user` is already HAL-free and `library/cover-art` keeps its `_links.cover` rule.
- Refactor the system suite so `/api/v1` is its only base literal: it resolves entry points from the index and follows
  representation links thereafter. Add a `test/system/api.py` feature module that proves the new capability.
- Keep out of scope: permission-aware write-action links, `Accept` negotiation, a `/` root redirect, and the E2E suite's
  hardcoded URLs.

## Capabilities

### New Capabilities

- `api`: the HTTP contract every endpoint shares — versioned base, request and response media types, HAL representation
  and link rules, collection navigation, the single error envelope and status meanings, the published OpenAPI document,
  and the public API index. Endpoint behaviour, data and authorization stay with their own capabilities.

### Modified Capabilities

- `authentication`: the login and token-regeneration responses become HAL representations carrying `_links.self`.
- `library/assets`: remove the restated `_links.self` clause from the file, image, audio and video reads; the
  `_links.file` and `_links.content` rules and all content-serving rules remain.
- `library/movies`: remove the HAL-representation requirement; the listing becomes an embedded partial catalog page and
  the detail read stays full.
- `library/series`: remove the HAL-representation requirement; the listing becomes an embedded partial catalog page and
  the detail reads stay full.
- `library/music-albums`: remove the HAL-representation requirement; the listing becomes an embedded partial catalog
  page and the page model and `search` move to `api`.
- `library/music-playlists`: remove the HAL-representation requirement and its API-level scenarios; keep the playlist's
  `tracks` relation, the track's `playlist` relation and the alias behaviour.
- `library/music-groups`: remove the inline `_links.self` and HAL-collection wording; the listing becomes an embedded
  partial catalog page and `_links.cover` and `_links.group` stay.

## Impact

- **API**: a new public `GET /api/v1` index; `POST /api/v1/identity/login`, `POST /api/v1/identity/token` and
  `GET /health` switch to `application/hal+json` with `_links.self`; the four catalog listings carry their page items
  under `_embedded.items` as partial representations. No route is removed and no request schema changes.
- **Code**: a new `handler/get_api_index.rs` with a typed `_links` struct built from the existing `Link` and `hal_json`;
  `handler/get_health.rs`, `handler/identity/post_login.rs` and `handler/identity/post_token.rs` serve HAL with
  `_links.self`; `handler/library.rs` gains the four partial summary structs and `_embedded` list fields; `handler.rs`
  and `rest.rs` register the index in both the runtime router and the document.
- **Rules and docs**: `rule_enforcer.yaml` API-031 keeps only `Vec` as a non-HAL body, API-033 stops treating
  `_embedded` as a wrapper key, and API-035 records the four catalog list response structs as the proven performance
  need; `TechnicalDesign.md` § API (API-032, API-033, API-035, the HAL introduction), § 5 (system-test URL rule),
  `Overview.md` and `Glossary.md` are updated.
- **Tests**: a narrow `test/system/api.py` for the link semantics Schemathesis cannot observe; Schemathesis owns the
  declarative contract — media types, statuses, the envelope, required `_links`/`_embedded` fields and the catalog
  schemas. `test/system/authentication.py`, `test/system/user.py` and `test/system/conftest.py` resolve routes through
  the index and links; `test/configuration/harness.py` gains the HAL helpers.
- **Persistence**: none.
- **Dependencies**: none.
- **Risk**: the uniform rule changes three success payloads and the catalog listings change their item envelope; a
  broken index would break every system case at once — which is the point: the suite then proves the entry point rather
  than assuming it.
