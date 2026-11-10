# Design

## Context

See `proposal.md` for the motivation. The implementations that constrain this design:

- The User context already declares `UserRepository::delete`, the Identity context declares
  `CredentialRepository::delete` and `PermissionRepository::delete_all`, and the Library and Asset contexts declare
  deletes for gallery, gallery item, playlist, upload, file, image, video and audio. `UserRepository::delete` is marked
  `dead_code` because no caller exercises it yet.
- `gallery_item` cascades from `gallery`; `music_track` cascades from `music_playlist`; `upload_chunk` cascades from
  `upload`. Nothing cascades from `user`: `file.user_id`, `gallery.user_id`, `music_playlist.user_id`, `upload.user_id`
  and `permission.user_id` are non-cascading foreign keys, so the use case orchestrates every removal (`ARCH-004`,
  `PERS-020`).
- `file.user_id` and `upload.user_id` are `NOT NULL`, so a retained file cannot be left pointing at a deleted account.
- `file.integrity_hash` is nullable with `uq_file_user_id_integrity_hash`. `SQLite` treats `NULL` as distinct, so the
  key does not apply to a null digest. Uploaded files carry a digest; scanned files carry none and are owned by the Root
  Admin.
- A media row's `scope` records its first attachment: a catalogue cover marks its image `LIBRARY`, a gallery item or
  playlist cover marks its image or video `USER`, and the scope is immutable afterwards
  (`20261102090000_enforce_asset_scope`).
- The authorization catalog has no `user.delete`; `user.update` carries the peer-admin relation guard in
  `Authorization::allows`.
- The user routes are registered by `handler/user.rs`, wired through `UserUseCaseFactory`; `FileSystemStorage` is a
  stateless adapter rebuilt per accessor, as the Library factory does for the gallery deletes.
- `delete_gallery` leaves bytes on disk behind a `TODO(ingest)` because scanned bytes are not the server's to remove.
  That rationale does not apply to a deletable user: every file of a non-root user came from the upload flow.

## Goals / Non-Goals

**Goals:**

- One atomic unit of work removes the account, its credential, its permission overrides and its owned data.
- A deterministic private-versus-shared disposition for every media row backed by the user's files.
- A new authorization action and target guard that match the existing permission model.
- An E2E file that observes the account, the private media, the shared media and the deleted token.

**Non-Goals:**

- Self-service account deletion; an Admin cannot delete its own account because the target is an Admin.
- Deduplicating transferred media by re-pointing referrers to an existing Root-owned media row. The digest is cleared
  instead, and content deduplication of transferred files is left for a future change.
- Deleting catalogue rows (films, episodes, albums, recordings, covers) that reference a user's media.
- A schema or migration change. This change is data movement only.
- Changing the scanned-file byte policy or the existing gallery-delete behavior.

## Decisions

### 1. Deletion is its own permission action

Add `Action::UserDelete` with wire name `user.delete`, `resource_kinds()` `[User]`, `role_scope()` `ALL` for Admin and
`NONE` for Standard, Root Admin `All`, and `is_configurable() == true`. The eleven exhaustive matches in
`domain/model/permission.rs` gain the variant, and `ACTIONS` grows from 19 to 20 entries.

Rationale: `Permissions.md` promises one action per authorizable operation. Reusing `UserUpdate` would tie deletion to
any override that grants or denies profile edits, and would make the documented matrix wrong. Because the permission
administration API iterates `ACTIONS`, the new action appears in the defaults and becomes grantable with no extra
plumbing.

Alternative considered: a role check (`caller.role() == Admin`) in the use case. Rejected — it would be the first role
check in a system that decides every operation by an action and an effective scope, and it would make a Standard User
impossible to grant deletion by override.

### 2. The target guard lives in `Authorization::allows`

Extend the existing relation guard:

```text
if action == UserUpdate  && target is privileged && target != caller && caller is not root -> deny
if action == UserDelete  && target is privileged &&                           caller is not root -> deny
```

The second clause has no `target != caller` exception: an Admin deleting itself is still deleting an Admin, so it is
refused. The Root Admin short-circuits before the guard and may delete an Admin. The `user_id = 0` target is refused by
the same guard for every non-root caller; the Root Admin's attempt reaches the database trigger
`tg_user_delete_root_admin`, which aborts, and the use case maps that conflict to `TargetNotModifiable` (`403`).

