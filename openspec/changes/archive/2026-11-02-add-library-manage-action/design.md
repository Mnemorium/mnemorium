# Design

## Context

`Action` is a closed enum of eighteen variants with one wire value each. The catalogue items (`Movie`, `Series`,
`MusicAlbum`) are unowned and globally visible, so `OWNED` and `PUBLIC_OR_OWNED` cannot describe them. See `proposal.md`
for motivation.

## Goals / Non-Goals

**Goals:**

- One action that guards every catalogue write.
- An admin-only default that a per-user override can widen or narrow, including per aggregate.
- Denial semantics consistent with the rest of the catalogue: conceal an unreadable item, reveal a readable one.

**Non-Goals:**

- The music write endpoints, which arrive in change 9.
- `file.use`, which continues to guard file writes.

## Decisions

### A dedicated `library.manage` rather than reusing `library.read`

Read and write have different defaults and different denial behavior. Reusing the read action would force every reader
to become a writer, and splitting the scope would require `library.read` to carry two meanings.

Alternative considered: a `library.write` name. Rejected to match the existing `gallery.manage`, `playlist.manage` and
`permission.manage` vocabulary.

### Admin-only preset, `NONE` for Standard

The catalogue has no owner, so there is no `OWNED` preset to grant. `NONE` for a Standard User means a reader cannot
rewrite shared metadata; `ALL` for an Admin means the operator can curate it.

### `ALL | LISTED | NONE` only

Introduce `LIBRARY_MANAGE_SCOPES` and return `PermissionError::UnsupportedScope` for `OWNED` and `PUBLIC_OR_OWNED`. The
existing `Permission::try_new` path already maps that error to `422 Unprocessable Content`.

### `read_action = LibraryRead`, `conceals_denial = false`

A caller who can read but not manage receives `403 Forbidden`; a caller who cannot read receives `404 Not Found`. That
is the same shape as `gallery.manage` and `playlist.manage`, and it avoids revealing an unreadable item through a write
attempt.

## Risks / Trade-offs

- **An Admin can rewrite any catalogue item** → intended for a self-hosted server; an override can narrow it to `LISTED`
  or `NONE`.
- **A new action grows the fixed matrix** → the action is additive and no existing scope changes.
- **A caller with `library.read` `LISTED` but `library.manage` `ALL`** can manage an album it cannot list → the two
  actions resolve independently, as they do for galleries. Documented, not changed.

## Migration Plan

No datastore change. Ship the action with the code; existing overrides remain valid.

## Open Questions

None.
