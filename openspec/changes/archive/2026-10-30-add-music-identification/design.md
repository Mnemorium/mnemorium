# Design

## Context

See `proposal.md` for motivation and `specs/library/music-identification/spec.md` for the requirements this design
satisfies.

The video identification path already establishes the shape this change follows: a domain port with `search` and
`lookup`, a provider-independent candidate, a failover chain over configured links, and adapters built on the shared
`reqwest` helpers. The music catalogue already exists on the read side — `MusicAlbum` owns mediums and recordings — but
nothing produces it.

Three current-state facts constrain the design:

- `MediumIndex` models only a compact disc, so a non-compact-disc edition cannot be stored even when a provider reports
  it.
- The shared `Genre` vocabulary is film and television, while `MusicAlbum` stores genre identifiers as free text.
- `classify_status` maps every 5xx to `DependencyFailure`, which is not what a provider's rate limit means.

## Goals / Non-Goals

**Goals:**

- A port whose vocabulary is provider-independent and complete enough to identify an album, one edition of it, and one
  recording.
- An adapter that owns everything MusicBrainz-specific, so no provider term leaks into the domain or the application.
- A configuration section that expresses "this provider needs no credential" without weakening the video section's key
  validation.

**Non-Goals:**

- Any HTTP surface, ingest, tag reading or MBID persistence.
- A second music provider, and therefore a music failover chain.
- Resolving the track listing of a release.

## Decisions

### A sibling vocabulary, sharing only the medium-neutral atoms

`MusicQuery`, `MusicKind`, `MusicCandidate`, `MusicTarget`, `MusicMetadata`, `Edition`, `MediumSummary`, `MediumFormat`,
`ReleaseStatus` and `ArtistCredit` live in three new model files and one new port. `ExternalRef`, `Artwork` and
`ExternalServiceError` are reused as they are; `ProviderId` gains a `MusicBrainz` variant, because an `ExternalRef` is
provider-tagged either way.

_Alternative:_ widen the video vocabulary — `VideoKind` → `MediaKind`, `MediaMetadata` gaining `Album` and `Recording`,
`VideoMetadataProvider` → `MetadataProvider`. Rejected: it is a rename across roughly fifteen files and the
`library/video-identification` spec text, for no behaviour change, and it would leave `Candidate` carrying an `artist`
field that is inert for every film and show. The two ports stay siblings the way the two catalogues already are.

### An album's identity is its release group; a release is an edition

A search for an album returns one candidate per work, not one per edition. The editions themselves are returned by the
album lookup, where each carries its mediums, and are what the compact-disc rule orders.

_Alternative:_ make the candidate a release. Rejected: an album search would return dozens of candidates sharing one
title, and the abstract album would have no identity to name.

This also places the local disc count where it belongs. Only the caller knows it, and it is the strongest signal for
choosing among compact-disc editions — which is why the port returns the ordered list rather than a single pick. A
manual choice, out of scope here, is the same list worked differently.

### A resolved release composes its edition

`MusicMetadata::Release` carries an `Edition` plus the artist credits, artwork and genres that only a full release
request returns, and an optional back-reference to its album.

_Alternative:_ a flat `Release` struct repeating `title`, `country`, `mediums`, `release_date` and `status`. Rejected:
eight shared fields duplicated for one level of access, with two shapes free to drift.

### The ordering rule is a pure domain service

`release_order` holds the compact-disc partition and the tie-breaks. The adapter produces values; it does not decide
their order beyond the provider's own.

_Alternative:_ order inside the adapter. Rejected: the rule is provider-independent, and testing it there would require
a stub server and a golden payload per case. As a domain service it is covered by `TEST-024`–`TEST-026` with plain
values.

The rule is a stable partition, not a sort: provider order survives inside each bucket, which keeps the "no relevance
ranking of its own" guarantee intact while still putting the actionable editions first.

### Pacing and retry live in the MusicBrainz adapter

The adapter enforces the interval, the bounded retry and the give-up log itself. It adds no type, no trait and no
module; the interval and attempt budget are constructor inputs, the way the request timeout already is.

_Alternatives:_ a shared governor called by every adapter, and a decorator wrapping the port. Both were considered and
set aside. The decorator's granularity — one pacing slot per port call — happens to be correct here only because one
MusicBrainz port call is exactly one musicbrainz.org request; the artwork download goes to a different host with its own
budget. Keeping the policy in the adapter means the video adapters are untouched and the retry does not have to be
justified against `library/video-identification`'s single-attempt requirement.

Two details this decision forces:

- **`503` and `429` are classified as `RateLimited` in the adapter**, before the shared `classify_status` sees them. The
  shared helper maps 5xx to `DependencyFailure`; without the local override the retry budget would never fire.
- **The failure is logged once, on give-up**, not per attempt, so a retry does not multiply `port_fault` events.

