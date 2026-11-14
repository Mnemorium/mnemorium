# Spec Delta

## Purpose

Turns a media name or a music query into provider-independent candidates, resolves a provider reference into
provider-independent metadata through the provider that issued it, and writes that metadata into the catalogue, for
music and video alike.

## ADDED Requirements

### Requirement: REQ-METADATA-ENRICHMENT-001 — Unavailable is not "no match"

The system SHALL report enrichment as unavailable when no provider of the addressed kind is configured, and SHALL
distinguish that outcome from a search that ran and returned no candidate. It SHALL NOT report an unavailable enrichment
as a failure, and SHALL NOT report an empty candidate list as unavailable.

#### Scenario: SCN-METADATA-ENRICHMENT-001-A — No provider configured

- **WHEN** no provider of the addressed kind is configured
- **THEN** enrichment reports unavailable
- **AND** the answer is not an error

#### Scenario: SCN-METADATA-ENRICHMENT-001-B — Search found nothing

- **WHEN** a configured provider returns no candidate
- **THEN** the search reports no candidate rather than unavailable

### Requirement: REQ-METADATA-ENRICHMENT-002 — A candidate is provider-independent and stays in the provider's order

The system SHALL return candidates whose shape does not depend on the provider that answered, each carrying an opaque
reference tagged with its provider, the kind of work, a title and, when the provider reports them, the group or artist
and the year. Candidates SHALL stay in the provider's order and the system SHALL NOT add a ranking of its own or drop
one the provider reported.

#### Scenario: SCN-METADATA-ENRICHMENT-002-A — The same shape from either provider

- **WHEN** a work is found through any configured provider of a kind
- **THEN** the candidate carries the same fields

#### Scenario: SCN-METADATA-ENRICHMENT-002-B — Provider order preserved

- **WHEN** a provider returns several candidates
- **THEN** the system returns them in that order
- **AND** none is dropped

#### Scenario: SCN-METADATA-ENRICHMENT-002-C — An unreported artist or year

- **WHEN** the provider reports neither an artist nor a year for a candidate
- **THEN** both are absent from the candidate rather than invented

#### Scenario: SCN-METADATA-ENRICHMENT-002-D — A release candidate names its album

- **WHEN** the candidate denotes a release
- **THEN** it also carries the album the release belongs to, its mediums and its status

### Requirement: REQ-METADATA-ENRICHMENT-003 — Video metadata is provider-independent

The system SHALL return film, show or episode metadata whose shape does not depend on the provider. A film SHALL carry a
title, a plot, genres, optional artwork, and an optional release date and country of origin; a show the same with an
optional first-air date; an episode a title, a plot, optional artwork, an optional air date and its season and episode
positions.

#### Scenario: SCN-METADATA-ENRICHMENT-003-A — Film metadata

- **WHEN** a film's metadata is resolved
- **THEN** it carries a title, a plot, its genres, optional artwork, and its release date and country of origin when the
  provider reports them

#### Scenario: SCN-METADATA-ENRICHMENT-003-B — Show metadata

- **WHEN** a show's metadata is resolved
- **THEN** it carries a title, a plot, its genres, optional artwork, and its first-air date and country of origin when
  the provider reports them

#### Scenario: SCN-METADATA-ENRICHMENT-003-C — Episode metadata

- **WHEN** an episode's metadata is resolved
- **THEN** it carries a title, a plot, its season and episode positions, an optional air date and optional artwork

### Requirement: REQ-METADATA-ENRICHMENT-004 — Music metadata is provider-independent

The system SHALL return album, release or recording metadata whose shape does not depend on the provider. An album SHALL
carry a title, artist credits, genres, optional artwork, an optional first-release date, an optional production type and
its editions; a release the title, date, country, status and mediums of one edition; a recording a title, artist
credits, genres and its ISRC codes.

#### Scenario: SCN-METADATA-ENRICHMENT-004-A — Album metadata

- **WHEN** an album is resolved
- **THEN** it carries a title, its artist credits, genres, its editions, and its artwork, first-release date and
  production type when the provider reports them

