# Spec Delta

## ADDED Requirements

### Requirement: Identification reads require library read access

The video identification routes SHALL require a valid bearer token and the caller's `library.read` permission. They read
a provider rather than the catalogue, so they address no single film or show; a caller the library is closed to SHALL
receive `404 Not Found`.

#### Scenario: Missing or invalid credentials

- **WHEN** a request reaches a video identification route without a valid bearer token
- **THEN** the system returns `401 Unauthorized`

#### Scenario: Account no longer exists

- **WHEN** an authenticated request reaches a video identification route but the caller's account no longer exists
- **THEN** the system returns `401 Unauthorized`

#### Scenario: The library is closed to the caller

- **WHEN** the caller's `library.read` permission is `NONE` or `OWNED`
- **THEN** the system returns `404 Not Found`

#### Scenario: The caller may read the library

- **WHEN** the caller's `library.read` permission reaches the library, including a `LISTED` scope
- **THEN** the system answers the identification route

### Requirement: A media name is identified through a kind-scoped read

The system SHALL expose name identification through `GET /api/v1/library/movie/identify` and
`GET /api/v1/library/series/identify`, each taking a non-empty `name`. The movie route SHALL search only film queries
and return only film candidates; the series route SHALL search only show queries and return only show candidates. An
empty name SHALL be rejected.

#### Scenario: A film name

- **WHEN** an authenticated caller submits a name under the movie route
- **THEN** the system searches the ordered film queries and returns the candidates of the first query that yields any

#### Scenario: A show name

- **WHEN** an authenticated caller submits a name under the series route
- **THEN** the system searches the ordered show queries and returns the candidates of the first query that yields any

#### Scenario: A name supports both kinds

- **WHEN** the interpreted queries include film and show hypotheses
- **THEN** the movie route searches and returns only the film hypotheses, and the series route only the show hypotheses

#### Scenario: An empty name

- **WHEN** the submitted name is empty
- **THEN** the system returns `422 Unprocessable Content`

#### Scenario: No candidate

- **WHEN** at least one provider is configured and none yields a candidate for the searched queries
- **THEN** the system returns an empty candidate list

#### Scenario: No provider is configured

- **WHEN** no video metadata provider is configured
- **THEN** the system reports identification as unavailable rather than as no candidate

### Requirement: A reference resolves to a metadata document

The system SHALL resolve a provider reference through `GET /api/v1/library/movie/identify/{provider}/{reference}` and
`GET /api/v1/library/series/identify/{provider}/{reference}`. It SHALL ask only the provider named in the path, SHALL
answer `404 Not Found` for a provider it does not serve or a reference the provider does not know, and SHALL return an
`available: false` document when no provider is configured.

#### Scenario: A film reference

- **WHEN** an authenticated caller resolves a film reference through the movie route
- **THEN** the system returns the provider-independent film metadata document

#### Scenario: A show reference

- **WHEN** an authenticated caller resolves a show reference through the series route without a position
- **THEN** the system returns the provider-independent show metadata document

#### Scenario: The reference names another provider

- **WHEN** the reference's provider differs from the provider named in the path
- **THEN** the system does not ask the other provider

#### Scenario: Unknown provider or reference

- **WHEN** the provider is not one the server serves for video metadata, or the provider does not know the reference
- **THEN** the system returns `404 Not Found`

#### Scenario: No provider is configured

- **WHEN** no video metadata provider is configured
- **THEN** the system returns an `available: false` document rather than a metadata document

#### Scenario: The provider fails

- **WHEN** the named provider fails while resolving the reference
- **THEN** the system reports the failure and does not ask another provider

### Requirement: An episode is identified by its show reference and position

The series reference route SHALL accept optional `season` and `episode` query parameters, which MUST be supplied
together. With both, the system SHALL resolve the addressed episode through the show's reference; with neither, it SHALL
resolve the show. Supplying only one SHALL be rejected.

#### Scenario: An episode position

- **WHEN** an authenticated caller supplies both `season` and `episode`
- **THEN** the system resolves that episode of the referenced show and returns the episode metadata document

#### Scenario: No episode position

- **WHEN** the caller supplies neither `season` nor `episode`
- **THEN** the system returns the show metadata document

#### Scenario: A partial position

- **WHEN** the caller supplies only one of `season` and `episode`
- **THEN** the system returns `422 Unprocessable Content`

#### Scenario: An out-of-range position

- **WHEN** a supplied position is not a 32-bit unsigned integer
- **THEN** the system returns `422 Unprocessable Content`

#### Scenario: The provider reports another position

- **WHEN** the provider resolves the reference to an episode with a season or episode position other than the request
- **THEN** the system rejects the response rather than returning that episode's metadata
