# Design

## Context

`test/system/` already shares one server instance per pytest worker, but only the URL and the token are shared: each
case still starts its own container unless it opts into the shared one, and the modules that need existing content read
`mnemorium.db` with `sqlite3` to find it. See `proposal.md` for why that is a problem. Three constraints shape the
approach:

- `src/lib/infrastructure/seed.rs` is compiled only under the `test-seed` feature and applies one fixture selected by
  `MNEMORIUM_SEED_PROFILE`; `src/lib/app.rs` maps everything that is not `perf` to the API fixture, so "no fixture"
  cannot be expressed.
- The API and performance fixtures live under `test/seed/{api,perf}`, outside the suites that consume them.
- OpenSpec refuses a **MODIFIED** requirement that omits a scenario the main spec still has, so a delta can add
  scenarios and rewrite their bullets but cannot drop one; only a whole requirement can be removed. This change
  therefore keeps every scenario whose _proof_ moves and removes only the three file-alias requirements.

## Goals / Non-Goals

**Goals:**

- One deterministic, browsable fixture that a worker's shared instance can carry for the thirteen reworked modules, with
  identifiers a case can name without querying the datastore.
- No system case reads the datastore, and no system case builds a route from a literal.
- No reworked case restates a route, a body or a header the served document declares: it drives the generated client, so
  the declaration is the only place a request's shape is written.
- A closed, explicit rule for when a case keeps its own container.
- One product surface — the stored-file collection — that makes a stored file reachable by the path a case owns.
- A task list split into reviewable, separately committable chunks, one per scope the repository's governance defines.

**Non-Goals:**

- Adopting the seed in the modules this change does not rework (`series.py`, `gallery.py`, `api.py` and the rest).
- Rewriting the Schemathesis or performance suites beyond moving their fixture trees and naming their profiles.
- The performance measurement tables, a `credential` link on the account representation, or `TechnicalDesign.md`'s
  "group 0" wording.
- Changing how any product operation decides an action; the product additions are one listing route and one resource row
  the ingest writes.

## Decisions

### 1. The profile is a configuration setting, not an environment variable

`seed.profile` joins the domain configuration behind the `test-seed` feature, with a case-insensitive `Deserialize` (a
preprocessor, since `rename_all = "lowercase"` is case-sensitive) and an unknown value rejected as a configuration error
naming the setting. `MNEMORIUM_SEED_PROFILE` reaches it through the envy adapter's existing prefix/path rule, so no
adapter field beyond `seed_profile` is needed. Absence means no fixture.

_Alternatives:_ keep reading the variable in `app.rs` (cannot express "no fixture" and bypasses the layering); a boolean
`seed.enabled` beside the variable (two sources for one decision). The `applied` report names the setting and never its
value, per `OBS-004`. `default_config.yaml` is untouched, so a shipped build rejects a `seed:` key like any unknown key.

### 2. One Cargo feature compiles every profile

`test-seed` gains a third value in `SeedProfile` rather than a feature per profile. Every suite's build already passes
one feature, and `seed.rs` becomes a small registry:

```text
apply(pool, pepper, library_root, profile: Option<SeedProfile>)
    None        -> Ok(())
    Some(Api)   -> seed_api(...)      // identities, files, _sqlx_seeds_api
    Some(Perf)  -> seed_api(...); seed_perf(...)
    Some(System)-> seed_system(...)   // identity, files, _sqlx_seeds_system
```

_Alternative:_ a per-profile feature — doubles the build matrix for no size win, because the profiles share the writer,
the identity seed and the file seed.

### 3. Each suite owns its seed tree

`test/seed/{api,perf}` move to `test/api/seed/` and `test/performance/seed/`, and the system suite gets
`test/system/seed/{sql,media}`. The performance profile still applies the API fixture first, and its bulk bases stay
generated at runtime. Ownership follows the consumer, so a suite's fixture is deleted with the suite and the embedded
paths stay greppable.

### 4. The system fixture seeds an identity, public files and per-module rows

