# Proposal

## Why

The music write API needs an album identity that includes its group, an optional release date, no recording-level first
release date, and a medium type that can be `CD` or `DIGITAL`. It also needs a seeded bucket so a recording that has no
metadata yet still has an album and a medium to live in.

## What Changes

- Add `music_group.cover_image_id` referencing `image`.
- Add `music_album.music_group_id` referencing `music_group`, with `UNIQUE(name, music_group_id)`.
- Make `music_album.release_date` nullable.
- Remove `music_recording.first_release_date` from the schema, the domain, both repositories, both read representations
  and the ERD.
- Add `DIGITAL` to the medium type check and to `MediumIndex`, keeping `CD` for multi-disc editions.
- Seed `genre('UNKNOWN')`, `music_group(0,'Unknown')`, `music_album(0,'Unknown', group 0, date NULL)` and its `DIGITAL`
  medium, and link the album to the `UNKNOWN` genre.
- Protect group 0, album 0 and the bucket medium from deletion and modification with triggers.
- Edit the create and seed migrations in place (`PERS-014`); no `ALTER TABLE`.
- Out of scope: the group endpoints (change 8), the write endpoints (change 9), and default recordings at ingest (change
  10).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library/music-albums`: the album representation reports a nullable release date and the medium type set includes
  `DIGITAL`.

## Impact

- Migrations for `music_group`, `music_album`, `music_medium`, `music_recording`, and the new seeds and triggers.
- `MusicAlbum`, `MusicMedia`/`MediumIndex`, `MusicRecording` and their SQLx adapters.
- The album and track read representations.
- `docs/development/TechnicalDesign.md` and its entity-relationship diagram.
