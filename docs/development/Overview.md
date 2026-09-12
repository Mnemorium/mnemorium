# Overview

## Source code structure

```text
└── src
    ├── bin
    │   ├── openapi_gen.rs
    │   └── server.rs
    └── lib
        ├── application
        │   ├── port
        │   └── use_case
        ├── domain
        │   ├── alias.rs
        │   ├── model
        │   ├── port
        │   │   └── error.rs
        │   └── service
        └── infrastructure
            ├── configuration.rs
            ├── inbound
            │   └── rest
            │       ├── api_error.rs
            │       ├── bootstrap.rs
            │       └── handler
            ├── logging.rs
            └── outbound
                ├── client
                ├── moka
                │   └── bootstrap.rs
                └── sqlx
                    ├── bootstrap.rs
                    ├── model
                    └── sqlite3
```

| Entity                                             | Description                                                           |
| -------------------------------------------------- | --------------------------------------------------------------------- |
| `docs`                                             |                                                                       |
| `src`                                              |                                                                       |
| `src/bin`                                          |                                                                       |
| `src/lib`                                          |                                                                       |
| `src/lib/application`                              | App Layer                                                             |
| `src/lib/application/port`                         | Interface declaration for usecase (one by file)                       |
| `src/lib/application/use_case`                     | Implementation of the **UseCase**                                     |
| `src/lib/domain`                                   | Domain Layer                                                          |
| `src/lib/domain/alias.rs`                          | Type alias for the project (ex: which integer to use for IDs)         |
| `src/lib/domain/model`                             | Aggregate, Entity, Value object declaration                           |
| `src/lib/domain/port`                              | Port interface declaration                                            |
| `src/lib/domain/port/error.rs`                     | Repository, External service error declaration                        |
| `src/lib/domain/service`                           | Domain Service implementation; see Terms Glossary for more info on it |
| `src/lib/infrastructure/inbound/rest`              | HTTP adapter layer                                                    |
| `src/lib/infrastructure/inbound/rest/api_error.rs` | API Error declaration                                                 |
| `src/lib/infrastructure/inbound/rest/handler`      | HTTP endpoint handler                                                 |
| `src/lib/infrastructure/inbound/rest/bootstrap.rs` | Setup the routes with axum                                            |
| `src/lib/infrastructure/outbound`                  | Outbound Port adapter declaration                                     |
| `src/lib/infrastructure/configuration.rs`          | Configuration related bootstrapping                                   |
| `src/lib/infrastructure/logging.rs`                | Logging related bootstrapping                                         |

## Server lifecycle

The server binary (`src/bin/server.rs`) shuts down gracefully on `SIGINT` and
`SIGTERM`:

1. The signal stops the listener: new connections are refused.
2. In-flight requests drain to completion; idle keep-alive connections are
   closed.
3. `axum::serve` returns and the process exits with code `0`.

Because the `/health` endpoint starts failing as soon as the drain begins,
orchestrators stop routing traffic to the instance while existing requests
finish.

The supervisor controls the hard termination window:

- **Docker**: `docker stop` sends `SIGTERM`, then `SIGKILL` after the grace
  period (default 10 s). Current endpoints complete well within it; raise the
  grace (`docker stop --time <seconds>`, or `stop_grace_period` in compose) once
  long-running uploads or media scans land.
- **systemd**: `KillSignal=SIGTERM` is the default; size `TimeoutStopSec` to the
  longest expected drain plus a margin for filesystem syncs
  (`TimeoutStopSec=120` is a safe starting point on networked storage).
- **launchd**: use `launchctl bootout` (sends `SIGTERM` and waits); avoid
  `launchctl kickstart -k`, which sends `SIGKILL` and truncates in-flight I/O.

Note: the `SQLite` pool is not wired into the server yet. When it is, the
shutdown sequence becomes: signal → drain → `pool.close().await` → exit.

## Domain model

