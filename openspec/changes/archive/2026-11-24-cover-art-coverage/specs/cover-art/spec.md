# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-COVER-ART-003 — A cover's scope follows the resource it decorates

A **Catalogue** cover SHALL be stored with the `LIBRARY` scope and a playlist cover with the `USER` scope. A scope SHALL
be fixed by the first attachment and SHALL NOT change. A `USER` image SHALL have at most one referrer, a `LIBRARY` image
MAY back more than one catalogue cover, and a cover SHALL NOT be gallery media.

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

#### Scenario: SCN-COVER-ART-003-E — A user image backs one cover

- **WHEN** a playlist cover names an image that already backs another playlist
- **THEN** the write is rejected as a conflict and nothing is stored
