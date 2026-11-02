# Design

## Context

`music_album` requires a name and a release date and has no group column; the group relation is only the many-to-many
`music_album_group`. `music_recording` requires `first_release_date`. `music_medium.type` admits only `CD`. There is no
seed for a default album, group or genre. The schema is unreleased (`PERS-014`). See `proposal.md` for motivation.

## Goals / Non-Goals

**Goals:**

- An album identity of `(name, group)`, enforceable by a unique index.
- Absence of a release date for the bucket and for albums the provider does not date.
- A medium that can represent a digital release and separate CDs.
- A bucket that cannot be renamed or deleted.

**Non-Goals:**

- The group HTTP resource (change 8).
- The write endpoints (change 9).
- Creating recordings at ingest (change 10).

## Decisions

### `music_group_id` on the album, not a text artist

The album references the existing `music_group` aggregate. `UNIQUE(name, music_group_id)` is the idempotency key for
album creation; `music_album_group` remains for the full credit list. A text `artist` column would duplicate the group
and could not carry a cover.

### Nullable `release_date`

The identification contract already treats an unreported date as absent. A sentinel date would fabricate one. The read
representation becomes `null`-able; the default bucket stores no date.

### Remove `first_release_date` entirely

A recording is bound to one file and, in this catalogue, placed in one album; the album's release date is the only date
the model needs. Removing the column removes it from the domain, both repositories, both read representations and the
ERD. The identification document also loses it (change 7).

### `CD` and `DIGITAL` only

A medium exists mainly to separate the discs of a multi-CD release. Any release that is not multi-CD becomes one
`DIGITAL` medium. `MediumIndex` becomes a type plus an index, with `Cd` and `Digital` variants. The check constraint
admits exactly these two values.

### Seeded bucket

Group 0, album 0 and its `DIGITAL` medium are seeded, linked to `genre('UNKNOWN')`. Triggers mirror the default gallery:
delete and update are refused. The bucket is readable like any album; it is not hidden.

## Risks / Trade-offs

- **Album sharing across groups** → `UNIQUE(name, music_group_id)` allows the same title for two groups and rejects a
  duplicate for the same group. It cannot distinguish two same-named albums by one group; that is accepted.
- **Removing `first_release_date` is destructive** → the schema is unreleased and no production data exists; the
  development database is reset.
- **The bucket appears in album listings** → intended; a later `is_default` flag can hide it if the product wants that.
- **A `DIGITAL` medium for a single CD** → follows the rule that only multi-CD releases get CD mediums.

## Migration Plan

1. Edit the create migrations and add the seed migration; reset the development database.
2. Verify the bucket exists with group 0, the `UNKNOWN` genre and one `DIGITAL` medium.
3. Verify the protection triggers reject a delete or rename of group 0, album 0 and the bucket medium.

## Open Questions

- Should the bucket be hidden from `GET /library/music/album`? The design leaves it visible.
