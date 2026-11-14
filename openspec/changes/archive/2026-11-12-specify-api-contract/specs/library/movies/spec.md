# Spec Delta

## REMOVED Requirements

### Requirement: Movie responses are HAL representations

**Reason**: The `api` capability now owns the response media type, the payload shape and `_links.self` for every
representation (`REQ-API-003`, `REQ-API-004`). Restating the rule here duplicated it and let the copies drift.
**Migration**: No client action; the served behavior is unchanged. The rule is read from `openspec/specs/api/spec.md`.

## MODIFIED Requirements

### Requirement: List movies

The system SHALL return the films the caller's `library.read` permission covers through `GET /api/v1/library/movie`. The
default permission covers every film for every authenticated caller; a per-user override may narrow the set. The
collection SHALL embed each page item's partial representation under `_embedded.items`; the item carries the film's
identifier, name and cover, and the detail read returns the full representation.

#### Scenario: Authenticated caller lists movies

- **WHEN** an authenticated caller with the default permission requests `GET /api/v1/library/movie`
- **THEN** the system returns `200 OK` with a collection whose `_embedded.items` carries the page's partial film
  representations

#### Scenario: A restricted caller lists only permitted films

- **WHEN** an authenticated caller whose `library.read` permission covers only some films requests the collection
- **THEN** the system returns only the films that permission covers

#### Scenario: A caller with no catalog access lists no films

- **WHEN** an authenticated caller whose `library.read` permission is `NONE` requests the collection
- **THEN** the system returns `200 OK` with an empty collection

#### Scenario: Movie representation content

- **WHEN** the system returns a film on a detail read
- **THEN** the representation carries the film's identifier, name, plot, country of origin, release date, genres, video
  summary and its cover
- **AND** the country of origin and the release date are `null` when the film stores none
- **AND** the cover is the identifier of the image holding the film's artwork, or `null` when the film stores none
- **AND** the video summary carries the video identifier, the backing file identifier, duration, codec, width, height
  and scan type
- **AND** the genres are the uppercase genre identifiers ordered by identifier

#### Scenario: A listing item is partial

- **WHEN** a film appears in the catalog listing
- **THEN** the item carries its identifier, name, cover image identifier, `self` link and `cover` link when artwork is
  set
- **AND** it carries no plot, genre list or video summary

### Requirement: Movie listing is filterable and paginated

The movie collection SHALL accept an optional `name` filter and SHALL be paginated as `api` defines. A `name` filter
SHALL return only the films whose name matches the supplied value.

#### Scenario: Filter and page

- **WHEN** an authenticated caller requests the movie collection with `name`, `limit` and `offset`
- **THEN** the system returns only the films whose name matches, starting at `offset` and holding at most `limit` items

#### Scenario: Out-of-range limit

- **WHEN** the caller requests the movie collection with a `limit` of 0 or greater than 100
- **THEN** the system returns `422 Unprocessable Content` with a stable message

#### Scenario: Malformed query string

- **WHEN** the caller requests the movie collection with a query string that cannot be decoded
- **THEN** the system returns `400 Bad Request` with a stable message
