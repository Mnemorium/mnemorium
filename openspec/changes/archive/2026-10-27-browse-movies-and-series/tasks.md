# Tasks

## 1. Branch

- [x] 1.1 Resolve the delivery metadata: Conventional-Commit type `feat`, the single `GOV-001` scope `api` (the change's
      dominant area — the new REST surface), slug `browse-movies-and-series`, and pull-request title
      `feat(api): browse movies and series`. The branch is `feat(api)/browse-movies-and-series`. Verified by the
      type/scope/slug/title all being non-empty and the scope appearing in the `GOV-001` scope table.
- [x] 1.2 Fetch and confirm the worktree state in `/home/will/project/mine/Mnemorium/wt3`: run `git fetch origin` and
      `git status --porcelain`; the only untracked path may be `openspec/changes/browse-movies-and-series/`. Verified by
      no modified tracked files and no unexpected untracked files.
- [x] 1.3 Create the delivery branch from `origin/main` in wt3 (`main` is checked out in another worktree, so do not
      switch to it): `git switch -c feat(api)/browse-movies-and-series origin/main`. The planning artifacts stay
      untracked and carry over. Verified by `git branch --show-current` printing the branch and `git status --porcelain`
      listing only the change directory.

## 2. Persistence

- [x] 2.1 Add `migrations/<timestamp>_create_series.up.sql` / `.down.sql` and
      `migrations/<timestamp>_create_series_genre.up.sql` / `.down.sql`, with the columns, primary keys, foreign keys
      and constraint names from `design.md` § 4. Verified by `cargo test --lib infrastructure::outbound::sqlx::sqlite3`
      applying the migrations through the in-memory `sqlx::migrate!` harness.
- [x] 2.2 Add `migrations/<timestamp>_create_season.up.sql` / `.down.sql` and
      `migrations/<timestamp>_create_episode.up.sql` / `.down.sql`, including the `uq_season_series_id_season_index`,
      `uq_episode_season_id_episode_index`, `uq_episode_video_id` constraints, the `0..=4294967295` index checks and the
      `ON DELETE CASCADE` links from series to season to episode. Verified by the same migration harness applying them.
- [x] 2.3 Add `migrations/<timestamp>_upper_series_country_of_origin.up.sql` / `.down.sql`, mirroring
      `tg_movie_upper_country_of_origin_insert/update` (`PERS-008`). Verified by the persistence test in § 2.4 reading
      an uppercase value after a lowercase insert.
- [x] 2.4 Add persistence invariant tests to the `src/lib/infrastructure/outbound/sqlx/sqlite3.rs` test module for the
      four new tables: the uppercase trigger, the `u32` index bounds (a value above `4294967295` is rejected), the
      `(series_id, season_index)`, `(season_id, episode_index)` and `video_id` uniqueness rules, and the series → season
      → episode cascade. Verified by `cargo test --lib infrastructure::outbound::sqlx::sqlite3` passing the new tests.

## 3. Domain model and ports

- [x] 3.1 Add `src/lib/domain/model/season.rs` and `src/lib/domain/model/episode.rs` with `SeasonIndex(u32)`,
      `EpisodeIndex(u32)`, a `Season` carrying its index, optional name and episodes, and a validating `Episode`
      carrying its index, name, plot, air date and `Video`. Register both modules in `src/lib/domain/model.rs`. Add no
      test module to the domain files (`TEST-019`); verified by `cargo build` and the use-case tests in § 5.
- [x] 3.2 Add `src/lib/domain/model/movie.rs` with `Movie` carrying its identifier, name, plot, country of origin,
      release date, genre identifiers and `Video`, and a constructor that rejects an empty name. Register the module and
      verify with `cargo build`.
- [x] 3.3 Add `src/lib/domain/model/series.rs` with `Series` carrying its identifier, name, plot, country of origin,
      first air date, genre identifiers and seasons, and a constructor that rejects an empty name. Verify with
      `cargo build`.
- [x] 3.4 Add `src/lib/domain/port/movie_repository.rs`: `MovieFilter { id, name }`, then the `MovieRepository` trait
      with `#[cfg_attr(test, mockall::automock)]` and the `search` method returning `Vec<Movie>`, plus the
      `#[cfg(test)]` `&mut T` forwarding impl, in the § 8.3 declaration order (`STY-RUST-051`, `STY-RUST-052`). Verified
      by `cargo test --lib` generating `MockMovieRepository`.
- [x] 3.5 Add `src/lib/domain/port/series_repository.rs`: `SeriesFilter { id, name }`, then the `SeriesRepository` trait
      with `search` returning `Vec<Series>`, plus the forwarding impl, following the same declaration order. The
      aggregate carries seasons, episodes and videos; no summary type is declared (`STY-RUST-081`). Verified by
      `cargo test --lib` generating `MockSeriesRepository`.
- [x] 3.6 Extend `src/lib/domain/port/library_unit_of_work.rs` with `movies()` and `series()` accessors, implement them
      for `TestUnitOfWork` in `src/lib/lib.rs`, and add the two mock repository fields to the fixture. Verified by
      `cargo test --lib` compiling and the existing Library tests passing.

