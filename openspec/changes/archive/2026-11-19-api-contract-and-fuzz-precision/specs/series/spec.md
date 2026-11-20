# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-SERIES-008 — A show is created

Creating a show SHALL require the `CreateChild` action on the catalogue with the show child kind. The request SHALL name
a name that is non-empty, at most 100 characters and does not begin or end with whitespace, and optional plot, first air
date, country of origin, genres and cover; the submitted name is stored unchanged. An unknown genre SHALL be stored on
demand. A new show SHALL answer `201 Created` with a `Location`; a repeated name SHALL create a second show. Unusable
input SHALL answer `422`; a non-library cover `409`; a denied caller `403`.

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

- **WHEN** the name is empty, too long, or begins or ends with whitespace, a genre identifier is empty, the first air
  date is not `YYYY-MM-DD`, or the country is not a two-letter code
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
optional one. A supplied name SHALL be non-empty, at most 100 characters, and SHALL NOT begin or end with whitespace;
the submitted value is stored unchanged. The show's seasons and episodes SHALL NOT change. An unknown or unviewable show
SHALL answer `404`; a readable show the caller may not edit `403`; an unusable value `422` or `409`.

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

#### Scenario: SCN-SERIES-009-F — An unusable name is refused

- **WHEN** the supplied name is empty, too long, or begins or ends with whitespace
- **THEN** the system returns `422 Unprocessable Content` and applies nothing

### Requirement: REQ-SERIES-012 — An episode is created or replaced

Writing an episode SHALL require `CreateChild` on the show with the episode child kind through
`PUT /api/v1/library/series/{series_id}/season/{season_index}/episode/{episode_index}`. It SHALL take a video, a name
that is non-empty, at most 100 characters and does not begin or end with whitespace, and optional plot and air date; the
submitted name is stored unchanged. A free index SHALL create and answer `201`; an existing index SHALL replace and
answer `200`.

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

- **WHEN** the video does not exist, the name is empty, too long, or begins or ends with whitespace, or the air date is
  not `YYYY-MM-DD`
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
stored value and `null` SHALL clear the air date. A supplied name SHALL be non-empty, at most 100 characters, and SHALL
NOT begin or end with whitespace; the submitted value is stored unchanged. The episode's video SHALL NOT change. An
unknown episode SHALL answer `404`; a malformed index `400`; a denied caller `403`; an unusable name `422`.

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

#### Scenario: SCN-SERIES-013-F — An unusable name is refused

- **WHEN** the supplied name is empty, too long, or begins or ends with whitespace
- **THEN** the system returns `422 Unprocessable Content` and applies nothing
