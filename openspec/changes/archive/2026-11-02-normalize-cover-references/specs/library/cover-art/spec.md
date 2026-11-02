# Spec Delta

## MODIFIED Requirements

### Requirement: Cover art is optional

A library resource MAY carry cover art, and every resource that exposes artwork SHALL report it as a nullable image
identifier named `cover_image_id`. A resource without artwork SHALL report `null` and SHALL NOT be rejected, hidden or
represented differently.

#### Scenario: Resource without artwork

- **WHEN** the system returns a film, show, season, album or playlist that stores no artwork
- **THEN** its representation carries `cover_image_id` as `null`
- **AND** the representation is otherwise unchanged

#### Scenario: Resource with artwork

- **WHEN** the system returns a film, show, season, album or playlist that stores artwork
- **THEN** its representation carries the identifier of the image row as `cover_image_id`

### Requirement: Cover art references an image file

A stored cover SHALL reference an image row, and a reference to an image that does not exist SHALL be rejected. A cover
SHALL NOT reference a file directly. Catalogue cover art (`movie`, `series`, `season` and `music_album`) SHALL be stored
with the `LIBRARY` scope, while a `music_playlist` cover SHALL be stored with the `USER` scope. A cover SHALL NOT be
used as gallery media.

#### Scenario: An image file is accepted

- **WHEN** a catalogue cover is stored pointing at an existing image row
- **THEN** the reference is stored and the image is marked `LIBRARY`

#### Scenario: A playlist cover is accepted

- **WHEN** a playlist cover is stored pointing at an existing image row
- **THEN** the reference is stored and the image is marked `USER`

#### Scenario: A non-image file is rejected

- **WHEN** a cover references an image that does not exist, or references a file directly
- **THEN** the write is rejected and nothing is stored

#### Scenario: Gallery media is not cover art

- **WHEN** a cover references an image already used as gallery media
- **THEN** the write is rejected as a conflict and nothing is stored

### Requirement: A cover exposes a file link

A representation that carries a non-null `cover_image_id` SHALL expose `_links.cover` naming the Asset image route for
that image, so a client never builds the route itself.

#### Scenario: Cover link is exposed

- **WHEN** the system returns a representation whose `cover_image_id` is set
- **THEN** `_links.cover` names the Asset image route for that image

#### Scenario: No cover, no link

- **WHEN** the system returns a representation whose `cover_image_id` is `null`
- **THEN** the representation carries no `_links.cover`

#### Scenario: Link precedes the route

- **WHEN** the Asset image route a cover link names is not served
- **THEN** the representation still exposes `_links.cover`
- **AND** following the link answers the standard error envelope rather than a representation

### Requirement: A cover is never inherited

The server SHALL report only the cover a resource itself stores. It SHALL NOT resolve a missing cover from a parent or
related resource.

#### Scenario: A season without its own cover

- **WHEN** a season stores no artwork while its show does
- **THEN** the season representation reports `cover_image_id` as `null`
- **AND** it carries no resolved or inherited artwork value
