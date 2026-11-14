# Spec Delta

## REMOVED Requirements

### Requirement: External metadata is applied to a film

**Reason**: A film's external metadata is the enrichment pipeline's output, not part of the film's catalogue contract.
The requirement mixed the provider reference, the authoritative write and the fields the write owns with the film's read
and edit surface, so the film capability could not change without restating the pipeline.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-024` for the authoritative write and `REQ-METADATA-ENRICHMENT-025`
for the film's field list, cover and video. No client-visible behavior changes: the route, payload and statuses stay as
they are.