```puml
@startuml

hide empty members
skinparam style strictuml



class MusicRecording {
  name: string
  first_release_date: date
  isrc_code: string
}

class MusicGroup {
  name: string
}

class MusicAlbum {
  name: string
  production_type: string
  release_date: date
}

class MusicMedium {
  format: string
  index: int
}

class MusicTrack {
  index: int
}


class MusicPlaylist {
  name: string
  created_at: date
  last_modified: date
}

class Movie {
  name: string
  plot: string
  country_of_origin: string
  release_date: date
}

class User {
  username: string
  email: string
  role: string
}

class Credential {
  hashSecret: string
  salt: string
}

class Person {
  given_name: string
  family_name: string
  gender: string
  birth_date: string
}

class Genre {
  name: string
}

class Video {
  duration_sec: int
  codec: string
  width: int
  height: int
  frame_count: int
  bitrate_bps: int
  bit_depth: int
  pixel_format: string
  scan_type: string
}

class AudioStream {
  index: int
  name: string
  default_track: bool
  commentary: bool
}


class Audio {
  name: string
  duration_sec: int
  codec: string
  channels: string
  channel_layout: string
  sample_rate_hz: float
  bit_depth: int
  bit_rate_bps: float
  block_size: int
}

class SubtitleStream {
  index: int
  name: string
  format: string
  default: bool
}

class Gallery {
  name: string
  created_at: date
  last_modified: date
  is_public: boolean
}

class GalleryItem {
  added_at: date
}

class Image {
  name: string
  width_px: int
  height_px: int
  orientation: string
  color_space: string
  color_depth: int
  created_at: date
}

class Language {
  name: string
  code: string
}


class File {
  path: string
  mime_type: string
  uploaded_at: date
  md5_integrity: string
  is_public: boolean
}

Movie "*" -- "*" Genre: Categorize by >
Movie "1" -- "1" Video: Subject in >

Person "1" -- "*" Movie: Direct >
Person "*" -- "*" Movie: Acted in >
Person "*" -- "*" MusicGroup: Is part of >


MusicAlbum "1" -- "*" MusicMedium : Contains >
MusicAlbum "*" -- "*" Genre: Categorize by >

MusicMedium "1" -- "*" MusicTrack : Contains >

MusicPlaylist "1" -- "*" MusicTrack : Contains >

MusicTrack "*" -- "1" MusicRecording : Reference >

MusicRecording "*" -- "*" MusicGroup: Made by >

Video "1" -- "*" AudioStream: Contains >
Video "1" -- "*" SubtitleStream: Contains >
Video "1" -- "1" File: Stored in >

AudioStream "*" -- "1" Language: Is in >

SubtitleStream "*" -- "*" Language: Is in >

Image "1" -- "*" Movie: Is thumbnail for >
Image "1" -- "1" File: Store in >


User "1" -- "1" Credential: Has >
User "1" -- "*" MusicPlaylist : Creates >
User "1" -- "*" Gallery: Creates >
User "1" -- "*" File : Manage >

Audio "1" -- "*" AudioStream: Describe >
Audio "1" -- "*" MusicRecording: Describe >
Audio "1" -- "1" File: Store in >

Gallery "*" -- "*" GalleryItem: Contains >
GalleryItem "1" -- "0..1" Image: References >
GalleryItem "1" -- "0..1" Video: References >

@enduml
```

| **Term**           | **Description**                                                                                                                                                                                                                                                                             |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Person**         | Fundamental identifying information representing an individual human being (e.g., real name, date of birth).                                                                                                                                                                                |
| **User**           | An account or actor actively interacting with the application.                                                                                                                                                                                                                              |
| **Credential**     | An authentication factor (e.g., password hash, API token, OAuth secret) used to verify a User's identity.                                                                                                                                                                                   |
| **MusicGroup**     | An entity representing a musical performer or ensemble, such as a band, orchestra, choir, or solo artist.                                                                                                                                                                                   |
| **MusicPlaylist**  | A user-created, ordered collection of **MusicRecording**.                                                                                                                                                                                                                                   |
| **MusicAlbum**     | A curated, top-level commercial or conceptual collection of **MusicRecording** by an **MusicGroup**.                                                                                                                                                                                        |
| **MusicMedium**    | The specific physical or digital container holding a sequence of **MusicTracks** (e.g., a specific CD, cassette tape, vinyl disc side, or digital volume).                                                                                                                                  |
| **MusicTrack**     | An ordered slot or position entry on a specific **MusicMedium** that points to an underlying **MusicRecording**.                                                                                                                                                                            |
| **MusicRecording** | A unique master audio capture or sound recording session, independent of the physical formats on which it is distributed.                                                                                                                                                                   |
| **Movie**          | A self-contained, long-form narrative or documentary film produced for artistic, cinematic, or entertainment purposes.                                                                                                                                                                      |
| **Video**          | An abstract domain representation of a video file                                                                                                                                                                                                                                           |
| **SubtitleStream** | A text data stream embedded within a video container that provides timed subtitle or caption text.                                                                                                                                                                                          |
| **AudioStream**    | A discrete audio track or channel stream embedded within a multi-stream media file.                                                                                                                                                                                                         |
| **Language**       | A standardized natural human language used for text, speech, or subtitle localization (e.g., ISO 639-1 code).                                                                                                                                                                               |
| **Genre**          | A categorical classification used to group media assets (e.g., **Movie**, **MusicAlbum**) by shared stylistic, thematic, or musical conventions.                                                                                                                                            |
| **File**           | A physical computer file stored within a file system or object storage.                                                                                                                                                                                                                     |
| **Image**          | An abstract domain representation of an image file                                                                                                                                                                                                                                          |
| **Gallery**        | An aggregated collection of visual assets, such as **ImageObjects** and **VideoObjects**. The first one is the default one containings all available **ImageObjects** and **VideoObjects** (other than movie). Deleting item in this **Gallery** will also delete permently from the system |
| **GalleryItem**    | A position-aware entry within a **Gallery** that references either an **ImageObject** or a **VideoObject**.                                                                                                                                                                                 |
| **Audio**          | An abstract domain representation of an audio file or a Audio Stream details                                                                                                                                                                                                                |

