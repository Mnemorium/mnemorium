# Use Cases

Catalog of the **business use cases** of Mnemorium. Each entry describes a capability initiated by an actor; it is a
business-level view, not a one-to-one list of the application's implementation units.

A business use case is implemented by one or more **application use cases** — the `port/<name>.rs` /
`use_case/<name>.rs` pair governed by [TechnicalDesign.md](TechnicalDesign.md) § 9 (Use cases). The catalog does not
list them: an entry describes the capability, not the code that implements it.

## Catalog

## UC-001 - Create a User Account

### Description

Allow an Administrator to create a new user in the system and assign the appropriate profile information.

### Primary actor

- Admin

### Pre condition(s)

- The caller is authenticated as an Admin.

### Trigger(s)

- User creation request

### Bounded context(s)

- Identity
- User

### Business rules

- Username must be at least 4 characters long.
- Email must be a valid email address or be null.
- Password must be at least 8 characters long and contain at least one symbol.

### Happy path

1. The Admin submits a user creation request carrying a username, an optional email, a password, and the role to grant.
2. The system validates the request against the business rules.
3. The system verifies the caller may grant the requested role.
4. The system verifies that no user already exists with the username or email.
5. The system secures the password and saves the new user with its credential.
6. The system returns the created account.

### Alternative flow

- 2a. The request is invalid — empty username, empty password, or any other business-rule violation.
- 3a. The Admin is not the Root Admin and requests the Admin role for the new user; the system rejects the request.

### Post condition(s)

- A new user account and its credential exist in the system.
- Username and email remain unique across users.

---

## UC-002 - User Authentication

### Description

Allow a registered user to securely authenticate with the system using valid credentials and gain access to authorized
features and resources.

### Primary actor

- Standard User

### Pre condition(s)

- The user is registered: an account with its credential exists in the system.

### Trigger(s)

- Authentication request

### Bounded context(s)

- Identity
- User

### Business rules

- The username must name an existing account and the password must match the one the account holds.
- A failed authentication does not reveal whether the username exists or the password was wrong.
- A successful authentication issues a token identifying the user; a failed one issues no token.

### Happy path

1. The user submits an authentication request carrying a username and a password.
2. The system finds the user by username.
3. The system verifies the password against the password the account holds.
4. The system issues a token identifying the user.
5. The user gains access to the authorized features and resources.

### Alternative flow

- 2a. Bad credentials: no account matches the username, or the password does not match the password the account holds;
  the system rejects the request.

### Post condition(s)

- The user holds a valid token granting access to authorized features and resources.

---

## UC-003 - Initialize Root Admin

### Description

On the first runtime the system creates the Root Admin account, issues and reveals a default password to the operator,
and lets the Root Admin replace it with a personal password. The default password stops being revealed once it has been
changed.

### Primary actor

- Root Admin

### Pre condition(s)

- First-Time Admin Authentication is enabled in configuration.
- The default password corresponding to the Root Admin account has not been replaced yet.

### Trigger(s)

- Start of system runtime while the configuration still enables First-Time Admin Authentication.

### Bounded context(s)

- Identity
- User
- Configuration

### Business rules

- The Root Admin account is created with a random default password when no Root Admin exists.
- The default password is shown to the operator only while the Root Admin still uses it.
- The new password must be at least 8 characters long and contain at least one symbol.

### Happy path

1. The system starts with First-Time Admin Authentication enabled.
2. The system checks whether a Root Admin account exists; none does, so the system creates the account with a generated
   default password.
3. The system shows the default password to the operator.
4. The Root Admin submits an authentication request carrying the default password and a new password.
5. The system verifies the default password against the password the account holds.
6. The system validates the new password against the business rules.
7. The system secures the new password, replaces the credential, and the default password stops being shown.
8. The system returns the updated account.

### Alternative flow

- 2a. A Root Admin already exists: the system stops at step 1 and the functionality stays inactive — no new account and
  no revealed password.
- 5a. Bad password: the default password does not match the password the account holds; the system rejects the request.
- 6a. The new password violates the business rules; the system rejects the request.

### Post condition(s)

- A Root Admin account exists in the system.
- The Root Admin holds a personal credential.
- The default password is no longer valid and is no longer shown to the operator while the configuration has First-Time
  Admin Authentication enabled.

---

## UC-004 - Load Configuration

