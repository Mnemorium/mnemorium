# Design

## Context

See `proposal.md` -- Why for motivation. The constraints that shape this design:

- `file`, `path` and `mime_type` belong to the **Asset** context; `movie`, `series`, `season`, `music_album` and
  `music_playlist` belong to **Library** (`ARCH-004`, `PERS-020`).
- `mime_type.type` already classifies every file as `AUDIO`, `VIDEO` or `IMAGE` (`chk_mime_type_type`), so "is this an
  image" is a stored fact, not a probe.
- The `image` table is gallery media: `file_id` is unique (`uq_image_file_id`) and a row is referenced by at most one
  `gallery_item` (`PERS-019`). It carries probe-derived `width_px`, `height_px` and `orientation`.
- No route serves file bytes. Six payloads already carry `file_id` without a link, and the HAL policy currently
  qualifies the file link with "once a file endpoint exists".
- The schema is unreleased, so migrations are mutable (`PERS-014`).
- No endpoint creates a film, show, season or album, and playlist create/update accept no artwork.

## Goals / Non-Goals

**Goals:**

- One column per owner, one datastore invariant, one link rule -- five resources gain artwork without five designs.
- A resource without artwork is byte-for-byte unchanged in behaviour, and its payload differs only by a `null` field.
- The cover reaches the payload through the row the read already loads: no extra query, port or unit-of-work accessor.

**Non-Goals:**

- Serving image bytes; that is the Asset fetch routes (`/api/v1/asset/{type}/{file_id}`).
- Deleting files, collecting orphaned covers, or changing the file-delete path.
- Any write path for artwork.
- Retro-fitting file links onto the six `file_id` fields the API already carries.

## Decisions

### D1. A nullable `cover_file_id` column on each owner, not a reference to `image`

The alternative carries the probed dimensions and the `Orientation` enum for free, but costs more than it returns here:
`uq_image_file_id` allows one `image` row per file, so a gallery that wants the same photograph as an album's cover
could not have its own row; and every cover would need an `image` row created before the reference exists. A `cover`
table cloned from `image` was rejected as `image` under another name. Three columns per owner (`cover_file_id`,
`cover_width_px`, `cover_height_px`) was rejected as twelve columns and four copies of a probe result.

**Accepted cost:** the payload reports no dimensions, so a client cannot reserve space before the bytes arrive. If
dimensions are wanted later, `cover_*` sibling fields are additive; a nested `cover` object would be a breaking rename
(see D4).

### D2. The reference crosses into Asset and declares no cascade

`cover_file_id` is a foreign key into `file`, which is Asset-owned. `ARCH-004` and `PERS-020` forbid `ON DELETE CASCADE`
across that boundary, so the column declares none and deletion stays orchestrated by the application.

This is safe because the existing delete path already anticipates an unknown cross-context reference:

```rust
// src/lib/application/use_case/delete_gallery.rs
// [...]
match unit_of_work.files().delete(file.id()).await {
    Ok(true) => removed_paths.push(PathBuf::from(file.path())),
    // [...] the datastore rejected the delete because a reference this guard
    // cannot see survived: the file is kept, bytes included.
    Ok(false) | Err(RepositoryError::DataIntegrityViolation) => {}
    Err(error) => {
        return Err(DeleteGalleryError::Unknown(error.into()));
    }
}
```

The guard probes `image` and `video` references before deleting a file; a surviving cover reference simply blocks the
delete and the file is kept with its bytes. No change is required to that path, and cover files are not garbage
collected -- the same outcome `audio.file_id` already produces.

### D3. The image-only invariant is enforced by triggers, not by the foreign key

SQLite cannot express a subquery in a `CHECK` constraint, so "the referenced file's `mime_type.type` is `IMAGE`" cannot
be a column constraint. Two options were considered: validate in the write use case, or enforce in the datastore.

