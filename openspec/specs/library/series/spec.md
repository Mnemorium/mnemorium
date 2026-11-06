# library/series Specification

## Purpose

Lets a client browse the shows the server holds through the series → season → episode hierarchy, reading each level's
metadata and the video summary of each episode.

## Requirements

### Requirement: Series reads require an authenticated account

Every series, season and episode endpoint SHALL require a valid bearer token, and a token that names an account which no
longer exists SHALL be rejected. A caller whose `library.read` permission does not cover the show SHALL receive
`404 Not Found` for the show, its seasons and its episodes.

#### Scenario: Missing or invalid credentials

- **WHEN** a request reaches a series, season or episode endpoint without a valid bearer token
- **THEN** the system returns `401 Unauthorized`

#### Scenario: Account no longer exists

- **WHEN** an authenticated request reaches a series, season or episode endpoint but the caller's account no longer
  exists
- **THEN** the system returns `401 Unauthorized`

#### Scenario: An authenticated caller is never forbidden

- **WHEN** an authenticated caller with the default permission reads any series, season or episode
- **THEN** the system returns the representation and never a `403 Forbidden`

#### Scenario: A restricted caller cannot read a show

- **WHEN** an authenticated caller whose `library.read` permission does not cover a show requests the show, a season or
  an episode
- **THEN** the system returns `404 Not Found`, indistinguishable from an unknown show

### Requirement: List series

The system SHALL return the shows the caller's `library.read` permission covers through `GET /api/v1/library/series`.
The default permission covers every show for every authenticated caller; a per-user override may narrow the set. Each
listed series SHALL carry only its identifier, its name and its cover.

#### Scenario: Authenticated caller lists series

- **WHEN** an authenticated caller with the default permission requests `GET /api/v1/library/series`
- **THEN** the system returns `200 OK` with a HAL collection of series summaries

#### Scenario: A restricted caller lists only permitted shows

- **WHEN** an authenticated caller whose `library.read` permission covers only some shows requests the collection
- **THEN** the system returns only the shows that permission covers

#### Scenario: A caller with no catalog access lists no shows

- **WHEN** an authenticated caller whose `library.read` permission is `NONE` requests the collection
- **THEN** the system returns `200 OK` with an empty collection

#### Scenario: Series summary content

- **WHEN** the system returns a series in the listing
- **THEN** the item carries only the series identifier, name, cover and its `_links.self`
- **AND** the cover is the identifier of the image holding the show's artwork, or `null` when the show stores none
- **AND** it carries no seasons, genres, dates or counts

### Requirement: Series listing is filterable and paginated

The series collection SHALL accept an optional `name` filter and optional `limit` and `offset` pagination. `limit` SHALL
default to 20 and SHALL be between 1 and 100.

#### Scenario: Filter and page

- **WHEN** an authenticated caller requests the series collection with `name`, `limit` and `offset`
- **THEN** the system returns only the shows whose name matches, starting at `offset` and holding at most `limit` items
- **AND** the collection's `_links.self` preserves the active filter and page

#### Scenario: Out-of-range limit

- **WHEN** the caller requests the series collection with a `limit` of 0 or greater than 100
- **THEN** the system returns `422 Unprocessable Content` with a stable message

#### Scenario: Malformed query string

- **WHEN** the caller requests the series collection with a query string that cannot be decoded
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: Read one series

The system SHALL return one show through `GET /api/v1/library/series/{series_id}`, carrying its metadata, genres, cover
and season summaries.

#### Scenario: Series of an existing identifier

- **WHEN** an authenticated caller requests a series identifier that exists
- **THEN** the system returns `200 OK` with the series representation

#### Scenario: Series representation content

- **WHEN** the system returns one series
- **THEN** the representation carries the series identifier, name, plot, country of origin, first air date, genres and
  cover
- **AND** the country of origin and the first air date are `null` when the show stores none
- **AND** the cover is the identifier of the image holding the show's artwork, or `null` when the show stores none
- **AND** it carries the seasons ordered by season index
- **AND** each season summary carries its index, its name when set, its episode count, its cover and links to the season
  and its episode collection

#### Scenario: Unknown series

- **WHEN** the requested series identifier is unknown
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed series identifier

- **WHEN** the caller supplies a `series_id` that is not a valid identifier
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: Season and episode identity

A season SHALL be identified by its index within its series, and an episode by its index within its season. Both indexes
SHALL be non-negative 32-bit integers, and the representation SHALL expose the index as `id`.

#### Scenario: Index is the identity

