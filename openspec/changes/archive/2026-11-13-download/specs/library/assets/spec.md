# Spec Delta

## REMOVED Requirements

### Requirement: Asset content is readable

**Reason**: Byte delivery now has its own capability, `download`, which owns the content routes, the media type they
serve, bounded streaming and `HEAD`.

**Migration**: The four content routes keep their paths. A request reads them with `Authorization: Bearer` and the
route's action — `View` for an image, `Stream` for an audio or video, `Download` for a file — as `download` states.

### Requirement: Content supports byte ranges

**Reason**: Range semantics are part of the byte-delivery contract and move to `download`.

**Migration**: None for a client beyond the authentication change; `Range`, `Accept-Ranges`, `206`, `416` and the
multi-range fallback are unchanged and stated by `download`.

### Requirement: Content carries validators

**Reason**: `ETag`, `Last-Modified`, `If-None-Match` and `If-Range` describe byte delivery and move to `download`.

**Migration**: None; the headers and their conditions are unchanged and stated by `download`.

### Requirement: A content link carries its own credential

**Reason**: Publications and delivery now share one authentication rule: a content request is decided on the spot for
the caller its session token resolves. The file-scoped, twelve-hour credential let a request skip the decision, and it
spent as `Download` after a `View` or `Stream` check, so it disagreed with the action contract it was meant to enforce.

**Migration**: **BREAKING** for a client that followed `_links.content` without an `Authorization` header. A content
request must carry the session bearer token; the `exp` and `sig` query values are refused with `401 Unauthorized`, and a
representation mints no credential.

### Requirement: A content outcome is distinguishable

**Reason**: The outcome taxonomy belongs to the byte-delivery capability, which also separates a discoverable asset
whose action refuses from one the caller cannot discover.

**Migration**: None; `404 Not Found`, `503 Service Unavailable` and `500 Internal Server Error` keep their meaning, and
`403 Forbidden` now answers a visible asset whose action refuses, as `download` states.