### Description

On every runtime the system loads the application configuration by layering the stored settings with the optional
configuration file and the environment, generating the secrets on the first runtime.

### Primary actor

- System

### Pre condition(s)

- The system's stored settings are available.

### Trigger(s)

- Start of system runtime.

### Bounded context(s)

- Configuration

### Business rules

- The configuration is layered: the stored settings are the base, the configuration file overrides them, and the
  environment overrides both.
- On the first runtime no stored settings exist; the system creates the default settings and freshly generated secrets
  before loading.
- The default settings enable diagnostic logging with daily rotation, keep seven rotated files and disable colour
  output.
- The default settings allow the Root Admin default password to be shown to the operator.
- Secrets left empty after the first runtime are never regenerated: the stored settings keep them stable across
  restarts.

### Happy path

1. The system checks whether stored settings exist; none do, so the system creates the default settings and freshly
   generated secrets.
2. The system loads the configuration: the stored settings are layered with the configuration file and the environment
   overrides.
3. The system returns the merged configuration to the runtime.

### Alternative flow

- 1a. Stored settings already exist: the system skips their creation and loads the configuration directly.

### Post condition(s)

- The stored settings exist.
- The runtime holds a complete configuration to wire its dependencies.

---

## UC-005 - Delete User Account

### Description

Allow an Administrator to remove a user account together with all the data it owns, and prevent the affected user from
accessing the system.

### Primary actor

- Admin

### Pre condition(s)

- The caller is authenticated as an Admin.

### Trigger(s)

- User deletion request

### Bounded context(s)

- Identity
- User
- Library
- Asset

### Business rules

- The Root Admin account cannot be deleted.
- Deletion removes the user, its credential, and everything the user owns: its files, its playlists, and the media
  records that depend on those files.
- Tokens issued to the deleted user stop being accepted immediately.

### Happy path

1. The Admin submits a user deletion request identifying the target user.
2. The system verifies the target user exists.
3. The system verifies the target user is not the Root Admin.
4. The system deletes the data owned by the user and then the account with its credential.
5. The system stops accepting the tokens issued to the deleted user.
6. The system confirms the deletion.

### Alternative flow

- 1a. The caller is not an Admin; the system rejects the request.
- 2a. No user exists for the requested identifier; the system rejects the request.
- 3a. The target user is the Root Admin; the system rejects the request.

### Post condition(s)

- The user account and its credential no longer exist.
- Every file, playlist, and dependent media record owned by the user no longer exists.
- The deleted user cannot authenticate and any token it held is rejected.

---

## UC-006 - Upload Media File

### Description

Allow a user to upload a supported media file to the service for storage, processing, cataloging, and distribution. The
upload is resumable and chunked, so it spans several requests.

### Primary actor

- Standard User

### Pre condition(s)

- The caller is authenticated.

### Trigger(s)

- Upload request

### Bounded context(s)

- Asset
- Library

### Business rules

- The uploaded content must match one of the supported media kinds (audio, video or image) the system recognises.
- The media kind is determined from the file content, not from the file name or declared type alone.
- The stored file path is unique across files.
- A content fingerprint is computed and stored with the file; a caller's file records are unique by fingerprint.
- Duplicate content is deduplicated per caller: when the caller already owns a file with the same fingerprint, the
  uploaded content is verified against the declared fingerprint and the existing file record is returned instead of
  storing a second copy. The upload is left unfinished.
- The uploaded file is private by default.

### Happy path

1. The user submits an upload request carrying the media file.
2. The system determines the media kind from the file content.
3. The system verifies the media kind is supported.
4. The system computes the content fingerprint.
5. The system stores the file and saves its record bound to the caller.
6. The system returns the created file record.

### Alternative flow

- 3a. The media kind is unsupported; the system rejects the upload and stores nothing.
- 4a. The caller already owns a file with the same fingerprint; the system verifies the uploaded content and returns the
  existing file record, storing nothing new and leaving the upload unfinished.

### Post condition(s)

- The file is stored under a unique path.
- A file record bound to the caller exists with its media kind, content fingerprint and private visibility.

---

## UC-007 - Create a Gallery

### Description

Allow an authenticated user to create a gallery they own to collect images and videos.

### Primary actor

- Standard User
- Admin

### Pre condition(s)

- The caller is authenticated.

### Trigger(s)

