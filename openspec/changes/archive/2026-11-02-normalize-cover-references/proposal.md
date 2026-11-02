# Proposal

## Why

A cover currently references a file, so five trigger pairs exist only to prove the file is an image. At the same time
the `image` table is shared by gallery items and cover art, so nothing stops cover art from entering a gallery or a
gallery image from becoming a film's artwork. Referencing the image row makes the image check structural and lets the
schema keep cover art and gallery media disjoint.

## What Changes

- Add `image.scope` and `video.scope` (`LIBRARY` or `USER`), set by the first attachment and immutable afterwards.
- Change `cover_file_id` to `cover_image_id`, referencing `image`, on `music_album`, `music_playlist`, `movie`, `series`
  and `season`.
- Delete the five `require_image_cover_*` migration pairs; the foreign key to `image` replaces them.
- Enforce disjointness: a `LIBRARY` image is only cover art, a `USER` image is only gallery media or a playlist cover,
  and a scope mismatch is a conflict. A `USER` image has exactly one referrer.
- Update the cover representation and input field to `cover_image_id`, and point `_links.cover` at the Asset image route
  (`TODO(image-link)` until change 5 serves it).
- Edit the create migrations in place (`PERS-014`); no `ALTER TABLE`.
- Out of scope: the Asset endpoints, the music group schema, and the bytes/content endpoint.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library/cover-art`: a cover references an image, carries a scope, and is disjoint from gallery media.
- `library/music-albums`: the album representation reports an image identifier.
- `library/movies`: the film representation reports an image identifier.
- `library/series`: the show and season representations report an image identifier.
- `library/music-playlists`: the playlist representation reports an image identifier.

## Impact

- Migrations for `image`, `video`, `music_album`, `music_playlist`, `movie`, `series`, `season`, `gallery_item` and the
  deleted cover triggers.
- `Image`, `Video`, `MusicAlbum`, `MusicPlaylist`, `Movie`, `Series` and `Season` models, their SQLx adapters and their
  HAL representations.
- `docs/development/TechnicalDesign.md`'s entity-relationship diagram and trigger notes.
