# Proposal

## Why

The music identification use case is internal, only searches, and is not wired into the library factory or the server. A
client cannot search for a candidate or resolve one into metadata, so the preview and auto-apply modes have no source.
The recording metadata also still carries a first release date the catalogue no longer stores.

## What Changes

- Wire the MusicBrainz adapter into the library use-case factory and the composition root.
- Add a resolve use case that calls the provider's `lookup` through the reference's provider.
- Add `GET /api/v1/library/music/identify` for candidates and
  `GET /api/v1/library/music/identify/{provider}/{kind}/{reference}` for the resolved metadata document.
- Remove `first_release_date` from `MusicMetadata::Recording` and the identification specification.
- Rename the query and candidate evidence from `artist` to `group`, keeping `ArtistCredit` as provider vocabulary.
- Out of scope: applying the metadata to the catalogue (change 9) and group artwork collection (change 12).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library/music-identification`: removes the recording first release date and adds the HTTP read surface.

## Impact

- `src/lib/application/use_case/identify_music.rs`, a new resolve use case, `LibraryUseCaseFactory` and
  `src/lib/infrastructure/use_case_factory/library.rs`.
- `src/bin/server.rs` builds the provider from configuration.
- New handlers and `docs/openapi.json`.
- The MusicBrainz mapping and its tests.