- Gallery creation request (`POST /api/v1/library/gallery`).

### Bounded context(s)

- Library

### Business rules

- The gallery name is required, is trimmed of surrounding whitespace, must not be empty or only whitespace, and must be
  at most 100 characters long.
- A caller's gallery names are unique: the same owner cannot create two galleries with the same name.
- A gallery is private by default; the caller may request it be public.
- The caller becomes the gallery owner.
- A newly created gallery is empty.

### Happy path

1. The caller submits a gallery creation request carrying a name and an optional visibility.
2. The system trims and validates the name against the business rules.
3. The system verifies the caller does not already own a gallery with the same name.
4. The system creates the gallery owned by the caller.
5. The system returns the created gallery.

### Alternative flow

- 2a. The name is empty or only whitespace, or longer than 100 characters; the system rejects the request.
- 3a. The caller already owns a gallery with the same name; the system rejects the request.

### Post condition(s)

- A gallery owned by the caller exists.
- The new gallery holds no item.

---

## UC-008 - Browse Galleries

### Description

Allow an authenticated user to browse the galleries and items they may see.

### Primary actor

- Standard User
- Admin
- Root Admin

### Pre condition(s)

- The caller is authenticated.

### Trigger(s)

- `GET /api/v1/library/gallery`
- `GET /api/v1/library/gallery/{id}`
- `GET /api/v1/library/gallery/{id}/item`
- `GET /api/v1/library/gallery/{id}/item/{item_id}`

### Bounded context(s)

- Library

### Business rules

- The listing shows every public gallery plus the caller's own galleries.
- A private gallery and its items may be read by their owner or by the Root Admin only.
- The listing may be filtered by name, visibility and owner, and paged.
- The built-in default gallery is public and visible to every authenticated user.
- Every gallery item references exactly one image or one video.

### Happy path

1. The caller lists galleries, optionally filtering and paging.
2. The system returns every gallery the caller may see, each with its item count.
3. The caller requests one gallery by its identifier.
4. The system returns the gallery with its items in order.
5. The caller requests one item of the gallery.
6. The system returns the item with its media metadata.

### Alternative flow

- 2a. The caller requests a private gallery it neither owns nor reads as the Root Admin; the system rejects the request.
- 3a. No gallery matches the requested identifier; the system rejects the request.
- 5a. No item matching the requested identifier belongs to the gallery; the system rejects the request.

### Post condition(s)

- The caller holds the requested galleries and items.

---

## UC-009 - Add an Image or Video to a Gallery

### Description

Allow an authenticated user to append one of their images or videos to a gallery they may contribute to.

### Primary actor

- Standard User
- Admin
- Root Admin

### Pre condition(s)

- The caller is authenticated.

### Trigger(s)

- `POST /api/v1/library/gallery/{id}/item`

### Bounded context(s)

- Library

### Business rules

- The caller may contribute to a public gallery; a private gallery accepts contributions from its owner or the Root
  Admin only.
- The image or video must belong to the caller, unless the caller is the Root Admin.
- The image or video must exist.
- An image or a video may belong to at most one gallery, and at most once.
- The item order is assigned by the system and appended after the last existing item.
- Adding an item updates the gallery's last-modified time.

### Happy path

1. The caller submits an add request carrying the media kind (image or video) and the backing file.
2. The system verifies the caller may contribute to the gallery.
3. The system resolves the image or video and its backing file.
4. The system verifies the caller owns the backing file, or is the Root Admin.
5. The system verifies the image or video is not already in a gallery.
6. The system appends the item at the next order position.
7. The system returns the created item.

### Alternative flow

- 1a. The media kind is neither image nor video; the system rejects the request.
- 2a. The gallery is private and the caller is neither its owner nor the Root Admin; the system rejects the request.
- 2b. No gallery matches the requested identifier; the system rejects the request.
- 3a. No image or video matches the requested media identifier; the system rejects the request.
- 4a. The image or video belongs to another user and the caller is not the Root Admin; the system rejects the request.
- 5a. The image or video already belongs to a gallery; the system rejects the request.

### Post condition(s)

- The gallery holds the new item at the next order position.
- The image or video belongs to this gallery.

---

## UC-010 - Remove an Image or Video from a Gallery

### Description

Allow an authenticated user to remove an image or video from a gallery, permanently deleting the item, its medium, and —
when the caller owns the file — the backing file.

