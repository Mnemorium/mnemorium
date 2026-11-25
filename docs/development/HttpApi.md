# HTTP API implementation notes

This page is the non-normative reference for the Mnemorium HTTP API: how the published document is generated, the
meaning of each response code, the error envelope, the HAL samples and the way stored content is served. The normative
contract is the `api` specification (`openspec/specs/api/spec.md`), which states the client-observable requirements as
`REQ-API-…`; how a handler or payload struct is written is in
[TechnicalDesign.md](TechnicalDesign.md#code-style-guidelines) § Rust (`STY-RUST-091`–`STY-RUST-122`). Where this page
and the specification disagree, the specification wins.

## OpenAPI generation

Each handler function in `src/lib/infrastructure/inbound/rest/handler` is annotated with a `#[utoipa::path(...)]` macro;
the macro produces one OpenAPI _path item_ for the endpoint. The declaration also carries the handler's path, so the
context route module registers the handler once with `utoipa-axum` and gets both the axum route and the path item from
it:

```rust
pub(crate) fn library_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_album_list::get_album_list))
        .routes(routes!(get_album::get_album))
        // [...]
}
```

A struct deriving `utoipa::OpenApi` carries only the document-scoped members — `info`, `servers`, `tags` and
`modifiers`. It declares no paths and no schemas, because `utoipa-axum` collects them from the registered routes:

```rust
#[derive(utoipa::OpenApi)]
#[openapi(
    info(
        title = "Mnemorium API",
        version = env!("CARGO_PKG_VERSION"),
        description = "HTTP API of the Mnemorium service"
    ),
    servers(
        (url = "http://0.0.0.0:4080/api/v1", description = "Local development server")
    ),
    tags(
        (name = "notes", description = "Note bounded context")
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;
```

Drawn from `src/lib/infrastructure/inbound/rest.rs`, `openapi` seeds an `OpenApiRouter` with that document, registers
the health route, nests each context under its own prefix and splits off the specification. The paths stay relative to
the `/api/v1` base the document's `servers` entry declares, so the runtime router adds that prefix and the document does
not:

```rust
#[must_use]
pub(crate) fn openapi() -> OpenApi {
    let (_, mut openapi) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(get_health::get_health))
        .nest("/library", library_router())
        // [...]
        .split_for_parts();
    utoipa::Modify::modify(&UnauthorizedChallengeAddon, &mut openapi);
    openapi
}
```

The server assembles the document at startup from the same registration, so the document it serves cannot drift from the
routes: a route's path item comes from the same declaration that registers it. The last step runs an addon over the
collected paths: every `401` response declares the `WWW-Authenticate: Bearer` challenge the server sends, declared once
instead of on each of the handler paths that can answer `401`. With the `api-docs` feature `setup_routes` serves an
interactive reference through `utoipa-scalar` at `/doc` and the same document as JSON at `/doc/openapi.json`, both
outside the `/api/v1` base; the feature is off by default, so a default build serves neither. There is no generated
artifact to commit — the served document is the published specification.

The document is also the input of the system suite's typed client. `script/openapi_client.sh` generates it from
`/doc/openapi.json` before the suite runs, so the handler declaration is the only place a request's shape is written: a
parameter, a header, a request body, a response body or a status a case drives is declared in the handler, never added
to a test. The suite's generated client fails the run when the document cannot express what a case sends. A case whose
subject is a link the server publishes reads it from the representation with `link_of` and follows its `href`, so it
proves the published relation rather than a route it builds.

## Response codes

Every status the server returns carries a meaning: the code says _why_ the request failed, not which branch the handler
happened to take. `400 Bad Request` means the server cannot parse the request — malformed JSON, undecodable header
syntax, an unparseable path or query parameter. `422 Unprocessable Content` means the request parses, but its shape (a
missing, unknown or wrong-typed field) or its values fail a schema, framing or business rule — this includes a
well-formed header whose value is inconsistent with the request, such as a `Content-Range` that does not match the body
length. A request that parses but omits a required parameter or header is a `422` shape failure, except where the
specification documents another outcome: a missing `Content-Type` is the media-type case that stays `415`, and an
authentication outcome stays `401`. Malformed or undecodable header syntax is a `400`, and `415` remains scoped to media
types (a digest algorithm is not one). The status therefore does not depend on which layer caught the failure: the same
class of failure yields the same code. An unclassified variant of a `#[non_exhaustive]` framework rejection enum is a
`500 Internal Server Error`: the server cannot establish why the request failed, so it must not report a client status.
Authentication outcomes are always `401 Unauthorized`, whatever the cause, so a missing token, an unknown user and a
wrong password share one code, and an unsupported media type — the request's `Content-Type` or a declared upload content
type — is always `415`.

The table lists every status the server currently emits, with a concrete example drawn from the codebase. It is extended
as new codes are introduced; completeness is not required.

| Code  | Name                   | Represents                                                                                                   | Example                                                                                                                           |
| ----- | ---------------------- | ------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------- |
| `200` | OK                     | The request succeeded; the body carries the representation when there is one.                                | `GET /api/v1/user/{id}`; `POST /api/v1/asset/upload/{id}/complete`.                                                               |
| `201` | Created                | A resource was created; the `Location` header names it.                                                      | `POST /api/v1/identity/register`; `POST /api/v1/asset/upload`.                                                                    |
| `204` | No Content             | The request succeeded and deliberately carries no body.                                                      | `PATCH /api/v1/identity/credential/{id}`.                                                                                         |
| `400` | Bad Request            | The server cannot parse the request.                                                                         | A register body that is not JSON; a malformed `Content-Range` on `PUT .../chunk/{n}`; a non-numeric `{id}`.                       |
| `401` | Unauthorized           | Authentication is required, or the credentials are invalid; the response carries `WWW-Authenticate: Bearer`. | `POST /api/v1/identity/login` with a wrong password; a missing or expired bearer token.                                           |
| `403` | Forbidden              | The caller is authenticated but not permitted to perform the request.                                        | A non-admin `PATCH /api/v1/user/{id}`; a non-root admin granting the `ADMIN` role.                                                |
| `404` | Not Found              | The addressed resource does not exist.                                                                       | An unknown `{id}` in `GET /api/v1/user/{id}`; an unknown upload session.                                                          |
| `405` | Method Not Allowed     | The method is not supported on the route; the response carries `Allow`.                                      | `DELETE /health` (only `GET` is routed; `hal_errors` keeps the `Allow` header).                                                   |
| `409` | Conflict               | The request conflicts with the current state of the resource.                                                | Registering a username that is already taken; a chunk on a finished upload.                                                       |
| `410` | Gone                   | The resource existed but is no longer available.                                                             | `PUT .../chunk/{n}` against an expired upload session.                                                                            |
| `413` | Content Too Large      | The request body (or a declared size) exceeds the server limit.                                              | A begin-upload declaring a file size above the maximum; a body over `DefaultBodyLimit`.                                           |
| `415` | Unsupported Media Type | The request's or a declared media type is not supported.                                                     | A JSON endpoint called without `Content-Type: application/json`; an unsupported upload content type.                              |
| `422` | Unprocessable Content  | The request parses, but its shape or values fail a schema, framing or business rule.                         | Registering with an empty username or a weak password; a missing required header; a `Content-Range` that does not match the body. |
| `429` | Too Many Requests      | The login limiter refused the request; the response carries `Retry-After`.                                   | A login burst from one address.                                                                                                   |
| `500` | Internal Server Error  | An unexpected server-side failure occurred.                                                                  | Any unmapped `Unknown(_)`; an unclassified framework rejection variant; the fixed body `"an unexpected error occurred"`.          |

## Error public payload

Every error response carries the same body, a HAL representation served as `application/hal+json`: the human-readable
`error` message at the root and a `_links.self` whose `href` is the request path the client called. This is the
operation-self rule applied to a failure: the envelope is an instance of the representation rule, not an exception to
it.

Drawn from `src/lib/infrastructure/inbound/rest/api_error.rs`:

```json
{
  "_links": {
    "self": {
      "href": "/api/v1/user/42"
    }
  },
  "error": "An error message"
}
```

`_links.self` carries the request path — the path component of the request URI — and never the query string. The query
is a raw client-supplied value that may carry personal data or a token, so it is withheld from the response for the same
reason it is withheld from logs.

The envelope is uniform across every error the server emits — use-case errors, authentication failures, routing
fallbacks (an unknown or malformed URL), extractor rejections, method rejections and body-limit failures. The
content-serving outcomes `206`, `304` and `416` belong to the asset routes and carry no envelope.

The `error` message is a stable, server-authored, client-safe string: it carries no request payload content, no
extractor or framework wording, and no internal detail such as field paths, expected types, parse positions or type
names. Detail from a rejection is logged, never returned.

## HAL payload guidelines

The examples below use `/orders` as the resource and show how each HTTP method maps onto HAL. Errors use the same
representation format; their body is defined in [§ Error public payload](#error-public-payload). The normative rules for
every representation are `REQ-API-002`–`REQ-API-005` and `REQ-API-016`–`REQ-API-021`.

Every successful response that carries a JSON body is an `application/hal+json` representation with `_links.self`. That
includes token issuance, token regeneration and the liveness probe. Stored content keeps its stored media type, an empty
or redirect response carries no JSON body, and the published OpenAPI document is the one JSON response outside this
rule.

A representation's `_links.self` names the resource it is about, by that resource's canonical relative URI. When a
representation reports the outcome of an action on a resource, `self` is the addressed resource, and a resource the
payload reports or references is exposed as a named link — never promoted to `self`. A representation of an operation
with no addressed resource — login, token regeneration, the probe — names the operation's request path. The canonical
URI is used even when the request used an alias: `GET /api/v1/user/me` returns `self` = `/api/v1/user/{id}`, and
`POST /api/v1/asset/upload/{upload_id}/complete` reports the completion of the session it addressed, so its `self` is
`/api/v1/asset/upload/{upload_id}` while the produced file is carried as `file_id` and linked by name. A failure follows
the same rule and names the request path: the error envelope is an instance of the rule, not an exception to it.

A HAL payload keeps business data at the root and publishes related resources as named links. A **catalog collection** —
the film, show, album and music-group listings — embeds each page item's **partial representation** under
`_embedded.items`; that is the one recorded performance need. The partial carries only the browse fields — identifier,
name and cover image identifier — plus `_links.self` and the `cover` link its capability declares; the detail route
returns the full representation. Every other collection keeps its items at the root under `items`.

A representation that carries a referenced file's `file_id` also links that file by name, to
`/api/v1/asset/{type}/{file_id}`. The link is published whether or not the route is served: a client that builds the URI
itself violates the navigability rule, and a missing route answers the standard error envelope rather than a
representation, so deferring the link until the route exists would force every client into a later breaking change.

---

### GET — single resource

Return the resource representation with the relevant links, so the client can read the resource and discover related
resources.

```json
{
  "_links": {
    "self": {
      "href": "/orders/123"
    },
    "customer": {
      "href": "/customers/456"
    }
  },
  "id": 123,
  "status": "OPEN"
}
```

---

### GET — collection

Return the collection with navigation and pagination links, so the client navigates through links rather than
constructing URLs itself.

```json
{
  "_links": {
    "self": {
      "href": "/orders?page=1"
    },
    "next": {
      "href": "/orders?page=2"
    }
  },
  "items": [
    {
      "_links": {
        "self": {
          "href": "/orders/123"
        }
      },
      "id": 123
    }
  ]
}
```

A collection that paginates accepts `limit`, defaulting to 20 and ranging from 1 to 100, and `offset` starting at 0. A
value outside a bound answers `422`. `self` names the active view with its filters and page preserved, `next` appears
when more items follow, and `prev` when the page does not start at the first item.

---

### POST — create

**Request** — the body contains only business data.

```json
{
  "status": "OPEN",
  "customerId": 456
}
```

**Response** — `201 Created` with the newly created HAL resource.

```json
{
  "_links": {
    "self": {
      "href": "/orders/123"
    }
  },
  "id": 123,
  "status": "OPEN"
}
```

The response also carries the canonical location of the resource:

```http
Location: /orders/123
```

---

### PUT — replace

**Request** — send the complete business representation.

```json
{
  "status": "CLOSED",
  "customerId": 456
}
```

**Response** — return the updated HAL resource.

```json
{
  "_links": {
    "self": {
      "href": "/orders/123"
    }
  },
  "id": 123,
  "status": "CLOSED"
}
```

---

### PATCH — partial update

**Request** — send only the fields being changed.

```json
{
  "status": "CLOSED"
}
```

**Response** — return the updated HAL resource.

```json
{
  "_links": {
    "self": {
      "href": "/orders/123"
    }
  },
  "id": 123,
  "status": "CLOSED"
}
```

---

### DELETE

**Request** — no request body.

**Response** — prefer `204 No Content`; no HAL response is required.

---

### Search and filtering

Expose search and filtering through a URI-templated link, so clients discover the available search operation from the
resource rather than relying on hardcoded API URLs.

```json
{
  "_links": {
    "search": {
      "href": "/orders{?status,page,size}",
      "templated": true
    }
  }
}
```

Avoid exposing query parameters as separate metadata fields in the payload; URI templates are the appropriate HAL
mechanism for this. The query string is a closed set: a parameter an operation does not declare answers `400` and
applies nothing.

See the [HAL] specification for details.

[HAL]: https://datatracker.ietf.org/doc/html/draft-kelly-json-hal

---

## Asset content retrieval

The content routes — `/asset/file/{file_id}/content`, `/asset/image/{image_id}/content`,
`/asset/audio/{audio_id}/content` and `/asset/video/{video_id}/content` — share one serving path. It streams from an
opened file in bounded chunks, advertises `Accept-Ranges: bytes`, and answers one `Range: bytes=` value:

| Request                                    | Answer                                                                 |
| ------------------------------------------ | ---------------------------------------------------------------------- |
| No range, an unknown unit or a multi-range | `200 OK` with the whole content                                        |
| A closed, open-ended or suffix range       | `206 Partial Content` with `Content-Range: bytes <start>-<end>/<size>` |
| A range starting past the end              | `416 Range Not Satisfiable` with `Content-Range: bytes */<size>`       |
| `HEAD`                                     | The same headers as `GET`, with no body                                |

The response carries the stored media type, `Content-Length`, `ETag` from the stored whole-file digest when one exists,
and `Last-Modified` from `modified_at` always. Every header the serving path emits is declared on the status that emits
it, with the schema its value obeys: `Content-Length` and `Retry-After` are integers, `Location` and the typed asset
links are URI references, `WWW-Authenticate` is a bearer challenge, `Last-Modified` is an HTTP-date, and `ETag`,
`Content-Range`, `Allow`, `Accept-Ranges` and `Cache-Control` carry the pattern their values take. The stored body is
published as a binary string with the wildcard media type, never as a JSON array of integers. A matching `If-None-Match`
answers `304 Not Modified`; an `If-Range` that does not match answers the whole content instead of a range. The opened
handle's metadata, not `file.size_bytes`, governs the response.

Every file, image, audio and video representation carries `_links.content`, whose `href` is the content route. A content
request authenticates with the session `Authorization: Bearer` token only and is decided on every request: the image
route decides `View`, the audio and video routes decide `Stream`, and the file route decides `Download`. The query
string is never a credential. The four representations answer `Cache-Control: private, max-age=60`, and the trace
middleware logs the path and not the query string.

The file content route answers `Content-Disposition: attachment`, naming the typed media row's original name or, for a
bare file, the final segment of its stored path. A name that a quoted string cannot carry is emitted as an RFC 6266
`filename*` value with a sanitized ASCII fallback. The image, audio and video content routes carry no disposition.

Content outcomes are distinguishable: an asset the caller cannot discover is `404 Not Found` and reads no bytes; a
discoverable asset whose action refuses is `403 Forbidden` and reads no bytes; an absent library root is
`503 Service Unavailable` without an authorization read; a registered file whose bytes are absent while the root exists
is `500 Internal Server Error`, which the next scan reconciles.
