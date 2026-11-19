# Spec Delta

## ADDED Requirements

### Requirement: REQ-API-017 — Date and time properties declare their wire format

Every date property, in a request or a response, SHALL declare `format: date` in `YYYY-MM-DD`; every timestamp property
SHALL declare `format: date-time` in RFC 3339 UTC. No date or timestamp property SHALL be published without its format.
A request value that does not match its declared format SHALL answer `422 Unprocessable Content`, and a response value
SHALL render in the declared format.

#### Scenario: SCN-API-017-A — A date property declares its format

- **WHEN** the published document declares a date property
- **THEN** its schema carries `format: date` and its value is `YYYY-MM-DD`

#### Scenario: SCN-API-017-B — A timestamp property declares its format

- **WHEN** the published document declares a timestamp property
- **THEN** its schema carries `format: date-time` and its value is RFC 3339 UTC ending in `Z`

#### Scenario: SCN-API-017-C — A malformed request value is rejected

- **WHEN** a request carries a date or timestamp that does not match its declared format
- **THEN** the system answers `422 Unprocessable Content` and stores nothing

### Requirement: REQ-API-018 — A closed vocabulary is published as an enum schema

Every closed vocabulary the server enforces SHALL be published as an enum schema on the property or parameter that
carries it. The handler SHALL still validate the raw wire value, so a value outside the vocabulary answers
`422 Unprocessable Content` with an `enum_validation_failed` event.

#### Scenario: SCN-API-018-A — A body vocabulary is an enum

- **WHEN** the document publishes a request property that carries a closed vocabulary
- **THEN** its schema enumerates the values the server accepts

#### Scenario: SCN-API-018-B — A parameter vocabulary is an enum

- **WHEN** the document publishes a path or query parameter that carries a closed vocabulary
- **THEN** its schema enumerates the values the server serves

#### Scenario: SCN-API-018-C — A value outside the vocabulary is rejected

- **WHEN** a request carries a value outside a published vocabulary
- **THEN** the server answers `422 Unprocessable Content` and records `enum_validation_failed`

### Requirement: REQ-API-019 — A response header is published with its value's schema

Every response header the server emits SHALL be declared on the response that emits it, with the schema its value obeys:
`Content-Length` an integer, `Location` a URI reference, `WWW-Authenticate` a bearer challenge, `Last-Modified` an
HTTP-date, and `ETag`, `Content-Range`, `Allow`, `Accept-Ranges` and `Cache-Control` the shape their values take.

#### Scenario: SCN-API-019-A — A content response declares its headers

- **WHEN** a content route answers `200`, `206`, `304` or `416`
- **THEN** the document declares every header that response emits, with the value's schema

#### Scenario: SCN-API-019-B — A creation declares its location

- **WHEN** an operation answers `201` or `303` with a `Location`
- **THEN** the document declares `Location` as a URI reference

#### Scenario: SCN-API-019-C — A rejection declares its challenge

- **WHEN** an operation answers `401`
- **THEN** the document declares `WWW-Authenticate` as a bearer challenge

### Requirement: REQ-API-020 — Stored content is published as bytes

A response body of stored content SHALL be published as a string with `format: binary`, never as a JSON array of
integers. A content route SHALL keep the wildcard media type, because the stored media type varies per row.

#### Scenario: SCN-API-020-A — A content body is binary

- **WHEN** the document publishes a content route's response
- **THEN** its body schema is a binary string

#### Scenario: SCN-API-020-B — A content type is the stored one

- **WHEN** a content route answers with bytes
- **THEN** its `Content-Type` is the stored media type, and the document keeps the wildcard

### Requirement: REQ-API-021 — A link target is a non-empty URI reference or template

A HAL link SHALL carry a non-empty `href` that is a relative URI reference, or an RFC 6570 URI template when the link is
templated. A link SHALL NOT carry an empty target.

#### Scenario: SCN-API-021-A — A plain link is a relative reference

- **WHEN** a representation publishes a plain link
- **THEN** its `href` is a non-empty relative URI reference

#### Scenario: SCN-API-021-B — A templated link keeps its expressions

- **WHEN** a representation publishes a templated link
- **THEN** its `href` is a non-empty RFC 6570 template and `templated` is `true`

### Requirement: REQ-API-022 — An undeclared query parameter is rejected

The query string SHALL be a closed set: a query parameter the operation does not declare SHALL answer `400 Bad Request`
and apply nothing, so an ignored parameter never changes a response silently.

#### Scenario: SCN-API-022-A — An undeclared parameter is refused

- **WHEN** a request carries a query parameter the operation does not declare
- **THEN** the system answers `400 Bad Request` and applies nothing