The profile seeds one Standard user, `coverart`, with its own fixed password beside `SEED_PASSWORD`, hashed with the
live pepper exactly as `seed_identities` does, and the rows `register_user` writes (a `USER` resource under `INSTANCE 0`
with the Admin owner grant). Every system file row is `is_public = 1` with a `PUBLIC` `FILE` resource, because the
module that proves covers must follow a cover link, and `get_image` authorises on the `FILE`. Seeded playlists are
`PRIVATE` and owned by `coverart`; seeded library rows are `PUBLIC`.

Each module's `resource` rows mirror what the create use cases write, and audio, video and image rows carry the
measurements a probe would report for the committed bytes, so a typed representation's read target matches its declared
field set.

### 5. The identifier band is fixed, blocked and documented

The system profile owns `10000`–`19999`, one 1000-wide block per module with the same internal layout (media range,
catalogue range), and the SQL header documents the slot order. Ten identifiers per case for cover art, eight for music
and movies; audio and file need no per-case slot. A new module claims the next free block.

_Alternatives:_ sequential identifiers assigned at seed time (a case cannot name one without reading the database);
UUID-like ranges (unreadable in a failure report).

### 6. The shared instance is the default; the exceptions are closed

A case runs shared unless its subject is one of: `first_boot`, `restart`, `global_filesystem`, `log_sink`,
`exact_collection` or `process_state`. The reason set already exists in `conftest.REASONS`, and the module-level
`pytestmark = own_instance(...)` goes away because the subject, not the module, decides.

Where a case observes a value a shared instance could satisfy from another case's work, it filters on a value it owns:
an album name shared by the case's own two albums, a provisioned owner's identifier, a session identifier, a file path
it wrote, or a log field carrying its own actor. `TEST-058` forbids asserting on a global collection another case can
extend, so a whole-collection claim keeps a container — user's `010-A`, file's own boot case, upload's directory and
startup-setting claims.

### 7. The stored-file collection replaces the datastore

No route lists stored files today, so this change adds `GET /api/v1/asset/file`:

- a `ListFilesUseCase` with a `ListFilesCommand`/`Response`/`Error` and a port, implemented like `list_galleries` and
  `list_movies`: per-item `Discover` through `allows_many`, `limit`/`offset` with the 20 default and 1–100 range, a
  templated `_links.search`, `next`/`prev` and a `self` that preserves the view;
- one exact root-relative `path` filter, so a case asks for the file it owns rather than a prefix match;
- items are the single-file representation, so `_links.self` of an item is the route `get_file` serves;
- a plain `file` relation in the API index, so the collection is published like every other context entry; a case drives
  it through the generated client's `list_files` with the exact path it owns.

The ingest's `register` also writes the `PRIVATE`, Root-Admin-owned `FILE` resource that `complete_upload` writes,
tolerating `AlreadyExist`, so a scanned file becomes discoverable by the identifier its own link promises and a rescan
stays idempotent.

_Alternative:_ let each module keep one SQLite read behind a helper — cheaper to build, but leaves the datastore as the
suite's second API and makes a row bug invisible to the served representations.

### 8. A sink is read as a collection, a file sink never

The shared instance's standard output is a stream of records, so the same owned-value technique applies: a record
carrying an `actor`, a `claimed_identity`, an `upload` identifier or a query sentinel is proved shared, while a record
whose fields are static (a provider, a stage, a count, a route) keeps a fresh sink. A file-sink assertion always keeps
its own container, because the file is complete only after `stop()`.

_Alternative:_ keep one container per logging case — 38 containers, most of them for a record the shared stream already
carries.

### 9. Three proofs leave the black-box suite, and their scenarios stay in the spec

A declared status, a media type or a `_links` field is the served document's contract, which Schemathesis validates; a
permission rule is `authorization`'s; a domain-invalid configuration value is `load_configuration`'s; a sealing race is
a use-case test's. The system suite stops proving them, and the task list names the owner. The scenarios remain in the
main specs, because the behaviour still holds and a specification states behaviour rather than its proof. Only
`REQ-AUDIO-004`, `REQ-IMAGE-004` and `REQ-VIDEO-004` are removed whole: each has one scenario whose only subject is a
declared `303`.

### 10. The seed module loses its unit tests and leaves the coverage gate