#### Scenario: SCN-METADATA-ENRICHMENT-004-B — An album without editions

- **WHEN** the provider lists no edition for the album
- **THEN** the album is returned with an empty list of editions

#### Scenario: SCN-METADATA-ENRICHMENT-004-C — Release metadata

- **WHEN** a release is resolved
- **THEN** it carries one edition's title, date, country, status and mediums, and its album reference when the provider
  reports one

#### Scenario: SCN-METADATA-ENRICHMENT-004-D — A medium

- **WHEN** the system reports a medium
- **THEN** it carries the medium's album-scoped position, its format and its track count

#### Scenario: SCN-METADATA-ENRICHMENT-004-E — A recording on several releases

- **WHEN** a recording appears on more than one release
- **THEN** its metadata is returned once, independently of any release

### Requirement: REQ-METADATA-ENRICHMENT-005 — Artwork is content and an apply never stores it

The system SHALL return artwork as image content with its media type and SHALL NOT report a provider address in any
metadata. A failed download SHALL yield no artwork without failing the resolution. An apply SHALL NOT store provider
artwork and SHALL leave the addressed entry's cover unchanged.

#### Scenario: SCN-METADATA-ENRICHMENT-005-A — Artwork from a provider

- **WHEN** a provider reports artwork
- **THEN** the system returns the image content and its media type, not the provider's address

#### Scenario: SCN-METADATA-ENRICHMENT-005-B — Artwork unavailable

- **WHEN** the provider lists no artwork or the download fails
- **THEN** the metadata carries no artwork and the resolution still succeeds

#### Scenario: SCN-METADATA-ENRICHMENT-005-C — An apply keeps the stored cover

- **WHEN** an entry with a cover is applied from a reference whose provider reports artwork
- **THEN** the entry keeps its stored cover and stores no provider artwork

### Requirement: REQ-METADATA-ENRICHMENT-006 — An unreported value is absent, never fabricated

The system SHALL report a date or a country only when the provider supplies a value it can map, and SHALL NOT substitute
a placeholder, the current date or a guess. An incomplete date SHALL be absent, and a placeholder such as `N/A` SHALL be
treated as absent.

#### Scenario: SCN-METADATA-ENRICHMENT-006-A — A partial date

- **WHEN** a provider reports only the year of a date
- **THEN** the date is absent

#### Scenario: SCN-METADATA-ENRICHMENT-006-B — A placeholder value

- **WHEN** a provider reports a placeholder such as `N/A` for a date or a country
- **THEN** the value is absent

#### Scenario: SCN-METADATA-ENRICHMENT-006-C — An unreportable country

- **WHEN** a provider reports a country the system cannot map to a code
- **THEN** the country is absent

### Requirement: REQ-METADATA-ENRICHMENT-007 — Genres come from a curated vocabulary

The system SHALL report genres only from the vocabulary that applies to the work: the canonical video vocabulary for a
film or a show, and the music provider's curated vocabulary for an album or a recording. It SHALL map equivalent
spellings and casing onto one name, SHALL NOT report a tag the vocabulary does not contain, and SHALL NOT apply the
video vocabulary to music.

#### Scenario: SCN-METADATA-ENRICHMENT-007-A — Equivalent spellings

- **WHEN** a provider reports a genre under a different spelling or casing
- **THEN** the system reports the vocabulary's name for it

#### Scenario: SCN-METADATA-ENRICHMENT-007-B — A tag outside the vocabulary

- **WHEN** a provider reports a tag the vocabulary does not contain
- **THEN** that tag is not reported

#### Scenario: SCN-METADATA-ENRICHMENT-007-C — No genres

- **WHEN** a provider reports no genre for a film, a show, an album or a recording
- **THEN** the genres are an empty list

#### Scenario: SCN-METADATA-ENRICHMENT-007-D — The vocabularies do not cross

- **WHEN** video metadata is resolved
- **THEN** its genres come only from the video vocabulary
- **AND** a music genre name is not reported for it

### Requirement: REQ-METADATA-ENRICHMENT-008 — A reference resolves through the provider that issued it

