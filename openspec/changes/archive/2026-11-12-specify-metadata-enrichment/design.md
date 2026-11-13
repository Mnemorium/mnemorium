# Design

## Context

See `proposal.md` — Why. The constraints that shape the approach:

- The pipeline is implemented in three layers: the adapters
  (`infrastructure/outbound/reqwest/{tmdb,omdb,musicbrainz}.rs` plus `infrastructure/outbound/metadata/failover.rs`),
  the domain vocabulary
  (`domain/model/{external_ref, video_candidate,music_candidate,media_metadata,music_metadata,artwork,genre}.rs` and
  `domain/service/{media_name_ interpreter,release_order,country_code}.rs`), and the application use cases
  (`identify_video`, `identify_music`, `resolve_video_metadata`, `resolve_music_metadata`, `apply_movie`,
  `apply_series`, `apply_series_episode`, `apply_music_album`).
- The routes are served from `handler/library/get_{movie,series}_identify*`, `get_music_identify*` and
  `post_{movie,series,series_episode,album}_apply`. They name `ViewMetadata` on `ResourceKind::Catalogue` for
  identification and `ViewMetadata` then `EditMetadata` on the addressed entry for an apply. Denial semantics belong to
  `authorization` (`REQ-AUTHORIZATION-038`), not to this capability.
- An unavailable identification answers `200` with an `available: false` document, not an error. The handlers reject a
  provider of the wrong kind with `404` before the use case runs.
- An apply that finds no provider, or whose provider fails, renders `500` and declares it in its `utoipa` response set,
  so the statuses are part of the API contract rather than this capability's prose.
- The provider base URLs are fixed at composition (`DEFAULT_BASE_URL` in each adapter; `configured_links` passes no
  override), so the container cannot reach a provider. Provider-backed behavior is proven in Rust with `wiremock` and
  the golden payloads under `outbound/reqwest/fixtures/` (31 files plus `malformed.json`).
- `openspec archive` deletes a capability only when the delta empties it and the change's `.openspec.yaml` sets
  `retire_capabilities: true`. A `REMOVED` naming a requirement the target no longer has is a warned no-op, not an
  error.
- `TEST-039` requires one `test/e2e/uc*.py` file per catalogued use case; `UC-025` and `UC-030` are catalogued and their
  files must keep existing. The Allure label convention (`epic`/`feature`/`story`) is implemented for the system suite
  and for Rust tests; no E2E module carries it.
- The in-flight `specify-logging` change removes one event requirement from each retired capability, and
  `specify-configuration` deferred the music provider-list requirement to the identification capability by name.

## Goals / Non-Goals

**Goals:**

- One normative owner for the external-metadata contract, with a requirement sequence a test can name.
- The nine duplicated rules stated once, and the kind-scoped rules stated beside them.
- Every behavior the code enforces stated, so nothing is blessed by omission.
- The retirement expressed through OpenSpec's own mechanism, with no hand-deleted spec.
- A traceable proof for every scenario, at the lowest level that can observe it.

**Non-Goals:**

- Changing Rust, a route, a payload, a status, a schema, a setting or a configuration layer.
- Persisting provider artwork, or avoiding the artwork download an apply discards.
- A live-provider drift canary.
- Business use cases for music enrichment, or a change to `TEST-039`.
- Restating who-may rules owned by `authorization`, or events owned by `logging`.

## Decisions

### 1. One top-level capability, and the URL prefix does not move

A functional area groups externally observable behavior, not a route tree (`specify-upload`, Decision 1). Enrichment's
subject is an external pipeline: a query or a media name enters, a provider is asked, and a catalogue row is written.
That is not the catalogue's own contract, which is why the apply requirements leave `library/movies`, `library/series`
and `library/music-albums` rather than the identification specs moving under them. The routes stay at
`/api/v1/library/*`; a client is unaffected.

**Alternative considered:** `library/metadata-enrichment`. Rejected: it leaves the area map saying the provider pipeline
belongs to the content-browsing area, and the capability then has to share a directory with the catalogue capabilities
whose requirements it removes.

### 2. Twenty-eight requirements, shared rules once, kind-scoped rules beside them

