# Tasks

## 1. Schema

- [x] 1.1 Add `music_group.cover_image_id` referencing `image` and verify the group insert accepts a valid image and
      rejects a missing one.
- [x] 1.2 Add `music_album.music_group_id` referencing `music_group` and `UNIQUE(name, music_group_id)`, and verify a
      duplicate `(name, group)` is rejected while the same name for another group is accepted.
- [x] 1.3 Make `music_album.release_date` nullable and verify an album with no date round-trips as `null`.
- [x] 1.4 Remove `music_recording.first_release_date` from the create migration and verify the column no longer exists.

## 2. Domain and adapters

- [x] 2.1 Add `cover_image_id` and `music_group_id` to the album model and `cover_image_id` to the group model, and
      verify the repository round-trips both.
- [x] 2.2 Make the album release date optional in the domain and remove `first_release_date` from `MusicRecording`, both
      repositories and both read representations, and verify the focused tests pass.
- [x] 2.3 Extend the medium type with `DIGITAL` and `MediumIndex` with a digital variant, and verify a stored `DIGITAL`
      medium is read and an unknown type is a data-integrity failure.

## 3. Seeds and invariants

- [x] 3.1 Seed `genre('UNKNOWN')`, `music_group(0,'Unknown')`, `music_album(0,'Unknown', group 0, date NULL)`, its
      `DIGITAL` medium and the album-genre link, and verify the bucket is readable with one empty medium.
- [x] 3.2 Add the protection triggers for group 0, album 0 and the bucket medium, and verify a delete or rename of each
      is refused.
- [x] 3.3 Update the entity-relationship diagram and `PERS` notes, and verify `PERS-011` is satisfied.

## 4. Specifications and documentation

- [x] 4.1 Update the album representation text to the nullable release date and the `CD`/`DIGITAL` medium set, and
      verify `openspec validate extend-music-catalogue-schema` passes.
- [x] 4.2 Run the Markdown gates over the changed documents and verify they pass.

## 5. Integration verification

- [x] 5.1 Verify the bucket resolves with group 0, the `UNKNOWN` genre and one `DIGITAL` medium, and that a multi-CD and
      a single-CD release map to the documented medium shapes.
- [x] 5.2 Run the full gate (`cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`,
      `cargo fmt --all -- --check`, `script/rule_enforcer.sh check`) and verify it passes.