The system SHALL resolve a reference only through the provider named by the reference and SHALL NOT ask another provider
for it. A failure of that provider SHALL be reported rather than retried at another provider.

#### Scenario: SCN-METADATA-ENRICHMENT-008-A — Routed resolution

- **WHEN** a candidate issued by one provider is resolved
- **THEN** only that provider is asked

#### Scenario: SCN-METADATA-ENRICHMENT-008-B — Resolution failure

- **WHEN** the reference's provider fails
- **THEN** the failure is reported and no other provider is asked

#### Scenario: SCN-METADATA-ENRICHMENT-008-C — A reference of another kind

- **WHEN** the provider answers with a work of another kind than the reference denotes
- **THEN** the answer is rejected rather than returned as metadata

### Requirement: REQ-METADATA-ENRICHMENT-009 — A media name yields ordered search queries

The system SHALL interpret a media name or root-relative path as an ordered list of search queries, each naming a media
kind, a non-empty title and an optional year, and a show query MAY carry a season and an episode position. Release
tokens SHALL NOT become part of the title, and a name carrying no usable title SHALL yield no query.

#### Scenario: SCN-METADATA-ENRICHMENT-009-A — Film with a folder year

- **WHEN** the name is `Movies/Inception (2010)/Inception.2010.1080p.BluRay.mkv`
- **THEN** the first query is a film titled `Inception` for the year 2010
- **AND** the release tokens are not part of the title

#### Scenario: SCN-METADATA-ENRICHMENT-009-B — Series episode

- **WHEN** the name is `Show/Season 01/Show.S01E02.1080p.mkv`
- **THEN** the first query is a show titled `Show` with season 1 and episode 2

#### Scenario: SCN-METADATA-ENRICHMENT-009-C — Numeric title

- **WHEN** the name is `1917.2019.1080p.mkv`
- **THEN** the query title is `1917` and the year is 2019

#### Scenario: SCN-METADATA-ENRICHMENT-009-D — Unusable name

- **WHEN** the name yields no usable title
- **THEN** no query is produced

### Requirement: REQ-METADATA-ENRICHMENT-010 — A query never fabricates an episode position

The system SHALL NOT report a season or an episode the name does not support. A name that carries an episode number
without a season SHALL yield a show-level query with no episode position.

#### Scenario: SCN-METADATA-ENRICHMENT-010-A — Absolute numbering

- **WHEN** the name is `Show - 12.mkv`
- **THEN** the query is a show titled `Show` with no season and no episode

#### Scenario: SCN-METADATA-ENRICHMENT-010-B — Season from the folder

- **WHEN** the name is `Show/Season 2/Episode 5.mkv`
- **THEN** the query is a show titled `Show` with season 2 and episode 5

### Requirement: REQ-METADATA-ENRICHMENT-011 — Queries are ordered by supporting evidence

The system SHALL order the queries from the most to the least supported by the folder and file names, and SHALL return
each distinct query at most once.

#### Scenario: SCN-METADATA-ENRICHMENT-011-A — Folder-backed title first

- **WHEN** the folder names the work and the file name is less specific
- **THEN** the folder-backed query precedes the file-backed one

#### Scenario: SCN-METADATA-ENRICHMENT-011-B — Duplicate suppression

- **WHEN** the folder and the file name yield the same query
- **THEN** the query is returned once

### Requirement: REQ-METADATA-ENRICHMENT-012 — Identification stops at the first query that yields candidates

The system SHALL search the ordered queries in order, return the candidates of the first query that yields any, and
SHALL NOT search the remaining queries. A failure SHALL stop the run rather than advance to the next query.

#### Scenario: SCN-METADATA-ENRICHMENT-012-A — First query hits

- **WHEN** the first query yields candidates
- **THEN** the later queries are not searched

#### Scenario: SCN-METADATA-ENRICHMENT-012-B — First query is empty

- **WHEN** the first query yields nothing
- **THEN** the next query is searched

#### Scenario: SCN-METADATA-ENRICHMENT-012-C — A failure stops the run

- **WHEN** a query fails
- **THEN** the remaining queries are not searched and the failure is reported

### Requirement: REQ-METADATA-ENRICHMENT-013 — Failover follows the configured index order

