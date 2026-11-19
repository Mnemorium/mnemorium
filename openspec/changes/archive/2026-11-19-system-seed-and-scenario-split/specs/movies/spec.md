# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-MOVIES-004 — A film is created from a video

Creating a film SHALL require the `CreateChild` action on the catalogue with the film child kind. The request SHALL name
a video, a non-empty name and optional plot, release date, country of origin, genres and cover. An unknown genre SHALL
be stored on demand. A new film SHALL answer `201 Created` with a `Location`; a film already backed by that video `200`;
a video already attached elsewhere `409`; a denied caller `403`.

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

- **WHEN** the video does not exist, the name or a genre identifier is empty, the release date is not `YYYY-MM-DD`, or
  the country of origin is not a two-letter code
- **THEN** the system returns `422 Unprocessable Content` and creates nothing

#### Scenario: SCN-MOVIES-004-F — The cover is not a library image

- **WHEN** the submitted cover is not an image the catalogue may use
- **THEN** the system returns `409 Conflict` and creates nothing

#### Scenario: SCN-MOVIES-004-G — The caller may not create a film

- **WHEN** the caller's `CreateChild` action on the catalogue refuses
- **THEN** the system returns `403 Forbidden` and creates nothing