- **WHEN** the system returns a season or an episode
- **THEN** its `id` is its parent-scoped index, not a separate identifier

#### Scenario: Index range

- **WHEN** a season or episode is stored at the largest 32-bit unsigned index
- **THEN** the system addresses and returns it

### Requirement: Read one season

The system SHALL return one season through `GET /api/v1/library/series/{series_id}/season/{season_index}`, scoped to the
series named in the path.

#### Scenario: Season of an existing series

- **WHEN** an authenticated caller requests a season index that belongs to a series that exists
- **THEN** the system returns `200 OK` with the season representation

#### Scenario: Season representation content

- **WHEN** the system returns a season
- **THEN** the representation carries its series identifier, its index, its name when set, its episode count, its cover
  and links to its series and its episode collection
- **AND** the cover is the identifier of the image holding the season's artwork, or `null` when the season stores none

#### Scenario: Unknown series or season

- **WHEN** the requested season index does not belong to the series, or the series identifier is unknown
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed season index

- **WHEN** the caller supplies a `season_index` that is not a valid 32-bit unsigned integer
- **THEN** the system returns `400 Bad Request` with a stable message

#### Scenario: Season indexes are scoped to the series

- **WHEN** two different series each hold a season with index 1
- **THEN** each series resolves its own season, and the two seasons are independent

### Requirement: List the episodes of a season

The system SHALL return the episodes of one season through
`GET /api/v1/library/series/{series_id}/season/{season_index}/episode`.

#### Scenario: Episodes of an existing season

- **WHEN** an authenticated caller requests the episodes of a season that belongs to a series that exists
- **THEN** the system returns `200 OK` with a HAL collection of episode representations

#### Scenario: Episode representation content

- **WHEN** the system returns an episode in the collection
- **THEN** the representation carries its index as `id`, the series identifier, the season index, its name, plot and air
  date, and its video summary
- **AND** the air date is `null` when the episode stores none
- **AND** the video summary carries the video identifier, the backing file identifier, duration, codec, width, height
  and scan type
- **AND** it carries links to its series and season

#### Scenario: Unknown series or season

- **WHEN** the caller requests the episodes of a series or season that does not exist, or the season does not belong to
  the series
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed identifiers

- **WHEN** the caller supplies a `series_id` that is not a valid identifier, or a `season_index` that is not a valid
  32-bit unsigned integer
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: Read one episode

The system SHALL return one episode through
`GET /api/v1/library/series/{series_id}/season/{season_index}/episode/{episode_index}`, scoped to the series and season
named in the path.

#### Scenario: Episode of the season

- **WHEN** an authenticated caller requests an episode index that exists in the named series season
- **THEN** the system returns `200 OK` with the episode representation

#### Scenario: Episode without an air date

- **WHEN** the requested episode stores no air date
- **THEN** the representation reports `air_date` as `null`

#### Scenario: Episode not in the season, or unknown series or season

- **WHEN** the requested `episode_index` does not belong to the named season, or the series or season identifier is
  unknown
- **THEN** the system returns `404 Not Found`

#### Scenario: Malformed episode index

- **WHEN** the caller supplies a `season_index` or `episode_index` that is not a valid 32-bit unsigned integer
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: Seasons and episodes are ordered by position

The system SHALL order the seasons of a series by ascending season index, and the episodes of a season by ascending
episode index, so repeated reads return a stable order.

#### Scenario: Stable order

- **WHEN** a series has several seasons, each holding several episodes
- **THEN** the series representation lists the seasons by ascending index
- **AND** each season's episode collection lists the episodes by ascending index

### Requirement: Series, season and episode responses are HAL representations

Series, season and episode success responses SHALL be served as `application/hal+json`, keep business data at the root
of the payload, and carry a `_links.self` naming the resource's canonical URI.

#### Scenario: Single resource representation is HAL

- **WHEN** the system returns one series, season or episode
- **THEN** the response is served as `application/hal+json` with business data at the root
- **AND** the representation carries `_links.self` naming the resource's canonical URI

#### Scenario: Collection representation is HAL

- **WHEN** the system returns the series collection or a season's episode collection
- **THEN** the response is served as `application/hal+json`
- **AND** the collection carries `_links.self`
- **AND** the series collection also carries a templated `_links.search` exposing the supported filters

### Requirement: A show is created

The system SHALL create a show through `POST /api/v1/library/series` from a non-empty name and optional plot, first air
date, country of origin, genres and cover image. A new show SHALL return `201 Created` with a `Location`. Creation is
insert-only because the schema has no unique show name. The write SHALL require `library.manage` over the whole
catalogue.

