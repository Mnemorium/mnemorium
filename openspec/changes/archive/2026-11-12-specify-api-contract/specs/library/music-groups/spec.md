# Spec Delta

## MODIFIED Requirements

### Requirement: List and read music groups

The system SHALL return the groups through `GET /api/v1/library/music/group`, with an optional `name` filter and
pagination as `api` defines, and one group through `GET /api/v1/library/music/group/{group_id}`. The collection SHALL
embed each page item's partial representation under `_embedded.items`, carrying the group's identifier, name and cover.
A group detail representation SHALL carry its identifier, name, `cover_image_id` and `_links.cover` when artwork is set.

#### Scenario: Authenticated caller lists groups

- **WHEN** an authenticated caller requests the group collection
- **THEN** the system returns a collection whose `_embedded.items` carries the page's partial group representations

#### Scenario: Name filter and page

- **WHEN** the caller requests the collection with `name` and `offset`
- **THEN** the system returns only the groups whose name matches, starting at `offset`

#### Scenario: Read one group

- **WHEN** an authenticated caller requests an existing group identifier
- **THEN** the system returns the group representation

#### Scenario: Unknown group

- **WHEN** the requested group identifier is unknown
- **THEN** the system returns `404 Not Found`
