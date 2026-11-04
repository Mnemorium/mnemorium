# Design

## Context

Change 6 supplies the schema, seeds and medium types; change 7 supplies identification; change 8 supplies the group
resource. The existing playlist write path shows the PATCH-with-command pattern. See `proposal.md` for motivation.

## Goals / Non-Goals

**Goals:**

- One write path for manual metadata and one for external metadata.
- A placement is addressable and moving it is idempotent.
- Album apply never invents a recording.

**Non-Goals:**

- A full library scan.
- Creating a recording from a provider title; a recording is bound to an audio file.
- Deleting albums, mediums or recordings.

## Decisions

### Create is an upsert by `(name, group_id)`

`POST /album` returns `201 Created` when it creates and `200 OK` with the existing album when the identity exists;
`PATCH` is the only way to change an existing album. The unique index from change 6 is the idempotency key, so a retried
apply does not duplicate.

### `PUT` to the destination for placement

`PUT /album/{a}/medium/{m}/track/{t}` with `{recording_id}` places the recording at that address. The server deletes the
recording's previous medium placement and inserts the new one in one transaction. `409` when the slot holds another
recording; `200` when it already holds this one; `422` when the recording is unknown. A partial unique index on
`music_recording_id` for medium placements makes "the previous placement" unique.

Alternative considered: a `transfer` action on the source. Rejected because the destination is the new identity and
`PUT` is idempotent.

### `PATCH` the track edits the shared recording

The track path is the only address the client has. The body carries `name` and `isrc_code`; a `null` clears an optional
value. The change is visible in every playlist that references the recording, and the endpoint description says so.

### Album apply is album-and-medium unless placements are supplied

The provider reports track positions, not local `recording_id`s. Without `placements`, apply writes album fields, the
group, the genre links and the medium structure. With `placements`, it moves the named recordings in the same
transaction. It never creates a recording.

### The bucket is immutable

`album_id = 0` is rejected with `422`; the protection trigger from change 6 refuses the write even if the use case
misses it. Apply creates or finds a real album instead.

### Medium shape follows the release

More than one compact-disc medium → one `CD` medium per reported position. A single compact disc or any other format →
one `DIGITAL` medium. The rule is applied by the album create/apply path, not by the read model.

## Risks / Trade-offs

- **A provider failure during apply** → reported as an external-service failure; the transaction writes nothing.
- **A track move changes the address** → the response carries the new `self`; the old URI no longer resolves.
- **`PATCH` on a track edits every playlist that references the recording** → documented in the endpoint; the client
  already sees `recording_id` on the track.
- **An album without placements cannot recollect an existing bucket recording** → the client must supply the placement;
  that is the explicit mapping the future scan will provide.

## Migration Plan

No schema change beyond change 6. Ship the use cases and handlers; regenerate `docs/openapi.json`.

## Open Questions

- Should `POST /album` with an existing identity apply the body or return the existing album untouched? The design
  returns it untouched and leaves changes to `PATCH`.
