# library/series Specification

## Purpose

Lets a client browse the shows the server holds through the series → season → episode hierarchy, reading each level's
metadata and the video summary of each episode.

## Requirements

### Requirement: Series reads require an authenticated account

Every series, season and episode endpoint SHALL require a valid bearer token, and a token that names an account which no
longer exists SHALL be rejected.

#### Scenario: Missing or invalid credentials

- **WHEN** a request reaches a series, season or episode endpoint without a valid bearer token
- **THEN** the system returns `401 Unauthorized`

#### Scenario: Account no longer exists

- **WHEN** an authenticated request reaches a series, season or episode endpoint but the caller's account no longer
  exists
- **THEN** the system returns `401 Unauthorized`

#### Scenario: An authenticated caller is never forbidden

- **WHEN** an authenticated caller reads any series, season or episode
- **THEN** the system returns the representation and never a `403 Forbidden`

### Requirement: List series

The system SHALL return the shows it holds through `GET /api/v1/library/series`. Series SHALL NOT be scoped to an owner,
and each listed series SHALL carry only its identifier and name.

#### Scenario: Authenticated caller lists series

- **WHEN** an authenticated caller requests `GET /api/v1/library/series`
- **THEN** the system returns `200 OK` with a HAL collection of series summaries

#### Scenario: Series summary content

- **WHEN** the system returns a series in the listing
- **THEN** the item carries only the series identifier and name, plus its `_links.self`
- **AND** it carries no seasons, genres, dates or counts

### Requirement: Series listing is filterable and paginated

The series collection SHALL accept an optional `name` filter and optional `limit` and `offset` pagination. `limit` SHALL
default to 20 and SHALL be between 1 and 100.

#### Scenario: Filter and page

- **WHEN** an authenticated caller requests the series collection with `name`, `limit` and `offset`
- **THEN** the system returns only the shows whose name matches, starting at `offset` and holding at most `limit` items
- **AND** the collection's `_links.self` preserves the active filter and page

#### Scenario: Out-of-range limit

- **WHEN** the caller requests the series collection with a `limit` of 0 or greater than 100
- **THEN** the system returns `422 Unprocessable Content` with a stable message

#### Scenario: Malformed query string

- **WHEN** the caller requests the series collection with a query string that cannot be decoded
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: Read one series

The system SHALL return one show through `GET /api/v1/library/series/{series_id}`, carrying its metadata, genres and
season summaries.

#### Scenario: Series of an existing identifier

- **WHEN** an authenticated caller requests a series identifier that exists
- **THEN** the system returns `200 OK` with the series representation

#### Scenario: Series representation content

- **WHEN** the system returns one series
- **THEN** the representation carries the series identifier, name, plot, country of origin, first air date and genres
- **AND** it carries the seasons ordered by season index
- **AND** each season summary carries its index, its name when set, its episode count, and links to the season and its
  episode collection

#### Scenario: Unknown series

- **WHEN** the requested series identifier is unknown
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed series identifier

- **WHEN** the caller supplies a `series_id` that is not a valid identifier
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: Season and episode identity

A season SHALL be identified by its index within its series, and an episode by its index within its season. Both indexes
SHALL be non-negative 32-bit integers, and the representation SHALL expose the index as `id`.

#### Scenario: Index is the identity

- **WHEN** the system returns a season or an episode
- **THEN** its `id` is its parent-scoped index, not a separate identifier

#### Scenario: Index range

- **WHEN** a season or episode is stored at the largest 32-bit unsigned index
- **THEN** the system addresses and returns it

### Requirement: Read one season

The system SHALL return one season through `GET /api/v1/library/series/{series_id}/season/{season_index}`, scoped to the
series named in the path.

#### Scenario: Season of an existing series

- **WHEN** an authenticated caller requests a season index that belongs to a series that exists
- **THEN** the system returns `200 OK` with the season representation

#### Scenario: Season representation content

- **WHEN** the system returns a season
- **THEN** the representation carries its series identifier, its index, its name when set, its episode count, and links
  to its series and its episode collection

#### Scenario: Unknown series or season

- **WHEN** the requested season index does not belong to the series, or the series identifier is unknown
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed season index

- **WHEN** the caller supplies a `season_index` that is not a valid 32-bit unsigned integer
- **THEN** the system returns `400 Bad Request` with a stable message

#### Scenario: Season indexes are scoped to the series

- **WHEN** two different series each hold a season with index 1
- **THEN** each series resolves its own season, and the two seasons are independent

### Requirement: List the episodes of a season

The system SHALL return the episodes of one season through
`GET /api/v1/library/series/{series_id}/season/{season_index}/episode`.

#### Scenario: Episodes of an existing season

- **WHEN** an authenticated caller requests the episodes of a season that belongs to a series that exists
- **THEN** the system returns `200 OK` with a HAL collection of episode representations

#### Scenario: Episode representation content

- **WHEN** the system returns an episode in the collection
- **THEN** the representation carries its index as `id`, the series identifier, the season index, its name, plot and air
  date, and its video summary
- **AND** the video summary carries the video identifier, the backing file identifier, duration, codec, width, height
  and scan type
- **AND** it carries links to its series and season

#### Scenario: Unknown series or season

- **WHEN** the caller requests the episodes of a series or season that does not exist, or the season does not belong to
  the series
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed identifiers

- **WHEN** the caller supplies a `series_id` that is not a valid identifier, or a `season_index` that is not a valid
  32-bit unsigned integer
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: Read one episode

The system SHALL return one episode through
`GET /api/v1/library/series/{series_id}/season/{season_index}/episode/{episode_index}`, scoped to the series and season
named in the path.

#### Scenario: Episode of the season

- **WHEN** an authenticated caller requests an episode index that exists in the named series season
- **THEN** the system returns `200 OK` with the episode representation

#### Scenario: Episode not in the season, or unknown series or season

- **WHEN** the requested `episode_index` does not belong to the named season, or the series or season identifier is
  unknown
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed episode index

- **WHEN** the caller supplies a `season_index` or `episode_index` that is not a valid 32-bit unsigned integer
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: Seasons and episodes are ordered by position

The system SHALL order the seasons of a series by ascending season index, and the episodes of a season by ascending
episode index, so repeated reads return a stable order.

#### Scenario: Stable order

- **WHEN** a series has several seasons, each holding several episodes
- **THEN** the series representation lists the seasons by ascending index
- **AND** each season's episode collection lists the episodes by ascending index

### Requirement: Series, season and episode responses are HAL representations

Series, season and episode success responses SHALL be served as `application/hal+json`, keep business data at the root
of the payload, and carry a `_links.self` naming the resource's canonical URI.

#### Scenario: Single resource representation is HAL

- **WHEN** the system returns one series, season or episode
- **THEN** the response is served as `application/hal+json` with business data at the root
- **AND** the representation carries `_links.self` naming the resource's canonical URI

#### Scenario: Collection representation is HAL

- **WHEN** the system returns the series collection or a season's episode collection
- **THEN** the response is served as `application/hal+json`
- **AND** the collection carries `_links.self`
- **AND** the series collection also carries a templated `_links.search` exposing the supported filters
