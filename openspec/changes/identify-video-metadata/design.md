# Design

## Context

See `proposal.md` for motivation and `specs/library/video-identification/spec.md` for the requirements. This document
records the technical choices that the requirements leave open.

Current constraints that shape the design:

- Ingest records `file`, `video`, `image` and `audio` rows; `MovieRepository` and `SeriesRepository` are read-only and
  nothing in production creates a film or a show row. This change builds the identification step only; persistence is a
  later change.
- The library root is mounted by the operator and scanned at startup. Uploaded files are promoted to
  `{upload_id}_{client_file_name}` at the library root, while scanned files keep their root-relative path, so a path can
  be rich (folders, release tags) or thin (`7_Inception.mkv`).
- Outbound domain ports use return-position `impl Future` and are held as concrete types or generics; ports held as
  trait objects use boxed futures. `ExternalServiceError` already covers the external-service family, and
  `reqwest`/`wiremock` are already declared in `Cargo.toml` § 6 but unused.
- Configuration is resolved before the datastore and is immutable for the process. Every setting has a built-in default,
  the generated file contains defaults only, and `default_config.yaml` is checked in and compared against the serialized
  defaults by an existing test.
- `OBS-003` forbids logging raw content, `OBS-006` requires catalogued events, and the configuration template currently
  states that instance secrets are not configuration.

## Goals / Non-Goals

**Goals:**

- A parser whose output is ordered, testable and never invents evidence.
- One provider-independent response shape that both an HTTP provider and a future non-HTTP provider can satisfy.
- A failover chain that is a drop-in implementation of the same port, selected and ordered by configuration.
- A key-handling story that is explicit about where the key lives and how it can never leak.
- A test strategy that proves the adapter mapping, the chain policy and the parser separately.

**Non-Goals:**

- Triggers, persistence, matching/ranking, artwork storage, user-entered metadata, HTTP endpoints and music
  identification (see `proposal.md`).
- Field-level merging of metadata from several providers.
- Retrying, back-off, circuit breakers or a per-provider request budget.

## Decisions

### D1 — Layer placement and naming

| Concern                                           | Location                                                                                           |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Parser (pure)                                     | `domain/service/media_name_interpreter.rs`                                                         |
| Query, candidate, metadata, artwork, genre values | `domain/model/media_query.rs`, `media_candidate.rs`, `media_metadata.rs`, `artwork.rs`, `genre.rs` |
| Port                                              | `domain/port/video_metadata_provider.rs`                                                           |
| HTTP adapters                                     | `infrastructure/outbound/reqwest/{tmdb,omdb}.rs`                                                   |
| Failover chain                                    | `infrastructure/outbound/metadata/failover.rs`                                                     |
| Use case                                          | `application/{port,use_case}/identify_video.rs`                                                    |
| Config section                                    | `domain/model/configuration.rs` and `partial_configuration.rs`                                     |

Names are video-scoped (`VideoMetadataProvider`, `IdentifyVideo`) so a future music provider is a sibling rather than a
third variant on the film contract. The chain lives under `outbound/metadata/` because it composes port implementations
and has no transport of its own.

### D2 — The parser is a pure domain service with ordered hypotheses

`MediaNameInterpreter::interpret(name: &str) -> Vec<MediaQuery>`:

- No `Result`: any string is a name, and "nothing usable" is the empty list.
- Best-first order; each distinct query once.
- Multiple hypotheses for numeric-title collisions (`Blade Runner 2049`, `1917`) rather than one guess.
- For an episode, the show title comes from the folder when available; the episode title is never the search title.
- Never fabricate a season: absolute numbering (`Show - 12`) yields a show-level query with no episode position.
- Samples, trailers and extras yield nothing.
- Input is the best human-meaningful name the caller has: the root-relative path for a scanned file, the original client
  file name for an upload. The interpreter knows nothing about the `{id}_` storage prefix.

_Alternatives:_ returning a single answer (wrong on some real libraries and loses the evidence needed to try
alternatives); living in `application/` (the vocabulary is business, not transport); parsing `File` instead of a string
(couples the service to storage and blocks the upload case).

The service rule requires two callers. This change supplies one (`IdentifyVideo`); the catalogue-enrichment use case
will be the second. Record the planned caller in the change notes so review can judge it as prospective rather than
invented.

### D3 — Canonical values

