# series Specification

## Purpose

Lets a caller browse the shows the server holds through the show → season → episode hierarchy, read each level's
metadata and video summary, create or replace a season or an episode, and update each level without changing its
children.

## Requirements

### Requirement: REQ-SERIES-001 — List shows

The system SHALL return the shows a caller may `Discover` through `GET /api/v1/library/series`, optionally filtered by
`name` and paginated as `api` defines. Each embedded item SHALL be the partial representation: show identifier, name,
cover and links. An out-of-range limit SHALL answer `422`; a malformed query `400`.

#### Scenario: SCN-SERIES-001-A — A caller lists shows

- **WHEN** an authenticated caller reads the show collection
- **THEN** the system returns `200 OK` with a collection whose `_embedded.items` carries the page's partial shows

#### Scenario: SCN-SERIES-001-B — Filter and page

- **WHEN** the caller requests the collection with `name`, `limit` and `offset`
- **THEN** the system returns only the shows whose name matches, starting at `offset` and holding at most `limit` items

#### Scenario: SCN-SERIES-001-C — The caller discovers no show

- **WHEN** the caller may discover no show
- **THEN** the system returns `200 OK` with an empty collection

#### Scenario: SCN-SERIES-001-D — A listing item is partial

- **WHEN** a show appears in the collection
- **THEN** the item carries its identifier, name, cover and links
- **AND** it carries no plot, genres, dates, seasons or counts

#### Scenario: SCN-SERIES-001-E — Out-of-range limit

- **WHEN** the caller requests a `limit` of 0 or greater than 100
- **THEN** the system returns `422 Unprocessable Content` with a stable message

#### Scenario: SCN-SERIES-001-F — Malformed query string

- **WHEN** the caller requests the collection with a query string that cannot be decoded
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: REQ-SERIES-002 — Read one show

A caller who may `View` a show SHALL read it through `GET /api/v1/library/series/{series_id}`. The representation SHALL
carry the name, plot, country of origin, first air date, genres ordered by identifier, the cover `cover-art` defines,
and season summaries ordered by index. A season summary SHALL carry its index, name when set, episode count, cover and
links.

#### Scenario: SCN-SERIES-002-A — A show of an existing identifier

- **WHEN** an authenticated caller requests a show it may view
- **THEN** the system returns `200 OK` with the show representation

#### Scenario: SCN-SERIES-002-B — A season summary is reported

- **WHEN** the show holds seasons
- **THEN** each summary carries its index, name when set, episode count, cover and links to the season and its episode
  collection

#### Scenario: SCN-SERIES-002-C — The show stores no date or country

- **WHEN** the show stores no first air date or no country of origin
- **THEN** the representation reports those fields as `null`

#### Scenario: SCN-SERIES-002-D — Unknown or unviewable show

- **WHEN** the show does not exist, or the caller may not view it
- **THEN** the system returns `404 Not Found`

#### Scenario: SCN-SERIES-002-E — Malformed show identifier

- **WHEN** the caller supplies a `series_id` that is not a valid identifier
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: REQ-SERIES-003 — Season and episode identity

A season SHALL be identified by its non-negative 32-bit index within its show, and an episode by its non-negative 32-bit
index within its season. The representation SHALL expose the index as `id`, not as a separate identifier.

#### Scenario: SCN-SERIES-003-A — The index is the identity

- **WHEN** the system returns a season or an episode
- **THEN** its `id` is its parent-scoped index

#### Scenario: SCN-SERIES-003-B — The index range is addressable

- **WHEN** a season or episode is stored at the largest 32-bit unsigned index
- **THEN** the system addresses and returns it

### Requirement: REQ-SERIES-004 — Read one season

A caller who may `View` a season SHALL read it through `GET /api/v1/library/series/{series_id}/season/{season_index}`,
scoped to the show. The representation SHALL carry the show identifier, index, name when set, episode count, effective
cover and links to the show and its episode collection. An index of another show, or an unknown show or season, SHALL
answer `404`; a malformed index `400`.

#### Scenario: SCN-SERIES-004-A — A season of an existing show

- **WHEN** an authenticated caller requests a season of a show it may view
- **THEN** the system returns `200 OK` with the season representation

#### Scenario: SCN-SERIES-004-B — The season indexes are scoped to the show

