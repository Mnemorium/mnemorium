# API documentation

This document describes how the OpenAPI specification of the Mnemorium HTTP API
is generated and declared, and defines the contract every endpoint handler must
satisfy.

The OpenAPI specification is generated with [Utoipa]. Handlers are declared in
`src/lib/infrastructure/inbound/rest/handler`, wired into the axum router in
`bootstrap.rs` (nested under `/api/v1`) and documented inline, at the source,
with `#[utoipa::path(...)]` macros.

[Utoipa]: https://docs.rs/utoipa

## OpenAPI generation

### Declaration

1. Each handler function in `src/lib/infrastructure/inbound/rest/handler` is
   annotated with a `#[utoipa::path(...)]` macro. The macro produces one OpenAPI
   _path item_ for the endpoint and must respect the
   [endpoint handler contract](#endpoint-handler-contract).
2. A dedicated struct derives `utoipa::OpenApi`. It aggregates all the path
   items, their tags and the reusable schemas:

   ```rust
   #[derive(utoipa::OpenApi)]
   #[openapi(
       paths(create_note, list_notes),
       components(schemas(Note, NewNote, ErrorBody)),
       tags(
           (name = "notes", description = "Note bounded context")
       )
   )]
   struct ApiDoc;
   ```

### Materializing the specification

`src/bin/openapi_gen.rs` consumes that derive and renders the specification to
`docs/development/api/openapi.json` — the same folder this documentation lives
in — so the spec always stays in sync with the source:

```rust
use std::fs;

fn main() {
    let spec = serde_json::to_string_pretty(&ApiDoc::openapi()).expect("serialize spec");

    fs::create_dir_all("docs/development/api").expect("create docs/development/api");
    fs::write("docs/development/api/openapi.json", spec)
        .expect("write openapi.json");
}
```

Run it with `cargo run --bin openapi_gen`. The resulting `openapi.json` is
committed alongside this document.

### Serving the specification at runtime

The running server can also expose the specification and an interactive Swagger
UI through `utoipa-swagger-ui`, so the API surface is browsable while the
service is up.

### Rendering the specification in MkDocs

`mkdocs.yml` uses the `neoteroi.mkdocsoad` plugin. Once `openapi_gen` has
emitted `docs/development/api/openapi.json`, embed the live specification in any
page with the `:::oas` directive:

````markdown
```yaml
:::oas spec.openapi
```
````

The plugin loads the OAS from the generated JSON, so shipping the documentation
and the spec together in `docs/development/api/` keeps them version-locked.

## Endpoint handler contract

Every endpoint handler must declare, inside its `#[utoipa::path(...)]` macro,
all of the following:

| Attribute                                   | Rule                                                                          |
| ------------------------------------------- | ----------------------------------------------------------------------------- |
| `operation_id`                              | Unique operation identifier for the endpoint.                                 |
| `get` / `post` / `put` / `patch` / `delete` | The HTTP method of the operation.                                             |
| `path`                                      | The route of the endpoint.                                                    |
| `tag`                                       | The bounded context the endpoint belongs to.                                  |
| `request_body`                              | The request body, see [request body](#request-body).                          |
| `params`                                    | The path, query, header and cookie parameters, see [parameters](#parameters). |
| `responses`                                 | **All** the possible responses, see [responses](#responses).                  |
| `security`                                  | The security scheme(s) protecting the endpoint.                               |
| `summary`                                   | A one line human readable summary of the endpoint.                            |

### Example

```rust
use utoipa::OpenApi;

#[derive(utoipa::OpenApi)]
#[openapi(
    paths(create_note, list_notes),
    components(schemas(Note, NewNote, ErrorBody)),
    tags(
        (name = "notes", description = "Note bounded context")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
struct ApiDoc;

/// Create a note.
///
/// Returns the created note, `400` if the payload is not valid
/// and `401` if the caller is unauthenticated.
#[utoipa::path(
    post,
    operation_id = "create_note",
    path = "/notes",
    tag = "notes",
    request_body = NewNote,
    responses(
        (status = CREATED, body = Note, description = "Note created"),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            description = "Invalid payload"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            description = "Missing or invalid credentials"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Create a new note"
)]
pub async fn create_note() -> axum::response::Json<Note> {
    unimplemented!()
}
```

## Parameters

Each parameter must declare, inside the `params(...)` attribute:

- `name` — the parameter name as it appears in the URL / header / cookie.
- `parameter_type` — one of `Path`, `Query`, `Header` or `Cookie`, bound to the
  Rust type of the parameter.
- `description` — a human readable description of the parameter.

Additional constraints may refine the value, see
[constraint attributes](#constraint-attributes).

Parameters can be declared either as inline tuples inside `params(...)`, or as a
dedicated `IntoParams` struct reused by several handlers.

### Inline tuples

```rust
#[utoipa::path(
    get,
    operation_id = "get_note",
    path = "/notes/{id}",
    tag = "notes",
    params(
        ("id", Path = uuid::Uuid, description = "Note id"),
        (
            "X-Request-Id",
            Header = uuid::Uuid,
            description = "Correlation id",
        ),
    ),
    responses(
        (status = OK, body = Note, description = "Note found"),
        (status = NOT_FOUND, body = ErrorBody, description = "Unknown note"),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Fetch a single note"
)]
pub async fn get_note() -> axum::response::Json<Note> {
    unimplemented!()
}
```

### `IntoParams` struct

```rust
use serde::{Deserialize, Serialize};
use utoipa::IntoParams;

/// List notes, paginated.
#[derive(Debug, Deserialize, Serialize, IntoParams)]
pub struct ListNotesParams {
    /// Maximum number of notes to return.
    #[param(maximum = 100, minimum = 1, default = 20, description = "Page size")]
    pub limit: u32,

    /// Offset of the first note to return.
    #[param(minimum = 0, default = 0, description = "Page offset")]
    pub offset: u32,

    /// Only return notes matching this title.
    #[param(min_length = 1, max_length = 128, description = "Title filter")]
    pub title: Option<String>,
}
```

In the macro:

```rust
#[utoipa::path(
    get,
    operation_id = "list_notes",
    path = "/notes",
    tag = "notes",
    params(ListNotesParams),
    responses(
        (status = OK, body = [Note], description = "Notes matching the query"),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "List notes"
)]
pub async fn list_notes() -> axum::response::Json<Vec<Note>> {
    unimplemented!()
}
```

### Constraint attributes

The following attributes can be applied to any parameter (and, through
`#[schema(...)]`, to any property of a `ToSchema` model):

| Attribute           | Type                         | Meaning                                                                                          |
| ------------------- | ---------------------------- | ------------------------------------------------------------------------------------------------ |
| `format`            | `KnownFormat` or open string | The data type format. See below.                                                                 |
| `write_only`        | flag                         | Property is used only in write operations (`POST`, `PUT`, `PATCH`), never in `GET`.              |
| `read_only`         | flag                         | Property is used only in read operations (`GET`), never in `POST`, `PUT`, `PATCH`.               |
| `nullable`          | flag                         | Property is nullable (note: different from non-required).                                        |
| `multiple_of`       | number                       | Value must be a multiple — the division must yield an integer. Value must be strictly above `0`. |
| `maximum`           | number                       | Inclusive upper bound for a number value.                                                        |
| `minimum`           | number                       | Inclusive lower bound for a number value.                                                        |
| `exclusive_maximum` | number                       | Exclusive upper bound for a number value.                                                        |
| `exclusive_minimum` | number                       | Exclusive lower bound for a number value.                                                        |
| `max_length`        | number                       | Maximum length for string types.                                                                 |
| `min_length`        | number                       | Minimum length for string types.                                                                 |
| `pattern`           | string                       | A valid regular expression in the ECMA-262 dialect the value must match.                         |
| `max_items`         | number                       | Maximum items allowed for array fields. Value must be a non-negative integer.                    |
| `min_items`         | number                       | Minimum items allowed for array fields. Value must be a non-negative integer.                    |

> `format` may either be a variant of the `KnownFormat` enum, or otherwise an
> open value as a string. By default the format is derived from the type of the
> property according to the OpenAPI specification.

These attributes apply identically on request/response body models through the
`#[schema(...)]` attribute of the `ToSchema` derive:

```rust
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Note {
    /// Unique note identifier.
    #[schema(read_only, format = "uuid", example = json!("2lJT"))]
    pub id: String,

    /// Note title.
    #[schema(min_length = 1, max_length = 128, example = json!("Habit tracking"))]
    pub title: String,

    /// Note content.
    #[schema(min_length = 1, max_length = 65536)]
    pub body: String,
}
```

## Examples

An `example` documents a concrete sample value for a schema, parameter or
response. It can be declared inline or as a named example with a `name`, a
`summary` and a `value`.

### Inline example

```rust
#[schema(example = json!({"title": "Habit tracking", "body": "Log daily streaks."}))]
```

### Named example

```rust
use utoipa::openapi::Example;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(example = Example(
    name = "minimal",
    summary = "A note with the smallest valid payload",
    value = json!({"title": "Todo", "body": "x"}),
))]
pub struct NewNote {
    /// Note title.
    #[schema(min_length = 1, max_length = 128)]
    pub title: String,

    /// Note content.
    #[schema(min_length = 1, max_length = 65536)]
    pub body: String,
}
```

## Request body

Each request body declares:

- `content_type` — the media type of the body (for example `application/json`).
- `body` — the schema of the body (`content` in the OpenAPI terms).
- `example` — an optional example value.

```rust
#[utoipa::path(
    post,
    operation_id = "create_note",
    path = "/notes",
    tag = "notes",
    request_body(
        content_type = "application/json",
        content = NewNote,
        example = json!({"title": "Habit tracking", "body": "Log daily streaks."}),
    ),
    responses(
        (status = CREATED, body = Note, description = "Note created"),
        (status = BAD_REQUEST, body = ErrorBody, description = "Invalid payload"),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Create a new note"
)]
pub async fn create_note() -> axum::response::Json<Note> {
    unimplemented!()
}
```

## Responses

Every response declares:

- `status` — the HTTP status code of the response.
- `description` — a human readable description of the response.
- `body` — the schema of the response payload, when there is one.
- `content_type` — the media type of the response payload, when there is one.
- `headers` — the response headers, when there are any.
- `example` — an optional example of the response payload.
- `link` — an optional link to another operation, expressed through the target
  operation's `operation_id`.

All the possible responses must be declared — the success case as well as every
error case the handler can produce.

```rust
use utoipa::openapi::{
    content::Content,
    header::Header,
    response::{Link, Response},
};

#[utoipa::path(
    post,
    operation_id = "create_note",
    path = "/notes",
    tag = "notes",
    request_body = NewNote,
    responses(
        (status = CREATED, body = Note, description = "Note created"),
        (status = BAD_REQUEST, body = ErrorBody, description = "Invalid payload"),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/json",
            headers(
                ("Location", Header = String, description = "URI of the conflicting note"),
            ),
            example = json!({"error": "A note with this title already exists"}),
            links(
                ("conflict" = Link("get_note") = ("id")),
            ),
            description = "A note with the same title already exists"
        ),
        (status = UNAUTHORIZED, body = ErrorBody, description = "Missing or invalid credentials"),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Create a new note"
)]
pub async fn create_note() -> axum::response::Json<Note> {
    unimplemented!()
}
```

`links` reference another operation declared in the same `OpenApi` derive by its
`operation_id`, and express how a field of this response maps to a parameter of
the linked operation. In the example above, on a `409 Conflict`, `create_note`
points to `get_note` using the `id` field of the response body.

> `write_only`, `read_only` and all the
> [constraint attributes](#constraint-attributes) apply on request/response body
> models through the `#[schema(...)]` attribute of the `ToSchema` derive,
> exactly as they do on parameters.

## HAL Payload Guidelines

### General rules

- Use `application/hal+json` for HAL responses.
- Always include `_links.self` on resource representations.
- Business data lives at the root of the payload.
- `_links` is reserved for navigation and related resources.
- **Do not use `_embedded`** unless there is a proven performance need.
- Expose search and filtering capabilities through URI templates
  (`templated: true`).
- Keep actions discoverable through links; clients should never construct or
  hardcode URLs.
- HAL is primarily a **response representation format**. Requests should
  generally contain only business data.

The examples below use `/orders` as the resource and show how each HTTP method
maps onto HAL.

### GET — single resource

Return the resource representation with the relevant links, so the client can
read the resource and discover related resources.

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

### GET — collection

Return the collection with navigation and pagination links, so the client
navigates through links rather than constructing URLs itself.

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

### DELETE

**Request** — no request body.

**Response** — prefer `204 No Content`; no HAL response is required.

### Search and filtering

Expose search and filtering through a URI-templated link, so clients discover
the available search operation from the resource rather than relying on
hardcoded API URLs.

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

Avoid exposing query parameters as separate metadata fields in the payload; URI
templates are the appropriate HAL mechanism for this.

See the [HAL] specification for details.

[HAL]: https://datatracker.ietf.org/doc/html/draft-kelly-json-hal
