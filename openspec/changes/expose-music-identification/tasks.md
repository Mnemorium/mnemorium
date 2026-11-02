# Tasks

## 1. Vocabulary and metadata

- [x] 1.1 Rename `MusicQuery.artist` and `MusicCandidate.artist` to `group`, keep `ArtistCredit`, and verify the query,
      candidate and provider mapping tests pass.
- [x] 1.2 Remove `first_release_date` from `MusicMetadata::Recording`, the MusicBrainz mapping and the specification,
      and verify `rg "first_release_date" src/lib/domain/model/music_metadata.rs` returns nothing.

## 2. Resolve use case

- [x] 2.1 Add `ResolveMusicMetadata` that takes a `MusicTarget`, calls the provider's `lookup` and returns `Ok(None)`
      for a missing item, and verify the missing, present and provider-failure cases.
- [x] 2.2 Add the corresponding port and `LibraryUseCaseFactory` method, and verify the factory mock can build it.

## 3. Composition and HTTP

- [x] 3.1 Build the MusicBrainz adapter from configuration in `src/bin/server.rs` and pass it to the library factory,
      and verify the server starts with and without a provider.
- [x] 3.2 Add `GET /library/music/identify` and `GET /library/music/identify/{provider}/{kind}/{reference}`, and verify
      the handler tests cover candidates, a resolved document, an unknown provider and an unavailable provider.
- [x] 3.3 Guard both routes with `library.read` and verify an unauthenticated request returns `401`.

## 4. Documentation

- [x] 4.1 Regenerate `docs/openapi.json` and verify the two routes and their responses are present.
- [x] 4.2 Run the Markdown and OpenAPI checks and verify they pass.

## 5. Integration verification

- [x] 5.1 Verify a search followed by a resolve through the returned reference yields metadata whose album, release or
      recording shape matches the identifier.
- [x] 5.2 Run the full gate (`cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`,
      `cargo fmt --all -- --check`, `script/rule_enforcer.sh check`) and verify it passes.