### Primary actor

- Standard User
- Admin
- Root Admin

### Pre condition(s)

- The caller is authenticated.

### Trigger(s)

- `DELETE /api/v1/library/gallery/{id}/item/{item_id}`

### Bounded context(s)

- Library
- Asset

### Business rules

- The caller may remove an item of a public gallery.
- An item of a private gallery may be removed by the gallery owner or by the Root Admin only.
- The item must belong to the gallery named in the request.
- The item and its image or video record are deleted permanently.
- The backing file and its stored content are deleted when the caller owns the file or is the Root Admin, and kept
  otherwise. The file's record is removed as part of the item's removal; the stored content is removed on a best-effort
  basis.
- Removing an item updates the gallery's last-modified time.

### Happy path

1. The caller submits a removal request identifying the gallery and the item.
2. The system verifies the caller may remove items of the gallery.
3. The system locates the item within the gallery.
4. The system deletes the item and its image or video record, and the backing file when the caller owns it.
5. The system confirms the removal.

### Alternative flow

- 2a. The gallery is private and the caller is neither its owner nor the Root Admin; the system rejects the request.
- 2b. No gallery matches the requested identifier; the system rejects the request.
- 3a. No item matching the requested identifier belongs to the gallery; the system rejects the request.

### Post condition(s)

- The item and its image or video record no longer exist.
- The backing file's record and stored content no longer exist when the caller owned the file.

---

## UC-011 - Remove a Gallery

### Description

Allow the owner to delete a gallery together with its items, their media, and the backing files.

### Primary actor

- Standard User
- Admin

### Pre condition(s)

- The caller is authenticated.

### Trigger(s)

- `DELETE /api/v1/library/gallery/{id}`

### Bounded context(s)

- Library
- Asset

### Business rules

- Only the gallery owner may delete it; the Root Admin has no override.
- The built-in default gallery is never deletable.
- Deletion is permanent and removes every item, its image or video record, and its backing file before deleting the
  gallery.

### Happy path

1. The caller submits a gallery deletion request identifying the gallery.
2. The system verifies the gallery is not the default gallery.
3. The system verifies the caller owns the gallery.
4. The system deletes every item, its image or video record, and its backing file.
5. The system deletes the gallery.
6. The system confirms the deletion.

### Alternative flow

- 1a. No gallery matches the requested identifier; the system rejects the request.
- 2a. The gallery is the default gallery; the system rejects the request.
- 3a. The caller is not the owner; the system rejects the request.

### Post condition(s)

- The gallery, its items, their media, and their backing files no longer exist.

---

## UC-012 - Browse Music Albums

### Description

Allow an authenticated user to browse the music albums the system holds, read each album's mediums, and read the tracks
each medium sequences.

### Primary actor

- Standard User
- Admin
- Root Admin

### Pre condition(s)

- The caller is authenticated.

### Trigger(s)

- `GET /api/v1/library/music/album`
- `GET /api/v1/library/music/album/{album_id}`
- `GET /api/v1/library/music/album/{album_id}/medium/{medium_index}`
- `GET /api/v1/library/music/album/{album_id}/medium/{medium_index}/track`
- `GET /api/v1/library/music/album/{album_id}/medium/{medium_index}/track/{track_index}`

### Bounded context(s)

- Library

### Business rules

- Albums are not owned and carry no visibility flag: every authenticated caller sees the same albums, and read access is
  never denied.
- The album collection can be narrowed by an optional filter.
- An album's mediums are ordered by ascending medium index, and each medium's tracks by ascending track index, so
  repeated reads return a stable order.
- A medium belongs to exactly one album and is addressed by its album-scoped medium index.
- A track is the placement of a recording in a medium; it is addressed by its medium index and track index, and a track
  of another album or medium is reported as not found.
- A track shows its position within the medium that sequences it and the recording it places.
- A track's name is the name of its music recording.

### Happy path

1. The caller asks to browse the music albums the system holds.
2. The system verifies the caller's account still exists.
3. The system returns the matching albums with their identifier, name, release date, production type, genres, track
   count and mediums, each medium carrying its index, type and track count.
