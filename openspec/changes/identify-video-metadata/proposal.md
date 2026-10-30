# Proposal

## Why

The library can already register files and read films and shows, but nothing populates the film and show catalogue: the
ingest records files, videos, images and audio, while the movie and series repositories are read-only. To fill it, the
server must turn a file or folder name into search terms and ask an external metadata service what the work is. That
identification step is what the future upload, manual and background triggers build on, and its port contract is the
part that cannot change later without reworking every adapter.

## What Changes

- Add a pure domain service that interprets a media name or root-relative path into ordered search queries (kind, title,
  year, optional season and episode), never fabricating a season or a year.
- Add canonical domain values for the query, the opaque provider-tagged candidate reference, the film/show/episode
  metadata, the downloaded artwork, and a shared genre vocabulary.
- Add the outbound port `VideoMetadataProvider` with a shallow `search` and a rich `lookup`, whose response has the same
  shape whichever provider answers.
- Add TMDB and OMDb adapters that map their payloads into that canonical response: release and first-air dates optional,
  country of origin as an ISO alpha-2 code, genres normalized into the shared vocabulary, and artwork delivered as image
  content rather than a provider URL.
- Add an ordered failover chain that implements the same port: providers are tried in configured order on no result,
  error or rate limit; a lookup is routed to the provider named by its reference and never retried elsewhere; a link is
  never retried internally.
- Add the `library.metadata.video` configuration section: an ordered provider list, each entry carrying a name, a
  pipeline index and an API key. The key is redacted in logs and never written back; a listed provider without a key, an
  unknown provider or a duplicate fails startup.
- Add an internal `IdentifyVideo` application use case that parses a name, searches the configured chain and returns the
  candidates of the first query that yields any. It has no HTTP entry point and persists nothing.
- Keep identification unavailable distinguishable from "no match" when no provider is configured.
- Record the new observability events (fallback, unmatched genre, artwork failure) in the event catalog.

Not included: triggers (upload, manual, background), matching and ranking, persistence of films/shows/seasons/episodes,
artwork storage, user-entered metadata, REST endpoints, OpenAPI, and music identification.

## Capabilities

### New Capabilities

- `library/video-identification`: interpreting a media name into search queries, querying external metadata providers
  through a provider-independent contract, and failing over between configured providers in order.

### Modified Capabilities

None. The configuration capability's requirements do not change; the provider list is a setting owned by this
capability.

## Impact

- New domain values and a domain service; a new outbound port.
- New adapters under `infrastructure/outbound/reqwest/` with wiremock golden-file tests; a new failover chain under
  `infrastructure/outbound/metadata/`.
- New application port and use case, plus composition-root and Library use-case factory wiring.
- `library.metadata.video` configuration: domain configuration model, partial model, file source, environment source,
  generated default template and the configuration test suite.
- `docs/development/Overview.md` module map and the `TechnicalDesign.md` event catalog.
- No database migration, no REST endpoint, no `docs/openapi.json` change, no ingest change.
- `reqwest` is already a dependency and `wiremock` is already a dev-dependency; no new crate is required.