For video, the system SHALL try the configured providers in ascending index order and return the first result. On no
result, a classified failure or a rate limit it SHALL try the next provider. It SHALL ask each provider at most once per
search and SHALL NOT fail over when the failure indicates the request the system formed was invalid.

#### Scenario: SCN-METADATA-ENRICHMENT-013-A — First provider hits

- **WHEN** the first provider returns candidates
- **THEN** the later providers are not asked

#### Scenario: SCN-METADATA-ENRICHMENT-013-B — First provider is empty or rate limited

- **WHEN** the first provider returns no candidate or reports a rate limit
- **THEN** the next provider is asked

#### Scenario: SCN-METADATA-ENRICHMENT-013-C — All providers are empty

- **WHEN** every configured provider returns no candidate
- **THEN** the search reports no candidate

#### Scenario: SCN-METADATA-ENRICHMENT-013-D — All providers fail

- **WHEN** every configured provider fails
- **THEN** the search reports the failure of the first provider

#### Scenario: SCN-METADATA-ENRICHMENT-013-E — No internal retry

- **WHEN** a provider fails
- **THEN** it is asked once and the next provider is tried

#### Scenario: SCN-METADATA-ENRICHMENT-013-F — An invalid request does not fail over

- **WHEN** a provider rejects the request the system sent as invalid
- **THEN** the failure is reported and the next provider is not asked

### Requirement: REQ-METADATA-ENRICHMENT-014 — Country of origin is an ISO alpha-2 code

The system SHALL report a film's or a show's country of origin as an uppercase ISO 3166-1 alpha-2 code, mapping a
provider's country name to its code and omitting a country it cannot map.

#### Scenario: SCN-METADATA-ENRICHMENT-014-A — A code from either spelling

- **WHEN** one provider reports `US` and another reports `United States`
- **THEN** both resolve to `US`

#### Scenario: SCN-METADATA-ENRICHMENT-014-B — An unmappable country

- **WHEN** a provider reports a country with no code in the mapping
- **THEN** the country is absent

### Requirement: REQ-METADATA-ENRICHMENT-015 — Provider configuration is an ordered list validated at startup

The configuration SHALL declare video providers under `library.metadata.video.providers` as entries carrying `name`, a
unique positive `index` and a non-empty `api_key`, and music providers under `library.metadata.music.providers` as
entries carrying `name` and a unique positive `index`. A `name` SHALL be one the entry's section admits. The system
SHALL refuse to start when an entry is invalid or missing a required field, naming the setting and never its value.

#### Scenario: SCN-METADATA-ENRICHMENT-015-A — The index decides the order

- **WHEN** `library.metadata.video.providers` lists `tmdb` with `index: 1` and `omdb` with `index: 2`, in either file
  order
- **THEN** TMDB is tried first
- **AND** the file order does not change the result

#### Scenario: SCN-METADATA-ENRICHMENT-015-B — No providers configured

- **WHEN** `library.metadata.video.providers` or `library.metadata.music.providers` is empty
- **THEN** enrichment for that kind is unavailable

#### Scenario: SCN-METADATA-ENRICHMENT-015-C — An invalid entry refuses startup

- **WHEN** a `name` is not one its section admits, two entries share a `name` or an `index`, an `index` is not positive,
  or a video entry's `api_key` is empty or absent
- **THEN** startup fails naming the offending setting

#### Scenario: SCN-METADATA-ENRICHMENT-015-D — A provider of the wrong kind

- **WHEN** `library.metadata.music.providers` names a video provider, or `library.metadata.video.providers` names a
  music provider
- **THEN** startup fails naming the offending entry

#### Scenario: SCN-METADATA-ENRICHMENT-015-E — A music entry takes no credential

- **WHEN** `library.metadata.music.providers` names `musicbrainz` with an `index` and no `api_key`
- **THEN** the entry is accepted and startup proceeds

### Requirement: REQ-METADATA-ENRICHMENT-016 — A provider credential never leaves the process

The system SHALL NOT write a configured API key to a log, an error message, a metric label or a request address it
emits. A provider that carries the key in a query string SHALL be sanitized before any failure is reported.

