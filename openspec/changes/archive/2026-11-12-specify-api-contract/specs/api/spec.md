# Spec Delta

## Purpose

Defines the HTTP contract every endpoint shares — the versioned base, request and response media types, the HAL
representation and link rules, collection navigation, the single error envelope and its status meanings, the generated
published document, and the public API index — so a client reaches the surface by following links and never depends on a
hand-built route.

## ADDED Requirements

### Requirement: REQ-API-001 — Business routes share the versioned base

Every business route SHALL be served under `/api/v1`. The liveness probe and the documentation routes SHALL sit outside
that base. The published document SHALL declare the base once in its server entry and SHALL declare its paths relative
to it.

#### Scenario: SCN-API-001-A — A business route carries the base

- **WHEN** a client calls a business endpoint
- **THEN** the route resolves under `/api/v1`

#### Scenario: SCN-API-001-B — The probe and the documentation sit outside the base

- **WHEN** a client calls the liveness probe or reads the documentation
- **THEN** the route resolves outside `/api/v1`

#### Scenario: SCN-API-001-C — The document declares the base once

- **WHEN** the published document is read
- **THEN** its server entry carries `/api/v1` and its paths omit the base

### Requirement: REQ-API-002 — Requests carry business data only

A request body SHALL be `application/json` and SHALL carry business data. A missing or unsupported media type SHALL
answer `415 Unsupported Media Type`, an unparseable body `400 Bad Request`, and a body whose shape or values fail a
schema, framing or business rule `422 Unprocessable Content`. A hypermedia member in a request SHALL have no effect.

#### Scenario: SCN-API-002-A — A missing content type is rejected

- **WHEN** a request with a JSON body omits `Content-Type: application/json`
- **THEN** the system answers `415 Unsupported Media Type`

#### Scenario: SCN-API-002-B — An unparseable body is rejected

- **WHEN** the request body is not valid JSON
- **THEN** the system answers `400 Bad Request` with the stable error envelope

#### Scenario: SCN-API-002-C — A shape or value failure is rejected

- **WHEN** the body parses but a required member is missing or a value fails its rule
- **THEN** the system answers `422 Unprocessable Content` and applies nothing

#### Scenario: SCN-API-002-D — A hypermedia member changes nothing

- **WHEN** a request body carries `_links` or `_embedded`
- **THEN** the member is ignored and the request is decided on its business data

### Requirement: REQ-API-003 — Every JSON success is a HAL representation

Every successful response that carries a JSON body SHALL be served as `application/hal+json`, with its business data at
the root and `_links` as a sibling member. Token issuance, token regeneration and the liveness probe are such
representations. Stored content keeps its own media type, an empty or redirect response carries no JSON body, and the
published OpenAPI document is the only JSON response outside this rule.

#### Scenario: SCN-API-003-A — A single resource is HAL

- **WHEN** the system returns one addressable resource
- **THEN** the response is `application/hal+json` with business data at the root and `_links` beside it

#### Scenario: SCN-API-003-B — A collection is HAL

- **WHEN** the system returns a collection
- **THEN** the response is `application/hal+json` with its item representations under `items`, or under
  `_embedded.items` for a catalog collection (`REQ-API-016`), and `_links` beside them

#### Scenario: SCN-API-003-C — Token issuance is HAL

- **WHEN** the caller logs in or regenerates a token
- **THEN** the response is `application/hal+json` carrying the token fields and `_links.self`

#### Scenario: SCN-API-003-D — The probe is HAL

- **WHEN** a client reads the liveness probe
- **THEN** the response is `application/hal+json` carrying the status and `_links.self`

#### Scenario: SCN-API-003-E — Content and empty responses are not HAL

- **WHEN** the system returns stored content or a response with no body
- **THEN** the content keeps its stored media type and the empty response carries no body

### Requirement: REQ-API-004 — `_links.self` names what the representation is about

Every representation SHALL carry `_links.self`, a relative URI reference. It SHALL name the canonical URI of the
resource the representation is about, even when the request used an alias, and the addressed resource when the
representation reports an action on one. A representation of an operation with no addressed resource, including a
failure, SHALL name the operation's request path.

