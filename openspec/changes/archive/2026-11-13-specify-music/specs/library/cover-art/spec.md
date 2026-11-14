# Spec Delta

## REMOVED Requirements

### Requirement: Cover art is optional

**Reason**: The capability moves to the top-level `cover-art` area, which states the contract without enumerating the
resources that carry art.

**Migration**: Replaced by `REQ-COVER-ART-001`.

### Requirement: Cover art references an image file

**Reason**: The reference and scope rules are split into the image reference and the scope that follows the decorated
**Resource**, so a catalogue cover and a playlist cover are stated once each.

**Migration**: Replaced by `REQ-COVER-ART-002` and `REQ-COVER-ART-003`.

### Requirement: A cover exposes a file link

**Reason**: The capability moves to the top-level `cover-art` area unchanged in substance: the link still follows the
cover the resource stores.

**Migration**: Replaced by `REQ-COVER-ART-004`.

### Requirement: A cover is never inherited

**Reason**: The ban is kept deliberately: the server reports only the cover a resource stores. A resource that owns no
artwork — an episode or a recording — reports none, and the client reads the cover of the album, season or show it
already fetched. Resolution would make a derived field caller-dependent and disclose the containing resource.

**Migration**: Restated by `REQ-COVER-ART-006`: a resource that owns no artwork reports none and inherits nothing. The
write outcomes are `REQ-COVER-ART-005`.