## Bounded context

| Name          | Description                                                               |
| ------------- | ------------------------------------------------------------------------- |
| Identity      | Authentication and identity validation                                    |
| User          | Managing user records, profiles, lifecycle, and administrative operations |
| Library       | User's collection of media                                                |
| Configuration | Manage service configuration                                              |
| Asset         | Manage files                                                              |

## Branch naming and PR title naming

PR titles follow the
[Conventional Commits v1.0.0](https://www.conventionalcommits.org/en/v1.0.0/)
specification: the type at the start of the title determines the semantic
version bump (see [Versioning](#versioning) below). The allowed types are
enforced by the semantic PR gating job in `.github/workflows/ci.yml`
(`amannn/action-semantic-pull-request`).

### Branch naming

Format: `{type}({scope})/{description}`, kebab-case.

Examples:

- `feat(core)/serve-media`
- `fix(devops)/fix-dockerfile`
- `docs(agent)/spellcheck-dead`

### PR title naming

Format: `{type}({scope}): {description}` — enforced by CI.

For intentional breaking changes, add `!` after the type/scope:

- `feat(core)!: rename endpoints`
- `fix(core)!: change response schema`

Examples:

- `feat(core): serve media via streaming`
- `fix(devops): fix Dockerfile registry`
- `release: 0.3.0`

### Scopes

A scope is an optional noun describing the area of the codebase affected. The
allowed scopes are:

| Scope       | Description                                       |
| ----------- | ------------------------------------------------- |
| agent       | opencode configuration, agents, and `AGENTS.md`   |
| lint        | Lint configuration files                          |
| devenv      | devenv environment files                          |
| test-e2e    | End-to-end tests                                  |
| test-system | System tests                                      |
| sqlite3     | SQLite3 datastore (migrations, sqlx SQLite layer) |
| development | Development documentation                         |
| api         | REST API and OpenAPI specification                |

Other scopes may be added later as new areas emerge. The `release` type carries
no scope.

### Type prefixes

| Prefix   | Description                                  | Version bump |
| -------- | -------------------------------------------- | ------------ |
| feat     | New feature                                  | MINOR        |
| fix      | Bug fix (including production fixes)         | PATCH        |
| refactor | Code cleanup/restructure, no behavior change | PATCH        |
| perf     | Performance improvement                      | PATCH        |
| docs     | Documentation changes                        | PATCH        |
| test     | Tests and test improvements                  | PATCH        |
| build    | Build system and dependency changes          | PATCH        |
| ci       | CI/CD workflow changes                       | PATCH        |
| chore    | Maintenance tasks                            | PATCH        |
| release  | Release version bumps, reserved for the bot  | explicit     |

## Versioning

The version follows [semantic versioning](https://semver.org/)
(`MAJOR.MINOR.PATCH`). The bump is computed automatically on merge by the
release workflow (`.github/workflows/cd.yml`) from the merged PR titles since
the last tag:

| Bump  | Trigger                                                                                 |
| ----- | --------------------------------------------------------------------------------------- |
| MAJOR | A PR title with a `!` marker (e.g. `feat(core)!:`)                                      |
| MINOR | At least one merged `feat` PR (and no `!` marker)                                       |
| PATCH | Any other merged PR (`fix`, `refactor`, `perf`, `docs`, `test`, `build`, `ci`, `chore`) |

When a release is needed, the workflow:

1. Bumps the version with `script/version.sh set <version>` and commits it as
   `github-actions[bot]` with a `release: v<version>` message.
2. Tags the commit (`v<version>`) and pushes it.
3. Publishes a GitHub release (with generated release notes and a source ZIP)
   and pushes the Docker image.

If any step fails after the push, the workflow reverts: it deletes the remote
tag and restores `main` to its previous commit. PRs titled `release*` are
skipped by the workflow; the `release` type is reserved for the bot's own
commits.
