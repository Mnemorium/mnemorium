# Proposal

## Why

The catalogue can already read music albums, their mediums and their tracks, but nothing in the server can turn what it
holds into album or recording metadata: `IngestMedia` registers an audio file and its audio row and stops there, and the
music tables therefore stay empty. Films, shows and episodes already have an identification path through TMDB and OMDb;
music has none. MusicBrainz is the natural first music provider because it needs no API key, addresses every entity by a
stable MBID, publishes a curated genre list and serves cover art.

## What Changes

- Introduce an outbound metadata port for music, covering the three kinds of work a music library needs to identify: an
  **album** (the abstract work), a **release** (one edition of that album, which is what owns mediums and tracks) and a
  **recording** (one performance, independent of the release it appears on).
- Introduce the provider-independent vocabulary the port answers with: a query, a shallow candidate, a resolution
  target, and metadata for an album, a release or a recording, together with the edition, medium, status and artist
  credit values those answers are made of.
- Introduce a pure ordering rule applied to every list of editions: editions whose every medium is a compact disc come
  first, then official status, then earliest release date, then identifier. The rule is a stable partition, so the
  provider's own order survives inside each bucket.
- Implement the port over MusicBrainz: query construction that escapes Lucene metacharacters, the descriptive User-Agent
  the service requires, request pacing and a bounded retry budget enforced inside the adapter, `503`/`429` classified as
  a rate limit, and artwork fetched from the Cover Art Archive.
- Add a music metadata configuration section whose provider entry carries no credential, alongside the existing video
  section. An empty list leaves music identification unavailable, so the server contacts no music provider until an
  operator opts in.
- Rename the video configuration value objects for symmetry with the new section. The configuration file's own shape is
  unchanged, and the configuration is not exposed over HTTP, so nothing externally observable changes.
- Add the internal identification use case the port needs: it holds an optional provider, so an operator who configures
  no music provider gets "unavailable", which stays distinct from a search that ran and found nothing. It has no HTTP
  surface, like the video identification use case.

Deliberately out of scope: any HTTP surface, album or recording ingest, audio tag reading, persisting an MBID, letting a
user pick an edition by hand, and resolving the track listing of a release.

## Capabilities

### New Capabilities

- `library/music-identification`: turns a music query into provider-independent candidates for an album, a release or a
  recording, resolves one of them into metadata through the provider that issued it, and states the ordering and absence
  rules every answer obeys.

### Modified Capabilities

None. `library/video-identification` keeps its own requirements untouched: the retry budget this change introduces
belongs to the MusicBrainz adapter alone, and the video adapters continue to make one attempt per request.

## Impact

- **Domain**: three new model files (query, candidate, metadata), one new domain service (edition ordering), one new
  outbound port. `configuration.rs` and `partial_configuration.rs` gain the music section.
- **Application**: `load_configuration` merges and validates the music provider list. An internal identification use
  case holds the optional provider, so "unavailable" and "no candidate" stay distinct. No HTTP surface is added.
- **Infrastructure**: a new `reqwest` adapter with golden fixtures; the shared artwork client gains the Cover Art
  Archive hosts and a new provider identifier; `default_config.yaml` gains a commented music section.
- **Documentation**: the module map in `docs/development/Overview.md` and the configuration table in
  `docs/development/TechnicalDesign.md`.
- **Not affected**: `docs/openapi.json` (no HTTP surface changes), the database schema and migrations, the dependency
  set, and the container image.