#### Scenario: SCN-API-004-A — A resource reports its canonical URI

- **WHEN** a caller reads an existing resource by identifier
- **THEN** `_links.self` names that resource's canonical relative URI

#### Scenario: SCN-API-004-B — An alias reports the canonical URI

- **WHEN** the caller reads its own account through `/api/v1/user/me`
- **THEN** `_links.self` names `/api/v1/user/{id}` for that caller

#### Scenario: SCN-API-004-C — An action reports what it addressed

- **WHEN** the caller completes an upload session
- **THEN** `_links.self` names the upload session and the produced file is a named link

#### Scenario: SCN-API-004-D — An operation reports the request path

- **WHEN** the caller logs in, regenerates a token or reads the liveness probe
- **THEN** `_links.self` names the operation's request path

#### Scenario: SCN-API-004-E — A failure reports the request path

- **WHEN** a request fails
- **THEN** `_links.self` names the request path without the query string

### Requirement: REQ-API-005 — Related resources are published as links

A representation SHALL publish each relation its capability's specification declares as a named link, keyed by that
relation's name. An identifier MAY remain at the root; the link is how the client navigates. `_links` SHALL NOT carry
business data, and related resources SHALL NOT be nested under `_embedded` without a proven performance need; the
catalog collections of `REQ-API-016` record that need.

#### Scenario: SCN-API-005-A — A file links its related resources

- **WHEN** the system returns a file representation
- **THEN** it carries `_links.content`, `_links.owner`, and the typed view link the file has

#### Scenario: SCN-API-005-B — A playlist links its tracks

- **WHEN** the system returns a playlist representation
- **THEN** `_links.tracks` names the playlist's track collection

#### Scenario: SCN-API-005-C — Related resources are not embedded

- **WHEN** the system returns a representation other than a catalog collection
- **THEN** its related resources are not nested under `_embedded`

### Requirement: REQ-API-006 — Collection filters are a templated link

A collection representation SHALL expose its filters as `_links.search`, an RFC 6570 URI template carrying
`templated: true`. It SHALL NOT publish its filter set as separate payload fields.

#### Scenario: SCN-API-006-A — A collection publishes its filter template

- **WHEN** the system returns a filtered collection
- **THEN** `_links.search` is a URI template naming the supported filters and `templated` is `true`

#### Scenario: SCN-API-006-B — Filters are not payload fields

- **WHEN** the system returns a collection representation
- **THEN** the supported filters appear only in the `search` template, not as root members

### Requirement: REQ-API-007 — A paginated collection is navigable by link

A collection that paginates SHALL accept `limit`, defaulting to 20 and ranging from 1 to 100, and `offset` starting at
0; a value outside a bound SHALL answer `422 Unprocessable Content`. `_links.self` SHALL name the active view with its
filters and page preserved, `next` SHALL appear when more items follow, and `prev` when the page does not start at the
first item.

#### Scenario: SCN-API-007-A — The first page uses the defaults

- **WHEN** a caller requests a paginated collection without `limit` or `offset`
- **THEN** the system returns at most 20 items starting at the first one

#### Scenario: SCN-API-007-B — Self preserves the active view

- **WHEN** a caller requests a collection page with a filter and a page
- **THEN** `_links.self` carries that filter and page

#### Scenario: SCN-API-007-C — Navigation appears only when a page exists

- **WHEN** the returned page is the last page
- **THEN** `next` is absent and `prev` is present unless the page is also the first

#### Scenario: SCN-API-007-D — An out-of-range limit is rejected

- **WHEN** the caller requests a page with `limit` 0 or greater than 100
- **THEN** the system answers `422 Unprocessable Content`

### Requirement: REQ-API-008 — Every failure uses one envelope

Every failure other than a content-serving outcome SHALL be served as `application/hal+json` with the `error` message at
the root and `_links.self` naming the request path without its query. The envelope SHALL cover use-case errors,
authentication failures, routing fallbacks, extractor rejections, method rejections and body-limit failures. The
content-serving outcomes `206`, `304` and `416` belong to `library/assets` and carry no envelope.

