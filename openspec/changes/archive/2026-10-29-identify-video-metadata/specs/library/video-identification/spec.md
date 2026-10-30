# Spec Delta

## Purpose

Turns a media file or folder name into search queries and asks configured external metadata providers what the work is,
returning one provider-independent answer for films, shows and episodes.

## ADDED Requirements

### Requirement: A media name yields ordered search queries

The system SHALL interpret a media name or root-relative path as an ordered list of search queries. Each query SHALL
state the media kind, a non-empty title and an optional year, and a show query MAY carry a season and an episode
position.

#### Scenario: Film with a folder year

- **WHEN** the name is `Movies/Inception (2010)/Inception.2010.1080p.BluRay.mkv`
- **THEN** the first query is a film titled `Inception` for the year 2010
- **AND** the release tokens are not part of the title

#### Scenario: Series episode

- **WHEN** the name is `Show/Season 01/Show.S01E02.1080p.mkv`
- **THEN** the first query is a show titled `Show` with season 1 and episode 2

#### Scenario: Numeric title

- **WHEN** the name is `1917.2019.1080p.mkv`
- **THEN** the query title is `1917` and the year is 2019

#### Scenario: Unusable name

- **WHEN** the name is `sample.mkv`
- **THEN** no query is produced

### Requirement: A query never fabricates an episode position

The system SHALL NOT report a season or an episode the name does not support. A name that carries an episode number
without a season SHALL yield a show-level query with no episode position.

#### Scenario: Absolute numbering

- **WHEN** the name is `Show - 12.mkv`
- **THEN** the query is a show titled `Show` with no season and no episode

#### Scenario: Season from the folder

- **WHEN** the name is `Show/Season 2/Episode 5.mkv`
- **THEN** the query is a show titled `Show` with season 2 and episode 5

### Requirement: Queries are ordered by supporting evidence

The system SHALL order the queries from the most to the least supported by the folder and file names, and SHALL return
each distinct query at most once.

#### Scenario: Folder-backed title first

- **WHEN** the folder names the work and the file name is less specific
- **THEN** the folder-backed query precedes the file-backed one

#### Scenario: Duplicate suppression

- **WHEN** the folder and the file name yield the same query
- **THEN** the query is returned once

### Requirement: A candidate is provider-independent

The system SHALL return search candidates whose shape does not depend on the provider that answered. A candidate SHALL
carry an opaque reference tagged with its provider, a media kind, a title and an optional year, and the candidates SHALL
stay in the provider's order without a ranking added by the system.

#### Scenario: Same shape from either provider

- **WHEN** a film is found through either configured provider
- **THEN** the candidate carries the same fields

#### Scenario: Provider order preserved

- **WHEN** a provider returns several candidates
- **THEN** the system returns them in that order

### Requirement: Metadata is provider-independent and artwork is content

The system SHALL return film, show or episode metadata whose shape does not depend on the provider. Artwork SHALL be
returned as image content with its media type, never as a provider address.

#### Scenario: Film metadata

- **WHEN** a film's metadata is resolved
- **THEN** it carries a title, a plot, an optional release date, an optional country of origin, genres and optional
  artwork

#### Scenario: Episode metadata

- **WHEN** an episode's metadata is resolved
- **THEN** it carries a title, a plot, an optional air date and the season and episode positions

#### Scenario: Artwork from either provider

- **WHEN** a provider reports artwork
- **THEN** the system returns the image content and its media type, not the provider's address

### Requirement: Unreported metadata is absent, never fabricated

The system SHALL report a date or a country of origin only when the provider supplies it, and SHALL NOT substitute the
current date or a placeholder for a value the provider does not report.

#### Scenario: Absent release date

- **WHEN** a provider reports no usable release date
- **THEN** the release date is absent

#### Scenario: Placeholder value

- **WHEN** a provider reports a placeholder such as `N/A` for a date
- **THEN** the date is absent

### Requirement: Genres come from one canonical vocabulary

The system SHALL report genres from a single canonical vocabulary, mapping every provider's spelling of an equivalent
genre to the same canonical value. It SHALL NOT report a genre outside that vocabulary.

#### Scenario: Equivalent spellings

- **WHEN** one provider reports `Sci-Fi` and another reports `Science Fiction`
- **THEN** both resolve to the same canonical genre

#### Scenario: Unknown genre

- **WHEN** a provider reports a genre the vocabulary does not contain
- **THEN** that genre is not reported

### Requirement: Country of origin is an ISO alpha-2 code

The system SHALL report a film's or show's country of origin as an uppercase ISO 3166-1 alpha-2 code, mapping a
provider's country name to its code and omitting a country it cannot map.

#### Scenario: Code from either provider

- **WHEN** one provider reports `US` and another reports `United States`
- **THEN** both resolve to `US`

#### Scenario: Unmappable country

- **WHEN** a provider reports a country with no code in the mapping
- **THEN** the country is absent

### Requirement: Metadata is resolved through the reference's provider

The system SHALL resolve a candidate's metadata through the provider named by the candidate's reference and SHALL NOT
resolve it through a different provider. A failure of that provider SHALL be reported rather than retried at another
provider.

#### Scenario: Routed lookup