## 4. SQLx adapter

- [x] 4.1 Add the movie row model under `src/lib/infrastructure/outbound/sqlx/model/` and implement
      `SqlxMovieRepository::search`: join `movie`, `movie_genre` and `video`, aggregate genres ordered by `genre_id`,
      map the stored scan type, build `Video` through its validating constructor, apply the id/name filter, and order by
      `movie_id`. Return `RepositoryError::DataIntegrityViolation` for a value the domain cannot hold. Verified by
      in-memory migrated-database tests covering the name filter, stable order, genre aggregation, the hydrated video,
      the empty result and a malformed stored video.
- [x] 4.2 Add the series row models and implement `SqlxSeriesRepository::search`: join `series`, `series_genre`,
      `season`, `episode` and `video`, order by `series_id, season_index, episode_index`, and hydrate one `Series` per
      group with left joins so a series without a season and a season without an episode stay readable. Validate every
      hydrated `Video` and index. Verified by tests for the name filter, stable order, genre aggregation, a series with
      no season, a season with no episode, multiple seasons and episodes, and a malformed stored video.
- [x] 4.3 Implement `movies()` and `series()` on `SqlxUnitOfWork` in
      `src/lib/infrastructure/outbound/sqlx/unit_of_work.rs`, wiring the two repositories. Verified by
      `cargo test --lib infrastructure::outbound::sqlx` passing the adapter and unit-of-work tests.

## 5. Application use cases

- [x] 5.1 Add `application/port/list_movies.rs` and `application/use_case/list_movies.rs` plus the `list_movies`
      accessor on `LibraryUseCaseFactory`: load the caller (deleted account → `NoSuchCaller`), read the matching movies,
      paginate in the use case, and return the full movie projection with a total. Verified by use-case tests for the
      happy path, filter/pagination forwarding, `NoSuchCaller` with rollback, repository failure and commit failure.
- [x] 5.2 Add `get_movie` (port, use case, factory accessor) returning one movie and `NoSuchMovie` when absent; load the
      caller as in § 5.1. Verified by tests for the happy path, unknown movie, `NoSuchCaller` and repository failure.
- [x] 5.3 Add `list_series` (port, use case, factory accessor) returning only `id` and `name` per item and the total;
      the use case reads full aggregates and projects the summary. Verified by tests asserting the projection drops
      seasons, genres and dates, plus filter/pagination forwarding, `NoSuchCaller`, repository failure and commit
      failure.
- [x] 5.4 Add `get_series` (port, use case, factory accessor) returning the series metadata, genres and season summaries
      with their episode counts; `NoSuchSeries` when absent. Verified by tests for the happy path, a series with no
      season, unknown series, `NoSuchCaller` and repository failure.
- [x] 5.5 Add `get_series_season` (port, use case, factory accessor) returning one season scoped to its series, with
      `NoSuchSeries`, `NoSuchSeason` and a per-series index check. Verified by tests for the happy path, unknown series,
      unknown season, scoping between two series, `NoSuchCaller` and repository failure.
- [x] 5.6 Add `list_series_episodes` (port, use case, factory accessor) returning a season's episodes in ascending index
      order, with `NoSuchSeries` and `NoSuchSeason`. Verified by tests for the ordered happy path, an empty season,
      unknown series, unknown season, `NoSuchCaller` and repository failure.
- [x] 5.7 Add `get_series_episode` (port, use case, factory accessor) returning one episode scoped to its series and
      season, with `NoSuchSeries`, `NoSuchSeason` and `NoSuchEpisode`. Verified by tests for the happy path, unknown
      series, unknown season, an episode outside the season, `NoSuchCaller` and repository failure.
- [x] 5.8 Wire the seven new accessors in `RuntimeLibraryUseCaseFactory`
      (`src/lib/infrastructure/use_case_factory/library.rs`). Verified by
      `cargo test --lib infrastructure::use_case_factory` and the factory's unit tests.

## 6. REST handlers, routes and OpenAPI

- [x] 6.1 Add the shared representations and helpers to `src/lib/infrastructure/inbound/rest/handler/library.rs`:
      `MovieResponse` with its `VideoResponse`, `MovieListLinks`/`MovieListResponse`, the series summary/list types, the
      series detail with season summaries, `SeasonResponse`, `EpisodeResponse` with its video summary,
      `EpisodeListResponse`, the HAL links for each level, the `parse_movie_id` / `parse_series_id` helpers (parse the
      `u32` season and episode indexes inline where a helper would be single-use, `STY-RUST-001`), and the
      `Error → ApiError` mappings. Register the `/movie` and `/series` route trees in `library_routes`, merged behind
      the existing `authenticate` layer. Verified by `cargo test --lib ...handler::library` compiling.
- [x] 6.2 Add `handler/library/get_movie_list.rs` with its `utoipa::path` declaration and tests: `200` HAL collection
      and full movie shape, filter/limit/offset forwarding, out-of-range `limit` → `422`, malformed query → `400`,
      missing credentials → `401`, dependency failure → `500`. Verified by the handler's tests.
