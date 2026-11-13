# movies Specification

## Purpose

Lets a caller browse the films the server holds, read one film's metadata and video summary, create a film from an
unused video, and update its metadata without changing that video.

## Requirements

### Requirement: REQ-MOVIES-001 — List films

The system SHALL return the films a caller may `Discover` through `GET /api/v1/library/movie`, optionally filtered by
`name` and paginated as `api` defines. Each embedded item SHALL be the partial representation: film identifier, name,
cover and links. An out-of-range limit SHALL answer `422`; a malformed query `400`.

#### Scenario: SCN-MOVIES-001-A — A caller lists films

- **WHEN** an authenticated caller reads the film collection
- **THEN** the system returns `200 OK` with a collection whose `_embedded.items` carries the page's partial films

#### Scenario: SCN-MOVIES-001-B — Filter and page

- **WHEN** the caller requests the collection with `name`, `limit` and `offset`
- **THEN** the system returns only the films whose name matches, starting at `offset` and holding at most `limit` items

#### Scenario: SCN-MOVIES-001-C — The caller discovers no film

- **WHEN** the caller may discover no film
- **THEN** the system returns `200 OK` with an empty collection

#### Scenario: SCN-MOVIES-001-D — A listing item is partial

- **WHEN** a film appears in the collection
- **THEN** the item carries its identifier, name, cover and links
- **AND** it carries no plot, genre list or video summary

#### Scenario: SCN-MOVIES-001-E — Out-of-range limit

- **WHEN** the caller requests a `limit` of 0 or greater than 100
- **THEN** the system returns `422 Unprocessable Content` with a stable message

#### Scenario: SCN-MOVIES-001-F — Malformed query string

- **WHEN** the caller requests the collection with a query string that cannot be decoded
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: REQ-MOVIES-002 — Films are ordered by identifier

The system SHALL order the film collection by ascending identifier so repeated reads return a stable order.

#### Scenario: SCN-MOVIES-002-A — Stable listing order

- **WHEN** the film collection holds several films
- **THEN** the collection lists them by ascending identifier, independent of the order in which they were stored

### Requirement: REQ-MOVIES-003 — Read one film

A caller who may `View` a film SHALL read it through `GET /api/v1/library/movie/{movie_id}`. The representation SHALL
carry the identifier, name, plot, country of origin, release date, genres ordered by identifier, the video summary
`video` defines and the cover `cover-art` defines. Country and release date SHALL be null when unset.

#### Scenario: SCN-MOVIES-003-A — A film of an existing identifier

- **WHEN** an authenticated caller requests a film it may view
- **THEN** the system returns `200 OK` with the film representation

#### Scenario: SCN-MOVIES-003-B — The film stores no date or country

- **WHEN** the film stores no release date or no country of origin
- **THEN** the representation reports `release_date` and `country_of_origin` as `null`

#### Scenario: SCN-MOVIES-003-C — Unknown or unviewable film

- **WHEN** the film does not exist, or the caller may not view it
- **THEN** the system returns `404 Not Found`

#### Scenario: SCN-MOVIES-003-D — Malformed film identifier

- **WHEN** the caller supplies a `movie_id` that is not a valid identifier
- **THEN** the system returns `400 Bad Request` with a stable message

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

### Requirement: REQ-MOVIES-005 — Film metadata is updated partially

Updating a film SHALL require `View` and then `EditMetadata` on the film through
`PATCH /api/v1/library/movie/{movie_id}`. An absent field SHALL keep its stored value and `null` SHALL clear an optional
one. The backing video SHALL NOT change. An unknown or unviewable film SHALL answer `404`; a readable film the caller
may not edit `403`; an unusable value `422` or `409`.

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
