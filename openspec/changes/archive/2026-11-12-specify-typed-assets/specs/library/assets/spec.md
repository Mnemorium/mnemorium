# Spec Delta

## REMOVED Requirements

### Requirement: A typed image is readable

**Reason**: The image entity and its characteristics are owned by the `image` capability.

**Migration**: Read the image through `REQ-IMAGE-003`; the route, representation and links are unchanged.

### Requirement: A typed image addresses its file

**Reason**: The file alias belongs to the image entity, which owns its backing file.

**Migration**: Read `REQ-IMAGE-004`; the route and `303` outcome are unchanged.

### Requirement: A typed audio is readable

**Reason**: The audio entity and its characteristics are owned by the `audio` capability.

**Migration**: Read `REQ-AUDIO-003` and `REQ-AUDIO-004`; the route, representation, links and file alias are unchanged.

### Requirement: A typed video is readable

**Reason**: The video entity and its characteristics are owned by the `video` capability.

**Migration**: Read `REQ-VIDEO-003` and `REQ-VIDEO-004`; the route, representation, links and file alias are unchanged.

### Requirement: An upload names its media row after the uploaded file

**Reason**: The name is a characteristic of an image or an audio, not of the serving capability; the `video` row has no
name. The intake capabilities keep the rule that chooses the name.

**Migration**: Read `REQ-IMAGE-002` and `REQ-AUDIO-002` for the name characteristic. The original-name rule stays with
`upload`; the path-derived rule stays with `library/media-ingest`.