#### Scenario: SCN-METADATA-ENRICHMENT-016-A — A failed request

- **WHEN** a provider request fails
- **THEN** the reported failure contains no key

#### Scenario: SCN-METADATA-ENRICHMENT-016-B — A refused startup

- **WHEN** startup fails because of the provider configuration
- **THEN** no key value is written to any sink

#### Scenario: SCN-METADATA-ENRICHMENT-016-C — An artwork failure

- **WHEN** an artwork download fails
- **THEN** the reported failure contains no key and no address carrying one

### Requirement: REQ-METADATA-ENRICHMENT-017 — Video identification is served by kind-scoped reads

The system SHALL expose video identification as a kind-scoped name read and a reference read for films and for shows. A
name read SHALL take a non-empty `name` and search only its kind; a reference read SHALL name the provider and the
reference, and the series read SHALL take `season` and `episode` together, rejecting a partial position. Every read
SHALL name the `ViewMetadata` action.

#### Scenario: SCN-METADATA-ENRICHMENT-017-A — A film name

- **WHEN** a caller submits a name to the film search
- **THEN** the system searches the ordered film queries and returns the candidates of the first query that yields any

#### Scenario: SCN-METADATA-ENRICHMENT-017-B — A show name

- **WHEN** a caller submits a name to the show search
- **THEN** the system searches the ordered show queries and returns the candidates of the first query that yields any

#### Scenario: SCN-METADATA-ENRICHMENT-017-C — A name supports both kinds

- **WHEN** the interpreted queries include film and show hypotheses
- **THEN** the film read searches and returns only the film hypotheses, and the show read only the show ones

#### Scenario: SCN-METADATA-ENRICHMENT-017-D — An empty name

- **WHEN** the submitted name is empty
- **THEN** the system rejects the request

#### Scenario: SCN-METADATA-ENRICHMENT-017-E — No candidate

- **WHEN** at least one provider is configured and none yields a candidate for the searched queries
- **THEN** the system returns an empty candidate list

#### Scenario: SCN-METADATA-ENRICHMENT-017-F — No provider is configured

- **WHEN** no video metadata provider is configured
- **THEN** the system reports enrichment as unavailable rather than as no candidate

#### Scenario: SCN-METADATA-ENRICHMENT-017-G — An unknown provider or reference

- **WHEN** the request names a provider the server does not serve for video, or the provider does not know the reference
- **THEN** the system reports the entry as not found

#### Scenario: SCN-METADATA-ENRICHMENT-017-H — An episode position

- **WHEN** a caller supplies both `season` and `episode`
- **THEN** the system resolves that episode of the referenced show and returns the episode metadata document

#### Scenario: SCN-METADATA-ENRICHMENT-017-I — A partial position

- **WHEN** the caller supplies only one of `season` and `episode`
- **THEN** the system rejects the request

#### Scenario: SCN-METADATA-ENRICHMENT-017-J — An out-of-range position

- **WHEN** a supplied position is not a 32-bit unsigned integer
- **THEN** the system rejects the request

#### Scenario: SCN-METADATA-ENRICHMENT-017-K — The provider reports another position

- **WHEN** the provider resolves the reference to an episode with a season or episode position other than the addressed
  one
- **THEN** the system rejects the answer rather than returning that episode's metadata

### Requirement: REQ-METADATA-ENRICHMENT-018 — A music query names its kind and carries a non-empty title

The system SHALL interpret a music query as an album, a release or a recording, carrying a non-empty title and, when the
caller knows them, a group and a year. A query without a title SHALL be rejected, and a release query SHALL address one
edition rather than the album as a whole.

#### Scenario: SCN-METADATA-ENRICHMENT-018-A — An album query

- **WHEN** the query names an album with a title, a group and a year
- **THEN** it searches for an album carrying those three values

#### Scenario: SCN-METADATA-ENRICHMENT-018-B — A release query

- **WHEN** the query names a release
- **THEN** it searches for one edition rather than for the album as a whole

#### Scenario: SCN-METADATA-ENRICHMENT-018-C — A recording query