The pacing state is per adapter instance. Because the factory rebuilds configuration-derived chains on every accessor
call (`STY-RUST-062`), that state would reset between calls if a music provider were ever resolved the same way. No
caller exists yet; the note is recorded so the future use case resolves the provider once rather than per call.

### No failover chain for music yet

A chain over one link is a pass-through. The "unavailable versus no match" distinction lives in an internal
identification use case that holds an optional provider, exactly as the video use case models it: `None` reports
`Unavailable`, while a search that ran and found nothing is an empty candidate list. That use case has no HTTP surface,
persists nothing and ranks nothing.

_Alternative:_ generalise the existing chain over a port family trait. Rejected for now: it is machinery for one
implementer, and it would make the video trait harder to use as a trait object.

### Per-medium configuration sections, credential derived from the provider

`library.metadata` gains a `music` sibling of `video`. A music entry carries `name` and `index` and no credential field,
because MusicBrainz takes none; a video entry keeps requiring `api_key`.

_Alternative:_ one flat provider list with a credential union and a capability filter. Rejected: it needs a migration of
every existing configuration file, a medium-capability filter, and a per-medium uniqueness rule for `index`, all to
support a provider that serves both media — which none does.

`VideoMetadata` becomes `VideoProviders` and the new settings type is `MusicProviders`, so the name `MusicMetadata`
stays free for the port's result type. The configuration file's shape is unchanged, and the configuration is not exposed
over HTTP, so nothing externally observable moves.

The User-Agent is a build constant rather than an operator setting: the service wants the maintainer's contact so it can
reach whoever controls the client's behaviour, and an operator-supplied string is the wrong party.

### Artwork reaches the Cover Art Archive through its JSON index

The shared artwork client refuses redirects by design, and the Cover Art Archive answers with redirects at two points:
the per-item index redirects to `archive.org`, and every image address the index names redirects again, ending on a
per-item CDN subdomain such as `dn710607.ca.archive.org`. The adapter therefore reads the archive's JSON index and then
fetches the image address that index names through a download that follows a redirect only when the hop stays inside the
adapter's own allow-list.

Three details this decision forces:

- **The index names images in plain http.** An allow-listed http address is re-schemed to https before it is requested,
  so nothing is ever fetched in the clear.
- **The allow-list matches subdomains.** An entry admits its own host and the dot-delimited subdomains of it, which is
  what reaches the archive's CDN; a look-alike such as `notarchive.org` stays rejected. The rule is the shared artwork
  client's, so every adapter's list is read the same way; only the redirect hop stays the archive's.
- **The index is read only from an allow-listed host.** The index request follows the archive's redirect, so its final
  address is checked against the same list before the body is decoded.

_Alternative:_ relax the artwork client's redirect policy for every adapter. Rejected: the video adapters still refuse
redirects outright, and the hop the archive needs is granted only to the client that asked for it.

### Caller-supplied values are escaped for the provider's query syntax

MusicBrainz search is Lucene syntax, so a title is escaped before it is placed in a field query. A folder called
`AC/DC (Live)` must search for that text, not rewrite the query.

## Risks / Trade-offs

| Risk                                                                             | Mitigation                                                                                                                  |
| -------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| A retry budget holds a caller for attempts × request timeout                     | The budget is bounded (three attempts, a short backoff) and the interval is the provider's own limit, not a guess           |
| Pacing state resets if the provider is rebuilt per call                          | Recorded above; the future use case resolves the provider once                                                              |
| A rate-limit burst blocks the whole source IP, not just the offending request    | The adapter serialises its own requests through one slot reservation, so concurrent callers queue instead of bursting       |
| The Cover Art Archive hop costs a second request                                 | It runs against a different host, so it does not consume the MusicBrainz budget                                             |
| Only compact discs can be materialised, so a vinyl-only album cannot be ingested | The port reports the edition anyway; the gap is in the schema, not in identification                                        |
| MusicBrainz data quality — partial dates, junk tags                              | Unmappable values are dropped rather than guessed, per the spec's absence rule                                              |
| `ProviderId` gaining a variant breaks exhaustive matches                         | It is a compile error at each site, which is the intent; the artwork host list and the failover chain are the two to update |
| A release search returns edition noise                                           | It is a separate kind from an album search; a caller that wants the album asks for the album                                |

## Migration Plan

No datastore or API migration. The configuration file's shape is unchanged: `library.metadata.music` is a new key beside
the existing `video` key, and `PartialConfiguration` denies unknown fields only within a section it already declares.
Existing deployments validate unchanged; `default_config.yaml` gains a commented music block, and the configuration
table in `docs/development/TechnicalDesign.md` gains the new rows. Rollback is deleting the music block, which returns
the system to "music identification unavailable".

## Open Questions

- How a future ingest chooses among compact-disc editions when the disc count does not settle it: the earliest date is a
  reasonable default, but that is an ingest concern and does not change this port.
