# Spec Delta

## Purpose

Lets a client browse the films the server holds and read each film's metadata, genres and video summary.

## ADDED Requirements

### Requirement: Movie reads require an authenticated account

Every movie endpoint SHALL require a valid bearer token, and a token that names an account which no longer exists SHALL
be rejected.

#### Scenario: Missing or invalid credentials

- **WHEN** a request reaches a movie endpoint without a valid bearer token
- **THEN** the system returns `401 Unauthorized`

#### Scenario: Account no longer exists

- **WHEN** an authenticated request reaches a movie endpoint but the caller's account no longer exists
- **THEN** the system returns `401 Unauthorized`

#### Scenario: An authenticated caller is never forbidden

- **WHEN** an authenticated caller reads any movie
- **THEN** the system returns the representation and never a `403 Forbidden`

### Requirement: List movies

The system SHALL return the films it holds through `GET /api/v1/library/movie`. Movies SHALL NOT be scoped to an owner:
every authenticated caller sees the same films.

#### Scenario: Authenticated caller lists movies

- **WHEN** an authenticated caller requests `GET /api/v1/library/movie`
- **THEN** the system returns `200 OK` with a HAL collection of movie representations

#### Scenario: Movie representation content

- **WHEN** the system returns a movie
- **THEN** the representation carries the film's identifier, name, plot, country of origin, release date, genres and
  video summary
- **AND** the video summary carries the video identifier, the backing file identifier, duration, codec, width, height
  and scan type
- **AND** the genres are the uppercase genre identifiers ordered by identifier

### Requirement: Movie listing is filterable and paginated

The movie collection SHALL accept an optional `name` filter and optional `limit` and `offset` pagination. `limit` SHALL
default to 20 and SHALL be between 1 and 100.

#### Scenario: Filter and page

- **WHEN** an authenticated caller requests the movie collection with `name`, `limit` and `offset`
- **THEN** the system returns only the films whose name matches, starting at `offset` and holding at most `limit` items
- **AND** the collection's `_links.self` preserves the active filter and page

#### Scenario: Out-of-range limit

- **WHEN** the caller requests the movie collection with a `limit` of 0 or greater than 100
- **THEN** the system returns `422 Unprocessable Content` with a stable message

#### Scenario: Malformed query string

- **WHEN** the caller requests the movie collection with a query string that cannot be decoded
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: Read one movie

The system SHALL return one film through `GET /api/v1/library/movie/{movie_id}`.

#### Scenario: Movie of an existing identifier

- **WHEN** an authenticated caller requests a film identifier that exists
- **THEN** the system returns `200 OK` with the movie representation

#### Scenario: Unknown movie

- **WHEN** the requested film identifier is unknown
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed movie identifier

- **WHEN** the caller supplies a `movie_id` that is not a valid identifier
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: Movies are ordered by identifier

The system SHALL order the movie collection by ascending film identifier so repeated reads return a stable order.

#### Scenario: Stable listing order

- **WHEN** the movie collection holds several films
- **THEN** the collection lists them by ascending identifier, independent of the order in which they were stored

### Requirement: Movie responses are HAL representations

Movie success responses SHALL be served as `application/hal+json`, keep business data at the root of the payload, and
carry a `_links.self` naming the film's canonical URI.

#### Scenario: Single movie representation is HAL

- **WHEN** the system returns one film
- **THEN** the response is served as `application/hal+json` with business data at the root
- **AND** the representation carries `_links.self` naming the film's canonical URI

#### Scenario: Movie collection is HAL

- **WHEN** the system returns the movie collection
- **THEN** the response is served as `application/hal+json`
- **AND** the collection carries `_links.self` and a templated `_links.search` exposing the supported filters