4. The caller asks to read one specific album.
5. The system verifies the album exists and returns it with the same details as in the listing.
6. The caller asks to browse one medium of one specific album.
7. The system verifies the album and medium exist and returns the medium.
8. The caller asks to browse the tracks of that medium.
9. The system verifies the album and medium exist and returns the medium's tracks in play order, each showing its
   position in the medium and the recording it places.
10. The caller requests one track of the medium by its position.
11. The system verifies the album, medium and track exist and returns the track.

### Alternative flow

- 1a. A request filter is invalid (an unknown, malformed or out-of-range filter); the system rejects the request.
- 4a. The album identifier is not valid; the system rejects the request.
- 4b. No album matches the requested identifier; the system rejects the request.
- 6a. The album identifier is not valid, or the medium index is not a valid index; the system rejects the request.
- 6b. No album matches the requested identifier, or the album has no medium at the requested index; the system rejects
  the request.
- 8a. The medium index or track index is not a valid index; the system rejects the request.
- 8b. No track of the medium matches the requested index; the system rejects the request.

### Post condition(s)

- The caller is shown the requested albums, album, medium, medium tracks or track.

---

## UC-013 - Create a Music Playlist

### Description

Allow an authenticated user to create a playlist they own to collect and order music recordings.

### Primary actor

- Standard User
- Admin

### Pre condition(s)

- The caller is authenticated.

### Trigger(s)

- Playlist creation request (`POST /api/v1/library/music/playlist`).

### Bounded context(s)

- Library

### Business rules

- The playlist name is required, is trimmed of surrounding whitespace, must not be empty or only whitespace, and must be
  at most 100 characters long.
- A caller's playlist names are unique: the same owner cannot create two playlists with the same normalized name.
  Different users may use the same name.
- A playlist is private by default; the caller may request it be public.
- The caller becomes the playlist owner.
- A newly created playlist is empty, and its last-modified time equals its creation time.

### Happy path

1. The caller submits a playlist creation request carrying a name and an optional visibility.
2. The system verifies the caller's account still exists.
3. The system trims and validates the name.
4. The system verifies the caller does not already own a playlist with the same normalized name.
5. The system creates the playlist owned by the caller, private unless requested otherwise.
6. The system returns the created playlist.

### Alternative flow

- 1a. The request is unauthenticated; the system rejects the request.
- 3a. The name is empty or only whitespace, or longer than 100 characters; the system rejects the request and creates
  nothing.
- 4a. The caller already owns a playlist with the same name; the system rejects the request and creates nothing.

### Post condition(s)

- A playlist owned by the caller exists and holds no track.

---

## UC-014 - Browse Music Playlists

### Description

Allow an authenticated user to list the playlists they may see, read one playlist, and read the tracks it sequences.

### Primary actor

- Standard User
- Admin
- Root Admin

### Pre condition(s)

- The caller is authenticated.

### Trigger(s)

- `GET /api/v1/library/music/playlist`
- `GET /api/v1/library/music/playlist/{playlist_id}`
- `GET /api/v1/library/music/playlist/{playlist_id}/track`
- `GET /api/v1/library/music/playlist/{playlist_id}/track/{track_index}`

### Bounded context(s)

- Library

### Business rules

- A public playlist is readable by every authenticated caller; a private playlist is readable only by its owner and the
  Root Admin. A playlist the caller cannot read is reported as not found, exactly like a missing one, without revealing
  which case applies.
- The listing can be narrowed by name, visibility and owner, and paged. An invalid filter or page request is rejected.
- A playlist's tracks are kept in a stable order.
- A playlist track shows its position in the order, the recording it places, its backing file and the recording's
  details.

### Happy path

1. The caller asks to list the music playlists it may see.
2. The system verifies the caller's account still exists, applies the requested filters, retains the playlists the
   caller can read and returns the requested page.
3. The caller asks to read one specific playlist.
4. The system verifies the playlist exists and the caller can read it, and returns its metadata and tracks in order.
5. The caller requests one track of the playlist by its position.
6. The system verifies the playlist and the position, and returns the track.

### Alternative flow

- 1a. The request is unauthenticated; the system rejects the request.
- 1b. A request filter or page is invalid; the system rejects the request.
- 3a. The playlist reference is not valid; the system rejects the request.
- 3b. No playlist matches the reference, or the caller cannot read it; the system reports it as not found and does not
  distinguish the two cases.
- 5a. The track position is not valid; the system rejects the request.
- 5b. The playlist has no track at the requested position; the system reports it as not found.

