# library/cover-art Specification

## Purpose

Lets every library resource carry optional artwork, so a client can render a library grid and a detail page from the
metadata the server reports rather than from a naming convention.

## Requirements

### Requirement: Cover art is optional

A library resource MAY carry cover art, and every resource that exposes artwork SHALL report it as a nullable file
identifier named `cover_file_id`. A resource without artwork SHALL report `null` and SHALL NOT be rejected, hidden or
represented differently.

#### Scenario: Resource without artwork

- **WHEN** the system returns a film, show, season, album or playlist that stores no artwork
- **THEN** its representation carries `cover_file_id` as `null`
- **AND** the representation is otherwise unchanged

#### Scenario: Resource with artwork

- **WHEN** the system returns a film, show, season, album or playlist that stores artwork
- **THEN** its representation carries the identifier of the backing file as `cover_file_id`

### Requirement: Cover art references an image file

A stored cover SHALL reference a file whose media type is an image. A reference to a file of any other media type SHALL
be rejected, and the rejection SHALL NOT be stored.

#### Scenario: An image file is accepted

- **WHEN** a cover is stored pointing at a file whose media type is an image
- **THEN** the reference is stored

#### Scenario: A non-image file is rejected

- **WHEN** a cover references a file whose media type is audio or video
- **THEN** the write is rejected as a conflict and nothing is stored

### Requirement: A cover exposes a file link

A representation that carries a non-null `cover_file_id` SHALL expose `_links.cover` naming the Asset file route for
that file, so a client never builds the route itself.

#### Scenario: Cover link is exposed

- **WHEN** the system returns a representation whose `cover_file_id` is set
- **THEN** `_links.cover` names the Asset route for that image file

#### Scenario: No cover, no link

- **WHEN** the system returns a representation whose `cover_file_id` is `null`
- **THEN** the representation carries no `_links.cover`

#### Scenario: Link precedes the route

- **WHEN** the Asset file route a cover link names is not served
- **THEN** the representation still exposes `_links.cover`
- **AND** following the link answers the standard error envelope rather than a representation

### Requirement: A cover is never inherited

The server SHALL report only the cover a resource itself stores. It SHALL NOT resolve a missing cover from a parent or
related resource.

#### Scenario: A season without its own cover

- **WHEN** a season stores no artwork while its show does
- **THEN** the season representation reports `cover_file_id` as `null`
- **AND** it carries no resolved or inherited artwork value