- **WHEN** two shows each hold a season with index 1
- **THEN** each show resolves its own season and the two are independent

#### Scenario: SCN-SERIES-004-C — Unknown show or season

- **WHEN** the season does not belong to the show, or either is unknown
- **THEN** the system returns `404 Not Found`

#### Scenario: SCN-SERIES-004-D — Malformed season index

- **WHEN** the caller supplies a `season_index` that is not a valid 32-bit unsigned integer
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: REQ-SERIES-005 — List the episodes of a season

The system SHALL return the episodes of one season through
`GET /api/v1/library/series/{series_id}/season/{season_index}/episode`. Each episode SHALL carry its index as `id`, the
show identifier, season index, name, plot, air date when set, the video summary `video` defines, and links to the show
and season. Unknown show or season, or a season of another show, SHALL answer `404`; malformed identifiers `400`.

#### Scenario: SCN-SERIES-005-A — The episodes of a season

- **WHEN** an authenticated caller requests the episodes of a season it may view
- **THEN** the system returns `200 OK` with the episode representations

#### Scenario: SCN-SERIES-005-B — The episode carries its fields

- **WHEN** the system returns an episode
- **THEN** it carries its index as `id`, the show identifier, season index, name, plot, air date and video summary
- **AND** the air date is `null` when the episode stores none

#### Scenario: SCN-SERIES-005-C — Unknown show or season

- **WHEN** the show or season does not exist, or the season belongs to another show
- **THEN** the system returns `404 Not Found`

#### Scenario: SCN-SERIES-005-D — Malformed identifiers

- **WHEN** the caller supplies a malformed `series_id` or `season_index`
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: REQ-SERIES-006 — Read one episode

A caller who may `View` an episode SHALL read it through
`GET /api/v1/library/series/{series_id}/season/{season_index}/episode/{episode_index}`, scoped to the show and season.
An episode of another season, or an unknown show, season or episode, SHALL answer `404`; a malformed index `400`.

#### Scenario: SCN-SERIES-006-A — An episode of the season

- **WHEN** an authenticated caller requests an episode of the named show and season
- **THEN** the system returns `200 OK` with the episode representation

#### Scenario: SCN-SERIES-006-B — The episode stores no air date

- **WHEN** the episode stores no air date
- **THEN** the representation reports `air_date` as `null`

#### Scenario: SCN-SERIES-006-C — The episode is not in the season

- **WHEN** the episode belongs to another season, or the show, season or episode is unknown
- **THEN** the system returns `404 Not Found`

#### Scenario: SCN-SERIES-006-D — Malformed episode index

- **WHEN** the caller supplies a malformed `season_index` or `episode_index`
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: REQ-SERIES-007 — Seasons and episodes are ordered by position

The show representation SHALL list seasons by ascending index, and each season's episode collection SHALL list episodes
by ascending index, so repeated reads return a stable order.

#### Scenario: SCN-SERIES-007-A — The stable order

- **WHEN** a show has several seasons, each holding several episodes
- **THEN** the show lists its seasons by ascending index
- **AND** each season's episode collection lists its episodes by ascending index

### Requirement: REQ-SERIES-008 — A show is created

Creating a show SHALL require the `CreateChild` action on the catalogue with the show child kind. The request SHALL name
a non-empty name and optional plot, first air date, country of origin, genres and cover. An unknown genre SHALL be
stored on demand. A new show SHALL answer `201 Created` with a `Location`; a repeated name SHALL create a second show.
Unusable input SHALL answer `422`; a non-library cover `409`; a denied caller `403`.

#### Scenario: SCN-SERIES-008-A — A new show

- **WHEN** a caller that may create a show submits a name
- **THEN** the system creates the show with no seasons and returns `201 Created` with the show representation and a
  `Location`

#### Scenario: SCN-SERIES-008-B — A repeated name

- **WHEN** the caller submits a name another show already uses
- **THEN** the system creates a second show rather than returning the first

#### Scenario: SCN-SERIES-008-C — An unknown genre is stored

- **WHEN** the submitted genre identifier is not in the catalogue yet
- **THEN** the system stores the genre and links it to the show

#### Scenario: SCN-SERIES-008-D — The input is unusable