### Post condition(s)

- The caller is shown the requested playlists, playlist, or track.

---

## UC-015 - Update a Music Playlist

### Description

Allow the owner of a playlist to rename it or change its visibility.

### Primary actor

- Standard User
- Admin

### Pre condition(s)

- The caller is authenticated and owns the playlist.

### Trigger(s)

- `PATCH /api/v1/library/music/playlist/{playlist_id}`

### Bounded context(s)

- Library

### Business rules

- Renaming a playlist and changing its visibility are owner-only. A caller who can read a public playlist but is not its
  owner, including the Root Admin, is rejected and nothing is applied.
- A playlist the caller cannot read is reported as not found, exactly like a missing one.
- The caller may send either change, both, or neither. An omitted change leaves that setting unchanged; a request with
  neither change is accepted and changes only the last-modified time.
- A new name is trimmed, non-empty and at most 100 characters long, and must be unique among the playlists its owner
  has. A duplicate name is rejected; an invalid name is rejected.
- A successful update advances the last-modified time.
- A request that also adds a track is governed by UC-016 and is all-or-nothing: if this use case's owner-only rule
  rejects the request, the track is not added either.

### Happy path

1. The caller sends one or both changes for one of its playlists.
2. The system verifies the caller's account still exists and the playlist exists.
3. The system verifies the caller can read the playlist and owns it.
4. The system validates the new name and its per-owner uniqueness, when present.
5. The system stores the changes, advances the last-modified time and returns the updated playlist.

### Alternative flow

- 1a. The request is unauthenticated; the system rejects the request.
- 2a. The playlist reference is not valid; the system rejects the request.
- 2b. No playlist matches the reference, or the caller cannot read it; the system reports it as not found.
- 3a. The caller can read the playlist but does not own it; the system rejects the request and applies nothing.
- 4a. The name is empty, whitespace only or too long; the system rejects the request and applies nothing.
- 4b. The name is already used by another playlist the caller owns; the system rejects the request and applies nothing.

### Post condition(s)

- The playlist carries the requested changes, and its last-modified time has advanced.

---

## UC-016 - Add a Track to a Music Playlist

### Description

Allow a caller who can read a playlist to append one occurrence of a shared music recording to it.

### Primary actor

- Standard User
- Admin
- Root Admin

### Pre condition(s)

- The caller is authenticated and can read the playlist.

### Trigger(s)

- `PATCH /api/v1/library/music/playlist/{playlist_id}`

### Bounded context(s)

- Library

### Business rules

- Adding a track is allowed to any caller who can read the playlist. On a private playlist this means the owner and the
  Root Admin. A caller who cannot read the playlist is rejected and nothing is applied.
- The requested recording must exist; an unknown recording is rejected.
- A recording appears at most once in one playlist: a second add is rejected. Album mediums may still sequence the same
  recording twice.
- A new track is appended after the last track in the order.
- The request is all-or-nothing. If it also asks for an owner-only change from a non-owner, the whole request is
  rejected and neither change is applied (see UC-015).
- If another change to the playlist lands first, the addition is rejected and nothing is applied.
- A successful add advances the last-modified time and returns the updated playlist, including the new track.

### Happy path

1. The caller asks to add an existing recording that is not already in the playlist.
2. The system verifies the caller's account still exists and the playlist exists.
3. The system verifies the caller can read the playlist.
4. The system verifies the recording exists and is not already present.
5. The system appends the track after the last one, advances the last-modified time and returns the updated playlist.

### Alternative flow

- 1a. The request is unauthenticated; the system rejects the request.
- 2a. The playlist reference is not valid; the system rejects the request.
- 2b. No playlist matches the reference, or the caller cannot read it; the system reports it as not found.
- 3a. The request also asks for an owner-only change and the caller is not the owner; the system rejects the request and
  applies neither change.
- 4a. The recording does not exist; the system rejects the request and applies nothing.
- 4b. The recording already appears in the playlist; the system rejects the request and applies nothing.
- 5a. Another change to the playlist lands first; the system rejects the request and nothing is applied. The caller may
  try again.

### Post condition(s)

- The playlist sequences one more recording occurrence, and its last-modified time has advanced.

---

## UC-017 - Remove a Track from a Music Playlist

### Description

Allow a caller who can read a playlist to remove one of its track occurrences.

