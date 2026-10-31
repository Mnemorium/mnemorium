# library/music-identification Specification

## Purpose

Turns a music query into provider-independent candidates for an album, a release and a recording, and resolves one of
them into the metadata the music catalogue needs.

## Requirements

### Requirement: A query names the kind of work and carries a non-empty title

The system SHALL interpret a music query as one of three kinds of work — an album, a release or a recording — carrying a
non-empty title and, when the caller knows them, an artist and a year. A query whose title is empty SHALL be rejected.

#### Scenario: An album query

- **WHEN** the query names an album with a title, an artist and a year
- **THEN** it searches for an album carrying those three values

#### Scenario: A release query

- **WHEN** the query names a release
- **THEN** it searches for one edition rather than for the album as a whole

#### Scenario: A recording query

- **WHEN** the query names a recording
- **THEN** it searches for a recording rather than for an album

#### Scenario: An empty title

- **WHEN** the query carries no title
- **THEN** the query is rejected

### Requirement: A candidate is provider-independent

The system SHALL return candidates whose shape does not depend on the provider that answered. A candidate SHALL carry an
opaque reference tagged with its provider, the kind of work it denotes, a title, and the artist and year the provider
reports.

#### Scenario: The same shape from either provider

- **WHEN** a work is found through any configured provider
- **THEN** the candidate carries the same fields

#### Scenario: A release candidate names its album

- **WHEN** the candidate denotes a release
- **THEN** it also carries the album the release belongs to, the mediums of the release and its status

#### Scenario: An unreported artist or year

- **WHEN** the provider reports neither an artist nor a year for a candidate
- **THEN** both are absent from the candidate rather than invented

### Requirement: Candidates stay in the provider's order

The system SHALL return the candidates in the order the provider reported them, SHALL NOT add a relevance ranking of its
own, and SHALL NOT drop a candidate the provider reported.

#### Scenario: Provider order preserved

- **WHEN** a provider returns several candidates
- **THEN** the system returns them in that order

### Requirement: Album metadata is provider-independent

The system SHALL return album metadata whose shape does not depend on the provider. The album SHALL carry a title, the
genres and artist credits the provider reports, an optional first release date, an optional production type, optional
artwork, and the editions the provider lists for it.

#### Scenario: Album metadata

- **WHEN** an album is resolved
- **THEN** it carries a title, its artist credits, its genres, its optional first release date, its optional production
  type, its optional artwork and its editions

#### Scenario: An album without editions

- **WHEN** the provider lists no edition for the album
- **THEN** the album is returned with an empty list of editions

### Requirement: Release metadata describes one edition

The system SHALL return release metadata whose shape does not depend on the provider. A release SHALL carry a title, the
genres and artist credits the provider reports, its mediums, optional artwork, an optional release date, an optional
country of release, an optional status and, when the provider reports it, a reference to the album it belongs to.

#### Scenario: Release metadata

- **WHEN** a release is resolved
- **THEN** it carries a title, its artist credits, its genres, its mediums, its optional date, country, status and
  artwork, and its album reference when the provider reports one

#### Scenario: Medium content

- **WHEN** the system reports a medium
- **THEN** it carries the medium's album-scoped position, its format and its track count

### Requirement: Recording metadata stands alone

The system SHALL return recording metadata whose shape does not depend on the provider. A recording SHALL carry a title,
the artist credits and genres the provider reports, an optional first release date, and the ISRC codes the provider
reports.

#### Scenario: Recording metadata

- **WHEN** a recording is resolved
- **THEN** it carries a title, its artist credits, its genres, its optional first release date and its ISRC codes

#### Scenario: A recording on several releases

- **WHEN** a recording appears on more than one release
- **THEN** its metadata is returned once, independently of any release

### Requirement: Artwork is content

The system SHALL return artwork as image content with its media type, and SHALL NOT report a provider address in any
metadata. A failed download SHALL yield no artwork without failing the resolution.

#### Scenario: Artwork from a provider

- **WHEN** a provider reports artwork
- **THEN** the system returns the image content and its media type, not the provider's address

#### Scenario: Artwork unavailable

- **WHEN** the provider lists no artwork or the download fails
- **THEN** the metadata carries no artwork and the resolution still succeeds

### Requirement: Unreported metadata is absent, never fabricated

The system SHALL report a date, a country, a status or a production type only when the provider supplies a value the
system can map, and SHALL NOT substitute a placeholder, the current date or a guess for a value the provider does not
report.

#### Scenario: A partial date

- **WHEN** a provider reports only the year of a date
- **THEN** the date is absent

#### Scenario: A placeholder value

- **WHEN** a provider reports a placeholder such as `N/A` for a date or a country
- **THEN** the value is absent

#### Scenario: An unmappable production type

- **WHEN** a provider reports a production type outside the catalogue's list
- **THEN** the production type is absent

### Requirement: Production type comes from one discrete list

The system SHALL report an album's production type from the catalogue's discrete list, mapping every provider's spelling
of an equivalent type onto the same value.

