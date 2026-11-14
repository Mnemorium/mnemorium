# Spec Delta

## REMOVED Requirements

### Requirement: Cover art is optional

**Reason**: The optional cover is owned by `cover-art`, which states the shared contract once.

**Migration**: Read `REQ-COVER-ART-001`; a resource without artwork still reports none.

### Requirement: Cover art references an image file

**Reason**: The reference a cover stores is owned by `cover-art`.

**Migration**: Read `REQ-COVER-ART-002` and `REQ-COVER-ART-003`; the image row the cover names and the scope its first
attachment fixes are unchanged.

### Requirement: A cover exposes a file link

**Reason**: The link a cover publishes is owned by `cover-art`.

**Migration**: Read `REQ-COVER-ART-004`; `_links.cover` still names the image resource.

### Requirement: A cover is never inherited

**Reason**: The boundary that a resource owning no artwork reports none is owned by `cover-art`.

**Migration**: Read `REQ-COVER-ART-006`; an episode and a recording still report no cover.
