# Spec Delta

## MODIFIED Requirements

### Requirement: Series reads require an authenticated account

Every series, season and episode endpoint SHALL require a valid bearer token, and a token that names an account which no
longer exists SHALL be rejected. A caller whose `catalog.read` permission does not cover the show SHALL receive
`404 Not Found` for the show, its seasons and its episodes.

#### Scenario: Missing or invalid credentials

- **WHEN** a request reaches a series, season or episode endpoint without a valid bearer token
- **THEN** the system returns `401 Unauthorized`

#### Scenario: Account no longer exists

- **WHEN** an authenticated request reaches a series, season or episode endpoint but the caller's account no longer
  exists
- **THEN** the system returns `401 Unauthorized`

#### Scenario: An authenticated caller is never forbidden

- **WHEN** an authenticated caller with the default permission reads any series, season or episode
- **THEN** the system returns the representation and never a `403 Forbidden`

#### Scenario: A restricted caller cannot read a show

- **WHEN** an authenticated caller whose `catalog.read` permission does not cover a show requests the show, a season or
  an episode
- **THEN** the system returns `404 Not Found`, indistinguishable from an unknown show

### Requirement: List series

The system SHALL return the shows the caller's `catalog.read` permission covers through `GET /api/v1/library/series`.
The default permission covers every show for every authenticated caller; a per-user override may narrow the set. Each
listed series SHALL carry only its identifier, its name and its cover.

#### Scenario: Authenticated caller lists series

- **WHEN** an authenticated caller with the default permission requests `GET /api/v1/library/series`
- **THEN** the system returns `200 OK` with a HAL collection of series summaries

#### Scenario: A restricted caller lists only permitted shows

- **WHEN** an authenticated caller whose `catalog.read` permission covers only some shows requests the collection
- **THEN** the system returns only the shows that permission covers

#### Scenario: A caller with no catalog access lists no shows

- **WHEN** an authenticated caller whose `catalog.read` permission is `NONE` requests the collection
- **THEN** the system returns `200 OK` with an empty collection

#### Scenario: Series summary content

- **WHEN** the system returns a series in the listing
- **THEN** the item carries only the series identifier, name, cover and its `_links.self`
- **AND** the cover is the identifier of the file holding the show's artwork, or `null` when the show stores none
- **AND** it carries no seasons, genres, dates or counts
