# Spec Delta

## REMOVED Requirements

### Requirement: External metadata is applied to a show

**Reason**: A show's external metadata is the enrichment pipeline's output, not part of the show's catalogue contract.
The requirement mixed the provider reference, the authoritative write and the fields the write owns with the show's read
and edit surface, so the series capability could not change without restating the pipeline.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-024` for the authoritative write and `REQ-METADATA-ENRICHMENT-026`
for the show's field list and the children an apply leaves untouched. No client-visible behavior changes: the route,
payload and statuses stay as they are.

### Requirement: External metadata is applied to an episode

**Reason**: An episode's external metadata is the enrichment pipeline's output, not part of the episode's catalogue
contract. The requirement also restated the position-match rule that identification already states, splitting one rule
across two capabilities.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-024` for the authoritative write and `REQ-METADATA-ENRICHMENT-027`
for the episode's field list, its position match and its video. No client-visible behavior changes: the route, payload
and statuses stay as they are.
