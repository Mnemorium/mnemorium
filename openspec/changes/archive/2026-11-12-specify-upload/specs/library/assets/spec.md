# Spec Delta

## REMOVED Requirements

### Requirement: An upload session is initialized

**Reason**: The intake contract belongs to the `upload` capability, which owns the session and its staging file.

**Migration**: Use `REQ-UPLOAD-001` and `REQ-UPLOAD-002` in `openspec/specs/upload/spec.md`.

### Requirement: An upload session belongs to one caller

**Reason**: The access rule is already stated, once, by `identity/authorization`; `upload` records who owns a session
and does not restate who is denied.

**Migration**: Ownership is recorded by `REQ-UPLOAD-001`. The rule remains `REQ-AUTHORIZATION-018` with
`SCN-AUTHORIZATION-002-E` and `SCN-AUTHORIZATION-015-B`.

### Requirement: A chunk is stored at its declared offset

**Reason**: The chunk protocol belongs to the `upload` capability.

**Migration**: Use `REQ-UPLOAD-004` in `openspec/specs/upload/spec.md`.

### Requirement: A chunk is rejected when the session is not writable

**Reason**: The refusal belongs to the `upload` capability.

**Migration**: Use `REQ-UPLOAD-005` in `openspec/specs/upload/spec.md`.

### Requirement: The session state is readable

**Reason**: Reading the session belongs to the `upload` capability.

**Migration**: Use `REQ-UPLOAD-006` in `openspec/specs/upload/spec.md`.

### Requirement: Completion verifies the whole-file digest

**Reason**: Completion belongs to the `upload` capability.

**Migration**: Use `REQ-UPLOAD-007` in `openspec/specs/upload/spec.md`, where the digest is named an **Integrity hash**.

### Requirement: Completion registers the typed media

**Reason**: Promotion and registration belong to the `upload` capability.

**Migration**: Use `REQ-UPLOAD-009` and `REQ-UPLOAD-017` in `openspec/specs/upload/spec.md`.

### Requirement: The declared media type is authoritative

**Reason**: The declared type's authority is part of the upload contract.

**Migration**: Use `REQ-UPLOAD-010` in `openspec/specs/upload/spec.md`.

### Requirement: Completion is sealed against concurrent writes

**Reason**: Sealing belongs to the `upload` capability.

**Migration**: Use `REQ-UPLOAD-008` in `openspec/specs/upload/spec.md`.

### Requirement: An upload session expires at a fixed instant

**Reason**: Expiry belongs to the `upload` capability.

**Migration**: Use `REQ-UPLOAD-012` in `openspec/specs/upload/spec.md`.

### Requirement: An upload session can be cancelled

**Reason**: Cancellation belongs to the `upload` capability.

**Migration**: Use `REQ-UPLOAD-013` in `openspec/specs/upload/spec.md`.

### Requirement: Expired, stale and orphaned sessions are reaped

**Reason**: Reaping belongs to the `upload` capability.

**Migration**: Use `REQ-UPLOAD-015` in `openspec/specs/upload/spec.md`.

### Requirement: A finished session keeps its identity

**Reason**: Finished-session identity belongs to the `upload` capability.

**Migration**: Use `REQ-UPLOAD-014` in `openspec/specs/upload/spec.md`.

### Requirement: Duplicate content is reused per caller

**Reason**: Per-caller dedupe is a completion outcome and belongs to the `upload` capability.

**Migration**: Use `REQ-UPLOAD-011` in `openspec/specs/upload/spec.md`.

### Requirement: The chunk size is fixed for the session

**Reason**: The chunk-size rule belongs to the `upload` capability.

**Migration**: Use `REQ-UPLOAD-003` in `openspec/specs/upload/spec.md`.

### Requirement: An absent library root is unavailable

**Reason**: The upload route's refusal belongs to the `upload` capability; the startup decision on an absent root
remains with `library/media-ingest`.

**Migration**: Use `REQ-UPLOAD-016` in `openspec/specs/upload/spec.md`; the startup rule is unchanged in
`library/media-ingest`.
