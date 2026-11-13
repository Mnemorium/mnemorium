# Proposal

## Why

**Metadata enrichment** is implemented end to end for both media kinds — a media name or a query becomes
provider-independent candidates, a **provider reference** resolves into provider-independent metadata, and an apply
writes that metadata into the **catalogue** — yet its contract is split across five specifications along two different
seams. `library/video-identification` and `library/music-identification` state nine rules twice and encode opposite
retry and failover policies; the apply contract is one requirement each in `library/movies`, `library/series` and
`library/music-albums`. None of the five carries a `REQ-*` identifier, so a test cannot name the rule it proves and a
reviewer cannot tell intended behavior from accident.

The audit found five behaviors the code enforces that no specification states: an album apply stores the `UNKNOWN` genre
when the provider reports none; a placement that names a medium the apply does not create is rejected; an album apply
resolves denials through the catalogue's conceal rule; music identification has no failover even though its
configuration is an ordered list; and every apply downloads the provider's artwork and then discards it. The
configuration capability also deferred the music provider-list requirement to the identification capability by name.

## What Changes

- Add a top-level `metadata-enrichment` capability that owns identification and application for music and video, with
  stable `REQ-*` and `SCN-*` identifiers.
- State the nine rules the two identification specs duplicate once: a provider-independent candidate, the provider's
  order, a provider-independent metadata document, artwork as content, absence over fabrication, a curated genre
  vocabulary, resolution through the reference's provider, unavailable distinct from no match, and the **action** each
  route names.
- Keep the kind-scoped rules alongside them: name interpretation, episode position, query ordering, first-hit search,
  index-order failover, ISO country of origin, the music kinds, production type, compact-disc ordering, Lucene escaping,
  request pacing and bounded retry.
- State the asymmetry as deliberate: video fails over in ascending index order and asks each provider at most once;
  music paces one provider, retries it within a bounded budget, and never fails over.
- Name the two provider lists, `library.metadata.video.providers` and `library.metadata.music.providers`, and the fields
  each entry carries, because `specify-configuration` deferred that requirement here and a reader of this capability
  needs the names.
- State the five unstated behaviors above, so the specification blesses them on purpose rather than by omission.
- Retire `library/video-identification` and `library/music-identification`, absorbing their requirements.
- Remove the four apply requirements from `library/movies`, `library/series` and `library/music-albums`, leaving those
  capabilities as the catalogue contract they uniquely own.
- Prove the capability: the provider-backed scenarios stay in Rust with the existing golden fixtures and gain the three
  Allure labels; a new `test/system/metadata_enrichment.py` proves what a provider-less container observes;
  `uc025_identify_video_metadata.py` and `uc030_apply_external_video_metadata.py` keep only their use-case flow, so no
  scenario is proven twice.
- Change no runtime behavior. No route, payload, status, schema, provider, setting or Rust module moves.

Deliberately out of scope:

- Persisting provider artwork on apply, and the `lookup` call that downloads artwork an apply discards.
- A live-provider canary that detects upstream payload drift; it needs credentials and a separate cadence, and no
  specification promises it.
- Business use cases for music enrichment; adding them would bind an E2E file to each under `TEST-039`.
- Moving the `/api/v1/library/*` routes, renaming the `library` handler or Rust modules, or touching the `logging`
  capability's events.

## Capabilities

### New Capabilities

- `metadata-enrichment`: turning a media name or a query into provider-independent candidates, resolving a reference
  into provider-independent metadata through the provider that issued it, and applying that metadata to the catalogue,
  for both music and video.

### Modified Capabilities

- `library/video-identification`: retired. Its 22 requirements are absorbed by `metadata-enrichment`, apart from the
  event requirement that `specify-logging` already removes.
- `library/music-identification`: retired. Its 17 requirements are absorbed by `metadata-enrichment`, apart from the
  event requirement that `specify-logging` already removes.
- `library/movies`: the `External metadata is applied to a film` requirement moves to `metadata-enrichment`.
- `library/series`: the `External metadata is applied to a show` and `External metadata is applied to an episode`
  requirements move to `metadata-enrichment`.
- `library/music-albums`: the `External metadata is applied to an album` requirement moves to `metadata-enrichment`.

## Impact

- **Specs**: one capability created, two retired, three reduced. `metadata-enrichment` becomes the single normative
  owner of the external-metadata contract; the catalogue specs keep their read and write contract without the provider
  pipeline.
- **Code**: none. The use cases (`identify_video`, `identify_music`, `resolve_video_metadata`, `resolve_music_metadata`,
  `apply_movie`, `apply_series`, `apply_series_episode`, `apply_music_album`), the adapters (`tmdb`, `omdb`,
  `musicbrainz`), the failover chain, the domain services and the configuration constructors are unchanged.
- **Tests**: about 149 Rust tests across 14 modules gain `epic`/`feature`/`story` labels and no assertion changes; one
  system module is added; the two E2E modules are reduced to their use-case flow. No fixture is added.
- **Documentation**: two glossary terms are added; `docs/development/UseCases.md` is unchanged, with the music coverage
  gap recorded in `design.md`.
- **Sequencing**: the change is authored against the tree after `specify-logging`, whose deltas remove one event
  requirement from each retired capability. Removing an already-absent requirement is a no-op, so the deltas also apply
  before it, at the cost of a warning.