Each new requirement owns one behavior. The mapping from the retired text:

| New requirement                                                | Absorbs                                                                                                                                                                                 |
| -------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 001 Unavailable is not "no match"                              | both identification capabilities' requirement of the same name                                                                                                                          |
| 002 A candidate is provider-independent and ordered            | `music` candidate + order, `video` candidate                                                                                                                                            |
| 003 Video metadata is provider-independent                     | the metadata half of `video`'s metadata requirement                                                                                                                                     |
| 004 Music metadata is provider-independent                     | `music` album, release and recording requirements                                                                                                                                       |
| 005 Artwork is content, and an apply never stores it           | `music` artwork, the artwork half of `video`'s metadata requirement                                                                                                                     |
| 006 An unreported value is absent                              | both absence requirements                                                                                                                                                               |
| 007 Genres come from a curated vocabulary                      | both genre requirements                                                                                                                                                                 |
| 008 A reference resolves through its provider                  | both routed-resolution requirements                                                                                                                                                     |
| 009–014, 016–017, 019–022                                      | the video-only rules: name interpretation, episode position, query order, first hit, failover, ISO country, key handling, routes, production type, compact-disc order, escaping, pacing |
| 015 Provider configuration is an ordered list                  | `video`'s list, its startup refusal, and the music list `specify-configuration` deferred here                                                                                           |
| 018 A music query names its kind and carries a title           | `music`'s query requirement                                                                                                                                                             |
| 023 Music identification is served over HTTP                   | `music`'s route requirement                                                                                                                                                             |
| 024 An apply resolves the reference and writes authoritatively | new shared statement of the four apply requirements' write semantics                                                                                                                    |
| 025–028 Film, show, episode and album applies                  | the four catalogue apply requirements, plus the unstated rules below                                                                                                                    |

The two retired capabilities hold 39 requirements between them; the merged capability states them in 28, nine of which
are statements that were duplicated.

`REQ-METADATA-ENRICHMENT-015` names the two provider lists and the fields each entry carries. That does not reopen
`specify-configuration`'s "no setting is specified" rule: that change deferred the provider-list requirement to the
identification capability by name, and it still owns how any setting is resolved from the defaults, the file and the
environment. Every other setting stays unnamed here, and the music list's entry carries no `api_key` because its only
admitted provider takes none.

**Alternative considered:** split by kind (`music`, `video`) or by stage (`identification`, `application`). Both were
rejected: the first keeps the duplication the change exists to remove, and the second forces the nine shared rules to be
cross-referenced between files and gives `application` a name the glossary already uses for a different concept.

### 3. The who-may rule is named, not restated

