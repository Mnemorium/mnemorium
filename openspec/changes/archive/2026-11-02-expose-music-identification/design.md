# Design

## Context

`IdentifyMusic` already searches through `MusicMetadataProvider` and reports `Unavailable` without a provider. The
provider port also exposes `lookup`, but no use case calls it. The factory has an `identify_video` method but no music
counterpart, and the composition root does not build the MusicBrainz adapter. See `proposal.md` for motivation.

## Goals / Non-Goals

**Goals:**

- A client can search and resolve music metadata over HTTP.
- The provider is selected from configuration, not hard-coded in the handler.
- The identification document matches the catalogue: no recording-level first release date.

**Non-Goals:**

- Persisting the result (change 9).
- Group image collection (change 12).
- A second provider.

## Decisions

### A separate resolve use case

`IdentifyMusic` stays a search. A new `ResolveMusicMetadata` takes a `MusicTarget`, calls `lookup` and reports a missing
item as `Ok(None)`. Keeping search and resolve separate preserves the existing contract and makes the apply step compose
both.

### The resolve route carries the kind

A provider reference alone does not say whether it denotes an album, a release or a recording. The route is
`/identify/{provider}/{kind}/{reference}`; the provider is validated against the configured one, and an unknown provider
is `404`.

### `library.read` guards both routes

Identification reads the provider, not the catalogue, but it is part of the library read surface and should not be open
to an unauthenticated caller. `library.read` is the natural gate and matches the album reads it feeds.

The route addresses no album, so there is no resource to scope the decision to: the use cases call
`Authorization::decide_library_surface`, which applies the rule shared catalogue artwork already follows. A caller the
library is closed to entirely (`NONE` or `OWNED`) is denied; every other scope, `LISTED` included, reaches the route.
The denial stays concealed (`404`), as every read denial does.

### The provider is rebuilt per request

The MusicBrainz adapter is configuration-derived, so the library factory rebuilds it through `configured_music_provider`
on every access, exactly as it already does for the video chain (`STY-RUST-062`). The composition root therefore does
not hold a process-lifetime adapter. The cost is that the adapter's request spacing restarts per request; the service's
own `429` retry bounds a burst.

### No provider is a document, not an error

The API has no "capability unavailable" status, and a generic `500` would not report identification as unavailable. Both
routes answer `200` with `available: false`: a client tells that apart from a search that ran and found nothing
(`available: true` with no candidate) and from a provider that does not know the reference (`404`).

### Rename `artist` to `group`

The query's evidence is the group name; the candidate reports the same evidence. `ArtistCredit` stays because it is the
provider's vocabulary for a credit line, not the local aggregate.

## Risks / Trade-offs

- **The provider adds network latency** → the adapter already paces and retries; the handler stays a thin read.
- **A resolve for an unknown provider** → `404`; no fallback to another provider.
- **A partial date** → still absent in the document; the catalogue stores absence.
- **The rename touches the candidate and query types** → mechanical; the provider mapping and tests cover it.

## Migration Plan

Configuration gains the music provider settings if they are not already present. No datastore change. Regenerate
`docs/openapi.json`.

## Open Questions

- Should the identify routes be `library.read` or a separate `identify` action? The design reuses `library.read`; a
  future provider-specific action can be added if the surface grows.