The write use case does not exist for four of the five owners, and artwork is populated out of band until an ingestion
path lands, so application-level validation would protect nothing today. The datastore is the only place the invariant
can hold for every writer. One migration pair per table, matching the `upper_*` precedent (one pair per table holding
that table's insert and update triggers):

```sql
CREATE TRIGGER tg_movie_cover_is_image_insert
BEFORE INSERT ON movie
FOR EACH ROW
WHEN new.cover_file_id IS NOT NULL
 AND (SELECT type FROM mime_type
      WHERE mime_type_id = (SELECT mime_type_id FROM file WHERE file_id = new.cover_file_id))
     != 'IMAGE'
BEGIN
    SELECT RAISE(ABORT, 'cover_file_id must reference an image file');
END;
```

`PERS-017` already maps `SQLITE_CONSTRAINT_TRIGGER` (`1811`) to `RepositoryError::Conflict`, so the rejection needs no
new error plumbing. `NULL` covers never fire the `WHEN` clause, and a dangling identifier falls through to the foreign
key rather than the trigger. The `UPDATE` counterpart exists now rather than later because the first playlist `PATCH`
that touches a cover would otherwise silently skip validation.

Cost is two primary-key lookups per row write. There is no bulk-insert path today; a future scanner that inserts
thousands of rows should measure it.

### D4. The payload field is a flat `cover_file_id`, not a nested `cover` object

`VideoResponse.file_id`, `GalleryItemResponse.file_id` and `UploadSessionResponse.file_id` all carry a bare identifier,
so a flat field is the established shape. A nested object would read better if artwork metadata ever arrives, but it
would also need a `_links` block of its own and duplicates the file identifier inside it. The growth path for a flat
field is `cover_width_px` / `cover_height_px` siblings, which is additive.

An absent cover serializes as `null`, matching `GalleryResponse.owner_id`, rather than being omitted.

### D5. `_links.cover` is published before its route is served

The tempting alternative -- expose `cover_file_id` and let the client build `/api/v1/asset/image/{id}` -- violates
`API-037`, which requires that clients never construct or hardcode URLs. Deferring the link until the route exists was
rejected because it forces a second, breaking change later: every client would have to switch from building the URI to
following a link.

So the link is published now and the route follows. The href is built by a single helper so the href shape and its TODO
live in exactly one place:

```rust
/// TODO(add-cover-art): the Asset image route is not served yet; revisit this
/// helper when `/api/v1/asset/image/{file_id}` lands.
fn asset_image_href(file_id: NumericID) -> String {
    format!("/api/v1/asset/image/{file_id}")
}
```

The route is keyed on `file_id`, not an image identifier, because a cover has no `image` row (D1); `image` is the
`mime_type` discriminator, not a record. A client following the link today receives the standard HAL `404` envelope,
because `hal_errors` is the outermost layer and re-wraps the routing fallback.

The link is `Option<Link>` with `skip_serializing_if = "Option::is_none"`, matching `MovieListLinks.next`.

### D6. The server never inherits a cover

The alternative -- report a season's cover as the season's own value, falling back to its show -- was rejected because
it moves display policy into the server and makes two equal representations disagree depending on which one was fetched.
The season representation already carries `series_id` and `_links.series`, so a client that wants the fallback has
everything it needs. The client decides; the server reports facts.

### D7. Covers appear in list and detail reads

Detail-only was rejected because cover art is a grid concern: a twenty-tile library screen would need one request per
tile. The three heavy lists already carry more than a file identifier (movies carry a full video summary, albums their
mediums, playlists their timestamps), so the cover is not a new weight class.

`library/series` is the one capability with a rule in the way: its listing is documented as "only its identifier and
name ... no seasons, genres, dates or counts". The spec delta amends that deliberately rather than excepting itself,
because a wall of shows is exactly the screen artwork is for.

### D8. A season's cover rides the existing aggregate read

`Season` carries no identifier into the API -- it is addressed by `(series_id, season_index)` -- and the domain model
has no `season_id`. The cover therefore lives on `season.season_id` but resolves through the aggregate read that already
loads seasons and episodes: one join on `season.cover_file_id`, and the series representation's embedded season
summaries get covers from the same rows. No new port, repository method or column on `Season` is exposed.

### D9. Migrations are edited in place, with separate trigger migrations

`PERS-014` allows the five `create_*` migrations to gain the column directly; no `ALTER TABLE` and no new schema
migration. Triggers stay in their own pairs because that is how every existing trigger is versioned, and `PERS-011`
requires the ER diagram to move in the same change: `cover_file_id: INTEGER <<FK>>` on each of the five entities plus
the `file` relationships, with the FK marker on the child (`PERS-013`).

### D10. The HAL policy sentence loses its qualifier

`docs/development/TechnicalDesign.md` currently reads "the produced file is carried as `file_id` and linked by name once
a file endpoint exists". That clause lets a reader conclude the link may wait, which is exactly what this change does
not do. It is trimmed, and the positive policy is stated in its own paragraph: a representation carries a referenced
file's `file_id` **and** links it to `/api/v1/asset/{type}/{file_id}`, published whether or not the route is served,
because a client that builds the URI itself violates `API-037`.

## Risks / Trade-offs

- **`_links.cover` points at a route that does not exist** -> it fails as the standard HAL envelope rather than a
  framework error; one helper carries the TODO; the field and link shape do not change when the route lands.
- **No dimensions in the payload** -> a grid cannot reserve space and will shift as images load. Accepted for now;
  adding them is additive as `cover_*` siblings.
- **`MovieResponse` carries a linked cover beside an unlinked `video.file_id`** -> visible inconsistency created by the
  new policy. Recorded in the proposal as out of scope; the retro-fit is its own change.
- **Covers are never garbage-collected** -> a file whose only remaining reference is a cover survives its gallery's
  deletion. Matches existing `audio.file_id` behaviour; the delete path tolerates it.
- **Nothing writes a cover, so every column stays `NULL` in practice** -> populated reads are covered by adapter,
  application and handler tests that seed or stub the cover; the container E2E suite stays on the uncovered and empty
  paths because it cannot create a film, show, season or album.
- **The trigger fires per row** -> two primary-key lookups per write. Acceptable while no bulk path exists; revisit if a
  scanner inserts in bulk.
- **`image` semantics are untouched** -> covers create no `image` rows, so `PERS-018` and `PERS-019` keep their current
  meaning and the gallery read is unaffected.

## Migration Plan

1. Edit the five `create_*` migrations to add the column and its foreign key.
2. Add one trigger migration pair per table (insert and update).
3. Update the ER diagram and the partial-index prose in the same change (`PERS-011`).
4. Reset the development database: delete `mnemorium.db` with its `-wal` and `-shm` sidecars. Editing an applied
   migration changes its checksum, so `sqlx::migrate!` fails at startup with `VersionMismatch` until the file is rebuilt
   -- this is the documented procedure, not a failure mode.

Rollback is the same reset: the schema is unreleased, so there is no deployed data to migrate back.

## Open Questions

- When the Asset fetch routes land, does the retro-fit of the six existing `file_id` links ship with them or as its own
  change? It belongs to whichever change introduces the routes, and answering it now would not alter this change's
  specs, approach or tasks.
