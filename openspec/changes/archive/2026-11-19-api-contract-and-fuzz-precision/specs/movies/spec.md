# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-MOVIES-004 — A film is created from a video

Creating a film SHALL require the `CreateChild` action on the catalogue with the film child kind. The request SHALL name
a video, a name that is non-empty, at most 100 characters and does not begin or end with whitespace, and optional plot,
release date, country of origin, genres and cover; the submitted name is stored unchanged. An unknown genre SHALL be
stored on demand. A new film SHALL answer `201 Created` with a `Location`; a film already backed by that video `200`; a
video already attached elsewhere `409`; a denied caller `403`.

#### Scenario: SCN-MOVIES-004-A — A new film

- **WHEN** a caller that may create a film submits a name and a video no film or episode backs
- **THEN** the system creates the film and returns `201 Created` with the film representation and a `Location`

#### Scenario: SCN-MOVIES-004-B — A film already backs the video

- **WHEN** the submitted video already backs a film
- **THEN** the system returns that film unchanged with `200 OK` and applies no change

#### Scenario: SCN-MOVIES-004-C — The video is already attached

- **WHEN** the submitted video backs another film or an episode
- **THEN** the system returns `409 Conflict` and creates nothing

#### Scenario: SCN-MOVIES-004-D — An unknown genre is stored

- **WHEN** the submitted genre identifier is not in the catalogue yet
- **THEN** the system stores the genre and links it to the film

#### Scenario: SCN-MOVIES-004-E — The video, name, genre or date is unusable

- **WHEN** the video does not exist, the name is empty, too long, or begins or ends with whitespace, a genre identifier
  is empty, the release date is not `YYYY-MM-DD`, or the country of origin is not a two-letter code
- **THEN** the system returns `422 Unprocessable Content` and creates nothing

#### Scenario: SCN-MOVIES-004-F — The cover is not a library image

- **WHEN** the submitted cover is not an image the catalogue may use
- **THEN** the system returns `409 Conflict` and creates nothing

#### Scenario: SCN-MOVIES-004-G — The caller may not create a film

- **WHEN** the caller's `CreateChild` action on the catalogue refuses
- **THEN** the system returns `403 Forbidden` and creates nothing

### Requirement: REQ-MOVIES-005 — Film metadata is updated partially

Updating a film SHALL require `View` and then `EditMetadata` on the film through
`PATCH /api/v1/library/movie/{movie_id}`. An absent field SHALL keep its stored value and `null` SHALL clear an optional
one. A supplied name SHALL be non-empty, at most 100 characters, and SHALL NOT begin or end with whitespace; the
submitted value is stored unchanged. The backing video SHALL NOT change. An unknown or unviewable film SHALL answer
`404`; a readable film the caller may not edit `403`; an unusable value `422` or `409`.

#### Scenario: SCN-MOVIES-005-A — The metadata changes

- **WHEN** a caller that may edit the film submits a name, plot, country of origin, release date, genre list or cover
- **THEN** the system stores the named values and returns the film representation

#### Scenario: SCN-MOVIES-005-B — An optional value is cleared

- **WHEN** the caller sends `release_date: null`, `country_of_origin: null` or `cover_image_id: null`
- **THEN** the film stores no value for that field and the representation reports `null`

#### Scenario: SCN-MOVIES-005-C — The backing video does not change

- **WHEN** the caller submits any combination of fields
- **THEN** the film's video identifier is the one it held before the update

#### Scenario: SCN-MOVIES-005-D — The caller may read but not edit

- **WHEN** the caller may view the film but `EditMetadata` refuses
- **THEN** the system returns `403 Forbidden` and applies nothing

#### Scenario: SCN-MOVIES-005-E — The film is unknown or unviewable

- **WHEN** the film does not exist, or the caller may not view it
- **THEN** the system returns `404 Not Found` and applies nothing

#### Scenario: SCN-MOVIES-005-F — The film identifier is malformed

- **WHEN** the supplied `movie_id` is not a valid identifier
- **THEN** the system returns `400 Bad Request` and applies nothing

#### Scenario: SCN-MOVIES-005-G — An unusable name is refused

- **WHEN** the supplied name is empty, too long, or begins or ends with whitespace
- **THEN** the system returns `422 Unprocessable Content` and applies nothing
