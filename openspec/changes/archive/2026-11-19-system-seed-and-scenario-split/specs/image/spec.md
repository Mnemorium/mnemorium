# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-IMAGE-003 — An image is readable

A caller who may read an image SHALL read it. The representation SHALL carry the image identifier, name, dimensions,
orientation, creation date and backing file identifier. An unknown or unreadable image SHALL answer `404 Not Found`,
indistinguishable from a missing one.

#### Scenario: SCN-IMAGE-003-A — An image of an existing identifier

- **WHEN** an authenticated caller requests an image it may read
- **THEN** the system returns `200 OK`

#### Scenario: SCN-IMAGE-003-B — Unknown or unreadable image

- **WHEN** the image does not exist, or the caller may not read it
- **THEN** the system returns `404 Not Found`

## REMOVED Requirements

### Requirement: REQ-IMAGE-004 — An image addresses its file

**Reason**: The `303 See Other` and its `Location` header are declared by the alias operation, so the API fuzz suite
validates that contract from the served document; the case also built the alias route by hand rather than following a
link.

**Migration**: No client change. The alias route stays served with the same `303` and `Location`.
