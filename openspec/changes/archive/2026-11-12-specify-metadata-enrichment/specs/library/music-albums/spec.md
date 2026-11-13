# Spec Delta

## REMOVED Requirements

### Requirement: External metadata is applied to an album

**Reason**: An album's external metadata is the enrichment pipeline's output, not part of the album's catalogue
contract. The requirement mixed the provider reference, the medium structure the apply materialises and the placements
it moves with the album's read and write surface, and it left the genre default and the rejected placement unstated.

**Migration**: Replaced by `REQ-METADATA-ENRICHMENT-024` for the authoritative write and `REQ-METADATA-ENRICHMENT-028`
for the album's field list, its medium structure, its placements, its genre default and its untouched cover and group.
No client-visible behavior changes: the route, payload and statuses stay as they are.