`seed.rs` becomes embeds, writes and SQL application with no branching logic worth a unit test; the media write, the
metadata read and the perf link step were its only assertions, and the perf dataset gate plus every suite run cover
them. The file joins the coverage exclusion beside the REST handler tree, and `.sqlfluffignore`'s rationale cites the
performance dataset gate instead of the deleted test.

### 11. The task list is split per committable scope

Each section of `tasks.md` is one scope from `GOV-001` and one reviewable commit: the mechanism, the stored-file
collection, the moved fixture trees and tooling, one section per reworked module, the shared helpers, the suites that
only lose imports, the documentation, and the OpenSpec change itself. A module's section can be reviewed and reverted
without touching another module's cases.

### 12. The suite drives a client generated from the document, never a hand-built request

Every declared operation a reworked case drives comes from the client `openapi-python-client` emits from the served
document, so a case names an operation and its arguments instead of a path, a body shape and a header. The client is
generated from the image under test: `script/system.sh` builds that image with `api-docs test-seed` — the pair the
performance image already uses — and `script/openapi_client.sh` reads `/doc/openapi.json` from a throwaway, unseeded
container before pytest. The output lives under `test/generated/`, which is ignored, so nothing is committed and no case
runs against a client the handlers have outgrown.

Two shapes stay hand-built, each with a one-line reason. A link the case's subject is gets read from the representation
and followed by its `href` (`link_of`, `Sut.follow`), because the published link is the assertion. A request the
document cannot declare goes through `Sut.request`: `HEAD` on a content route, the operator-configured client-address
header, and the `/health` probe, which sits outside the `/api/v1` base the client is bound to.

A request the client cannot express is a missing declaration, so the fix is in the product declaration rather than the
test: a path, query or header parameter, a request body, a response body or a status is added to the handler's
`#[utoipa::path(...)]`/`ToSchema`, and the fuzz suite covers it in the same change. The single generator-side alias is
the content responses' dynamic media type (`*/*`), which `test/openapi_client.yaml` maps onto
`application/octet-stream`; generation fails on any other warning, so no operation or response is dropped silently. A
declared field is read from the model, whose `None`/`UNSET` distinction keeps "cleared" apart from "omitted", while a
field-set assertion reads `payload(response)`, because a model drops what the document does not declare.

_Alternatives:_ keep building requests from the index and from links (the state this change replaces: every case
restates a route, a body shape and a header the document already declares, and a rename surfaces as a broken test rather
than a changed type); commit the generated package (a reviewed artifact that rots, which this repository already refuses
for the performance suite); generate from a local debug build (the CI job has no Rust toolchain, and the image under
test is the honest source).

## Risks / Trade-offs

- **Seeded rows reach the other shared cases** → every seeded row is `PUBLIC` library data plus `coverart`'s private
  playlist and gallery; the parallel run is the proof, and a module that needs isolation claims its own block.
- **No seed unit tests** → the perf dataset gate applies those scripts and asserts their ranges, and every suite run
  exercises the writes; `.sqlfluffignore`'s rationale is reworded.
- **A shipped build rejects `seed:`** → documented in the seed-data prose; the setting is accepted from `config.yaml`
  only, never written into `default_config.yaml`.
- **One product route rides a test change** → `GET /api/v1/asset/file` and the ingest's `FILE` resource ship as their
  own `feat(asset)` commit with use-case tests and a Schemathesis run, so the suite commits can be reviewed apart.
- **The bare fixtures claim a media type they cannot back** → `file-bare.png` and `download-bare.png` carry undecodable
  bytes under a media name, because a probe that succeeded would backfill a typed row; a probe that learns to accept
  them fails `003-D`/`007-B` loudly, which is the intended alarm.
- **The collection is load-bearing for four modules** → file's `003-A`/`004-C` are its contract proof, and the ingest's
  resource write is what makes a scanned record discoverable; a change to the exact-path filter breaks four modules at
  once, which is the point of keeping their proofs there.
- **A shared observer can never use the ingest report** → the report names no file, so its counts cannot be attributed
  to a case; shared ingest cases poll the collection by path, and a future report field carrying a file would break
  `REQ-MEDIA-INGEST-016`, not just a case.
- **The shared sink is a global stream** → a shared logging case proves its record by the value it owns; a record with
  only static fields keeps an own sink, and the module docstring states the rule for the next module that reads a sink.
