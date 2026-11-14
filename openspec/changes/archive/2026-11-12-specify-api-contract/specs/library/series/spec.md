# Spec Delta

## REMOVED Requirements

### Requirement: Series, season and episode responses are HAL representations

**Reason**: The `api` capability now owns the response media type, the payload shape and `_links.self` for every
representation (`REQ-API-003`, `REQ-API-004`). Restating the rule here duplicated it and let the copies drift.
**Migration**: No client action; the served behavior is unchanged. The rule is read from `openspec/specs/api/spec.md`.

## MODIFIED Requirements

### Requirement: List series

The system SHALL return the shows the caller's `library.read` permission covers through `GET /api/v1/library/series`.
The default permission covers every show for every authenticated caller; a per-user override may narrow the set. The
collection SHALL embed each page item's series summary under `_embedded.items`. Each listed series SHALL carry only its
identifier, its name and its cover as business data; the detail read returns the full representation.

#### Scenario: Authenticated caller lists series

- **WHEN** an authenticated caller with the default permission requests `GET /api/v1/library/series`
- **THEN** the system returns `200 OK` with a collection whose `_embedded.items` carries the page's series summaries

#### Scenario: A restricted caller lists only permitted shows

- **WHEN** an authenticated caller whose `library.read` permission covers only some shows requests the collection
- **THEN** the system returns only the shows that permission covers

#### Scenario: A caller with no catalog access lists no shows

- **WHEN** an authenticated caller whose `library.read` permission is `NONE` requests the collection
- **THEN** the system returns `200 OK` with an empty collection

#### Scenario: Series summary content

- **WHEN** the system returns a series in the listing
- **THEN** the item carries only the series identifier, name and cover
- **AND** the cover is the identifier of the image holding the show's artwork, or `null` when the show stores none
- **AND** it carries no seasons, genres, dates or counts

### Requirement: Series listing is filterable and paginated

The series collection SHALL accept an optional `name` filter and SHALL be paginated as `api` defines. A `name` filter
SHALL return only the shows whose name matches the supplied value.

#### Scenario: Filter and page

- **WHEN** an authenticated caller requests the series collection with `name`, `limit` and `offset`
- **THEN** the system returns only the shows whose name matches, starting at `offset` and holding at most `limit` items

#### Scenario: Out-of-range limit

- **WHEN** the caller requests the series collection with a `limit` of 0 or greater than 100
- **THEN** the system returns `422 Unprocessable Content` with a stable message

#### Scenario: Malformed query string

- **WHEN** the caller requests the series collection with a query string that cannot be decoded
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: List the episodes of a season

The system SHALL return the episodes of one season through
`GET /api/v1/library/series/{series_id}/season/{season_index}/episode`.

#### Scenario: Episodes of an existing season

- **WHEN** an authenticated caller requests the episodes of a season that belongs to a series that exists
- **THEN** the system returns `200 OK` with a collection of episode representations

#### Scenario: Episode representation content

- **WHEN** the system returns an episode in the collection
- **THEN** the representation carries its index as `id`, the series identifier, the season index, its name, plot and air
  date, and its video summary
- **AND** the air date is `null` when the episode stores none
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