Rationale: the guard is a policy over the action and the target, exactly where `user.update`'s guard already lives. It
keeps every caller, including an override holder, subject to the same rule.

Alternative considered: checking the target's role in the use case. Rejected — it would duplicate a policy the
authorization service already owns and would drift from the update path.

### 3. One unit of work across all four context views

`DeleteUser<F, S>` opens one `begin_write()` unit of work and bounds `F::Uow` by `AssetUnitOfWork + IdentityUnitOfWork

- LibraryUnitOfWork +
  UserUnitOfWork` (`STY-RUST-039`, `STY-RUST-040`). Repositories are requested sequentially (`STY-RUST-041`).

The use case holds `Arc<S: FileStorage>` for the post-commit byte removal, mirroring `DeleteGallery`.

### 4. Deletion order

```text
 1. load caller, target; refuse target 0
 2. decide user.delete; apply the target guard
 3. permissions.delete_all(target)
 4. for each gallery owned by target: galleries.delete(id)          (items CASCADE)
 5. for each playlist owned by target: music_playlists.delete(id)   (tracks CASCADE)
 6. for each upload owned by target:  uploads.delete(id)            (chunks CASCADE, stage queued)
 7. for each file owned by target: classify and dispose
 8. users.delete(target)
 9. credentials.delete(target.credential_id)
10. commit
11. best-effort byte removal for the queued paths
```

Steps 4 and 5 come before step 7 deliberately: once the user's own galleries and playlists are gone, every remaining
referrer of a media row is necessarily someone else's, so the classifier never has to distinguish public from private
ownership.

Step 8 precedes step 9 because `user.credential_id` references the credential; the reverse order violates the foreign
key. The root trigger aborts step 8 for `user_id = 0`, which the use case maps to `TargetNotModifiable`.

### 5. Media disposition

For each file owned by the target, load the media row backed by it (image, video or audio; at most one per kind) and
classify:

```text
no media row                         -> private
image.scope == LIBRARY               -> shared (catalogue artwork)
image/video.scope == USER:
    gallery_item exists in a gallery not owned by the target   -> shared
    playlist cover exists on a playlist not owned by the target -> shared
    otherwise                                                  -> private
video.scope == NULL:
    movies.find_by_video(id) is Some
      or series.find_episode_by_video(id) is Some              -> shared
    otherwise                                                  -> private
audio:
    music_albums.find_album_by_audio(id) is Some               -> shared
    otherwise                                                  -> private
```

A `LIBRARY` image is never also a gallery item or playlist cover: the scope triggers abort the second attachment, and
the scope is immutable. A `USER` image or video has at most one referrer, so the two probes are sufficient.

Private disposition:

```text
image -> clear_cover_references (defensive) -> image.delete
video -> delete_streams -> video.delete
audio -> audio.delete (removes its streams)
then  -> file.delete, and queue media_path(file.path()) for byte removal
```

Shared disposition:

```text
file.reassign_owner(id, 0); queue nothing; keep media row and bytes
```

The transfer is an explicit repository method rather than a general `save`: `save` currently updates path, visibility
and digest but not the owner, and widening it would let the ingest path rewrite an owner by accident. Two narrow methods
keep the mutation explicit:

```rust
fn reassign_owner(&mut self, id: NumericID, new_owner: NumericID)
    -> impl Future<Output = Result<bool, RepositoryError>> + Send;
fn clear_integrity_hash(&mut self, id: NumericID)
    -> impl Future<Output = Result<bool, RepositoryError>> + Send;
```

Both report `Ok(false)` for a missing row (`STY-RUST-046`).

### 6. The digest collision

Before `reassign_owner(id, 0)`, search `FileFilter { user_id: Some(0), integrity_hash: Some(digest) }`. When a file is
found, call `clear_integrity_hash(id)` first, then transfer. The second call cannot violate the unique key because the
row now carries `NULL`, which the key ignores.

Rationale: the collision needs two different users to upload byte-identical content and both to be deleted. Refusing
would make deletion fail for a reason unrelated to the target; re-pointing the media row and every referrer to Root's
existing file would need media `file_id` mutations the repositories do not expose. Clearing the digest preserves the
content and only costs that one file its ETag (`get_asset_content` omits the ETag when the digest is absent) and its
participation in per-caller deduplication.