- **WHEN** the query names a recording
- **THEN** it searches for a recording rather than for an album

#### Scenario: SCN-METADATA-ENRICHMENT-018-D — An empty title

- **WHEN** the query carries no title
- **THEN** the query is rejected

### Requirement: REQ-METADATA-ENRICHMENT-019 — Production type comes from one discrete list

The system SHALL report an album's production type from the catalogue's discrete list, mapping every provider's spelling
of an equivalent type onto the same value and omitting a type the list does not admit. A qualifying secondary type SHALL
take precedence over the primary one.

#### Scenario: SCN-METADATA-ENRICHMENT-019-A — Equivalent spellings

- **WHEN** a provider reports a type the list admits under a different spelling
- **THEN** the system reports the list's value

#### Scenario: SCN-METADATA-ENRICHMENT-019-B — A secondary type takes precedence

- **WHEN** a provider reports both a primary and a qualifying secondary type
- **THEN** the reported type reflects the secondary one

#### Scenario: SCN-METADATA-ENRICHMENT-019-C — An unmappable type

- **WHEN** a provider reports a production type outside the list
- **THEN** the production type is absent

### Requirement: REQ-METADATA-ENRICHMENT-020 — Compact-disc editions come first

The system SHALL place the editions whose every medium is a compact disc before the other editions in every list of
editions it returns, SHALL keep the provider's relative order within each group, and SHALL treat an edition whose
mediums are unknown or of mixed format as not being a compact-disc edition.

#### Scenario: SCN-METADATA-ENRICHMENT-020-A — Compact discs first

- **WHEN** a provider lists compact-disc and other editions of one album
- **THEN** the compact-disc editions precede the others

#### Scenario: SCN-METADATA-ENRICHMENT-020-B — Order preserved within a group

- **WHEN** several editions share the same format
- **THEN** they appear in the provider's order

#### Scenario: SCN-METADATA-ENRICHMENT-020-C — A mixed edition

- **WHEN** an edition holds both compact discs and another format
- **THEN** it is not treated as a compact-disc edition

#### Scenario: SCN-METADATA-ENRICHMENT-020-D — An edition with no reported medium

- **WHEN** the provider reports an edition without mediums
- **THEN** it is not treated as a compact-disc edition

### Requirement: REQ-METADATA-ENRICHMENT-021 — A query value is escaped for the provider's search syntax

The system SHALL escape every caller-supplied value before placing it in a provider's query, so that no part of a title
or a group name can change the structure of the request the provider receives.

#### Scenario: SCN-METADATA-ENRICHMENT-021-A — Syntax characters

- **WHEN** a title carries characters the provider's search syntax reserves
- **THEN** the provider receives the title as a literal value

#### Scenario: SCN-METADATA-ENRICHMENT-021-B — The query's own operators

- **WHEN** a title carries a conjunction or a field operator
- **THEN** the operator is treated as text and not as part of the query

### Requirement: REQ-METADATA-ENRICHMENT-022 — A provider's request limit is respected

The system SHALL send requests to a music provider no faster than the limit it documents, SHALL serialise concurrent
callers, and SHALL retry a rate-limited, timed-out or transiently failed request within a bounded budget. It SHALL NOT
retry a request the provider rejected as invalid, nor a response it could not parse, nor ask another provider instead.

#### Scenario: SCN-METADATA-ENRICHMENT-022-A — Sequential requests are spaced

- **WHEN** two requests are issued one after the other
- **THEN** the second is sent no sooner than the documented interval after the first

#### Scenario: SCN-METADATA-ENRICHMENT-022-B — Concurrent requests are serialised

- **WHEN** two callers ask the same provider at once
- **THEN** the requests are spaced by the same interval rather than sent together

#### Scenario: SCN-METADATA-ENRICHMENT-022-C — A rate limit is retried

- **WHEN** a provider reports that the request was rate limited
- **THEN** the request is attempted again after a wait, within the budget

#### Scenario: SCN-METADATA-ENRICHMENT-022-D — The budget is exhausted

- **WHEN** every attempt fails
- **THEN** the failure is reported once and no further attempt is made

