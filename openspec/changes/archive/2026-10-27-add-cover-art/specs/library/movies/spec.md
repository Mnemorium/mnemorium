# Spec Delta

## MODIFIED Requirements

### Requirement: List movies

The system SHALL return the films it holds through `GET /api/v1/library/movie`. Movies SHALL NOT be scoped to an owner:
every authenticated caller sees the same films.

#### Scenario: Authenticated caller lists movies

- **WHEN** an authenticated caller requests `GET /api/v1/library/movie`
- **THEN** the system returns `200 OK` with a HAL collection of movie representations

#### Scenario: Movie representation content

- **WHEN** the system returns a movie, in a listing or on a detail read
- **THEN** the representation carries the film's identifier, name, plot, country of origin, release date, genres, video
  summary and its cover
- **AND** the cover is the identifier of the file holding the film's artwork, or `null` when the film stores none
- **AND** the video summary carries the video identifier, the backing file identifier, duration, codec, width, height
  and scan type
- **AND** the genres are the uppercase genre identifiers ordered by identifier
