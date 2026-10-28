# Spec Delta

## MODIFIED Requirements

### Requirement: Movie reads require an authenticated account

Every movie endpoint SHALL require a valid bearer token, and a token that names an account which no longer exists SHALL
be rejected. A caller whose `catalog.read` permission does not cover the film SHALL receive `404 Not Found`.

#### Scenario: Missing or invalid credentials

- **WHEN** a request reaches a movie endpoint without a valid bearer token
- **THEN** the system returns `401 Unauthorized`

#### Scenario: Account no longer exists

- **WHEN** an authenticated request reaches a movie endpoint but the caller's account no longer exists
- **THEN** the system returns `401 Unauthorized`

#### Scenario: An authenticated caller is never forbidden

- **WHEN** an authenticated caller with the default permission reads any film
- **THEN** the system returns the representation and never a `403 Forbidden`

#### Scenario: A restricted caller cannot read a film

- **WHEN** an authenticated caller whose `catalog.read` permission does not cover a film requests it
- **THEN** the system returns `404 Not Found`, indistinguishable from an unknown film

### Requirement: List movies

The system SHALL return the films the caller's `catalog.read` permission covers through `GET /api/v1/library/movie`. The
default permission covers every film for every authenticated caller; a per-user override may narrow the set.

#### Scenario: Authenticated caller lists movies

- **WHEN** an authenticated caller with the default permission requests `GET /api/v1/library/movie`
- **THEN** the system returns `200 OK` with a HAL collection of movie representations

#### Scenario: A restricted caller lists only permitted films

- **WHEN** an authenticated caller whose `catalog.read` permission covers only some films requests the collection
- **THEN** the system returns only the films that permission covers

#### Scenario: A caller with no catalog access lists no films

- **WHEN** an authenticated caller whose `catalog.read` permission is `NONE` requests the collection
- **THEN** the system returns `200 OK` with an empty collection

#### Scenario: Movie representation content

- **WHEN** the system returns a movie, in a listing or on a detail read
- **THEN** the representation carries the film's identifier, name, plot, country of origin, release date, genres, video
  summary and its cover
- **AND** the cover is the identifier of the file holding the film's artwork, or `null` when the film stores none
- **AND** the video summary carries the video identifier, the backing file identifier, duration, codec, width, height
  and scan type
- **AND** the genres are the uppercase genre identifiers ordered by identifier
