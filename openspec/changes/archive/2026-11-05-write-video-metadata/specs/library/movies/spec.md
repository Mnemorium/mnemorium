# Spec Delta

## MODIFIED Requirements

### Requirement: List movies

The system SHALL return the films the caller's `library.read` permission covers through `GET /api/v1/library/movie`. The
default permission covers every film for every authenticated caller; a per-user override may narrow the set.

#### Scenario: Authenticated caller lists movies

- **WHEN** an authenticated caller with the default permission requests `GET /api/v1/library/movie`
- **THEN** the system returns `200 OK` with a HAL collection of movie representations

#### Scenario: A restricted caller lists only permitted films

- **WHEN** an authenticated caller whose `library.read` permission covers only some films requests the collection
- **THEN** the system returns only the films that permission covers

#### Scenario: A caller with no catalog access lists no films

- **WHEN** an authenticated caller whose `library.read` permission is `NONE` requests the collection
- **THEN** the system returns `200 OK` with an empty collection

#### Scenario: Movie representation content

- **WHEN** the system returns a movie, in a listing or on a detail read
- **THEN** the representation carries the film's identifier, name, plot, country of origin, release date, genres, video
  summary and its cover
- **AND** the country of origin and the release date are `null` when the film stores none
- **AND** the cover is the identifier of the image holding the film's artwork, or `null` when the film stores none
- **AND** the video summary carries the video identifier, the backing file identifier, duration, codec, width, height
  and scan type
- **AND** the genres are the uppercase genre identifiers ordered by identifier

### Requirement: Read one movie

The system SHALL return one film through `GET /api/v1/library/movie/{movie_id}`.

#### Scenario: Movie of an existing identifier

- **WHEN** an authenticated caller requests a film identifier that exists
- **THEN** the system returns `200 OK` with the movie representation

#### Scenario: Film without a date or country

- **WHEN** the requested film stores no release date or no country of origin
- **THEN** the representation reports `release_date` and `country_of_origin` as `null`

#### Scenario: Unknown movie

- **WHEN** the requested film identifier is unknown
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed movie identifier

- **WHEN** the caller supplies a `movie_id` that is not a valid identifier
- **THEN** the system returns `400 Bad Request` with a stable message

## ADDED Requirements

### Requirement: A film is created from a video

The system SHALL create a film through `POST /api/v1/library/movie` from a `video_id`, a non-empty name, and optional
plot, release date, country of origin, genres and cover image. A genre identifier the catalogue does not hold SHALL be
stored on demand. A new film SHALL return `201 Created` with a `Location`; a film already backed by that video SHALL be
returned unchanged with `200 OK`. The write SHALL require `library.manage` over the whole catalogue.

#### Scenario: A new film

- **WHEN** an administrator submits a name and a video that no film or episode backs
- **THEN** the system creates the film and returns `201 Created` with the film representation and a `Location`

#### Scenario: A film already backed by the video

- **WHEN** the submitted video already backs a film
- **THEN** the system returns that film unchanged with `200 OK` and applies no change

#### Scenario: The video is already attached

- **WHEN** the submitted video backs another film or an episode
- **THEN** the system returns `409 Conflict` and creates nothing

#### Scenario: An unknown genre is stored

- **WHEN** the submitted genre identifier is not in the catalogue yet
- **THEN** the system stores the genre and links it to the film

#### Scenario: Unknown video, empty name or invalid cover

- **WHEN** the video does not exist, the name is empty, a genre identifier is empty, or the cover is not a library image
- **THEN** the system returns `422 Unprocessable Content` or `409 Conflict` and creates nothing

#### Scenario: Malformed release date

- **WHEN** the submitted release date does not use `YYYY-MM-DD`
- **THEN** the system returns `422 Unprocessable Content` and creates nothing

### Requirement: Film metadata is updated partially

The system SHALL update a film through `PATCH /api/v1/library/movie/{movie_id}`. An absent field SHALL keep its stored
value and a `null` SHALL clear an optional one. The film's backing video SHALL NOT change. The write SHALL require
`library.manage` on the film.

#### Scenario: Change the metadata

- **WHEN** an administrator submits a name, plot, country of origin, release date, genre list or cover
- **THEN** the system stores the named values and returns the film representation

#### Scenario: Clear an optional value

- **WHEN** an administrator sends `release_date: null`, `country_of_origin: null` or `cover_image_id: null`
- **THEN** the film stores no value for that field and the representation reports `null`

#### Scenario: The film is not visible

- **WHEN** the caller's `library.read` permission does not cover the film
- **THEN** the system returns `404 Not Found`, indistinguishable from an unknown film

#### Scenario: The film is readable but not manageable

- **WHEN** the caller may read the film but `library.manage` does not cover it
- **THEN** the system returns `403 Forbidden` and applies nothing

#### Scenario: Unknown or malformed film

- **WHEN** the film identifier is unknown, or is not a valid identifier
- **THEN** the system returns `404 Not Found` or `400 Bad Request` and applies nothing

### Requirement: External metadata is applied to a film

The system SHALL resolve a provider reference and write a film through `POST /api/v1/library/movie/{movie_id}/apply`. It
SHALL write the name, plot, release date, country of origin and genres, SHALL clear each of those the provider does not
report, and SHALL leave the cover and the backing video unchanged. The write SHALL require `library.manage` on the film.

#### Scenario: Apply the reported metadata

- **WHEN** an administrator applies a reference the provider resolves to a film
- **THEN** the system stores the film's name, plot, release date, country of origin and genres and returns the film
  representation

#### Scenario: The provider reports an absent value

- **WHEN** the provider reports no release date, no country of origin or no genres
- **THEN** the film stores `null` or an empty genre list for that field

#### Scenario: The cover and the video are untouched

- **WHEN** a film with a cover and a backing video is applied
- **THEN** both keep their stored values

#### Scenario: The reference is unknown or resolves another kind

- **WHEN** the provider does not know the reference, or resolves it to a show or an episode
- **THEN** the system returns `404 Not Found` or `422 Unprocessable Content` and writes nothing

#### Scenario: The provider fails

- **WHEN** the configured provider fails while resolving the reference
- **THEN** the system reports the failure and writes nothing
