# Spec Delta

## Purpose

Lets a caller create, browse, read and delete an owned ordered collection of images and videos, and add or remove one
item at a time, with the position, item count and last-modified time the collection reports.

## ADDED Requirements

### Requirement: REQ-GALLERY-001 — A gallery is an ordered collection of visual items

A gallery SHALL be an ordered collection of items, each referencing exactly one image or one video. It SHALL carry a
name, an owner, a public flag, a creation time, a last-modified time and an item count.

#### Scenario: SCN-GALLERY-001-A — An item references one medium

- **WHEN** the system returns a gallery with items
- **THEN** each item names exactly one image or one video

### Requirement: REQ-GALLERY-002 — The default gallery is public and undeletable

A gallery with identifier `0` SHALL be seeded, ownerless and public. It SHALL NOT be deleted and SHALL NOT hold an item
automatically, and it SHALL be readable like any other gallery.

#### Scenario: SCN-GALLERY-002-A — The default gallery is ownerless and public

- **WHEN** the system returns the default gallery
- **THEN** its owner identifier is absent
- **AND** its public flag is set

#### Scenario: SCN-GALLERY-002-B — The default gallery holds no item automatically

- **WHEN** a fresh instance returns the default gallery
- **THEN** its item count is zero

#### Scenario: SCN-GALLERY-002-C — The default gallery cannot be deleted

- **WHEN** a caller deletes the default gallery
- **THEN** the system returns `409 Conflict` and deletes nothing

### Requirement: REQ-GALLERY-003 — Create a gallery

Creating a gallery SHALL require the `CreateChild` action on `GALLERY_COLLECTION`. The name SHALL be trimmed and SHALL
be non-empty and at most 100 characters. `is_public` SHALL default to `false`. A duplicate name for the same owner SHALL
answer `409 Conflict`; success SHALL answer `201 Created` with a `Location`. A refused creation SHALL store nothing.

#### Scenario: SCN-GALLERY-003-A — A gallery is created

- **WHEN** an authenticated caller creates a gallery with a valid name
- **THEN** the system returns `201 Created` with the gallery representation and a `Location`
- **AND** the gallery is owned by the caller and private

#### Scenario: SCN-GALLERY-003-B — A public gallery is created

- **WHEN** the caller creates a gallery with `is_public` true
- **THEN** the gallery is `Public`

#### Scenario: SCN-GALLERY-003-C — The owner already has that name

- **WHEN** the caller already owns a gallery with the same trimmed name
- **THEN** the system returns `409 Conflict` and creates nothing

#### Scenario: SCN-GALLERY-003-D — The name is unusable

- **WHEN** the submitted name is empty, only whitespace, or longer than 100 characters
- **THEN** the system returns `422 Unprocessable Content` and creates nothing

#### Scenario: SCN-GALLERY-003-E — The caller may not create a gallery

- **WHEN** the caller's grant on `GALLERY_COLLECTION` denies `CreateChild`
- **THEN** the system returns `403 Forbidden` and creates nothing

### Requirement: REQ-GALLERY-004 — Browse galleries

The system SHALL return the galleries a caller may `Discover`. The collection SHALL accept optional `name`, `is_public`
and `owner_id` filters. Each item SHALL carry the gallery's fields and item count, and no item list.

#### Scenario: SCN-GALLERY-004-A — A caller lists galleries

- **WHEN** an authenticated caller reads the gallery collection
- **THEN** the system returns `200 OK` with the galleries the caller may discover

#### Scenario: SCN-GALLERY-004-B — Filter

- **WHEN** the caller requests the collection with `name`, `is_public` and `owner_id`
- **THEN** the system returns only the matching galleries

#### Scenario: SCN-GALLERY-004-C — A caller may discover no gallery

- **WHEN** the caller may discover no gallery
- **THEN** the system returns `200 OK` with an empty collection

#### Scenario: SCN-GALLERY-004-D — Malformed query string

- **WHEN** the caller requests the collection with a query string that cannot be decoded
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: REQ-GALLERY-005 — Read a gallery

A caller who may `View` a gallery SHALL read it. The representation SHALL carry the gallery's fields and its items
ordered by position. An unknown or unreadable gallery SHALL answer `404 Not Found`, indistinguishable from a missing
one.

#### Scenario: SCN-GALLERY-005-A — A gallery of an existing identifier

- **WHEN** an authenticated caller requests a gallery it may view
- **THEN** the system returns `200 OK` with the gallery representation and its items

#### Scenario: SCN-GALLERY-005-B — Unknown or unreadable gallery

- **WHEN** the gallery does not exist, or the caller may not view it
- **THEN** the system returns `404 Not Found`

#### Scenario: SCN-GALLERY-005-C — Malformed gallery identifier

- **WHEN** the caller supplies a gallery identifier that is not a valid identifier
- **THEN** the system returns `400 Bad Request` with a stable message

### Requirement: REQ-GALLERY-006 — Delete a gallery

Deleting a gallery SHALL require the `Delete` action on it. The system SHALL remove the gallery, its items and their
media rows. The backing file row SHALL be removed only when the caller may `View` the file, and stored bytes SHALL NOT
be touched. The default gallery SHALL answer `409`; a readable gallery the caller may not delete `403`; an unreadable
one `404`; success `204`.

#### Scenario: SCN-GALLERY-006-A — The owner deletes a gallery