- **WHEN** the name is empty, a genre identifier is empty, the first air date is not `YYYY-MM-DD`, or the country is not
  a two-letter code
- **THEN** the system returns `422 Unprocessable Content` and creates nothing

#### Scenario: SCN-SERIES-008-E — The cover is not a library image

- **WHEN** the submitted cover is not an image the catalogue may use
- **THEN** the system returns `409 Conflict` and creates nothing

#### Scenario: SCN-SERIES-008-F — The caller may not create a show

- **WHEN** the caller's `CreateChild` action on the catalogue refuses
- **THEN** the system returns `403 Forbidden` and creates nothing

### Requirement: REQ-SERIES-009 — Show metadata is updated partially

Updating a show SHALL require `View` and then `EditMetadata` on the show through
`PATCH /api/v1/library/series/{series_id}`. An absent field SHALL keep its stored value and `null` SHALL clear an
optional one. The show's seasons and episodes SHALL NOT change. An unknown or unviewable show SHALL answer `404`; a
readable show the caller may not edit `403`; an unusable value `422` or `409`.

#### Scenario: SCN-SERIES-009-A — The metadata changes

- **WHEN** a caller that may edit the show submits a name, plot, country, first air date, genre list or cover
- **THEN** the system stores the named values and returns the show representation

#### Scenario: SCN-SERIES-009-B — An optional value is cleared

- **WHEN** the caller sends `first_air_date: null`, `country_of_origin: null` or `cover_image_id: null`
- **THEN** the show stores no value for that field and the representation reports `null`

#### Scenario: SCN-SERIES-009-C — The children are untouched

- **WHEN** a show with seasons and episodes is updated
- **THEN** every season and episode keeps its stored values

#### Scenario: SCN-SERIES-009-D — The caller may read but not edit

- **WHEN** the caller may view the show but `EditMetadata` refuses
- **THEN** the system returns `403 Forbidden` and applies nothing

#### Scenario: SCN-SERIES-009-E — The show is unknown or unviewable

- **WHEN** the show does not exist, or the caller may not view it
- **THEN** the system returns `404 Not Found` and applies nothing

### Requirement: REQ-SERIES-010 — A season is created or replaced

Writing a season SHALL require `CreateChild` on the show with the season child kind through
`PUT /api/v1/library/series/{series_id}/season/{season_index}`. A free index SHALL answer `201 Created`; an existing
index SHALL replace the name and cover, keep the episodes and answer `200 OK`. An unknown show `404`; an out-of-range
index `400`; a non-library cover `409`; a denied caller `403`.

#### Scenario: SCN-SERIES-010-A — A season is created

- **WHEN** a caller that may write the season puts a name at a free index
- **THEN** the system creates the season with no episodes and returns `201 Created`

#### Scenario: SCN-SERIES-010-B — A season is replaced

- **WHEN** the caller puts a new name or cover at an existing index
- **THEN** the system stores them, keeps the season's episodes and returns `200 OK`

#### Scenario: SCN-SERIES-010-C — The episodes are kept

- **WHEN** an existing season with episodes is replaced
- **THEN** every episode keeps its stored values

#### Scenario: SCN-SERIES-010-D — The show is unknown or the index is out of range

- **WHEN** the show does not exist, or the index is outside the 32-bit unsigned range
- **THEN** the system returns `404 Not Found` or `400 Bad Request` and applies nothing

#### Scenario: SCN-SERIES-010-E — The cover is not a library image

- **WHEN** the submitted cover is not an image the catalogue may use
- **THEN** the system returns `409 Conflict` and applies nothing

#### Scenario: SCN-SERIES-010-F — The caller may not write the season

- **WHEN** the caller's `CreateChild` action on the show refuses
- **THEN** the system returns `403 Forbidden` and applies nothing

### Requirement: REQ-SERIES-011 — Season metadata is updated partially

Updating a season SHALL require `View` and then `EditMetadata` on the show through
`PATCH /api/v1/library/series/{series_id}/season/{season_index}`. An absent field SHALL keep its stored value and `null`
SHALL clear the name or cover. The season's episodes SHALL NOT change. An unknown or malformed season SHALL answer `404`
or `400`; a denied caller `403`.

#### Scenario: SCN-SERIES-011-A — The season is renamed or re-covered