```text
VideoKind        = Movie | Series
EpisodePosition  = { season: u32, episode: u32 }
MediaQuery       = { kind, title, year?, episode? }          parser output = search input
ProviderId       = Tmdb | Omd
ExternalRef      = { provider: ProviderId, id: String }      opaque, provider-tagged
Candidate        = { reference, kind, title, year? }
Target           = Movie(ref) | Series(ref) | Episode { ref, season, episode }
Artwork          = { bytes, media_type }
MediaMetadata    = Movie { title, plot, release_date?, country_of_origin?,
                           genres, artwork? }
                 | Series { title, plot, first_air_date?, country_of_origin?,
                            genres, artwork? }
                 | Episode { title, plot, air_date?, season, episode, artwork? }
Genre            = canonical enum with a canonical uppercase id
```

- `Candidate` is deliberately shallow: it contains only what one search call can produce on both providers.
- `ExternalRef` is provider-tagged because a reference from one provider means nothing to the other, and because the
  future manual flow sends a selection back.
- Metadata mirrors the catalogue columns; it carries no provider URL, score, popularity, runtime or language, because
  the catalogue has no place for them.
- Absence is `None` or an empty list, never a fabricated date, empty country or default genre.

### D4 — Port shape, and why the chain is generic rather than `dyn`

`VideoMetadataProvider`:

```text
search(query) -> Result<Vec<Candidate>, ExternalServiceError>
lookup(target) -> Result<Option<MediaMetadata>, ExternalServiceError>
```

- Two operations because both providers' search endpoints are shallow: a rich single response would force one detail
  request per hit. `search` is cheap; `lookup` is one item and carries the artwork.
- `lookup` returns `Option`: a missing item is not a port error (`STY-RUST-016`).
- `lookup` for a series episode takes the season and episode explicitly. Neither provider needs a full season tree; OMDb
  charges one request per episode, so a tree-shaped contract would be a fan-out disguised as a field.
- The port keeps return-position `impl Future`; the wrapper is generic over its link type
  (`FailoverVideoMetadataProvider<L: VideoMetadataProvider>`), and production instantiates `L` with a closed
  `ConfiguredProvider` enum. `Vec<Arc<dyn VideoMetadataProvider>>` would not compile with return-position `impl Future`,
  and boxing the port just to hold links would diverge from every other outbound port.

_Alternatives:_ one `identify(query)` operation that searches and resolves internally (pushes candidate selection into
the adapter, which is business policy); a tree-shaped series response (OMDb cost); dyn links (object safety).

### D5 — Adapters: canonical mapping, bounded artwork, classified failures

- TMDB: `/search/movie` and `/search/tv` for `search`; `/movie/{id}`, `/tv/{id}` and `/tv/{id}/season/{s}/episode/{e}`
  for `lookup`. Prefer bearer authentication when the configured key supports it; a v3 query key is allowed but falls
  under the sanitizing rule.
- OMDb: `?s=&type=` for `search`; `?i=` (or `?t=&season=&episode=`) for `lookup`. `Response:"False"` with HTTP 200 is
  "not found"; HTTP 401 or `Invalid API key!` is `Unauthorized`.
- Mapping: dates parsed to `Option<NaiveDate>` (`N/A` → `None`); country resolved to an uppercase ISO alpha-2 code,
  unknown names dropped; genres mapped through the shared vocabulary; artwork downloaded from the provider's address and
  returned as bytes plus a sniffed media type.
- Artwork fetch rules: https only, provider host allow-list, bounded size, bounded time, no cross-host redirects,
  sniffed content type. A failed download yields `artwork: None` and an event; it does not fail the lookup.
- Every `reqwest` failure is classified into a value-less `ExternalServiceError` variant at the adapter boundary. The
  raw error (whose `Display` can carry the request URL, and therefore an OMDb key) is never propagated or logged. If a
  URL is needed for diagnosis, only scheme, host and path are emitted.
- Base URL and request timeout are constructor inputs so `wiremock` can stand in for the provider.
- No retries; a transient failure is returned and the chain decides.

_Alternatives:_ returning provider URLs and downloading in the caller (leaks provider differences and puts HTTP in the
application layer); trusting declared content types (unsafe for a stored image); following redirects (forwards the OMDb
query key to another host).

### D6 — Failover chain

- `FailoverVideoMetadataProvider<L>` holds `Vec<(ProviderId, L)>` already ordered by index.
- `search`: iterate in order. First non-empty result wins. No result, `Timeout`, `Unavailable`, `RateLimited`,
  `CommunicationFailure`, `DependencyFailure`, `RetryableFailure`, `DeserializationFailure`, `Unauthorized`/`Forbidden`
  (with an operator event) and `Unknown` fall over. `InvalidRequest` and `SerializationFailure` do not, because the
  request was the system's own defect.