- **WHEN** a caller that may delete the gallery deletes it
- **THEN** the system returns `204 No Content`
- **AND** the gallery, its items and their media rows no longer exist

#### Scenario: SCN-GALLERY-006-B — The default gallery is refused

- **WHEN** a caller deletes the default gallery
- **THEN** the system returns `409 Conflict` and deletes nothing

#### Scenario: SCN-GALLERY-006-C — The caller may read but not delete

- **WHEN** the caller may view the gallery but the `Delete` action refuses
- **THEN** the system returns `403 Forbidden` and deletes nothing

#### Scenario: SCN-GALLERY-006-D — The caller may not read the gallery

- **WHEN** the caller may not view the gallery
- **THEN** the system returns `404 Not Found` and deletes nothing

#### Scenario: SCN-GALLERY-006-E — The stored content survives

- **WHEN** a gallery is deleted
- **THEN** the bytes under the library root are unchanged

### Requirement: REQ-GALLERY-007 — Add an item to a gallery

Adding an item SHALL require the `CreateChild` action on the gallery and `View` on the backing file. The request SHALL
name the backing file identifier and the declared medium type. The system SHALL resolve the typed row, require it be
unassigned to another gallery, append it at the next position, advance the last-modified time and answer `201 Created`
with a `Location`. An assigned medium or a scope conflict SHALL answer `409`; a missing or unowned medium `422`.

#### Scenario: SCN-GALLERY-007-A — An image is added

- **WHEN** a caller that may add items adds an image by its backing file identifier
- **THEN** the system returns `201 Created` with the item and a `Location`

#### Scenario: SCN-GALLERY-007-B — A video is added

- **WHEN** a caller that may add items adds a video by its backing file identifier
- **THEN** the system returns `201 Created` with the item and a `Location`

#### Scenario: SCN-GALLERY-007-C — The item is appended

- **WHEN** the gallery already holds items
- **THEN** the new item is placed after the last existing item

#### Scenario: SCN-GALLERY-007-D — The medium is already assigned

- **WHEN** the named image or video already belongs to a gallery
- **THEN** the system returns `409 Conflict` and creates nothing

#### Scenario: SCN-GALLERY-007-E — The medium is missing or unowned

- **WHEN** no typed row matches the backing file, or the caller may not use the file
- **THEN** the system returns `422 Unprocessable Content` and creates nothing

#### Scenario: SCN-GALLERY-007-F — The caller may not add an item

- **WHEN** the caller may view the gallery but `CreateChild` refuses
- **THEN** the system returns `403 Forbidden` and creates nothing

### Requirement: REQ-GALLERY-008 — Read one item of a gallery

A caller who may `View` the gallery SHALL read one item through `GET /api/v1/library/gallery/{id}/item/{item_id}`. The
representation SHALL carry the item identifier, gallery identifier, added-at instant, medium identifier, medium type,
display name, backing file identifier and `_links.self`. The display name SHALL be the image name or, for a video, the
file path. An unknown item, or one of another gallery, SHALL answer `404`.

#### Scenario: SCN-GALLERY-008-A — An image item is read

- **WHEN** an authenticated caller reads an image item of a gallery it may view
- **THEN** the representation carries the item, its medium identifier and type, the image name and the file identifier

#### Scenario: SCN-GALLERY-008-B — A video item is read

- **WHEN** an authenticated caller reads a video item of a gallery it may view
- **THEN** the display name is the video file's path

#### Scenario: SCN-GALLERY-008-C — The item belongs to another gallery

- **WHEN** the item identifier exists in a gallery other than the addressed one, or does not exist
- **THEN** the system returns `404 Not Found`

### Requirement: REQ-GALLERY-010 — Remove an item from a gallery

Removing an item SHALL require the `Delete` action on the gallery. The system SHALL remove the item and its media row
and advance the last-modified time. The backing file row SHALL be removed only when the caller may `View` it, and stored
bytes SHALL NOT be touched. Success SHALL answer `204`; an item of another gallery or an unknown item SHALL answer
`404`.

#### Scenario: SCN-GALLERY-010-A — The item is removed

- **WHEN** a caller that may delete the gallery removes one of its items
- **THEN** the system returns `204 No Content`
- **AND** the item and its media row no longer exist

#### Scenario: SCN-GALLERY-010-B — The medium is gone

- **WHEN** the removed item's media row is looked up or re-added
- **THEN** the medium does not exist

#### Scenario: SCN-GALLERY-010-C — The stored content survives

- **WHEN** an item is removed
- **THEN** the bytes under the library root are unchanged

#### Scenario: SCN-GALLERY-010-D — The item is not in the gallery

- **WHEN** the item identifier belongs to another gallery, or does not exist
- **THEN** the system returns `404 Not Found` and removes nothing

### Requirement: REQ-GALLERY-011 — Items are ordered by position

An added item SHALL be appended after the last existing item. The gallery read SHALL list items by ascending position
independent of insertion order. Every successful change SHALL advance the gallery's last-modified time, including a
change that stores no new value.

#### Scenario: SCN-GALLERY-011-A — The listing order is stable

- **WHEN** a gallery holds several items
- **THEN** the read lists them by ascending position, independent of insertion order

#### Scenario: SCN-GALLERY-011-B — A change advances the last-modified time

- **WHEN** a gallery change succeeds
- **THEN** its last-modified time advances