Scanned files carry no digest and are owned by the Root Admin, so they can never collide.

### 7. Byte removal after the commit

Collect absolute paths with `upload_layout::media_path(file.path())` for deleted files and `staged_path(upload_id)` for
deleted uploads. After the commit, call `FileStorage::delete_upload_file` for each, logging a warning on failure and
never failing the request. A crash between commit and cleanup leaves an orphan file, which is preferable to deleting
bytes and then rolling back the rows.

Only `Origin::Uploaded` files queue byte removal. A scanned file is the operator's content, and the existing
`delete_gallery` policy keeps those bytes; a non-root scanned row is treated as scanned and kept.

### 8. API and error mapping

`DELETE /api/v1/user/{user_id}`, declared with `#[utoipa::path(delete, ...)]` and registered next to `get_user` and
`patch_user` in `handler/user.rs`. It reuses `parse_user_id` and returns `StatusCode::NO_CONTENT`.

```text
NoSuchCaller        -> 401 Unauthorized
NotAdmin            -> 403 Forbidden       (user.delete denied)
TargetNotModifiable -> 403 Forbidden       (root target or Admin target from a non-root caller)
NoSuchUser          -> 404 Not Found
Unknown             -> 500 Internal Server Error
```

The refusal order is: unknown caller, unknown target, root target, `user.delete` decision, relation guard. Checking the
target before the permission keeps `404` for an unknown identifier instead of leaking a decision.

### 9. Wiring and composition

`UserUseCaseFactory` gains `delete_user()`. `RuntimeUserUseCaseFactory` builds
`DeleteUser::new(unit_of_work_factory, Arc::new(FileSystemStorage::new()))` per accessor, as the Library factory does
for the gallery deletes. No composition-root dependency changes are needed.

### 10. Tests

- Use-case tests: happy path for a Standard and an Admin target; Standard caller refused; Admin-to-Admin refused;
  self-deletion refused; root target refused; unknown caller and unknown target; private image/video/audio/orphan file
  removed; shared image in another user's gallery transferred; video backing a film or episode transferred; audio
  backing an album transferred; digest collision clears the digest; commit failure rolls back; byte-removal failure does
  not fail the request; staged upload cleanup.
- Handler tests: `204`, `401`, `403` (both causes), `404`, and the OpenAPI route.
- E2E `test/e2e/user/uc005_delete_a_user_account.py`: an Admin deletes a Standard user; the deleted user's token and
  login are refused; a private gallery, its items and their asset are gone; a shared image in another user's gallery
  survives and the Root Admin can still read its file; the Root Admin's own account and an Admin peer are refused.
- `rule_enforcer.yaml`: remove `UC-005` from `TEST-039`'s `exempt_use_cases` and delete the `TODO` comment.

## Risks / Trade-offs

- **A shared file changes owner.** After the transfer, a gallery owner who is a Standard User can no longer delete the
  file through their gallery (`file.use` resolves to `OWNED`), so the item is removed but the bytes are kept. This
  matches the existing gallery-delete rule for another caller's file.
- **Clearing the digest loses the ETag** for that one transferred file, and a later identical upload by Root is not
  deduplicated against it. Rare and content-preserving.
- **Post-commit cleanup can orphan bytes** on a crash. Logged as a warning; no reconciliation job is introduced.
- **Classification is a query per file.** A user with many uploads holds a write transaction for longer. Acceptable for
  a home server; the alternatives (bulk SQL) would move policy into the adapter.
- **A user's media can back a catalogue row** even though catalogue rows are not user-owned. The transfer keeps the
  catalogue intact. Deleting the catalogue row was rejected as a surprising cross-context destructive cascade.
- **`delete_gallery` still leaves bytes**; this change does not alter it. The two policies differ on purpose: a
  deletable user's files are uploaded, while a gallery may reference scanned files.

## Migration Plan

No schema change and no new migration. Deploy is a normal release containing the route, the action and the adapter
methods. Rolling back the code is safe for the schema, but the deletion itself is irreversible: deleted accounts and
private media are not restored by a rollback, and transferred files remain Root-owned. Run the E2E only against an
ephemeral container (`script/e2e.sh`), never a production instance.
