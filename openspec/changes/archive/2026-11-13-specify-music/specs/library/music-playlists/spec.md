# Spec Delta

## REMOVED Requirements

### Requirement: Playlist endpoints require an authenticated account

**Reason**: The capability is retired in favour of `music`, which names the action each playlist operation decides
against. The token requirement is `authentication`'s, not a music rule.

**Migration**: The token requirement is owned by `REQ-AUTHENTICATION-009`. No music read requirement replaces this one;
the playlist reads it guarded are `REQ-MUSIC-018` and `REQ-MUSIC-021`.

### Requirement: Playlist path identifiers are validated

**Reason**: Validation is an outcome of each addressed operation, not a rule of its own; a path segment that is not a
valid identifier is a `400` wherever it appears and an index outside its range is a `422`.

**Migration**: Replaced by the malformed-identifier scenarios of `REQ-MUSIC-021`, `REQ-MUSIC-022` and `REQ-MUSIC-025`.

### Requirement: Playlist reads follow visibility

**Reason**: The requirement encoded the deleted `playlist.read` scope and its `PUBLIC_OR_OWNED`, `LISTED`, `OWNED` and
`NONE` overrides. **Visibility** is now a grant `authorization` owns, and the read decides against `Discover` and
`View`.

**Migration**: Replaced by `REQ-MUSIC-018` and `REQ-MUSIC-021`.

### Requirement: Track moderation follows playlist visibility

**Reason**: The rule contradicted the shipped server and the authorization model: a `Public` playlist grants the viewer
actions, never a write, so a non-owner's modification is refused. The new capability names the write each operation
requires instead of a `playlist.moderate` permission.

**Migration**: Replaced by `REQ-MUSIC-025`, `REQ-MUSIC-026` and `REQ-MUSIC-027`.

### Requirement: Playlist metadata and deletion are owner-only

**Reason**: The rule restated ownership that `authorization` derives from the resource, and named the caller with a role
rather than an **Action**.

**Migration**: Replaced by `REQ-MUSIC-024` and `REQ-MUSIC-028`, which require `EditMetadata` and `Delete` on the
playlist.

### Requirement: List music playlists

**Reason**: The listing is one of the operations the new `music` capability owns.

**Migration**: Replaced by `REQ-MUSIC-018`.

### Requirement: Create a playlist

**Reason**: The new capability states the creation with the `CreateChild` check `authorization` seeds, which the
implementation performed not at all.

**Migration**: Replaced by `REQ-MUSIC-019`.

### Requirement: Playlist names are normalized and unique per owner

**Reason**: The capability is retired in favour of `music`.

**Migration**: Replaced by `REQ-MUSIC-020`.

### Requirement: Read a playlist

**Reason**: The capability is retired in favour of `music`.

**Migration**: Replaced by `REQ-MUSIC-021`.

### Requirement: Read one playlist track

**Reason**: The capability is retired in favour of `music`.

**Migration**: Replaced by `REQ-MUSIC-022`.

### Requirement: Playlist tracks are 0-based and ordered by index

**Reason**: The capability is retired in favour of `music`, which also states that a playlist track carries no artwork
of its own.

**Migration**: Replaced by `REQ-MUSIC-023`.

### Requirement: Update playlist metadata

**Reason**: The capability is retired in favour of `music`.

**Migration**: Replaced by `REQ-MUSIC-024`.

### Requirement: Add a recording through PATCH

**Reason**: The capability is retired in favour of `music`, which states both the container and the target check that
`authorization` requires of a playlist entry.

**Migration**: Replaced by `REQ-MUSIC-025`.

### Requirement: No duplicate recordings within a playlist

**Reason**: Duplicate refusal is part of appending a recording, not a separate behaviour.

**Migration**: Replaced by `REQ-MUSIC-025`.

### Requirement: Remove a track from a playlist

**Reason**: The capability is retired in favour of `music`.

**Migration**: Replaced by `REQ-MUSIC-026`.

### Requirement: Reorder playlist tracks

**Reason**: The capability is retired in favour of `music`.

**Migration**: Replaced by `REQ-MUSIC-027`.

### Requirement: Delete a playlist

**Reason**: The capability is retired in favour of `music`.

**Migration**: Replaced by `REQ-MUSIC-028`.
