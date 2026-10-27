# Spec Delta

## MODIFIED Requirements

### Requirement: List series

The system SHALL return the shows it holds through `GET /api/v1/library/series`. Series SHALL NOT be scoped to an owner,
and each listed series SHALL carry only its identifier, its name and its cover.

#### Scenario: Authenticated caller lists series

- **WHEN** an authenticated caller requests `GET /api/v1/library/series`
- **THEN** the system returns `200 OK` with a HAL collection of series summaries

#### Scenario: Series summary content

- **WHEN** the system returns a series in the listing
- **THEN** the item carries only the series identifier, name, cover and its `_links.self`
- **AND** the cover is the identifier of the file holding the show's artwork, or `null` when the show stores none
- **AND** it carries no seasons, genres, dates or counts

### Requirement: Read one series

The system SHALL return one show through `GET /api/v1/library/series/{series_id}`, carrying its metadata, genres, cover
and season summaries.

#### Scenario: Series of an existing identifier

- **WHEN** an authenticated caller requests a series identifier that exists
- **THEN** the system returns `200 OK` with the series representation

#### Scenario: Series representation content

- **WHEN** the system returns one series
- **THEN** the representation carries the series identifier, name, plot, country of origin, first air date, genres and
  cover
- **AND** the cover is the identifier of the file holding the show's artwork, or `null` when the show stores none
- **AND** it carries the seasons ordered by season index
- **AND** each season summary carries its index, its name when set, its episode count, its cover and links to the season
  and its episode collection

#### Scenario: Unknown series

- **WHEN** the requested series identifier is unknown
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed series identifier

- **WHEN** the caller supplies a `series_id` that is not a valid identifier
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: Read one season

The system SHALL return one season through `GET /api/v1/library/series/{series_id}/season/{season_index}`, scoped to the
series named in the path.

#### Scenario: Season of an existing series

- **WHEN** an authenticated caller requests a season index that belongs to a series that exists
- **THEN** the system returns `200 OK` with the season representation

#### Scenario: Season representation content

- **WHEN** the system returns a season
- **THEN** the representation carries its series identifier, its index, its name when set, its episode count, its cover
  and links to its series and its episode collection
- **AND** the cover is the identifier of the file holding the season's artwork, or `null` when the season stores none

#### Scenario: Unknown series or season

- **WHEN** the requested season index does not belong to the series, or the series identifier is unknown
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed season index

- **WHEN** the caller supplies a `season_index` that is not a valid 32-bit unsigned integer
- **THEN** the system returns `400 Bad Request` with a stable message

#### Scenario: Season indexes are scoped to the series

- **WHEN** two different series each hold a season with index 1
- **THEN** each series resolves its own season, and the two seasons are independent