- **WHEN** a candidate from the second provider is resolved
- **THEN** only the second provider is asked

#### Scenario: Lookup failure

- **WHEN** the reference's provider fails
- **THEN** the failure is reported and no other provider is asked

#### Scenario: Unknown reference

- **WHEN** the reference names a provider that is not configured
- **THEN** no metadata is returned

### Requirement: Failover follows the configured index order

The system SHALL try the configured providers in ascending index order and SHALL return the first result. On no result,
a classified failure or a rate limit it SHALL try the next provider.

#### Scenario: First provider hits

- **WHEN** the first provider returns candidates
- **THEN** the later providers are not asked

#### Scenario: First provider is empty

- **WHEN** the first provider returns no candidate
- **THEN** the next provider is asked

#### Scenario: First provider is rate limited

- **WHEN** the first provider reports a rate limit
- **THEN** the next provider is asked

#### Scenario: All providers are empty

- **WHEN** every configured provider returns no candidate
- **THEN** the search reports no candidate

#### Scenario: All providers fail

- **WHEN** every configured provider fails
- **THEN** the search reports the failure of the first provider

### Requirement: A provider is asked at most once per search

The system SHALL ask each configured provider at most once during one search and SHALL NOT retry it internally.

#### Scenario: No internal retry

- **WHEN** a provider fails
- **THEN** it is asked once and the next provider is tried

### Requirement: Failover is not attempted for a request the system formed incorrectly

The system SHALL NOT fail over when the failure indicates that the request the system sent was invalid, because asking
another provider would conceal a defect rather than serve the caller.

#### Scenario: Invalid request

- **WHEN** a provider rejects the request as invalid
- **THEN** the failure is reported and the next provider is not asked

### Requirement: Identification stops at the first query that yields candidates

The system SHALL search the ordered queries in order, return the candidates of the first query that yields any, and
SHALL NOT search the remaining queries. A failure SHALL stop the identification rather than advance to the next query.

#### Scenario: First query hits

- **WHEN** the first query yields candidates
- **THEN** later queries are not searched

#### Scenario: First query is empty

- **WHEN** the first query yields nothing
- **THEN** the next query is searched

#### Scenario: Failure stops the run

- **WHEN** a query fails
- **THEN** the remaining queries are not searched and the failure is reported

### Requirement: Unavailable is not "no match"

The system SHALL report identification as unavailable when no provider is configured, distinct from the outcome of a
search that found no candidate.

#### Scenario: No provider configured

- **WHEN** no provider is configured
- **THEN** identification reports unavailable

#### Scenario: Search found nothing

- **WHEN** at least one provider is configured and none yields a candidate
- **THEN** identification reports no candidate

### Requirement: Provider configuration is an ordered list

The configuration SHALL declare video metadata providers as entries carrying a provider name, a unique positive pipeline
index and an API key. The index SHALL define the failover order, independent of the order the entries appear in the
file.

#### Scenario: Two providers configured

- **WHEN** the file lists `tmdb` with index 1 and `omdb` with index 2
- **THEN** TMDB is tried first

#### Scenario: File order is ignored

- **WHEN** the entries appear in the file in the reverse of their index order
- **THEN** the index order still decides

#### Scenario: No providers configured

- **WHEN** the provider list is empty
- **THEN** identification is unavailable

### Requirement: Invalid provider configuration fails startup

The system SHALL refuse to start when a provider name is unknown, when two entries share a name or an index, or when a
listed provider has no API key. The refusal SHALL name the offending setting and SHALL NOT reveal the key.

#### Scenario: Unknown provider

- **WHEN** the list names a provider the system does not support
- **THEN** startup fails naming the entry

#### Scenario: Duplicate name or index

- **WHEN** two entries share a name or an index
- **THEN** startup fails

#### Scenario: Missing key

- **WHEN** a listed provider has no key
- **THEN** startup fails naming the setting

#### Scenario: Key not revealed

- **WHEN** startup fails for any of these reasons
- **THEN** the message contains no API key value

### Requirement: An API key never leaves the process

The system SHALL NOT write a configured API key to a log, an error message, a metric label or a request address it
emits. A provider that carries the key in a query string SHALL be sanitized before any failure is reported.

#### Scenario: Failed request

- **WHEN** a provider request fails
- **THEN** the reported failure contains no key

#### Scenario: Startup refusal

- **WHEN** startup fails because of the provider configuration
- **THEN** no key value is written to a log

#### Scenario: Artwork failure

- **WHEN** an artwork download fails
- **THEN** the reported failure and its log contain no key and no address carrying one

### Requirement: Identification outcomes are observable without exposing content

The system SHALL record a provider fallback, a failure, an unmatched genre and a failed artwork download as catalogued
events that never contain the query text, a title, a file name or an API key.

#### Scenario: Fallback event

- **WHEN** a provider is skipped because of a failure
- **THEN** an event records the provider and the failure classification only

#### Scenario: Unmatched genre

- **WHEN** a provider reports a genre outside the vocabulary
- **THEN** an event records the fact without the term

#### Scenario: No content in events

- **WHEN** any identification event is emitted
- **THEN** it contains no title, file name or query text
