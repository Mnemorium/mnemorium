# Tasks

Schemathesis already proves a requirement's declarative contract against the served document: every declared status,
media type, response header, envelope shape and `_links`/`_embedded` field belongs to the API fuzz test
(`script/api_fuzz.sh`). Do not add a system case that restates it — a system case proves only the effect the fuzzer
cannot observe, and the task names the fuzz operation that covers the rest.

Do not run the E2E suite (`script/e2e.sh` or `pytest test/e2e`), the configuration suite (`script/configuration.sh`) or
`devenv test` in this change, with no exceptions.

The specification is the artifact this change delivers; the tasks below prove it. No production code changes: the two
tests that do not exist yet characterize behavior the code already enforces, and the rest add Allure labels. A label set
is `epic=Metadata enrichment`, `feature=<REQ>`, `story=<SCN>`; a test that proves two scenarios carries two `story`
labels. A test that proves no scenario is left unlabelled and named in its group's completion note.

A scenario is proven at the lowest level that observes it: the container proves what a provider-less server answers, and
Rust proves everything that needs a provider answer, because the provider base URLs are fixed at composition.

## 1. The provider-less system module

- [x] 1.1 Create `test/system/metadata_enrichment.py` with a module docstring naming the capability and a case per
      identification route asserting the unavailable document: both name reads, both film and show reference reads, the
      series read with `season` and `episode`, and both music routes. Label `feature=REQ-METADATA-ENRICHMENT-001` and
      `-017`/`-023` with the matching `story`. Verify: `pytest test/system/metadata_enrichment.py` collects and passes
      against the current image.
- [x] 1.2 Add the validation cases: an empty `name`, a partial `season`/`episode` position, an out-of-range position,
      and a `kind` outside the music kinds. Label `story=SCN-METADATA-ENRICHMENT-017-D`, `-017-I`, `-017-J` and
      `-018-D`. Verify: they pass, and the failure body is the standard error envelope.
- [x] 1.3 Add the refusal cases observable without a provider: a provider the route does not serve (film route with a
      music provider, music route with a video provider), an unknown entry, and an apply that finds no provider. Label
      `story` `-017-G`, `-023-C` and `-024-E`, asserting that the apply leaves the entry unchanged. Verify: they pass.
- [x] 1.4 Delete the cases in `test/e2e/library/uc025_identify_video_metadata.py` that 1.1–1.3 now prove, keeping one
      use-case-level case per kind that asserts the document's `available` flag and `_links.self`. Verify: the module is
      still present and no assertion it keeps duplicates a system case.
- [x] 1.5 Delete the cases in `test/e2e/library/uc030_apply_external_video_metadata.py` that 1.3 now proves, keeping one
      use-case-level refusal case. Verify: the module is still present, and `script/rule_enforcer.sh check TEST-039`
      passes for `UC-025` and `UC-030`.

## 2. Video identification labels

- [x] 2.1 Label the two tests in `domain/service/media_name_interpreter.rs` with the scenarios of
      `REQ-METADATA-ENRICHMENT-009` to `-011` they prove. Verify: `cargo test media_name_interpreter` passes and the new
      labels name existing scenario identifiers.
- [x] 2.2 Label the ten tests in `application/use_case/identify_video.rs` with `-001`, `-009`, `-012` and `-017`.
      Verify: `cargo test identify_video` passes.
- [x] 2.3 Label the eight tests in `application/use_case/resolve_video_metadata.rs` with `-003`, `-008` and `-017`.
      Verify: `cargo test resolve_video_metadata` passes.
- [x] 2.4 Label the twenty tests in `outbound/reqwest/tmdb.rs` and the seventeen in `outbound/reqwest/omdb.rs` with
      `-002`, `-003`, `-005`, `-006`, `-007`, `-014` and `-016`. Verify: `cargo test --lib reqwest` passes and every
      fixture read by a labelled test names the scenario it proves.