- **WHEN** a caller that may edit the show submits a name or cover
- **THEN** the system stores it and returns the season representation

#### Scenario: SCN-SERIES-011-B — The name or cover is cleared

- **WHEN** the caller sends `name: null` or `cover_image_id: null`
- **THEN** the season stores no value for that field and the representation reports `null`

#### Scenario: SCN-SERIES-011-C — The episodes are untouched

- **WHEN** a season with episodes is updated
- **THEN** every episode keeps its stored values

#### Scenario: SCN-SERIES-011-D — The season is unknown or malformed

- **WHEN** the index does not belong to the show, or is not a valid 32-bit unsigned integer
- **THEN** the system returns `404 Not Found` or `400 Bad Request` and applies nothing

#### Scenario: SCN-SERIES-011-E — The caller may not edit the season

- **WHEN** the caller may view the show but `EditMetadata` refuses
- **THEN** the system returns `403 Forbidden` and applies nothing

### Requirement: REQ-SERIES-012 — An episode is created or replaced

Writing an episode SHALL require `CreateChild` on the show with the episode child kind through
`PUT /api/v1/library/series/{series_id}/season/{season_index}/episode/{episode_index}`. It SHALL take a video, a
non-empty name and optional plot and air date. A free index SHALL create and answer `201`; an existing index SHALL
replace and answer `200`.

#### Scenario: SCN-SERIES-012-A — An episode is created

- **WHEN** a caller that may write the episode puts a video and a name at a free index
- **THEN** the system creates the episode and returns `201 Created` with the episode representation

#### Scenario: SCN-SERIES-012-B — An episode is replaced

- **WHEN** the caller puts new metadata at an existing index
- **THEN** the system stores it, keeps the episode address and returns `200 OK`

#### Scenario: SCN-SERIES-012-C — The video is replaced

- **WHEN** the request names a different video for an existing episode
- **THEN** the episode stores the new video and the previous video becomes unattached

#### Scenario: SCN-SERIES-012-D — The video is already attached

- **WHEN** the submitted video backs another film or episode
- **THEN** the system returns `409 Conflict` and applies nothing

#### Scenario: SCN-SERIES-012-E — The video or input is unusable

- **WHEN** the video does not exist, the name is empty, or the air date is not `YYYY-MM-DD`
- **THEN** the system returns `422 Unprocessable Content` and applies nothing

#### Scenario: SCN-SERIES-012-F — The show or season is unknown or the index is out of range

- **WHEN** the show or season does not exist, or an index is outside the 32-bit unsigned range
- **THEN** the system returns `404 Not Found` or `400 Bad Request` and applies nothing

#### Scenario: SCN-SERIES-012-G — The caller may not write the episode

- **WHEN** the caller's `CreateChild` action on the show refuses
- **THEN** the system returns `403 Forbidden` and applies nothing

### Requirement: REQ-SERIES-013 — Episode metadata is updated partially

Updating an episode SHALL require `View` and then `EditMetadata` on the show through
`PATCH /api/v1/library/series/{series_id}/season/{season_index}/episode/{episode_index}`. An absent field SHALL keep its
stored value and `null` SHALL clear the air date. The episode's video SHALL NOT change. An unknown episode SHALL answer
`404`; a malformed index `400`; a denied caller `403`.

#### Scenario: SCN-SERIES-013-A — The metadata changes

- **WHEN** a caller that may edit the show submits a name, plot or air date
- **THEN** the system stores the named values and returns the episode representation

#### Scenario: SCN-SERIES-013-B — The air date is cleared

- **WHEN** the caller sends `air_date: null`
- **THEN** the episode stores no air date and the representation reports `null`

#### Scenario: SCN-SERIES-013-C — The video is untouched

- **WHEN** an episode with a backing video is updated
- **THEN** the video keeps its stored value

#### Scenario: SCN-SERIES-013-D — The episode is unknown or the caller may not edit

- **WHEN** the episode does not exist, or the caller may view the show but `EditMetadata` refuses
- **THEN** the system returns `404 Not Found` or `403 Forbidden` and applies nothing

#### Scenario: SCN-SERIES-013-E — The index is malformed

- **WHEN** the caller supplies a malformed `season_index` or `episode_index`
- **THEN** the system returns `400 Bad Request` and applies nothing
