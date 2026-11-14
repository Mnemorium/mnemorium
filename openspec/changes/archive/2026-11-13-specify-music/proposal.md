# Proposal

## Why

Music is implemented end to end — albums, mediums, recordings, groups and playlists — but no single capability describes
it, and the three that do still speak the permission model `specify-authorization` deleted. They cite `library.read`,
`library.manage`, `playlist.read` and `playlist.moderate`, and the `PUBLIC_OR_OWNED`, `LISTED`, `OWNED` and `NONE`
scopes that no longer exist. That change removed its `library/*` deltas and deferred the repointing to a follow-up; this
is that follow-up for music.

Capturing the contract surfaced four shipped behaviours the specifications get wrong or omit. A playlist whose
**Visibility** is `Public` is specified as moderatable by any **Caller**, while the server refuses a non-owner with
`403` and the E2E suite already asserts it. Appending a recording checks only the playlist's `CreateChild`, not the
`ViewMetadata` that `authorization` requires of a playlist entry. Creating a playlist performs no `CreateChild` check at
all, although `authorization` seeds the grant that admits it. And a music group stores artwork that `library/cover-art`
never lists, while a playlist has a cover column, trigger, representation field and scenario but no request that accepts
the value.

The artwork contract also falls short as a shared area: it enumerates the resources that carry art, which is how the
group escaped it. A playlist has a cover column, trigger, representation field and scenario but no request that accepts
the value.

## What Changes

- Add a `music` capability owning the album, medium, recording, group and playlist contract as one requirement sequence
  with `REQ-MUSIC-*`/`SCN-MUSIC-*` identifiers.
- Add a `cover-art` capability owning the shared cover contract: the stored cover, the image it references, its scope,
  its link, and the resources that own no artwork.
- Retire `library/music-albums`, `library/music-groups`, `library/music-playlists` and `library/cover-art`.
- Replace each who-may restatement with the **action** the route requires, leaving the rule to `authorization`.
- Correct the playlist moderation contract to the shipped rule: only a caller holding the playlist's write action may
  add, remove or reorder its tracks.
- Check both actions when a recording is appended to a playlist, and check `CreateChild` when a playlist is created, so
  the music surface obeys the `authorization` contract it declares.
- Give a playlist the cover field its representation already reports, and let a group clear its cover.
- Keep one cover field. Album, group, film, show, season and playlist report the `cover_image_id` they store, and
  `_links.cover` follows it. An episode and a recording own no artwork: they report no cover, and a client reads the
  album's or season's cover it already fetched.
- Add `test/system/music.py` and `test/system/cover_art.py` as the black-box proof of both capabilities.

**BREAKING**: none. No route, payload field or status changes, and every existing `cover_image_id` keeps its meaning.

## Capabilities

### New Capabilities

- `music`: browsing albums, mediums and recordings; curating albums and groups; owning playlists and their ordered
  tracks, with the action each operation requires.
- `cover-art`: the optional artwork a library resource may carry, the image it references, its scope, its link, and the
  resources that own no artwork.

### Modified Capabilities

- `library/music-albums`: retired. Its read and write requirements move to `music`; its apply requirement is owned by
  `metadata-enrichment`.
- `library/music-groups`: retired. Its five requirements move to `music`.
- `library/music-playlists`: retired. Its seventeen requirements move to `music`, with moderation corrected.
- `library/cover-art`: retired. Its four requirements move to `cover-art`, with inheritance replacing the ban.

## Impact

- **Specs**: two capabilities created, four retired.
- **API**: playlist create and patch accept `cover_image_id`; a group patch distinguishes an absent cover from `null`;
  every art representation keeps `cover_image_id` and `_links.cover`, and an episode or a track exposes neither.
- **Code**: the music handlers' served doc comments; `create_playlist` and `update_playlist` gain the actions
  `authorization` requires; `update_music_group` and its command; playlist create, patch and their commands; the removal
  of cover resolution from the album-track, playlist-track, season and episode reads.
- **Tests**: `test/system/music.py` and `test/system/cover_art.py` become the proof; the E2E modules they replace are
  deleted in the same change.
- **Docs**: `Overview.md` follows the module changes; `Glossary.md` gains **Cover art**; `UseCases.md` is unchanged.
- **Sequencing**: authored against the tree after `specify-metadata-enrichment`, which owns identify and apply and edits
  one of the retired specs.
