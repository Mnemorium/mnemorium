# Spec Delta

## REMOVED Requirements

### Requirement: A file is readable by its identifier

**Reason**: The stored-file resource is owned by `file`, which states the record, the read and the links separately.

**Migration**: Read `REQ-FILE-001`, `REQ-FILE-002` and `REQ-FILE-003`; the route and representation fields are
unchanged.

### Requirement: Asset content is readable

**Reason**: Byte delivery is owned by `download`.

**Migration**: Read `REQ-DOWNLOAD-003` and `REQ-DOWNLOAD-004`; the four content routes, the stored media type, `HEAD`
and bounded streaming are unchanged.

### Requirement: Content supports byte ranges

**Reason**: Range semantics are owned by `download`.

**Migration**: Read `REQ-DOWNLOAD-005`; the closed, open-ended, suffix, unsatisfiable and multi-range outcomes are
unchanged.

### Requirement: Content carries validators

**Reason**: The entity validators and conditional requests are owned by `download`.

**Migration**: Read `REQ-DOWNLOAD-006`; `ETag`, `Last-Modified`, `If-None-Match` and `If-Range` are unchanged.

### Requirement: A content link carries its own credential

**Reason**: `download` removed the minted content credential; a content request authenticates with the session token
only.

**Migration**: Read `REQ-DOWNLOAD-001`; `_links.content` is a plain route link and no representation publishes a
credential.

### Requirement: A content outcome is distinguishable

**Reason**: The outcome taxonomy of a content read is owned by `download`.

**Migration**: Read `REQ-DOWNLOAD-008` and `REQ-DOWNLOAD-009`; `404`, `503`, `500` and the no-bytes rule are unchanged.