- [x] 6.3 Add `handler/library/get_movie.rs` with its declaration and tests: `200` movie shape, malformed `movie_id` →
      `400`, unknown movie → `404`, missing credentials → `401`. Verified by the handler's tests.
- [x] 6.4 Add `handler/library/get_series_list.rs` with its declaration and tests: `200` collection whose items carry
      only `id` and `name` plus `_links.self`, filter/limit/offset forwarding, `422`, `400`, `401` and `500`. Verified
      by the handler's tests.
- [x] 6.5 Add `handler/library/get_series.rs` with its declaration and tests: `200` metadata plus season summaries with
      episode counts, malformed `series_id` → `400`, unknown series → `404`, `401`. Verified by the handler's tests.
- [x] 6.6 Add `handler/library/get_series_season.rs` with its declaration and tests: `200` season summary, malformed
      `series_id`/`season_index` → `400`, unknown series or season → `404`, `401`. Verified by the handler's tests.
- [x] 6.7 Add `handler/library/get_series_episode_list.rs` with its declaration and tests: `200` episode collection with
      video summaries, malformed identifiers → `400`, unknown series or season → `404`, `401`. Verified by the handler's
      tests.
- [x] 6.8 Add `handler/library/get_series_episode.rs` with its declaration and tests: `200` episode shape, malformed
      identifiers → `400`, unknown series, season or episode → `404`, `401`. Verified by the handler's tests.
- [x] 6.9 Add route-reach tests to the `handler/library.rs` test module proving each of the seven routes reaches its
      handler through the authentication layer. Verified by `cargo test --lib ...handler::library` passing.
- [x] 6.10 Register the seven paths and their schemas in `src/lib/infrastructure/inbound/rest.rs`, then regenerate
      `docs/openapi.json` with `cargo run --bin openapi_gen`. Verified by all seven paths and the new schemas appearing
      in `docs/openapi.json` and `git diff` containing no unrelated spec change.

## 7. Documentation and end-to-end coverage

- [x] 7.1 Add the Video subsection to `docs/development/UseCases.md` with its PlantUML diagram, then
      `UC-022 - Browse Movies` (two endpoints) and `UC-023 - Browse Series` (five endpoints) in the `DOC-005` format:
      description, actors, parameters, preconditions, triggers, happy path, alternative flows and post conditions.
      Verified by the Markdown gates (`markdownlint-cli2`, `prettier`).
- [x] 7.2 Add `Series`, `Season` and `Episode` to the `docs/development/Overview.md` domain model and glossary, with the
      series → season → episode and video associations. Verified by the Markdown gates.
- [x] 7.3 Extend the `docs/development/TechnicalDesign.md` ERD with `series`, `series_genre`, `season` and `episode` and
      their relations, and include `series.country_of_origin` in `PERS-008`. Verified by the Markdown gates.
- [x] 7.4 Add `test/e2e/library/uc022_browse_movies.py` and `test/e2e/library/uc023_browse_series.py` covering missing
      credentials → `401`, the empty movie/series collection shape, malformed identifiers → `400`, unknown resources →
      `404`, out-of-range `limit` → `422`, and the `u32` season/episode index bounds. Document in each module docstring
      that populated paths are covered by the SQLx integration tests because no endpoint writes titles yet. Verified by
      `pytest -p no:cacheprovider test/e2e` against the `mnemorium-e2e` container.

## 8. Verification

- [x] 8.1 Run `cargo fmt --all -- --check` and `cargo clippy --all-targets --all-features -- -D warnings`; fix any
      finding. Verified by both commands exiting clean.
- [x] 8.2 Run `cargo test` and confirm the full unit and integration suite is green.
- [x] 8.3 Run `cargo llvm-cov --lib --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80` and confirm
      the coverage gate holds.
- [x] 8.4 Run `devenv test` and confirm every pre-commit hook passes, including the Markdown and SQL gates.
- [x] 8.5 Regenerate `docs/openapi.json` one final time and confirm `git diff -- docs/openapi.json` is empty, so the
      committed specification matches the source.

## 9. Pull request

- [x] 9.1 Stage and commit the work once with a Conventional Commit whose type is `feat` and whose scope is `api`,
      running git through the environment (`devenv shell -- git commit`) so the commit hook has its tooling. Push with
      `git push -u origin feat(api)/browse-movies-and-series`. Verified by the branch existing on the remote with the
      commit.
- [x] 9.2 Open a pull request titled `feat(api): browse movies and series`, and fill its body from
      `.github/pull_request_template.md`: what changed, the out-of-scope items (writes, ingestion, poster/cast,
      playback) and the `browse-movies-and-series` change reference. Verified by the semantic pull-request gate
      accepting the title and the description being complete.

## Workflow follow-up

- The change is read-only with no data backfill: the migrations only add empty tables and a trigger, so archiving needs
  no data step.
- Archive the change once the pull request is merged and the project's review requirements are satisfied.
