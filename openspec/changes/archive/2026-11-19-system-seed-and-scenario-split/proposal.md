# Proposal

## Why

Every case under `test/system/` boots its own container, and the modules that need existing content read `mnemorium.db`
directly to find it. That makes the suite slow and makes its assertions describe rows rather than the behaviour a client
observes. Seeding has no answer to it: the `test-seed` boot only knows an API fixture and a performance volume, a boot
that names no profile silently gets the API fixture, and no fixture describes a browsable catalogue. So a module like
music or movies builds its preconditions by uploading bytes and querying SQLite for what it just created.

Two other kinds of case have also accumulated there. A declared `400`/`415`/`422` status, a media type or a `_links`
field is the served document's contract, which Schemathesis already proves from it; and a permission rule proved by a
system case is `authorization`'s business. Keeping both in the black-box suite duplicates a proof, hides which suite
owns the behaviour, and pays a container for it.

## What Changes

- Add a `system` **seed profile** beside `api` and `perf`, applied by the server at boot from a feature-gated setting:
  `seed.profile` in `config.yaml` and `MNEMORIUM_SEED_PROFILE` in the environment, accepted case-insensitively, and
  **absent means no fixture at all**. An unknown value is refused naming the setting. One Cargo feature compiles every
  profile.
- Move each suite's **seed** tree under the suite that consumes it: `test/api/seed/{sql,media}`, `test/performance/seed`
  and a new `test/system/seed/{sql,media}`. The performance profile still applies the API fixture first.
- Make the system profile own a fixed identifier band (`10000`–`19999`, one 1000-wide block per module), seed the
  `coverart` identity with its own credential, seed its **stored files** as `PUBLIC` library files at **Library root**
  paths, and provide catalogue rows for cover art, music, movies, audio, download and file. No module seeds what every
  case creates for itself.
- Rework thirteen system modules to run their cases on the one instance a worker already shares, each case acting on a
  seeded row it only reads or on a value it creates — a provisioned account, a playlist, an **Upload session**, a
  gallery, a file under the root. A case keeps its own container only when its subject is process state, the global
  filesystem, a log sink or a whole unscoped collection.
- Move the permission-shaped cases from music, movies and file to `authorization`, which runs entirely shared as a
  result, and delete the duplicate refusals their neighbours already prove.
- Remove the declared rejections from the black-box suite (unknown `404`s, malformed `400`s, out-of-range `422`s,
  missing-token `401`s, alias `303`s), leaving them to the API fuzz suite, and remove the representation cases whose
  field sets and `_links` the served document declares. The logging verbosity refusals move to the configuration use
  case's tests. The scenarios themselves stay in their specifications, which state a behaviour rather than a proof; the
  task list names the suite that now proves each one.
- **BREAKING** (test tooling): a `test-seed` boot that names no profile now starts unseeded. Every caller names its
  profile — the system suite, the API fuzz script, the ZAP scan, the TypeScript workflow, the README's seeded run and
  the seeded-run rows in `AGENTS.md` — and `script/system.sh` builds with `CARGO_FEATURES=test-seed`. A shipped build
  still rejects a `seed:` key as unknown.
- Add one product surface so no system case has to read the datastore: `GET /api/v1/asset/file`, a paginated HAL
  collection of the stored files a **Caller** may `Discover`, filterable by one exact root-relative `path`, with a
  `file` relation in the API index. The ingest also registers the `FILE` **Resource** it omits today, so a file the scan
  found becomes reachable by the identifier its own link promises.
- Give the system suite a typed client generated from the served document: `script/openapi_client.sh` reads
  `/doc/openapi.json` from the image under test and writes `test/generated/`, which is never committed. A case then
  drives a declared operation through the client instead of restating a route, a body or a header, and generation fails
  on any document gap — so a request the suite could not express is declared in the handler's `utoipa` macros. Only the
  requests the document cannot declare (a `HEAD` on a content route, the configured client-address header, `/health`)
  keep a hand-built request, and a link the scenario proves is still followed from its representation.