#### Scenario: SCN-API-008-A — An unknown route uses the envelope

- **WHEN** a request names no route
- **THEN** the system answers a `4xx` HAL envelope whose `error` is a stable message

#### Scenario: SCN-API-008-B — An extractor rejection uses the envelope

- **WHEN** a path, query or body extractor rejects the request
- **THEN** the system answers the same envelope with the status the rejection classifies

#### Scenario: SCN-API-008-C — The request path never carries its query

- **WHEN** a failing request carries a query string
- **THEN** `_links.self` names the path component only

### Requirement: REQ-API-009 — Failure headers survive the envelope

An error envelope SHALL preserve the headers the protocol requires: `Allow` on `405 Method Not Allowed`,
`WWW-Authenticate: Bearer` on every `401 Unauthorized`, and `Retry-After` on `429 Too Many Requests`. Rewriting the body
SHALL NOT drop them.

#### Scenario: SCN-API-009-A — A method rejection keeps `Allow`

- **WHEN** a request uses a method the route does not serve
- **THEN** the envelope answers `405 Method Not Allowed` and carries `Allow`

#### Scenario: SCN-API-009-B — An authentication rejection carries the challenge

- **WHEN** a request fails authentication
- **THEN** the envelope answers `401 Unauthorized` and carries `WWW-Authenticate: Bearer`

#### Scenario: SCN-API-009-C — A rate rejection carries `Retry-After`

- **WHEN** the login limiter rejects a request
- **THEN** the envelope answers `429 Too Many Requests` and carries `Retry-After`

### Requirement: REQ-API-010 — Error text is stable and client-safe

The `error` message SHALL be a stable, server-authored string. It SHALL NOT carry request payload content, extractor or
framework wording, or internal detail such as field paths, expected types, parse positions or type names. Rejection
detail SHALL be logged, never returned.

#### Scenario: SCN-API-010-A — A schema rejection stays generic

- **WHEN** a body field fails its schema
- **THEN** the message names no field path, expected type or parser position

#### Scenario: SCN-API-010-B — A client value is never echoed

- **WHEN** a value fails validation
- **THEN** the message does not repeat the submitted value

### Requirement: REQ-API-011 — A failure status classifies the cause

The status SHALL classify the failure independently of the layer that caught it: `400` cannot parse, `401`
authentication, `403` authorized but denied, `404` an absent URI-addressed resource, `405` method, `409` conflict, `410`
gone, `413` too large, `415` media type, `422` shape or value, `429` rate limited, `500` unclassified, `503`
unavailable.

#### Scenario: SCN-API-011-A — Parse, media and shape failures are distinct

- **WHEN** a body cannot be parsed, carries an unsupported media type, or fails its schema
- **THEN** the statuses are `400`, `415` and `422` respectively

#### Scenario: SCN-API-011-B — An unclassified rejection is a server failure

- **WHEN** the server cannot establish why the request failed
- **THEN** it answers `500 Internal Server Error`, never a client status

#### Scenario: SCN-API-011-C — Conflict and gone are distinct

- **WHEN** a request conflicts with the current state, or addresses a resource that no longer exists
- **THEN** the statuses are `409 Conflict` and `410 Gone` respectively

### Requirement: REQ-API-012 — A success status states the outcome

A request that creates a resource SHALL answer `201 Created` with `Location` naming the resource's canonical URI. An
idempotent request that returns an existing resource SHALL answer `200 OK`, and a successful request with no
representation SHALL answer `204 No Content`.

#### Scenario: SCN-API-012-A — A creation names its resource

- **WHEN** a request creates a resource
- **THEN** the response is `201 Created` and `Location` names the resource's canonical URI

#### Scenario: SCN-API-012-B — An idempotent request returns the existing resource

- **WHEN** a creation request names a resource that already exists
- **THEN** the system answers `200 OK` with that representation and creates nothing

#### Scenario: SCN-API-012-C — A deletion carries no body

- **WHEN** a request deletes a resource
- **THEN** the system answers `204 No Content`

### Requirement: REQ-API-013 — A refusal does not disclose existence