#### Scenario: A new show

- **WHEN** an administrator submits a name
- **THEN** the system creates the show with no seasons and returns `201 Created` with the show representation and a
  `Location`

#### Scenario: A repeated name

- **WHEN** an administrator submits a name another show already uses
- **THEN** the system creates a second show rather than returning the first

#### Scenario: An unknown genre is stored

- **WHEN** the submitted genre identifier is not in the catalogue yet
- **THEN** the system stores the genre and links it to the show

#### Scenario: Invalid input

- **WHEN** the name is empty, a genre identifier is empty, the first air date does not use `YYYY-MM-DD`, or the cover is
  not a library image
- **THEN** the system returns `422 Unprocessable Content` or `409 Conflict` and creates nothing

### Requirement: Show metadata is updated partially

The system SHALL update a show through `PATCH /api/v1/library/series/{series_id}`. An absent field SHALL keep its stored
value and a `null` SHALL clear an optional one. The show's seasons and episodes SHALL NOT change. The write SHALL
require `library.manage` on the show.

#### Scenario: Change the metadata

- **WHEN** an administrator submits a name, plot, country of origin, first air date, genre list or cover
- **THEN** the system stores the named values and returns the show representation

#### Scenario: Clear an optional value

- **WHEN** an administrator sends `first_air_date: null`, `country_of_origin: null` or `cover_image_id: null`
- **THEN** the show stores no value for that field and the representation reports `null`

#### Scenario: Seasons and episodes are untouched

- **WHEN** a show with seasons and episodes is updated
- **THEN** every season and episode keeps its stored values

#### Scenario: The show is not visible

- **WHEN** the caller's `library.read` permission does not cover the show
- **THEN** the system returns `404 Not Found`, indistinguishable from an unknown show

#### Scenario: The show is readable but not manageable

- **WHEN** the caller may read the show but `library.manage` does not cover it
- **THEN** the system returns `403 Forbidden` and applies nothing

### Requirement: A season is created or replaced

The system SHALL create or replace the season at `PUT /api/v1/library/series/{series_id}/season/{season_index}` from an
optional name and cover. A free index SHALL return `201 Created`; an existing index SHALL be replaced and return
`200 OK`. The season's episodes SHALL be kept. The write SHALL require `library.manage` on the show.

#### Scenario: Create a season

- **WHEN** an administrator puts a name at a free index
- **THEN** the system creates the season with no episodes and returns `201 Created`

#### Scenario: Replace a season

- **WHEN** an administrator puts a new name or cover at an existing index
- **THEN** the system stores them, keeps the season's episodes and returns `200 OK`

#### Scenario: Unknown show or out-of-range index

- **WHEN** the show does not exist, or the index is outside the 32-bit unsigned range
- **THEN** the system returns `404 Not Found` or `400 Bad Request` and applies nothing

#### Scenario: Not visible or not manageable

- **WHEN** the caller cannot read the show, or can read it but `library.manage` does not cover it
- **THEN** the system returns `404 Not Found` or `403 Forbidden` and applies nothing

### Requirement: Season metadata is updated partially

The system SHALL update a season through `PATCH /api/v1/library/series/{series_id}/season/{season_index}`. An absent
field SHALL keep its stored value and a `null` SHALL clear the name or cover. The season's episodes SHALL NOT change.
The write SHALL require `library.manage` on the show.

#### Scenario: Rename or cover a season

- **WHEN** an administrator submits a name or cover
- **THEN** the system stores it and returns the season representation

#### Scenario: Clear the name or cover

- **WHEN** an administrator sends `name: null` or `cover_image_id: null`
- **THEN** the season stores no value for that field and the representation reports `null`

#### Scenario: Unknown or malformed season

- **WHEN** the season index does not belong to the show, or the index is not a valid 32-bit unsigned integer
- **THEN** the system returns `404 Not Found` or `400 Bad Request` and applies nothing

### Requirement: An episode is created or replaced

The system SHALL create or replace the episode at
`PUT /api/v1/library/series/{series_id}/season/{season_index}/episode/{episode_index}` from a `video_id`, a non-empty
name and optional plot and air date. A free index SHALL return `201 Created`; an existing index SHALL be replaced and
return `200 OK`, replacing its video when a different one is named. The write SHALL require `library.manage` on the
show.

#### Scenario: Create an episode

- **WHEN** an administrator puts a video and a name at a free index
- **THEN** the system creates the episode and returns `201 Created` with the episode representation

#### Scenario: Replace an episode