- [x] 2.5 Label the twelve tests in `outbound/metadata/failover.rs` with the scenarios of `-013`. Verify:
      `cargo test failover` passes.
- [x] 2.6 Label the nine tests in `domain/service/release_order.rs` with the scenarios of `-020`. Verify:
      `cargo test release_order` passes.

## 3. Music identification labels

- [x] 3.1 Label the six tests in `application/use_case/identify_music.rs` with `-001`, `-002`, `-018` and `-023`.
      Verify: `cargo test identify_music` passes.
- [x] 3.2 Label the five tests in `application/use_case/resolve_music_metadata.rs` with `-004`, `-008` and `-023`.
      Verify: `cargo test resolve_music_metadata` passes.
- [x] 3.3 Label the thirty tests in `outbound/reqwest/musicbrainz.rs` with `-004`, `-007`, `-019`, `-020`, `-021`,
      `-022` and `-023`. Verify: `cargo test musicbrainz` passes, and the retry and pacing tests carry `-022`.

## 4. Apply labels and the two missing tests

- [x] 4.1 Label the nine tests in `application/use_case/apply_movie.rs` with `-024` and `-025`. Verify:
      `cargo test apply_movie` passes.
- [x] 4.2 Label the seven tests in `application/use_case/apply_series.rs` with `-024` and `-026`. Verify:
      `cargo test apply_series` passes.
- [x] 4.3 Label the seven tests in `application/use_case/apply_series_episode.rs` with `-024` and `-027`. Verify:
      `cargo test apply_series_episode` passes.
- [x] 4.4 Label the seven tests in `application/use_case/apply_music_album.rs` with `-024` and `-028`. Verify:
      `cargo test apply_music_album` passes.
- [x] 4.5 Add the test for `SCN-METADATA-ENRICHMENT-028-G`: a placement naming a medium the apply does not create is
      rejected and nothing is stored. Verify: it passes against the current use case and fails when the placement check
      is removed.
- [x] 4.6 Add the tests for the album apply's denials: a caller without `EditMetadata` is refused, and a caller the
      catalogue conceals receives the not-found outcome. Label `feature=REQ-METADATA-ENRICHMENT-024` and, because the
      decision is `authorization`'s, add `story=SCN-AUTHORIZATION-027-A` and `-B` alongside it. Verify: they pass, and
      they fail when the authorization step is removed.

## 5. Provider configuration labels and documentation

- [x] 5.1 Label the provider tests in `application/use_case/load_configuration.rs` — defaults, a two-provider list,
      index order, an unknown name, a duplicate name, a missing key, a music provider, a wrong-kind entry, a duplicate
      music provider and a zero music index — with `-015` and `-016`. Verify: `cargo test load_configuration` passes.
- [x] 5.2 Confirm the two glossary rows added by this change (`Metadata enrichment`, `Provider reference`) render in
      `docs/development/Glossary.md` beside the terms they relate to. Verify: `markdownlint-cli2 '**/*.md'` and
      `prettier --check` pass.
- [x] 5.3 Record in `design.md`, under Risks, any scenario the labelling found that no test proves and no code enforces.
      Verify: the design's Risks section names it, and no requirement is left without a proof.

## 6. Integration checks

- [x] 6.1 Run `openspec validate specify-metadata-enrichment --strict` and confirm it passes before archiving. Verify:
      the command reports the change valid.
- [ ] 6.2 Run `cargo test` and `script/rule_enforcer.sh check TEST-039`; confirm the only source changes are inside
      `#[cfg(test)]` modules and the E2E modules. Verify: both pass, and `git diff --stat -- src migrations` lists only
      test modules.

## Workflow follow-up

- Archive the change with `openspec archive`, which creates `openspec/specs/metadata-enrichment/spec.md`, deletes the
  two emptied identification specs, and applies the three catalogue deltas.
- Verify the archived tree: the new capability exists with 28 requirements, `library/video-identification` and
  `library/music-identification` are gone, and the three catalogue specs no longer carry an apply requirement.
