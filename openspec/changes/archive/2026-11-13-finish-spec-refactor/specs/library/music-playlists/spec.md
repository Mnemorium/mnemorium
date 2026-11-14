# Spec Delta

## REMOVED Requirements

### Requirement: Playlist endpoints require an authenticated account

**Reason**: Authentication and authorization own who may read; `music` names the action only.

**Migration**: Read `REQ-MUSIC-018` and `REQ-MUSIC-021`; the bearer requirement stays with `authentication`, and the
decision with `authorization`.

### Requirement: Playlist path identifiers are validated

**Reason**: Identifier validation is `api`'s contract, and every playlist read states its own outcome.

**Migration**: Read `REQ-API-011` and `REQ-MUSIC-021`; an invalid identifier still answers `400 Bad Request`.

### Requirement: Playlist reads follow visibility

**Reason**: Visibility is one grant rule owned by `authorization`, not a per-capability rule.

**Migration**: Read `REQ-AUTHORIZATION-016`; `music` states the `View` action the route requires.

### Requirement: Track moderation follows playlist visibility

**Reason**: `music` states the corrected rule the server serves: only a caller holding the playlist's write action may
add, remove or reorder its tracks.

**Migration**: Read `REQ-MUSIC-025`, `REQ-MUSIC-026` and `REQ-MUSIC-027`; `authorization` owns who holds the action.

### Requirement: Playlist metadata and deletion are owner-only

**Reason**: Ownership is `authorization`'s rule; `music` names the actions the two writes require.

**Migration**: Read `REQ-MUSIC-024` and `REQ-MUSIC-028`; the owner-only outcome is unchanged.

### Requirement: List music playlists

**Reason**: The playlist collection is owned by `music`.

**Migration**: Read `REQ-MUSIC-018`; the filters, pagination and partial items are unchanged.

### Requirement: Create a playlist

**Reason**: The playlist write is owned by `music`.

**Migration**: Read `REQ-MUSIC-019`; the payload, the `CreateChild` check and the statuses are unchanged.

### Requirement: Playlist names are normalized and unique per owner

**Reason**: Name normalization and per-owner uniqueness are owned by `music`.

**Migration**: Read `REQ-MUSIC-020`; the normalization and the conflict outcome are unchanged.

### Requirement: Read a playlist

**Reason**: The playlist representation is owned by `music`.

**Migration**: Read `REQ-MUSIC-021`; the representation fields and the track order are unchanged.

### Requirement: Read one playlist track

**Reason**: The playlist track representation is owned by `music`.

**Migration**: Read `REQ-MUSIC-022`; the representation fields are unchanged.

### Requirement: Playlist tracks are 0-based and ordered by index

**Reason**: Index addressing and ordering are owned by `music`.

**Migration**: Read `REQ-MUSIC-023`; the zero base and the ascending order are unchanged.

### Requirement: Update playlist metadata

**Reason**: The partial update is owned by `music`; the obsolete who-may wording is replaced by the action.

**Migration**: Read `REQ-MUSIC-024`; the absent and null semantics are unchanged.

### Requirement: Add a recording through PATCH

**Reason**: Appending a track is owned by `music`, which also checks the entry's own read action.

**Migration**: Read `REQ-MUSIC-025`; the append position and the statuses are unchanged.

### Requirement: No duplicate recordings within a playlist

**Reason**: The duplicate refusal is part of the `music` append contract.

**Migration**: Read `REQ-MUSIC-025`; a recording already in the playlist still answers `409 Conflict`.

### Requirement: Remove a track from a playlist

**Reason**: Removing a track is owned by `music`.

**Migration**: Read `REQ-MUSIC-026`; the reindexing and the statuses are unchanged.

### Requirement: Reorder playlist tracks

**Reason**: Reordering is owned by `music`.

**Migration**: Read `REQ-MUSIC-027`; the complete-set rule and the reindexing are unchanged.

### Requirement: Delete a playlist

**Reason**: The playlist deletion is owned by `music`.

**Migration**: Read `REQ-MUSIC-028`; the outcome and the statuses are unchanged.