- All links empty → `Ok(empty)`. At least one failure and no result → the first link's error (the primary's diagnosis).
- `lookup`: route by `reference.provider`; an unconfigured reference is `Ok(None)` plus an event; a routed failure is
  returned, never retried at another provider.
- Each link is asked at most once.
- Zero links is not a chain state: the composition root passes no provider to the use case, which reports "unavailable".
  This keeps "feature off" distinct from "all providers unavailable".

_Alternatives:_ a composite that also merges candidates (needs identity matching, deferred); falling back on `lookup`
(impossible without re-searching and re-matching); `wiremock` to test the chain (it would re-test the adapters and
cannot produce every port error variant).

### D7 — Configuration and the key

```yaml
library:
  metadata:
    video:
      providers:
        - name: tmdb
          index: 1
          api_key: "..."
        - name: omdb
          index: 2
          api_key: "..."
```

- `index` is the only order; the file order is ignored. Entries are validated (`VideoMetadata::try_new`): the name is a
  known `ProviderId`, names and indices are unique, indices are positive, and every entry has a non-empty key. The
  constructor stores the entries sorted by index.
- The default is `providers: []` (feature off). The generated template shows an empty list and a commented example;
  because the key fields are `skip_serializing`, the template stays byte-comparable with the serialized defaults and no
  key is ever written back.
- The key is a redacting `ApiKey` value: manual `Debug` prints `"**"`, deserialization accepts the YAML string, and
  serialization is skipped. No derived `Debug` on any containing struct can leak it.
- The key is configuration, not instance identity: it stays out of `state.yaml`. It is config-file-only; the environment
  override layer does not name it, which is a documented exception to "every setting can be overridden from the
  environment". The template comment is reworded so "secrets are not configuration" refers to the instance secrets only.
- Invalid configuration fails startup and names the setting, never the value. An empty provider list is valid and means
  identification is unavailable.

_Alternatives:_ keys in the environment only (rejected by the product decision); keys in `state.yaml` (wrong owner and
save-once semantics); a map keyed by provider name (equivalent, but the list keeps the explicit `name` field and makes
duplicate validation visible); a separate `api_keys` map (two places to edit).

### D8 — Genre vocabulary

- `Genre` is a closed enum in the domain with a canonical uppercase id (for example `SCIENCE FICTION`), shared by both
  adapters and reusable by a future music provider.
- Normalization is structural first: casefold, replace non-alphanumerics with spaces, collapse whitespace. Only then
  does a small alias table map the remaining synonyms (`sci fi` and `science fiction` both → `SCIENCE FICTION`). This
  keeps the table short and extensible one line at a time.
- An unknown term is dropped and counted as an event; the canonical response never carries a value outside the
  vocabulary.
- Adapters call the constructor; this is a value rule, not a domain service, so `ARCH-005` is untouched.

_Alternatives:_ provider genre ids (two providers, two id spaces); raw names (drift such as `SCI-FI` beside
`SCIENCE FICTION`); a database-backed vocabulary (a persistence concern for the later change).

### D9 — Use case and composition

- `IdentifyVideo<P: VideoMetadataProvider>` holds `Option<Arc<P>>`. `None` returns `Unavailable`; otherwise it parses
  the name, iterates the queries, calls `search`, returns the first non-empty result, advances on empty, and stops on
  error.
- It never calls `lookup` in this change; resolving a chosen candidate belongs with persistence. `lookup` is still part
  of the port and is covered by adapter tests.
- It persists nothing, ranks nothing and logs no name.
- The composition root builds the configured adapters in index order, wraps them, and passes the wrapper to
  `RuntimeLibraryUseCaseFactory`, which exposes `IdentifyVideo`. With no provider, the factory builds the use case with
  `None`.
- The factory's concrete wrapper type is `FailoverVideoMetadataProvider<ConfiguredProvider>`; the use case is generic
  over it, so no trait object is needed for the provider.

_Alternatives:_ returning every query's results (hides the parser's ranking and forces the future matcher to sift
noise); the factory returning `Option<Arc<dyn IdentifyVideoUseCase>>` (moves the "unavailable" case out of the tested
use case); ranking candidates here (matching is out of scope).

### D10 — Observability

