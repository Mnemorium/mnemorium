# Tasks

## 1. Schema and read contract

- [x] 1.1 Edit `20260904012851_create_movie` so `release_date` and `country_of_origin` are nullable and the date default
      is gone; verify a fresh in-memory database accepts a film with both omitted and reports `NULL` for each.
- [x] 1.2 Edit `20261026090000_create_series` so `first_air_date` and `country_of_origin` are nullable and the date
      default is gone; verify the same two cases for a show.
- [x] 1.3 Edit `20261026090003_create_episode` so `air_date` is nullable; verify an episode with no air date stores
      `NULL`.
- [x] 1.4 Add a migration pair with `BEFORE INSERT` and `BEFORE UPDATE OF video_id` triggers on `movie` and `episode`
      that refuse a video already backed by the other table; verify both directions raise `ABORT` and map to
      `RepositoryError::Conflict`.
- [x] 1.5 Change `Movie.release_date`, `Movie.country_of_origin`, `Series.first_air_date`, `Series.country_of_origin`
      and `Episode.air_date` to optional values, and update every constructor; verify the domain unit tests cover
      present and absent values.
- [x] 1.6 Update the `SQLx` movie and series projections and row models for the optional columns; verify the repository
      tests return `None` for a stored null and the value otherwise.
- [x] 1.7 Update the movie, series and episode read handlers and their tests so the representations report `null` for an
      absent date or country; verify one handler test per field for both the null and the populated case.
- [x] 1.8 Update the entity-relationship diagram in `docs/development/TechnicalDesign.md` to drop the `NN`/`DF` markers
      on the changed columns; verify `script/rule_enforcer.sh check` passes `PERS-011`.
- [x] 1.9 Run the stage gate: `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`,
      `cargo fmt --all -- --check`, and `script/rule_enforcer.sh check`.

## 2. Identification

- [x] 2.1 Add a kind to `IdentifyVideoCommand` and filter the interpreted queries before the search loop; verify unit
      tests prove a film route skips show hypotheses, a show route skips film hypotheses, and the first-hit policy still
      holds.
- [x] 2.2 Add the reference-resolution use case over the existing provider `lookup`, covering `Movie`, `Series` and
      `Episode` targets; verify unit tests for routed lookup, unknown reference, unavailable provider, provider failure
      and an episode position that differs from the request.
- [x] 2.3 Add the HAL response types for the candidate collection, the metadata document and the `available: false`
      document; verify serialization tests cover film, show, episode and unavailable.
- [x] 2.4 Add the four handlers and routes under `/api/v1/library/movie/identify` and `/api/v1/library/series/identify`;
      verify handler tests for `200`, `401`, the concealed `404`, `422` on an empty name or partial/out-of-range
      position, and `available: false` with no provider.
- [x] 2.5 Register the routes, add the `serves_video` provider predicate, and regenerate `docs/openapi.json`; verify the
      four paths and their parameters are present.
- [x] 2.6 Record the identification reads in `docs/development/UseCases.md` and the handler map in
      `docs/development/Overview.md`; verify the Markdown lint task passes.
- [x] 2.7 Run the stage gate: `cargo test`, Clippy, `cargo fmt`, and `script/rule_enforcer.sh check`.

## 3. Create and edit

- [x] 3.1 Add `video_id` to `MovieFilter` and add `create`/`update` to `MovieRepository` and its `SQLx` adapter; verify
      adapter tests for creation, update with genre replacement, a duplicate video, and a conflict on the cover or
      cross-kind trigger.
- [x] 3.2 Add `create`/`update`/`put_season`/`update_season`/`put_episode`/`update_episode` plus the video lookup to
      `SeriesRepository` and its `SQLx` adapter; verify adapter tests for each operation, including keeping episodes on
      a season replace and replacing an episode's video.
- [x] 3.3 Add the create-film use case: `library.manage` over the surface, upsert by `video_id`, cross-kind check, genre
      replacement; verify use-case tests for `201`, `200`, `409`, unknown video, empty name and invalid cover.
- [x] 3.4 Add the patch-film use case with absent-keeps and null-clears, the video untouched, and the read/manage
      decision; verify `200`, concealed `404`, `403`, `409`, `422` and malformed `400`.
- [x] 3.5 Add the create-show use case; verify `201`, insert-only behaviour for a repeated name, open-genre insertion,
      and input rejection.
- [x] 3.6 Add the patch-show use case; verify metadata replacement, null clearing, and that seasons and episodes are
      untouched.
- [x] 3.7 Add the put-season and patch-season use cases; verify `201`/`200` by index, episode retention, null name or
      cover clearing, and `404`/`403`/`400`.
- [x] 3.8 Add the put-episode and patch-episode use cases; verify `201`/`200`, video replacement and detachment,
      cross-kind `409`, unknown video, and that patch leaves the video unchanged.
- [x] 3.9 Add the seven handlers and routes, wire them into the use-case factory, and regenerate `docs/openapi.json`;
      verify handler tests for every status named in the two write specs and every new path in the document.
- [x] 3.10 Record the film, show, season and episode writes in `docs/development/UseCases.md` and the handler map in
      `docs/development/Overview.md`; verify the Markdown lint task passes.
- [x] 3.11 Run the stage gate: `cargo test`, Clippy, `cargo fmt`, and `script/rule_enforcer.sh check`.

## 4. Apply

- [x] 4.1 Add the apply-film use case: authoritative write of name, plot, date, country and genres; cover and video
      untouched; verify reported, absent, provider-failure, unknown-reference, wrong-kind and unavailable cases.
- [x] 4.2 Add the apply-show use case with the same authority, proving it creates no season or episode and leaves the
      cover untouched; verify the same error cases.
- [x] 4.3 Add the apply-episode use case over `Target::Episode`, writing name, plot and air date, checking the returned
      position and leaving the video untouched; verify the position-mismatch rejection.
- [x] 4.4 Add the three handlers and routes with the album-apply error mapping (`422` invalid/unknown provider, `404`
      unknown reference, `409` conflict, `500` provider failure), and regenerate `docs/openapi.json`; verify handler
      tests for each status and each new path.
- [x] 4.5 Record the three apply reads in `docs/development/UseCases.md`; verify the Markdown lint task passes.
- [x] 4.6 Run the stage gate: `cargo test`, Clippy, `cargo fmt`, and `script/rule_enforcer.sh check`.

## 5. Integration

- [x] 5.1 Seed a video, a film, a show, a season and an episode, then exercise create → patch → apply → read for each
      entity; verify the read returns the written values and `null` for every cleared field.
- [x] 5.2 Verify the two-kind flow: a film apply leaves the episode untouched and an episode apply leaves the film
      untouched; a video attached to one is refused by the other.
- [x] 5.3 Run the full gate: `cargo test --all-targets --all-features`, Clippy, `cargo fmt --check`,
      `script/rule_enforcer.sh check`, and the coverage command from `AGENTS.md`; verify it passes.

## Workflow follow-up

- File the follow-up issue for the open video genre vocabulary and the closed canonical vocabulary question.
- File the follow-up issue for video discovery (`video_id` not discoverable for an unattached video, and the
  `_links.video` drift in the Asset file representation).
- Archive the change after the project's review requirements are satisfied.