Identification names the `ViewMetadata` action on the catalogue; an apply names `EditMetadata` on the addressed entry.
The requirement does not state which caller is denied or what a denial renders, because `authorization` owns that
(`REQ-AUTHORIZATION-038`, and the glossary's **who-may rule**). `library/video-identification`'s
`Identification reads require library read access` is therefore retired rather than absorbed: its scenarios duplicate
`test/system/authorization.py`.

**Alternative considered:** carry the 401/404 scenarios over. Rejected: they would reassert a rule a lower-level
capability owns, and a change to the concealment rule would then have to be made twice.

### 4. The asymmetry is stated, not unified

Video fails over across providers in ascending index order and asks each provider at most once. Music paces one provider
at one request per second, retries a rate limit within a bounded budget, and never fails over, because the music section
admits one provider and `configured_music_provider` builds a single adapter rather than a chain.
`REQ-METADATA-ENRICHMENT-013` and `-022` state the two policies in their own requirements, so a reader cannot infer one
from the other.

**Alternative considered:** generalize a single retry/failover policy and implement it for both. Rejected: it changes
behavior, and the services' terms are genuinely different — failover trades one provider's answer for another's, while
pacing trades latency for respecting a documented limit.

### 5. Five unstated behaviors are blessed; one is deferred

The specification states, as deliberate:

- an album apply stores `UNKNOWN` when the provider reports no genre, matching the album-creation rule that already
  states it;
- an album apply rejects a placement naming a medium it does not create;
- an apply resolves denials through the catalogue's conceal rule, without restating it;
- music has no failover, and its ordered list validates but orders nothing today;
- an apply never stores provider artwork and leaves the stored cover alone.

Deferred: `lookup` downloads artwork whether or not the caller will store it, so every apply pays for an image it
discards. The contract does not promise otherwise, so the waste is a follow-up, not a specification defect.

**Alternative considered:** stop the download now. Rejected: it needs a lookup variant or a port flag, which is a
behavior and interface change this contract-only change does not carry.

The other half of the loop stays where it is. A caller edits metadata and sets a cover through the catalogue's partial
updates (`library/movies`, `library/series`, `library/music-albums`, each accepting `cover_image_id` as an absent-keeps,
null-clears, value-sets field) and through `library/cover-art`, which owns the rule that a stored cover references an
image row. Provider artwork becomes a stored cover by the caller uploading the content identification returned and
setting `cover_image_id`; that chain composes three capabilities, and this one neither restates it nor sets the cover. A
reader looking for "where do I type this in by hand" is sent to the catalogue, not here.

### 6. Retire through the archive mechanism, with the event requirements listed defensively

Both retired capabilities get `REMOVED`-only deltas that name every requirement the main spec holds today, including the
two event requirements. `specify-logging` removes those two; if it has archived first, the two blocks are warned no-ops
and the remaining blocks still empty the spec. `retire_capabilities: true` authorises the deletion.
`openspec validate --strict` passes on the delta set, and no `spec.md` is deleted by hand.

**Alternative considered:** declare a hard dependency and omit the event blocks. Rejected: it makes the change
unarchivable until another lands, for no benefit, when the no-op path is supported.

### 7. Provider-backed proofs stay in Rust; the provider-less surface moves to a system module

The split follows what the container can observe:

- Rust owns everything that needs a provider answer: candidate shape and order, metadata mapping, absence and
  placeholder handling, genre and country mapping, artwork as content, failover order and routing, retry and pacing,
  redaction, and the apply writes. The existing `wiremock` tests and golden payloads already prove them; the change adds
  the three Allure labels (`epic=Metadata enrichment`, `feature=<REQ>`, `story=<SCN>`) and no assertions.
- `test/system/metadata_enrichment.py` proves what a provider-less container observes: the unavailable document on all
  six identification routes, the empty-name and partial/out-of-range-position rejections, the unknown provider, and the
  apply refusals that happen before a provider is consulted.
- `test/configuration/library_settings.py` keeps proving the provider settings' effects — an empty list, two configured
  providers starting without a request, and the key never reaching a log sink — and `load_configuration.rs` keeps
  proving the refusals.

`uc025_identify_video_metadata.py` and `uc030_apply_external_video_metadata.py` keep a use-case-level case each: the
document a kind-scoped read returns and the refusal an apply without a provider returns. Their scenario-level cases that
the system module takes over are deleted in the same change, so no outcome is proven twice and `TEST-039` keeps a file
per catalogued use case.

**Alternative considered:** delete the E2E modules once the system module exists. Rejected: `TEST-039` has no exemption
for a use case proven elsewhere, and adding one is a rule change outside this capability.

### 8. Statuses and paths stay with the API contract

The specification states outcomes, not codes: "reports the entry as not found", "rejects the request", "reports the
failure". The exact status of each outcome is declared by the handler's `utoipa` response set and proven by the
Schemathesis fuzz run (`docs/development/TechnicalDesign.md` § 5), so the capability spec does not restate them. This
also keeps the apply routes' declared `500` for "no provider configured, or the provider failed" out of the prose, where
it would read as a blessed design choice.

It states operations, not routes: a candidate search, a metadata resolution, an apply to a film. The path a request
takes and the status it returns are the served OpenAPI document's subject, and a reader who needs either follows the API
index. A rule states a parameter only where the rule depends on it — a non-empty `name`, a `season` and `episode`
supplied together — never as a URL.

**Alternative considered:** state `404`, `422`, `500` and the paths inline as the retired specs do. Rejected: it
duplicates the served specification and makes the spec wrong the day a handler changes a code or a route.

### 9. Documentation stops at the glossary

Two terms enter `docs/development/Glossary.md`: **metadata enrichment** and **provider reference**. `UseCases.md` is
untouched. Its video entries (`UC-025`, `UC-030`) already cover the pipeline at business level, and there are no music
entries; adding them would bind an E2E file to each under `TEST-039` and force a use case whose provider-backed half the
container cannot exercise. The gap is recorded here rather than closed.

**Alternative considered:** add `UC-0NN Identify Music Metadata` and `UC-0NN Apply External Music Metadata`. Deferred:
it is a documentation change with its own test obligation, and it would not change a requirement.

## Risks / Trade-offs

- **The `specify-logging` collision** → the two event requirements are listed defensively: present, they empty the
  retired specs; already removed, they are warned no-ops. No spec is edited by hand and no ordering is required.
- **About 149 Rust tests gain labels and no assertions** → a mislabel points a scenario at the wrong test. Mitigated by
  one task per module, mapping each test by name to the scenario it proves, and by `openspec validate --strict` plus
  review. A test that proves no scenario is left unlabelled and named in the task's completion note.
- **A scenario may not match the code** → the re-authoring is a contract read, not a rewrite. Where a divergence
  appears, the task records it in this design and either the spec or a follow-up changes; the code does not move here.
- **Requirement 028 states four music-only behaviors in one requirement** → it exceeds the one-behavior preference but
  fits the 500-character bound. Splitting it would add three requirements that no other kind shares; the scenarios carry
  the detail.
- **The system module cannot prove "no candidate"** → that outcome needs a configured provider answer, so it stays in
  Rust. The module proves only outcomes a provider-less container can produce, and the design records why.
- **Retiring two capabilities orphans their history** → `git log` keeps it, and the `REMOVED` migration column maps
  every old requirement to its replacement, so a reader can trace either direction.
- **The labelling surfaced tests with no published scenario** → the adapters' failure-classification cases
  (`tmdb_search_classifies_*`, `omdb_search_classifies_*`, `musicbrainz_classifies_each_status`), the
  credential-transport cases (`tmdb_sends_a_v4_token_as_a_bearer_credential`,
  `musicbrainz_sends_the_user_agent_and_the_format`,
  `musicbrainz_artwork_policy_accepts_the_archive_and_rejects_the_api`), and the unreferenced `ReleaseOrder` cases
  (`order_editions_keeps_an_empty_list_empty`, `compare_editions_*`, `order_candidates_moves_only_release_candidates`)
  prove a mechanism rather than a scenario, so they stay unlabelled and are named here. The comparators are dead code:
  `order_candidates` reorders releases in a candidate list, which contradicts `SCN-METADATA-ENRICHMENT-002-B` if ever
  wired, so deleting or specifying them is a follow-up.
- **Some scenarios are code-enforced without a direct assertion** → `SCN-METADATA-ENRICHMENT-004-D` (a medium's position
  and track count), `-005-C` and `-026-C` (the apply keeps the stored cover), `-007-D` (a music genre is never reported
  for video), `-016-C` (an artwork failure carries no key or key-bearing address), `-018-B`/`-C` (a release or recording
  query), `-027-B` (an episode with no air date) and `-028-H` (the album's cover and group are untouched) have no test
  that asserts the specific outcome. Each requirement still has a proof through another scenario; the missing assertions
  are a test follow-up, not a specification change.
- **The authorization citations moved** → this change was authored against `REQ-AUTHORIZATION-027`; `specify-logging`
  retired and re-added it as `REQ-AUTHORIZATION-038`, so the citations here and the new album-apply denial labels use
  `-038`, which is the identifier the main spec holds.

## Migration Plan

1. Apply the spec deltas with `openspec archive`, which creates `metadata-enrichment/spec.md` from the ADDED set,
   deletes the two emptied capability specs, and prunes any directory the deletion leaves empty.
2. No deployment step: no route, payload, status, setting, schema or migration changes, and no restart behavior.

**Rollback:** revert the change's commit. The archive is a file operation, so the retired specs return intact from git
and the catalogue deltas are undone with them.