- **A scenario's identity is stable, so a broadened one keeps its title** → OpenSpec compares scenario names when a
  requirement is modified, so `SCN-AUTHORIZATION-001-C` keeps "unknown resource" while its WHEN now covers an unreadable
  one, and the two music-catalogue creation refusals are a new `SCN-AUTHORIZATION-023-G` rather than a relabelled
  `023-B` whose title names a film. The label a case carries is the scenario's id, and no reader is sent to a different
  behaviour.
- **The client is a prerequisite, not a committed file** → a bare `pytest test/system` stops with the command that
  produces it (`script/system.sh`, or `devenv tasks run system:client`), and the image under test now carries `api-docs`
  beside `test-seed`, as the performance image already does.
- **A generated model drops what the document omits** → a case whose subject is the field set reads `payload(response)`;
  a declared field keeps the model's `None`/`UNSET` distinction, so a case that confuses the two fails rather than
  passes silently.
- **A declaration the client needs may be a product change** → adding the parameter, body or status the suite drives
  extends what Schemathesis fuzzes in the same commit; that is the intended price of one written contract.
- **The change's new helpers are dead code until their callers land** → `[tool.vulture]` ignores them by name with a
  transition note, and the final verification section deletes those entries; a helper the reworks never call then fails
  the gate instead of lingering.
- **`REQ-AUTHORIZATION-040` restates `REQ-AUTHORIZATION-026`** → this change deliberately gives the moved recording case
  a requirement of its own instead of leaving the append refusal under the creation rule; a reviewer who prefers one
  statement should relabel the case `026-B` rather than keep both.
- **The `credential` identifier is inferred** → the index publishes `credential` only as a URI template and no
  representation names the credential its account holds, so rotation cases infer `1` for the root and `user_id + 1` for
  a fresh account, which holds on a fresh instance only. Adding the link is out of scope and recorded here.
- **Upload's refusals keep only their effect** → the specific `422`/`415`/`413`/`400` is Schemathesis's, so a fuzz suite
  that skipped a declared rejection would leave that status unproven — the same trade every reworked module makes.
- **The reaping report's proof rides an upload case** → `upload.py`'s reaper case gains `REQ-LOGGING-017` and
  `SCN-LOGGING-017-B`; a rewrite of that case must keep the report assertions its labels claim.
- **`series.py` loses its imports** → the non-reworked module inlines the three helpers it took from `video.py` in the
  same commit, so the reworked video module can be helper-free.
- **A shared instance exposes a delete that leaves its resource** → `delete_playlist` removed the playlist row but kept
  its `MUSIC_PLAYLIST` resource, so a later creation reused the freed identifier and collided with the stale resource.
  The use case now removes the resource in the same unit of work, with a unit test, keeping create-after-delete usable.
  `delete_user` had the same gap: its `USER` resource survived, so registering an account that reused the freed
  identifier answered `500`. It now removes the resource too, with unit tests for the resource and the transferred
  file's cleared digest. `delete_gallery` and `delete_gallery_item` had the same gap for `GALLERY` and `GALLERY_ITEM`: a
  deleted gallery left its resource, so the next gallery reused the freed identifier and creation answered `500`. Both
  use cases now remove their resource, with unit tests for the gallery, its items and a single item.

## Migration Plan

1. Land the mechanism: the setting, the profile dispatch, the moved trees and the profile names in the scripts. No
   database migration is involved; a `test-seed` boot starts unseeded until a caller names a profile.
2. Land the stored-file collection and the ingest's resource write as one `feat(asset)` commit with its unit tests.
3. Land the system fixture and the module reworks one section at a time, oldest proof first, so each module's cases can
   be run alone against the shared instance.
4. Land the documentation and the OpenSpec change last.
5. Rollback is per commit: the setting and the collection are additive, and a `NO_BUILD=1` run against a pre-change
   image fails the seeded modules with `404`s — the script's error message names `test-seed`, which is the intended
   signal.

## Open Questions

None. The profile vocabulary, the identifier band and the collection's contract are fixed here; the remaining module
inventory is a task-list detail that can be adjusted without changing a requirement.