- Catalogued events: provider fallback (provider + classification), lookup against an unconfigured provider, unmatched
  genre (count only), artwork download failure, and an invalid configuration refusal (startup).
- None of them contains a query, title, file name, URL or key. Events go through the application's event constructors,
  which infrastructure may call (as `MediaIngestRuntime` already does).
- `TechnicalDesign.md` § 9 gains the new event rows; the reserved `security` target rules are unchanged.

### D11 — Test strategy

| Layer          | Method                                                                                                                                                                                                                        |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Parser         | `rstest` corpus in the same file; invariants: non-empty titles, `episode => season`, no release token survives, deterministic order                                                                                           |
| `Genre`        | covered through both adapters with equivalent and unknown terms                                                                                                                                                               |
| Port           | declaration plus `mockall::automock`; no direct test                                                                                                                                                                          |
| Adapters       | `wiremock` + golden payloads from `reqwest/fixtures/`, same-file module, every response: hit, empty, not found, 401/403/429/5xx, malformed, timeout, connection refused, artwork success/failure/oversize/redirect            |
| Cross-provider | the same film mapped from a TMDB payload and an OMDb payload yields the same canonical kind, year, country and genre set                                                                                                      |
| Chain          | `MockVideoMetadataProvider` links; the full outcome matrix, call counts, error-classification table, lookup routing, no retry                                                                                                 |
| Use case       | mock port plus the real parser; first-hit, empty-then-hit, error-stop, unusable name, unavailable, `lookup` never called, no name in logs                                                                                     |
| Config         | Rust model/source tests (defaults, valid list, unknown/duplicate provider, duplicate index, missing key, index order, template/partial-model tests); Python section module asserts startup and the absence of the key in logs |
| Secrets        | sentinel key in every adapter test: the request carries it, the port error's `Display`/`Debug` and our log lines do not                                                                                                       |

No E2E file: this change adds no catalogued business use case or route (`TEST-041`). No real network call: `wiremock` is
the boundary.

Small fixtures are duplicated per adapter rather than shared, because `TEST-046` restricts shared Rust helpers to
`test_helpers` and `TEST-047` requires approval.

## Risks / Trade-offs

- **Parser heuristics are never complete** → the ordered-hypothesis contract limits the damage (a wrong first guess can
  still be followed), and the golden corpus is the regression net; future user confirmation can override a match.
- **OMDb's free quota is small** → no internal retries, failover only on failure/empty, and the config order is the
  budget knob; a cooldown is a later feature.
- **The key sits on disk** → redacted `Debug`, `skip_serializing`, never logged, file permissions are the operator's; no
  env fallback by decision.
- **Canonical genre drift across providers** → structural normalization plus the alias table; unknown terms are dropped
  and counted rather than silently admitted.
- **Provider API drift** → golden fixtures fail loudly in the adapter tests; the mapping is the single place to update.
- **Country-name mapping is incomplete** → unmappable countries are dropped, not guessed.
- **Artwork is untrusted content fetched from a provider-supplied address** → https allow-list, size/time bounds, no
  cross-host redirects, sniffed type.
- **`reqwest` on the Alpine/musl image** → confirm the TLS backend builds in the container; switch to the rustls backend
  if native TLS is unavailable. This is a § 6/build risk, not a design change.
- **No consumer route** → the internal use case keeps the slice exercised without inventing an API; there is no E2E
  coverage until the trigger change.
- **The domain-service "two use cases" rule** → only one caller exists now; the design records the planned enrichment
  caller so the justification is visible at review.
- **`lookup` has no application caller** → adapter tests prove it; the persistence change consumes it.
- **The config test suite is weak without an endpoint** → its section module asserts startup and log redaction;
  rejection cases stay in Rust (`TEST-057`).

## Migration Plan

- No database migration and no API change. The configuration section is additive and defaults to an empty provider list,
  so existing installations start unchanged with identification unavailable.
- Deployment: add the `library.metadata.video.providers` entries and their keys to `config.yaml`, then restart;
  configuration and providers are immutable at startup.
- Rollback: remove the section and restart. No persisted state exists to clean up because this change writes none.

## Open Questions

- Whether the canonical genre vocabulary is later seeded into the `genre` table (belongs to the persistence change).
- Whether a provider key should also accept an environment override (explicitly deferred; config-file-only for now).
- Exact artwork size cap and request timeout values (implementation constants for the task phase).
- How a multi-episode file is represented once persistence exists, since the schema allows one video per episode; the
  parser's behavior is documented, not solved, here.
