# Tasks

## 1. Album writes

- [x] 1.1 Add the album create use case with upsert by `(name, group_id)`, `UNKNOWN` genre default, optional date,
      production type and cover, and verify the create, existing, missing-group and empty-name cases.
- [x] 1.2 Add the album patch use case with absent-keeps and null-clears semantics, and verify the group-collision `409`
      and the partial update.
- [x] 1.3 Add the `POST` and `PATCH` handlers with `library.manage`, and verify the handler tests cover `201`, `200`,
      `409` and `422`.

## 2. Medium writes

- [x] 2.1 Add the medium put use case, idempotent by `(album, index)`, accepting `CD` and `DIGITAL`, and verify create,
      retype, unknown type and out-of-range index.
- [x] 2.2 Add the handler and verify it rejects the protected bucket medium and returns the medium representation.

## 3. Track writes

- [x] 3.1 Add the placement use case: resolve the destination, refuse an occupied slot, delete the previous medium
      placement and insert the new one in one transaction, and verify place, move, same-address, occupied and unknown
      recording.
- [x] 3.2 Add the partial unique index on `music_recording_id` for medium placements, and verify two medium placements
      for one recording are refused.
- [x] 3.3 Add the recording patch use case editing name and ISRC, and verify null clears, a duplicate ISRC is `409` and
      an invalid ISRC is `422`.
- [x] 3.4 Add both handlers and verify the track representation and its new `self` link after a move.

## 4. Apply

- [x] 4.1 Add the apply use case: resolve through the provider, upsert the album, replace the medium structure, and move
      only the supplied placements in one transaction, and verify the no-placement, placement, provider-failure and
      bucket cases.
- [x] 4.2 Add the handler with `library.manage` and verify the album representation and the `422` for the default album.

## 5. Documentation and verification

- [x] 5.1 Regenerate `docs/openapi.json` and verify every write route, request and response is present.
- [x] 5.2 Verify a manual album, medium and track flow returns the read representation unchanged, and that an apply
      without placements leaves the bucket tracks in place.
- [x] 5.3 Run the full gate (`cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`,
      `cargo fmt --all -- --check`, `script/rule_enforcer.sh check`) and verify it passes.