- **WHEN** an administrator puts new metadata at an existing index
- **THEN** the system stores it, keeps the episode address and returns `200 OK`

#### Scenario: Replace the video

- **WHEN** the request names a different video for an existing episode
- **THEN** the episode stores the new video and the previous video becomes unattached

#### Scenario: The video is already attached

- **WHEN** the submitted video backs another film or episode
- **THEN** the system returns `409 Conflict` and applies nothing

#### Scenario: Unknown video or invalid input

- **WHEN** the video does not exist, the name is empty, or the air date does not use `YYYY-MM-DD`
- **THEN** the system returns `422 Unprocessable Content` and applies nothing

#### Scenario: Unknown show or season, or out-of-range index

- **WHEN** the show or season does not exist, or an index is outside the 32-bit unsigned range
- **THEN** the system returns `404 Not Found` or `400 Bad Request` and applies nothing

### Requirement: Episode metadata is updated partially

The system SHALL update an episode through
`PATCH /api/v1/library/series/{series_id}/season/{season_index}/episode/{episode_index}`. An absent field SHALL keep its
stored value and a `null` SHALL clear the air date. The episode's video SHALL NOT change. The write SHALL require
`library.manage` on the show.

#### Scenario: Change the metadata

- **WHEN** an administrator submits a name, plot or air date
- **THEN** the system stores the named values and returns the episode representation

#### Scenario: Clear the air date

- **WHEN** an administrator sends `air_date: null`
- **THEN** the episode stores no air date and the representation reports `null`

#### Scenario: The video is untouched

- **WHEN** an episode with a backing video is updated
- **THEN** the video keeps its stored value

#### Scenario: Unknown episode, or not visible or not manageable

- **WHEN** the episode does not exist, the caller cannot read the show, or `library.manage` does not cover it
- **THEN** the system returns `404 Not Found` or `403 Forbidden` and applies nothing

### Requirement: External metadata is applied to a show

The system SHALL resolve a provider reference and write a show through `POST /api/v1/library/series/{series_id}/apply`.
It SHALL write the name, plot, first air date, country of origin and genres, SHALL clear each of those the provider does
not report, and SHALL leave the cover, seasons and episodes unchanged. The write SHALL require `library.manage` on the
show.

#### Scenario: Apply the reported metadata

- **WHEN** an administrator applies a reference the provider resolves to a show
- **THEN** the system stores the show's name, plot, first air date, country of origin and genres and returns the show
  representation

#### Scenario: The provider reports an absent value

- **WHEN** the provider reports no first air date, no country of origin or no genres
- **THEN** the show stores `null` or an empty genre list for that field

#### Scenario: Seasons, episodes and cover are untouched

- **WHEN** a show with seasons, episodes and a cover is applied
- **THEN** every stored season, episode and cover keeps its value

#### Scenario: The reference is unknown or resolves another kind

- **WHEN** the provider does not know the reference, or resolves it to a film or an episode
- **THEN** the system returns `404 Not Found` or `422 Unprocessable Content` and writes nothing

#### Scenario: The provider fails

- **WHEN** the configured provider fails while resolving the reference
- **THEN** the system reports the failure and writes nothing

### Requirement: External metadata is applied to an episode

The system SHALL resolve a show reference and write one episode through
`POST /api/v1/library/series/{series_id}/season/{season_index}/episode/{episode_index}/apply`. It SHALL write the name,
plot and air date, SHALL clear an air date the provider does not report, and SHALL leave the video unchanged. The
provider's season and episode positions SHALL match the request. The write SHALL require `library.manage` on the show.

#### Scenario: Apply the reported metadata

- **WHEN** an administrator applies a show reference for the addressed season and episode
- **THEN** the system stores the episode's name, plot and air date and returns the episode representation

#### Scenario: The provider reports no air date

- **WHEN** the provider reports no air date for the episode
- **THEN** the episode stores no air date and the representation reports `null`

#### Scenario: The position does not match

- **WHEN** the provider reports a season or episode position other than the addressed one
- **THEN** the system returns `422 Unprocessable Content` and writes nothing

#### Scenario: The video is untouched

- **WHEN** an episode with a backing video is applied
- **THEN** the video keeps its stored value

#### Scenario: The reference is unknown or resolves another kind

- **WHEN** the provider does not know the reference, or resolves it to a film or a show without the addressed episode
- **THEN** the system returns `404 Not Found` or `422 Unprocessable Content` and writes nothing

#### Scenario: The provider fails

- **WHEN** the configured provider fails while resolving the reference
- **THEN** the system reports the failure and writes nothing