#### Scenario: SCN-METADATA-ENRICHMENT-022-E — An invalid request is not retried

- **WHEN** a provider rejects the request as invalid
- **THEN** the failure is reported without another attempt

#### Scenario: SCN-METADATA-ENRICHMENT-022-F — An undecodable payload is not retried

- **WHEN** a provider returns a response the system cannot parse
- **THEN** the failure is reported without another attempt

### Requirement: REQ-METADATA-ENRICHMENT-023 — Music identification is served over HTTP

The system SHALL expose music identification as a candidate search and a metadata resolution over HTTP. Both SHALL name
the `ViewMetadata` action, and both SHALL report enrichment as unavailable when no music provider is configured.

#### Scenario: SCN-METADATA-ENRICHMENT-023-A — Candidate search

- **WHEN** a caller searches for an album with a title, a group and a year
- **THEN** the system returns the provider's candidates in the provider's order

#### Scenario: SCN-METADATA-ENRICHMENT-023-B — Metadata resolution

- **WHEN** a caller resolves a candidate issued by the configured provider
- **THEN** the system returns the provider-independent metadata document

#### Scenario: SCN-METADATA-ENRICHMENT-023-C — An unknown provider

- **WHEN** the request names a provider the server does not serve for music
- **THEN** the system reports the entry as not found

#### Scenario: SCN-METADATA-ENRICHMENT-023-D — No provider configured

- **WHEN** no music provider is configured
- **THEN** the system reports enrichment as unavailable, distinct from an empty candidate list

#### Scenario: SCN-METADATA-ENRICHMENT-023-E — An unknown reference

- **WHEN** the configured provider does not know the reference
- **THEN** the system reports the entry as not found

### Requirement: REQ-METADATA-ENRICHMENT-024 — An apply resolves the reference and writes authoritatively

The system SHALL resolve the request's reference through its provider and write the reported metadata in one
transaction: every field the provider reports SHALL replace the stored value, and a value it does not report SHALL be
cleared. A reference that cannot be resolved, resolves another kind, or fails SHALL write nothing. The write SHALL name
the `EditMetadata` action on the addressed entry.

#### Scenario: SCN-METADATA-ENRICHMENT-024-A — Reported values replace

- **WHEN** an entry is applied from a reference that resolves
- **THEN** every field the provider reports replaces the stored value

#### Scenario: SCN-METADATA-ENRICHMENT-024-B — An absent value clears

- **WHEN** the provider reports no date, country or genre for the entry
- **THEN** the stored value is cleared rather than kept or invented

#### Scenario: SCN-METADATA-ENRICHMENT-024-C — A reference of another kind

- **WHEN** the provider resolves the reference to a work of another kind than the entry addressed
- **THEN** the write is rejected and nothing is stored

#### Scenario: SCN-METADATA-ENRICHMENT-024-D — An unknown reference

- **WHEN** the provider does not know the reference
- **THEN** the write is rejected and nothing is stored

#### Scenario: SCN-METADATA-ENRICHMENT-024-E — The provider fails

- **WHEN** the configured provider fails while resolving the reference
- **THEN** the failure is reported and nothing is stored

#### Scenario: SCN-METADATA-ENRICHMENT-024-F — The write is atomic

- **WHEN** a write fails after part of the entry has been changed
- **THEN** no part of the change is stored

### Requirement: REQ-METADATA-ENRICHMENT-025 — External metadata is applied to a film

The system SHALL write a film from the metadata its provider reports for the request's reference, writing the name,
plot, release date, country of origin and genres, and SHALL leave the cover and the backing video unchanged.

#### Scenario: SCN-METADATA-ENRICHMENT-025-A — Apply the reported metadata

- **WHEN** a caller applies a reference the provider resolves to a film
- **THEN** the system stores the film's name, plot, release date, country of origin and genres and returns the film
  representation

#### Scenario: SCN-METADATA-ENRICHMENT-025-B — The provider reports an absent value

- **WHEN** the provider reports no release date, no country of origin or no genres
- **THEN** the film stores no value for that field

#### Scenario: SCN-METADATA-ENRICHMENT-025-C — The cover and the video are untouched

