# Spec Delta

## MODIFIED Requirements

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

#### Scenario: SCN-COVER-ART-006-D — A playlist track reports no artwork

- **WHEN** the system returns a playlist track whose recording's album stores a cover
- **THEN** the track carries no `cover_image_id` and no `_links.cover`
- **AND** the album it belongs to still reports that cover