- Move the eight HAL helper unit cases from `test/system/hal_test.py` to `test/api/`, so the black-box suite no longer
  collects them.
- Revise ten existing specifications to match: the scenarios a case no longer proves stay in the spec where the
  behaviour still holds, and only the three file-alias requirements — whose whole subject is a declared `303` — are
  removed. The remaining edits broaden a scenario to the cases that now prove it, name the music default album and group
  as terms, add the logging severity ladder and the disabled-colour scenario, add the stored-file collection, and add
  the configuration scenario that ties a resolved setting to the artefact its consumer emits.

## Capabilities

### New Capabilities

None. No capability is introduced: the stored-file collection is a requirement of the existing `file` capability, and
every other change revises an existing capability.

### Modified Capabilities

A scenario whose system case leaves keeps its place in its specification: the behaviour it states still holds, and the
task list records which suite now proves it. Only a requirement whose whole subject leaves is removed.

- `audio`: remove the file-alias requirement, whose whole subject is a declared `303`.
- `authorization`: broaden the concealment, catalogue-read, catalogue-write and filtered-listing scenarios to the cases
  that now prove them, broaden the Admin-creation scenario to name its refusal, and add the denied playlist creation,
  the music catalogue creations and the requirement that an operation checks the caller's action on a second resource.
- `configuration`: add the scenario tying a setting resolved from a layer to the artefact its consumer emits.
- `cover-art`: add the playlist-track scenario beside the album-track one that already exists, so the split case has a
  label of its own.
- `file`: add the stored-file collection requirement and its three scenarios.
- `image`: reduce the readability scenario to its status, and remove the file-alias requirement, whose whole subject is
  a declared `303`.
- `logging`: add the disabled-colour scenario and reword the verbosity scenario to the severity ladder.
- `movies`: name the country beside the date in the unusable-value scenario's WHEN, so the moved refusal cases prove it.
- `music`: fold the invalid-update refusal into the creation scenario's WHEN, remove the artwork clause from the
  playlist-track scenario, and reword the default album and group as named terms everywhere the seed's identifier `0`
  was written out.
- `video`: reduce the readability scenario to its status, and remove the file-alias requirement for the same reason.

`media-ingest`, `download`, `user`, `authentication` and `upload` are not listed: their behaviour is unchanged and every
scenario this change stops proving stays in their specification, with the proof's new owner recorded in the task list.

## Impact

- **Code** — domain `configuration` (the seed setting and its partial), application `load_configuration` and a new
  `list_files` use case with its port, infrastructure `seed.rs` (profile dispatch, embeds and the system fixture), the
  envy adapter, the REST handler and index relation for the stored-file collection, the ingest's resource write, the
  composition root that passes the resolved profile, and `delete_playlist`'s, `delete_user`'s, `delete_gallery`'s and
  `delete_gallery_item`'s resource cleanup.
- **API** — one added operation, `GET /api/v1/asset/file`, one added index relation, and one `FILE` resource the scan
  now registers. Nothing else changes on the wire.
- **Tests** — the system suite's thirteen reworked modules and `authorization`, plus `series.py`, `harness.py` and
  `conftest.py`; the moved HAL unit cases in `test/api/`; the unit tests for the seed setting, `list_files` and the
  colliding-digest transfer.
- **Tooling and docs** — the suite's typed OpenAPI client (its generator script and config, the devenv tasks, the
  `requirements.txt` entries, and the `pythonpath`/ruff/pyright/`ls-lint`/`.gitignore` entries), the suite scripts'
  profiles and image features, the SQL linters' paths, the coverage exclusion, `pyproject.toml`'s `testpaths`, the CI
  workflows, and `TechnicalDesign.md`, `Test.md`, `Glossary.md`, `UseCases.md`, `README.md` and `AGENTS.md` wherever
  they describe seeding, the system suite, how a case addresses a route or the removed requirements.