- **WHEN** a film with a cover and a backing video is applied
- **THEN** both keep their stored values

### Requirement: REQ-METADATA-ENRICHMENT-026 — External metadata is applied to a show

The system SHALL write a show from the metadata its provider reports for the request's reference, writing the name,
plot, first air date, country of origin and genres, and SHALL leave the cover, the seasons and the episodes unchanged.

#### Scenario: SCN-METADATA-ENRICHMENT-026-A — Apply the reported metadata

- **WHEN** a caller applies a reference the provider resolves to a show
- **THEN** the system stores the show's name, plot, first air date, country of origin and genres and returns the show
  representation

#### Scenario: SCN-METADATA-ENRICHMENT-026-B — The provider reports an absent value

- **WHEN** the provider reports no first air date, no country of origin or no genres
- **THEN** the show stores no value for that field

#### Scenario: SCN-METADATA-ENRICHMENT-026-C — Seasons, episodes and cover are untouched

- **WHEN** a show with seasons, episodes and a cover is applied
- **THEN** every stored season, episode and cover keeps its value

### Requirement: REQ-METADATA-ENRICHMENT-027 — External metadata is applied to an episode

The system SHALL write one episode from the metadata its provider reports for the addressed show reference, writing the
name, plot and air date, and SHALL leave the video unchanged. The provider's season and episode positions SHALL match
the addressed ones.

#### Scenario: SCN-METADATA-ENRICHMENT-027-A — Apply the reported metadata

- **WHEN** a caller applies a show reference for the addressed season and episode
- **THEN** the system stores the episode's name, plot and air date and returns the episode representation

#### Scenario: SCN-METADATA-ENRICHMENT-027-B — The provider reports no air date

- **WHEN** the provider reports no air date for the episode
- **THEN** the episode stores no air date

#### Scenario: SCN-METADATA-ENRICHMENT-027-C — The position does not match

- **WHEN** the provider reports a season or episode position other than the addressed one
- **THEN** the write is rejected and nothing is stored

#### Scenario: SCN-METADATA-ENRICHMENT-027-D — The video is untouched

- **WHEN** an episode with a backing video is applied
- **THEN** the video keeps its stored value

### Requirement: REQ-METADATA-ENRICHMENT-028 — External metadata is applied to an album

The system SHALL write an album and its medium structure from the metadata its provider reports for the request's
reference. It SHALL move recordings only for the placements the request names, SHALL reject the default album and a
placement naming a medium it does not create, and SHALL leave the cover and the group unchanged.

#### Scenario: SCN-METADATA-ENRICHMENT-028-A — Apply without placements

- **WHEN** an album is applied with no placements
- **THEN** the album's fields and medium structure are written and every track placement is unchanged

#### Scenario: SCN-METADATA-ENRICHMENT-028-B — Apply with placements

- **WHEN** the request names a recording for each destination position
- **THEN** the named recordings move in the same transaction

#### Scenario: SCN-METADATA-ENRICHMENT-028-C — More than one compact disc

- **WHEN** the resolved release holds more than one compact disc
- **THEN** the album carries one compact-disc medium per disc

#### Scenario: SCN-METADATA-ENRICHMENT-028-D — Any other release

- **WHEN** the resolved release holds one medium, or another format
- **THEN** the album carries one digital medium

#### Scenario: SCN-METADATA-ENRICHMENT-028-E — The provider reports no genre

- **WHEN** the provider reports no genre for the album
- **THEN** the album stores the `UNKNOWN` genre

#### Scenario: SCN-METADATA-ENRICHMENT-028-F — The default album

- **WHEN** the target album is the seeded `Unknown` album
- **THEN** the write is rejected and nothing is stored

#### Scenario: SCN-METADATA-ENRICHMENT-028-G — A placement names no medium

- **WHEN** a placement names a medium the apply does not create
- **THEN** the write is rejected and nothing is stored

#### Scenario: SCN-METADATA-ENRICHMENT-028-H — The cover and the group are untouched

- **WHEN** an album with a cover is applied
- **THEN** its cover and its group keep their stored values