### Primary actor

- Standard User
- Admin
- Root Admin

### Pre condition(s)

- The caller is authenticated and can read the playlist.

### Trigger(s)

- `DELETE /api/v1/library/music/playlist/{playlist_id}/track/{track_index}`

### Bounded context(s)

- Library

### Business rules

- Removing a track is allowed to any caller who can read the playlist. A caller who cannot read the playlist is rejected
  and nothing is applied.
- The addressed track must exist; an unknown position is reported as not found.
- The remaining tracks keep their positions, leaving a gap. A later add appends after the last remaining track.
- The shared recording, its audio and its file survive the removal.
- A successful removal advances the last-modified time and confirms the deletion.

### Happy path

1. The caller asks to remove a track by its position in the order.
2. The system verifies the caller's account still exists and the playlist exists.
3. The system verifies the caller can read the playlist and the track position exists.
4. The system removes the track, advances the last-modified time and confirms the deletion.

### Alternative flow

- 1a. The request is unauthenticated; the system rejects the request.
- 1b. The playlist reference or track position is malformed; the system rejects the request.
- 2a. No playlist matches the reference, or the caller cannot read it; the system reports it as not found.
- 3a. The playlist has no track at the requested position; the system reports it as not found.

### Post condition(s)

- The addressed track no longer exists, the remaining positions are unchanged, and the shared recording survives.

---

## UC-018 - Reorder the Tracks of a Music Playlist

### Description

Allow a caller who can read a playlist to replace its track order with a requested permutation.

### Primary actor

- Standard User
- Admin
- Root Admin

### Pre condition(s)

- The caller is authenticated and can read the playlist.

### Trigger(s)

- `PUT /api/v1/library/music/playlist/{playlist_id}/track`

### Bounded context(s)

- Library

### Business rules

- Reordering is allowed to any caller who can read the playlist. A caller who cannot read the playlist is rejected and
  nothing is applied.
- The requested order must name every current track exactly once. A missing, repeated or unknown track is rejected and
  leaves the order unchanged.
- The tracks take the requested order, with no gaps.
- Reordering an empty playlist is accepted and changes only the last-modified time.
- A successful reorder advances the last-modified time and returns the reordered playlist.

### Happy path

1. The caller sends every current track exactly once in a new order.
2. The system verifies the caller's account still exists and the playlist exists.
3. The system verifies the caller can read the playlist and the order is complete.
4. The system applies the order, advances the last-modified time and returns the reordered playlist.

### Alternative flow

- 1a. The request is unauthenticated; the system rejects the request.
- 1b. The playlist reference is malformed or the request cannot be understood; the system rejects the request.
- 2a. No playlist matches the reference, or the caller cannot read it; the system reports it as not found.
- 3a. The order omits a current track, repeats a track or names an unknown track; the system rejects the request and
  leaves the order unchanged.

### Post condition(s)

- The playlist tracks appear in the requested order with no gaps.

---

## UC-019 - Delete a Music Playlist

### Description

Allow the owner of a playlist to delete it and its track placements.

### Primary actor

- Standard User
- Admin

### Pre condition(s)

- The caller is authenticated and owns the playlist.

### Trigger(s)

- `DELETE /api/v1/library/music/playlist/{playlist_id}`

### Bounded context(s)

- Library

### Business rules

- Deleting a playlist is owner-only. A caller who can read a public playlist but is not its owner, including the Root
  Admin, is rejected and deletes nothing.
- A playlist the caller cannot read is reported as not found, exactly like a missing one.
- Deleting a playlist removes its track placements with it.
- The shared recordings, their audio and their files survive.
- A successful deletion confirms the removal.

### Happy path

1. The caller deletes one of its playlists.
2. The system verifies the caller's account still exists and the playlist exists.
3. The system verifies the caller can read the playlist and owns it.
4. The system deletes the playlist and its track placements and confirms the removal.

### Alternative flow

- 1a. The request is unauthenticated; the system rejects the request.
- 1b. The playlist reference is malformed; the system rejects the request.
- 2a. No playlist matches the reference, or the caller cannot read it; the system reports it as not found.
- 3a. The caller can read the playlist but does not own it; the system rejects the request and deletes nothing.

### Post condition(s)

- The playlist and its track placements no longer exist; the recordings, audio and files it referenced still exist.
