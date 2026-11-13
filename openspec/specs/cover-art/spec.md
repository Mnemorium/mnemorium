# cover-art Specification

## Purpose

Lets every library resource that owns artwork carry an optional cover and expose the image it references, so a client
renders a grid or a detail page from the metadata the server reports. A resource that owns no artwork — an episode, a
recording — reports none: the client reads the cover of the album, season or show that contains it.

## Requirements

### Requirement: REQ-COVER-ART-001 — A resource reports its cover as a nullable image identifier

A **Resource** that carries artwork SHALL report the cover it stores as `cover_image_id`, nullable. A resource that
stores no cover SHALL report `null` and SHALL NOT be rejected, hidden or represented differently.

#### Scenario: SCN-COVER-ART-001-A — A resource with its own artwork

- **WHEN** the system returns a resource that stores a cover
- **THEN** `cover_image_id` carries the image's identifier

#### Scenario: SCN-COVER-ART-001-B — A resource without artwork

- **WHEN** the system returns a resource that stores no cover
- **THEN** `cover_image_id` is `null` and the representation is otherwise unchanged

### Requirement: REQ-COVER-ART-002 — A cover references an image row

A stored cover SHALL reference an image row. A reference to an image that does not exist, or to a file directly, SHALL
be rejected and nothing SHALL be stored.

#### Scenario: SCN-COVER-ART-002-A — An image is accepted

- **WHEN** a cover is stored pointing at an existing image row
- **THEN** the reference is stored

#### Scenario: SCN-COVER-ART-002-B — An unknown image is rejected

- **WHEN** a cover references an image that does not exist
- **THEN** the write is rejected and nothing is stored

#### Scenario: SCN-COVER-ART-002-C — A file is not a cover

- **WHEN** a cover references a file identifier rather than an image
- **THEN** the write is rejected and nothing is stored

### Requirement: REQ-COVER-ART-003 — A cover's scope follows the resource it decorates

A **Catalogue** cover SHALL be stored with the `LIBRARY` scope and a playlist cover with the `USER` scope. A scope SHALL
be fixed by the first attachment and SHALL NOT change. An image SHALL have at most one referrer, and a cover SHALL NOT
be gallery media.

#### Scenario: SCN-COVER-ART-003-A — A catalogue cover

- **WHEN** an album, group, film, show or season stores a cover
- **THEN** the image is marked `LIBRARY`

#### Scenario: SCN-COVER-ART-003-B — A playlist cover

- **WHEN** a playlist stores a cover
- **THEN** the image is marked `USER`

#### Scenario: SCN-COVER-ART-003-C — A library image cannot back a playlist

- **WHEN** a playlist cover names an image already marked `LIBRARY`
- **THEN** the write is rejected as a conflict and nothing is stored

#### Scenario: SCN-COVER-ART-003-D — Gallery media is not cover art

- **WHEN** a cover names an image already used as gallery media
- **THEN** the write is rejected as a conflict and nothing is stored

### Requirement: REQ-COVER-ART-004 — A cover exposes a link to its image

A representation whose `cover_image_id` is set SHALL expose `_links.cover` naming the Asset image route for that image,
so a client never builds the route itself. A representation whose `cover_image_id` is `null` SHALL carry no
`_links.cover`.

#### Scenario: SCN-COVER-ART-004-A — The link is exposed

- **WHEN** the system returns a representation whose `cover_image_id` is set
- **THEN** `_links.cover` names the Asset image route for that image

#### Scenario: SCN-COVER-ART-004-B — No cover, no link

- **WHEN** the system returns a representation whose `cover_image_id` is `null`
- **THEN** the representation carries no `_links.cover`

#### Scenario: SCN-COVER-ART-004-C — The link precedes the route

- **WHEN** the Asset image route a cover link names is not served
- **THEN** the representation still exposes `_links.cover`
- **AND** following the link answers the standard error envelope rather than a representation

### Requirement: REQ-COVER-ART-005 — Attach, replace and clear a cover

A resource that stores its own cover SHALL accept an image identifier to attach or replace it and `null` to clear it,
and SHALL leave the stored cover unchanged when the field is absent. Every such resource SHALL support all three
outcomes.

#### Scenario: SCN-COVER-ART-005-A — A cover is attached

- **WHEN** a write names an image for a resource that stores none
- **THEN** the cover is stored

#### Scenario: SCN-COVER-ART-005-B — A cover is replaced

- **WHEN** a write names an image for a resource that stores another
- **THEN** the stored cover is replaced

#### Scenario: SCN-COVER-ART-005-C — A cover is cleared

- **WHEN** a write sends `null` for the cover
- **THEN** the stored cover is cleared

#### Scenario: SCN-COVER-ART-005-D — An absent field keeps the cover

- **WHEN** a write omits the cover field
- **THEN** the stored cover is unchanged

### Requirement: REQ-COVER-ART-006 — A resource that owns no artwork reports none and inherits nothing

The server SHALL NOT resolve a cover from a resource that contains another. An episode and a recording SHALL NOT carry
`cover_image_id` or `_links.cover`; a client collects their artwork from the season, show or album it reads beside them.

#### Scenario: SCN-COVER-ART-006-A — A recording reports no artwork

- **WHEN** the system returns an album track or a playlist track
- **THEN** the representation carries no `cover_image_id` and no `_links.cover`

#### Scenario: SCN-COVER-ART-006-B — An episode reports no artwork

- **WHEN** the system returns an episode
- **THEN** the representation carries no `cover_image_id` and no `_links.cover`

#### Scenario: SCN-COVER-ART-006-C — The container still reports its cover

- **WHEN** an album stores a cover and the system returns that album
- **THEN** the album reports `cover_image_id` and `_links.cover`

### Requirement: REQ-COVER-ART-007 — Attach artwork to a playlist

A playlist SHALL carry an optional cover, reported as `cover_image_id` and accepted on creation and metadata update. A
playlist's cover SHALL follow the scope rule of `REQ-COVER-ART-003` and SHALL NOT be inherited.

#### Scenario: SCN-COVER-ART-007-A — A cover is set on creation

- **WHEN** a playlist creation request names an image
- **THEN** the playlist stores it and reports it as `cover_image_id`

#### Scenario: SCN-COVER-ART-007-B — A cover is replaced or cleared

- **WHEN** a playlist metadata update names an image, or sends `null`
- **THEN** the cover is replaced or cleared respectively

#### Scenario: SCN-COVER-ART-007-C — A playlist with no cover

- **WHEN** the playlist stores no image
- **THEN** it reports `cover_image_id` as `null` and no `_links.cover`

#### Scenario: SCN-COVER-ART-007-D — An unusable image

- **WHEN** the named image does not exist or is a `LIBRARY` image
- **THEN** the system answers `409 Conflict` or `422 Unprocessable Content` and applies nothing
