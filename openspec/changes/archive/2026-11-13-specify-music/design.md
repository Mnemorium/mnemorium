# Design

## Context

See `proposal.md` — Why. The behaviour is implemented; what follows is the state that shapes how it is stated.

The music surface is one route family, `/api/v1/library/music/*`, backed by three aggregates that share one table
family: `music_group`, `music_album`, `music_medium`, `music_recording`, `music_playlist` and `music_track`.
`music_track` holds either a medium placement or a playlist placement, and `uq_music_track_medium_recording` allows a
recording **one** medium placement across the whole catalogue.

Artwork is six nullable `cover_image_id` foreign keys into `image` (`movie`, `series`, `season`, `music_album`,
`music_group`, `music_playlist`). A trigger pair per table fixes the image's `scope` (`LIBRARY` for a catalogue cover,
`USER` for a playlist cover), refuses a scope that does not match, and allows one referrer. `music_recording` and
`episode` have no cover column and own no artwork.

`authorization` replaced the permission model in the same branch this change is authored on. Its decision is one
`Action` over one `ResourceRef`; `test/system/authorization.py` is the primary proof, and feature specs are expected to
name the action without restating the rule (the glossary's **Who-may rule**).

`specify-metadata-enrichment` is in flight. It retires `library/music-identification` and removes the album apply
requirement from `library/music-albums`, so this change retires a capability that change is also editing.

## Goals / Non-Goals

**Goals:**

- Publish one `music` capability for the whole route family and one `cover-art` contract capability.
- Repoint every music who-may statement at the action the route decides, and remove the role restatements.
- State that a resource's cover is the artwork it stores, and that a container's cover is read from the container.
- Prove both capabilities black box, one module each, with the three Allure labels.

**Non-Goals:**

- No route, request field, response field or status changes; no Rust module or handler is renamed.
- No business use case is added. The music catalog gap (no group or album-curation entry) stays where
  `specify-metadata-enrichment` records it.
- Gallery items, films, shows and seasons are not re-authored; only the artwork contract they use moves.
- Provider artwork is still downloaded and discarded by an apply; persisting it is `metadata-enrichment`'s deferred
  work.
- A track keeps `file_id` and gains no `_links.file`; playback links are an `assets` concern and a separate change.
- The album metadata update has no requirement in this change. The operation keeps its current behavior but is left
  unspecified until another capability owns it.
- The gallery route's missing `CreateChild` check is the same defect as the playlist one but out of this area; it is
  recorded, not fixed here.

## Decisions

### 1. One `music` capability, top-level

**Decision:** replace the three music capabilities with `openspec/specs/music/spec.md`, one `REQ-MUSIC-*` sequence.

**Why:** the three are observed through one route family and one client journey — an album links its group, a playlist
links the same recordings an album places, and a track representation is the same shape in both. The difference between
them is per-requirement (a catalogue write versus an owned resource), not per-capability, and it is visible in the
action each requirement names. One capability gives one identifier sequence, one `Music` epic and one proof module for
the capability.

**Alternative considered:** `library/music/{albums,groups,playlists}`. It keeps the catalogue/user-content seam as a
spec boundary, but triples the delta surface and splits one route family's proof across three modules for a seam the
spec text already makes explicit.

### 2. `cover-art` is a top-level contract capability, like `api`

**Decision:** move artwork out of `library/` to `openspec/specs/cover-art/spec.md`, stated for any **Resource** that
carries art, with no resource list.

**Why:** the contract spans catalogue resources, user content and video, and no single bounded context owns it. The old
requirement enumerated the resources that carry art, which is exactly how `music_group` came to be missing. A contract
capability is precedented by `api`, which owns the shared HTTP shape and has no routes of its own; cover art's rules
surface as field values, rejections and links on other capabilities' routes.

**Alternative considered:** keep `library/cover-art` and add the group to its enumeration. Rejected: it repeats the
enumeration defect and files a video concern under the library's content browsing area.

### 3. One cover field, and it is the one the resource stores

**Decision:** `cover_image_id` is the resource's own stored cover. There is no second, resolved field, and
`_links.cover` follows `cover_image_id`.

**Why:** a client editing a representation round-trips exactly the value the resource stores, and one name carries one
meaning on both read and write. The schema already stores the cover on the resources that own one; a resolved value adds
a caller-dependent field and a resolution read to every child route without adding information the client cannot get
from the containing resource.

**Alternative considered:** `cover_image_id` as the stored value plus an `effective_cover_image_id` for the resolved
one. Rejected: it publishes two covers per representation, makes the link's target depend on the caller, and forces an
album lookup into every track read.

### 4. Nothing is inherited; the containing resource carries the cover

**Decision:** an album, group, film, show, season and playlist report the cover they store. An episode and a recording
carry no cover field and no cover link; a client renders their artwork from the season, show or album it reads beside
them. An album with no cover reports `null`, never its group's.

**Why:** an album is a release and its artwork identifies that release; the group's artwork identifies the performer,
and borrowing it would label a release with art it does not have. A recording is placed in exactly one album, so the
client has one unambiguous source. Client-side collection keeps the server's representations independent of caller and
of the tree walk the client already performs.

**Alternative considered:** resolve season → episode and album → recording on the server. Rejected: it adds a
caller-scoped field, an authorization decision per inherited value, and a second source of truth for the same image.

### 5. No resolved cover means no caller-scoped derived value

**Decision:** the change adds no value derived from a resource the caller may not read. Every cover a representation
reports comes from that representation's own row.

**Why:** the disclosure concern only exists because a resolved field could name artwork from an unreadable source.
Removing resolution removes the whole class: there is no derived cover to withhold, and `_links.cover` always points at
an image the resource itself references.

### 6. The write stays on the resource, and is tri-state everywhere

**Decision:** each resource that stores art keeps `cover_image_id` in its own create or patch body. A freshly written
field is tri-state: absent keeps, `null` clears, an identifier sets. `music` gains the missing group-clear and
playlist-set paths; `cover-art` states the three outcomes once.

**Why:** every current write is atomic with the resource it decorates, and a cover is metadata of that resource. The
tri-state is already the convention in `FieldUpdate`; the group's plain `Option` and the playlist's absent field are the
two places that diverge.

**Alternative considered:** one `PUT/DELETE .../cover` operation for every resource. Rejected: it is a breaking API
change, adds a second way to set a cover during creation, and duplicates what the create body already does.

### 7. The music spec names the action, never the rule

**Decision:** each write requirement names the action (`CreateChild` with its child kind, `EditMetadata`, `Delete`) and
the resource it is decided against. Role names and "only the owner" statements leave the music spec; the observable
denial and concealment outcomes stay only where `authorization` does not already own them.

**Why:** the glossary's **Who-may rule** assigns the rule to the capability that owns it. The retired specs cited
permissions that no longer exist, so every one had to change anyway.

**Alternative considered:** keep the old wording and add a note that it is historical. Rejected: it would publish a
contract that cannot be satisfied.

### 8. The two missing checks are added where `authorization` already promises them

**Decision:** creating a playlist decides `CreateChild` for a music playlist against the playlist collection, and
appending a recording decides `CreateChild` on the playlist **and** `ViewMetadata` on the recording.

**Why:** `REQ-AUTHORIZATION-023` seeds the creation grant and `REQ-AUTHORIZATION-026` requires both checks for a
playlist entry. The implementation performs neither, so the gallery path's pair is the only one enforced. The spec
cannot bless the omission.

**Alternative considered:** leave the code and state the weaker rule. Rejected: it would contradict `authorization` and
make a `Deny` on a collection or a recording unenforceable through these routes.

### 9. Proof moves to the system suite

**Decision:** `test/system/music.py` proves the `music` scenarios and `test/system/cover_art.py` proves the `cover-art`
scenarios. The E2E modules UC-012 to UC-019 are reduced to their business flow, and a case the system module proves is
deleted. Rust tests remain for provider fixtures and mechanisms the container cannot observe.

**Why:** the system suite is the declared proof for a capability's scenarios (`TEST-053`–`TEST-058`), and both modules
need their own container fixtures — a catalogue with a group, an album with mediums, and two callers with different
grant sets.

### 10. Sequencing

**Decision:** author against the tree after `specify-metadata-enrichment`. The album apply retirement here becomes a
no-op if that change lands first; when this change lands first, apply would briefly have no owner.

## Risks / Trade-offs

- **A client must know which resource owns the art** → the mapping is the containment it already navigates: a track
  names its album, an episode names its season and show, and a season is read under its show. No new route is needed.
- **The playlist track representation loses a field the change had proposed** → the field never shipped; the archived
  `library/music-playlists` requirement said only what the representation carries, and the replacement states the
  no-artwork rule plainly.
- **A season stores its own cover while its show stores another** → a season keeps reporting its own; the season's cover
  is editable, so the client never needs the show's when it has the season. The no-inheritance rule keeps that
  unambiguous.
- **Enforcing playlist creation may refuse a caller who relied on the missing check** → no default caller is affected;
  the Standard and Admin roles hold the seeded grant.
- **Retiring a capability another open change edits** → sequencing above; the removal is written to tolerate an
  already-absent requirement.
- **A large removal delta** → the four retirement files name every retired requirement with its replacement, so the
  archive keeps a traceable mapping.

## Migration Plan

No persistence, route or payload migration. The change is specification plus the corrections the specification forces:
the two authorization checks, the group's tri-state cover, the playlist's cover field, and the removal of cover
resolution from the season, episode, album-track and playlist-track reads. Rollback is reverting the change.
