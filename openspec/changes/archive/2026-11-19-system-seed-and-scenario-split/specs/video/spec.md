# Spec Delta

## MODIFIED Requirements

### Requirement: REQ-VIDEO-003 — A video is readable

A caller who may read a video SHALL read it. The representation SHALL carry the video identifier, codec, colour
identifier, duration, frame count, width, height, scan type and backing file identifier. An unknown or unreadable video
SHALL answer `404 Not Found`, indistinguishable from a missing one.

#### Scenario: SCN-VIDEO-003-A — A video of an existing identifier

- **WHEN** an authenticated caller requests a video it may read
- **THEN** the system returns `200 OK`

#### Scenario: SCN-VIDEO-003-B — Unknown or unreadable video

- **WHEN** the video does not exist, or the caller may not read it
- **THEN** the system returns `404 Not Found`

## REMOVED Requirements

### Requirement: REQ-VIDEO-004 — A video addresses its file

**Reason**: The `303 See Other` and its `Location` header are declared by the alias operation, so the API fuzz suite
validates that contract from the served document; the case also built the alias route by hand rather than following a
link.

**Migration**: No client change. The alias route stays served with the same `303` and `Location`.
