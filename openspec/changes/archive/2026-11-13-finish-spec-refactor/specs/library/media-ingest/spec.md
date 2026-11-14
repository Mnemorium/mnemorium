# Spec Delta

## REMOVED Requirements

### Requirement: Registered files share one fixed root

**Reason**: The **Library root** every registered file is addressed under is owned by `media-ingest`.

**Migration**: Read `REQ-MEDIA-INGEST-001`; the single fixed root and root-relative addressing are unchanged.

### Requirement: An absent library root is decided at startup

**Reason**: The startup decision on an absent root is owned by `media-ingest`.

**Migration**: Read `REQ-MEDIA-INGEST-002`; the wait window, the refusal and the empty reconciliation are unchanged.

### Requirement: Ingest does not delay readiness

**Reason**: The boundary between the scan and readiness is owned by `media-ingest`.

**Migration**: Read `REQ-MEDIA-INGEST-003`; the non-blocking scan and the conditional startup wait are unchanged.

### Requirement: Media type is determined from content

**Reason**: Content-based type detection is owned by `media-ingest`.

**Migration**: Read `REQ-MEDIA-INGEST-004`; the name and extension are still ignored.

### Requirement: Only supported image, video and audio files are registered

**Reason**: The accepted set and the skip rule are owned by `media-ingest`.

**Migration**: Read `REQ-MEDIA-INGEST-005`; an unsupported or non-media file is still skipped without aborting the scan.

### Requirement: Registration records the file and its media row together

**Reason**: The atomic registration of a file and its media row is owned by `media-ingest`.

**Migration**: Read `REQ-MEDIA-INGEST-006`; the probed characteristics and the all-or-nothing rule are unchanged.

### Requirement: A registered file carries the size it has on disk

**Reason**: The measurements a registered file records are owned by `media-ingest` and `file`.

**Migration**: Read `REQ-MEDIA-INGEST-007` and `REQ-FILE-001`; the size and modification instant are unchanged.

### Requirement: A file the scan did not store belongs to the Root Admin

**Reason**: The attribution of a found file is owned by `media-ingest`.

**Migration**: Read `REQ-MEDIA-INGEST-008`; the attribution to the **Root Admin** is unchanged, and the who-may clause
it carried is now `authorization`'s.

### Requirement: A registered file is verified rather than re-registered

**Reason**: Recognising a file it has already recorded is owned by `media-ingest`.

**Migration**: Read `REQ-MEDIA-INGEST-009`; the untouched, changed and removed outcomes are unchanged.

### Requirement: A media row the scan can complete is completed

**Reason**: Completing a missing media row is owned by `media-ingest`.

**Migration**: Read `REQ-MEDIA-INGEST-010`; the completion is unchanged.

### Requirement: The ingest never changes the content it examines

**Reason**: The read-only boundary of the ingest is owned by `media-ingest`.

**Migration**: Read `REQ-MEDIA-INGEST-011`; content is still left byte-identical.

### Requirement: Removing a gallery reference never deletes content

**Reason**: The rule that a removed reference leaves the content alone is owned by `media-ingest`.

**Migration**: Read `REQ-MEDIA-INGEST-012`; the content under the **Library root** is still untouched.

### Requirement: A file added while the server runs is registered without a restart

**Reason**: Observing the root while the server runs is owned by `media-ingest`.

**Migration**: Read `REQ-MEDIA-INGEST-013`; the in-flight file, in-place modification and no-restart rules are
unchanged.

### Requirement: A renamed file keeps its record

**Reason**: Following a renamed file is owned by `media-ingest`.

**Migration**: Read `REQ-MEDIA-INGEST-014`; the record and the media row follow the file.

### Requirement: A removed file is reconciled

**Reason**: Reconciling a removed file is owned by `media-ingest`.

**Migration**: Read `REQ-MEDIA-INGEST-015`; the one-unit-of-work removal, the retained record on failure and the
no-identifier-reuse rule are unchanged.

### Requirement: A removal removes what depends on it

**Reason**: The cascade a removed record triggers is owned by `media-ingest`.

**Migration**: Read `REQ-MEDIA-INGEST-016`; the cover, gallery item, film, episode, recording and placement outcomes are
unchanged, and the content under the **Library root** is still untouched.