A request whose outcome depends on whether a resource the caller does not own exists SHALL answer one status and one
client-safe message for both the absent and the not-owned outcome. `404 Not Found` is reserved for the URI-addressed
resource; a resource named only by a request body is a body value and answers `422 Unprocessable Content`.

#### Scenario: SCN-API-013-A — Another caller's private resource is concealed

- **WHEN** a caller requests a private resource owned by another account
- **THEN** the answer is `404 Not Found`, indistinguishable from an unknown identifier

#### Scenario: SCN-API-013-B — A body reference is a value failure

- **WHEN** a request body names a resource that does not exist
- **THEN** the answer is `422 Unprocessable Content`, not `404 Not Found`

### Requirement: REQ-API-014 — The published contract is generated from the routes

When the documentation feature is enabled, the server SHALL serve an interactive reference at `/doc` and the OpenAPI
document at `/doc/openapi.json`. The document SHALL be generated at startup from the same registration that serves the
routes, SHALL declare every served route, and SHALL declare each operation's security and the bearer scheme. No
generated document SHALL be committed.

#### Scenario: SCN-API-014-A — The document carries every context

- **WHEN** the documentation feature is enabled and the document is read
- **THEN** it declares every context's routes, the bearer scheme and the security requirement of each operation

#### Scenario: SCN-API-014-B — A route cannot drift from the document

- **WHEN** a route is registered
- **THEN** its path item comes from the same declaration, so the served route and the document agree

#### Scenario: SCN-API-014-C — The feature is off by default

- **WHEN** the server is built without the documentation feature
- **THEN** neither `/doc` nor `/doc/openapi.json` is served and the error envelope answers the request

### Requirement: REQ-API-015 — A public index is the entry point

`GET /api/v1` SHALL return a public HAL representation whose `_links.self` names `/api/v1` and whose named links reach
the authentication operations, each context's entry point (its collection or its create operation) and the liveness
probe. The index SHALL be caller-independent and SHALL NOT list individual item routes.

#### Scenario: SCN-API-015-A — The index is public

- **WHEN** an unauthenticated client requests `/api/v1`
- **THEN** the system answers `200 OK` with an `application/hal+json` index and `_links.self` naming `/api/v1`

#### Scenario: SCN-API-015-B — The index reaches the surface

- **WHEN** a client follows the index's links
- **THEN** it reaches the login and registration operations, each context's entry point and the liveness probe

#### Scenario: SCN-API-015-C — A protected entry still authenticates

- **WHEN** an unauthenticated client follows the index's link to a protected collection
- **THEN** the system answers the `401 Unauthorized` envelope

#### Scenario: SCN-API-015-D — The index is caller-independent

- **WHEN** two callers with different permissions read the index
- **THEN** both receive the same links

### Requirement: REQ-API-016 — A catalog collection embeds partial items

A catalog collection SHALL embed each page item's partial representation under `_embedded.items` and SHALL NOT repeat it
at the root. Each embedded item SHALL carry `_links.self`, the cover relation its capability declares, and the browse
fields — identifier, name and cover image identifier. The detail route SHALL return the full representation.

#### Scenario: SCN-API-016-A — A catalog page embeds its items

- **WHEN** a caller reads a film, show, album or music-group listing
- **THEN** the response carries `_embedded.items` with that page's items, each naming its own `self`, and no root
  `items`

#### Scenario: SCN-API-016-B — The embedded item is partial

- **WHEN** an item appears in a catalog collection
- **THEN** it carries its identifier, name, cover image identifier, `_links.self` and the `cover` link its capability
  declares
- **AND** it carries no detail-only field such as a plot, genre list, video summary or medium list

#### Scenario: SCN-API-016-C — One request serves the catalog view

- **WHEN** a client renders a catalog page from the response
- **THEN** it needs no request per item to obtain the browse fields

#### Scenario: SCN-API-016-D — The detail route is complete

- **WHEN** the client follows an embedded item's `self` link
- **THEN** the detail representation carries the full fields its capability declares

#### Scenario: SCN-API-016-E — Non-catalog collections keep root items

- **WHEN** a caller reads a gallery, playlist, user or track collection
- **THEN** its items stay at the root under `items`