#### Scenario: Equivalent spellings

- **WHEN** a provider reports a type the list admits under a different spelling
- **THEN** the system reports the list's value

#### Scenario: A secondary type takes precedence

- **WHEN** a provider reports both a primary and a qualifying secondary type
- **THEN** the reported type reflects the secondary one

### Requirement: Genres come from the provider's curated vocabulary

The system SHALL report genres from the vocabulary the provider curates, normalized so that equivalent spellings resolve
to the same name, and SHALL NOT report a tag the vocabulary does not contain. The film and television genre vocabulary
SHALL NOT apply to music metadata.

#### Scenario: Equivalent spellings

- **WHEN** a provider reports a genre under a different spelling or casing
- **THEN** the system reports the vocabulary's name for it

#### Scenario: A non-genre tag

- **WHEN** a provider reports a tag that is not in the genre vocabulary
- **THEN** that tag is not reported

#### Scenario: No genres

- **WHEN** a provider reports no genre for an album or a recording
- **THEN** the genres are an empty list

### Requirement: Compact-disc editions come first

The system SHALL place the editions whose every medium is a compact disc before the other editions in every list of
editions it returns, SHALL keep the provider's relative order within each group, and SHALL treat an edition whose
mediums are unknown or of mixed format as not being a compact-disc edition.

#### Scenario: Compact discs first

- **WHEN** a provider lists compact-disc and other editions of one album
- **THEN** the compact-disc editions precede the others

#### Scenario: Order preserved within a group

- **WHEN** several editions share the same format
- **THEN** they appear in the provider's order

#### Scenario: A mixed edition

- **WHEN** an edition holds both compact discs and another format
- **THEN** it is not treated as a compact-disc edition

#### Scenario: An edition with no reported medium

- **WHEN** the provider reports an edition without mediums
- **THEN** it is not treated as a compact-disc edition

### Requirement: Metadata is resolved through the reference's provider

The system SHALL resolve a candidate's metadata through the provider named by the candidate's reference and SHALL NOT
resolve it through a different provider. A failure of that provider SHALL be reported rather than retried at another
provider.

#### Scenario: Routed resolution

- **WHEN** a candidate issued by one provider is resolved
- **THEN** only that provider is asked

#### Scenario: Resolution failure

- **WHEN** the reference's provider fails
- **THEN** the failure is reported and no other provider is asked

#### Scenario: Unknown reference

- **WHEN** the reference names a provider that is not configured
- **THEN** no metadata is returned

### Requirement: Unavailable is not "no match"

The system SHALL report identification as unavailable when no music provider is configured, which is distinct from the
outcome of a search that ran and found no candidate.

#### Scenario: No provider configured

- **WHEN** no music provider is configured
- **THEN** identification reports unavailable

#### Scenario: Search found nothing

- **WHEN** a configured provider returns no candidate
- **THEN** the search reports no candidate

### Requirement: A query is escaped for the provider's search syntax

The system SHALL escape every caller-supplied value before placing it in a provider's query, so that no part of a title
or an artist name can change the structure of the request the provider receives.

#### Scenario: A title carrying syntax characters

- **WHEN** a title carries characters the provider's search syntax reserves
- **THEN** the provider receives the title as a literal value

#### Scenario: A title carrying the query's own operators

- **WHEN** a title carries a conjunction or a field operator
- **THEN** the operator is treated as text and not as part of the query

### Requirement: A provider's request limit is respected

The system SHALL send requests to a provider no faster than the limit that provider documents, and SHALL retry a
rate-limited, timed-out or transiently failed request within a bounded budget. It SHALL NOT retry a request the provider
rejected as invalid, nor a response it could not parse, nor ask another provider instead.

#### Scenario: Sequential requests are spaced

- **WHEN** two requests are issued one after the other
- **THEN** the second is sent no sooner than the documented interval after the first

#### Scenario: Concurrent requests are serialised

- **WHEN** two callers ask the same provider at once
- **THEN** the requests are spaced by the same interval rather than sent together

#### Scenario: A rate limit is retried

- **WHEN** a provider reports that the request was rate limited
- **THEN** the request is attempted again after a wait, within the budget

#### Scenario: The budget is exhausted

- **WHEN** every attempt fails
- **THEN** the failure is reported once and no further attempt is made

#### Scenario: An invalid request is not retried

- **WHEN** a provider rejects the request as invalid
- **THEN** the failure is reported without another attempt

### Requirement: Identification events carry no content

The system SHALL record a provider failure and a failed artwork download as catalogued events that carry the provider
and the failure classification, and SHALL NOT record a query, a title, an artist name, a file name or an address
carrying a credential.

#### Scenario: A failure event

- **WHEN** a provider request fails for good
- **THEN** an event records the provider and the classification only

#### Scenario: No content in events

- **WHEN** any identification event is emitted
- **THEN** it contains no title, artist name, file name or query text

#### Scenario: One event per failed operation

- **WHEN** a request is retried and every attempt fails
- **THEN** exactly one failure event is recorded
