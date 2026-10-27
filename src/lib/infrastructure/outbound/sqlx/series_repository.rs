use std::collections::HashMap;

use sqlx::QueryBuilder;
use sqlx::Sqlite;
use sqlx::Transaction;

use crate::domain::alias::NumericID;
use crate::domain::model::episode::Episode;
use crate::domain::model::episode::EpisodeIndex;
use crate::domain::model::season::Season;
use crate::domain::model::season::SeasonIndex;
use crate::domain::model::series::Series;
use crate::domain::model::video::Video;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::series_repository::SeriesFilter;
use crate::domain::port::series_repository::SeriesRepository;

use super::error_mapping::stored_data_invalid;
use super::model::series::SeriesRow;
use super::model::video::parse_scan_type;
use super::query_filter::push_filter;

/// Repository querying shows, their seasons and their episodes, backed by a
/// `SQLite` transaction.
pub struct SqlxSeriesRepository<'transaction> {
    /// Transaction the repository reads from.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxSeriesRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }
}

/// A season being hydrated: its surrogate identifier, domain identity, name
/// and the episodes read so far.
type HydratedSeason = (NumericID, SeasonIndex, Option<String>, Vec<Episode>);

impl SeriesRepository for SqlxSeriesRepository<'_> {
    async fn search(&mut self, filter: &SeriesFilter) -> Result<Vec<Series>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT
                series.series_id,
                series.name,
                series.plot,
                series.country_of_origin,
                series.first_air_date,
                COALESCE((
                    SELECT GROUP_CONCAT(genre_id, ',')
                    FROM (
                        SELECT genre_id
                        FROM series_genre
                        WHERE series_genre.series_id = series.series_id
                        ORDER BY genre_id
                    )
                ), '') AS genres,
                season.season_id,
                season.season_index,
                season.name AS season_name,
                episode.episode_index,
                episode.name AS episode_name,
                episode.plot AS episode_plot,
                episode.air_date,
                video.video_id,
                video.duration_ms,
                video.codec,
                video.frame_count,
                video.width,
                video.height,
                video.color_id,
                video.scan_type,
                video.file_id
            FROM series
            LEFT JOIN season ON season.series_id = series.series_id
            LEFT JOIN episode ON episode.season_id = season.season_id
            LEFT JOIN video ON video.video_id = episode.video_id",
        );
        let mut first = true;

        if let Some(id) = filter.id {
            push_filter(&mut first, &mut builder, "series.series_id = ", id);
        }
        if let Some(name) = filter.name.as_deref() {
            push_filter(&mut first, &mut builder, "series.name = ", name);
        }

        // The show, its seasons and their episodes make one aggregate read, so
        // the order fixes both the season collection order and each season's
        // episode order. A stable show identifier also makes the caller's
        // offset/limit pagination deterministic (`ListSeries` pages in memory).
        builder.push(" ORDER BY series.series_id, season.season_index, episode.episode_index");

        let rows = builder
            .build_query_as::<SeriesRow>()
            .fetch_all(&mut **self.transaction)
            .await?;

        // The rows repeat a show once per episode and arrive ordered by show,
        // then season index, then episode index, so one pass groups them
        // without re-sorting. The season surrogate identifier keys the group;
        // the domain identity is the validated `SeasonIndex`.
        let mut seasons: HashMap<NumericID, Vec<HydratedSeason>> = HashMap::new();
        let mut series_ids: Vec<NumericID> = Vec::new();

        for row in &rows {
            if series_ids.last() != Some(&row.series_id) {
                series_ids.push(row.series_id);
            }
            let Some(season_id) = row.season_id else {
                continue;
            };
            let season_index_raw = row
                .season_index
                .ok_or_else(|| stored_data_invalid("missing_season_index"))?;
            let season_index = u32::try_from(season_index_raw)
                .map_err(|_| stored_data_invalid("season_index_out_of_range"))?;
            let series_seasons = seasons.entry(row.series_id).or_default();
            if series_seasons.last().map(|season| season.0) != Some(season_id) {
                series_seasons.push((
                    season_id,
                    SeasonIndex::new(season_index),
                    row.season_name.clone(),
                    Vec::new(),
                ));
            }
            let Some(episode_index_raw) = row.episode_index else {
                continue;
            };
            let episode_index = u32::try_from(episode_index_raw)
                .map_err(|_| stored_data_invalid("episode_index_out_of_range"))?;
            let scan_type_raw = row
                .scan_type
                .clone()
                .ok_or_else(|| stored_data_invalid("missing_scan_type"))?;
            let scan_type = parse_scan_type(&scan_type_raw)
                .ok_or_else(|| stored_data_invalid("unknown_scan_type"))?;
            let video = Video::try_new(
                row.video_id
                    .ok_or_else(|| stored_data_invalid("missing_video_id"))?,
                row.duration_ms
                    .ok_or_else(|| stored_data_invalid("missing_duration_ms"))?,
                row.codec
                    .clone()
                    .ok_or_else(|| stored_data_invalid("missing_codec"))?,
                row.frame_count
                    .ok_or_else(|| stored_data_invalid("missing_frame_count"))?,
                row.width
                    .ok_or_else(|| stored_data_invalid("missing_width"))?,
                row.height
                    .ok_or_else(|| stored_data_invalid("missing_height"))?,
                row.color_id
                    .clone()
                    .ok_or_else(|| stored_data_invalid("missing_color_id"))?,
                scan_type,
                row.file_id
                    .ok_or_else(|| stored_data_invalid("missing_file_id"))?,
            )
            .map_err(|_| stored_data_invalid("invalid_video"))?;
            let episode = Episode::try_new(
                EpisodeIndex::new(episode_index),
                row.episode_name
                    .clone()
                    .ok_or_else(|| stored_data_invalid("missing_episode_name"))?,
                row.episode_plot
                    .clone()
                    .ok_or_else(|| stored_data_invalid("missing_episode_plot"))?,
                row.air_date
                    .ok_or_else(|| stored_data_invalid("missing_air_date"))?,
                video,
            )
            .map_err(|_| stored_data_invalid("invalid_episode"))?;
            if let Some(season) = series_seasons.last_mut() {
                season.3.push(episode);
            }
        }

        let mut result = Vec::with_capacity(series_ids.len());
        for series_id in series_ids {
            let row = rows
                .iter()
                .find(|row| row.series_id == series_id)
                .ok_or_else(|| stored_data_invalid("missing_series_row"))?;
            let series_seasons = seasons
                .remove(&series_id)
                .unwrap_or_default()
                .into_iter()
                .map(|(_, index, name, episodes)| Season::new(index, name, episodes))
                .collect();
            result.push(
                Series::try_new(
                    series_id,
                    row.name.clone(),
                    row.plot.clone(),
                    row.country_of_origin.clone(),
                    row.first_air_date,
                    row.genres
                        .split(',')
                        .filter(|genre| !genre.is_empty())
                        .map(str::to_owned)
                        .collect(),
                    series_seasons,
                )
                .map_err(|_| stored_data_invalid("invalid_series"))?,
            );
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io as stdio;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::PoisonError;

    use chrono::NaiveDate;
    use sqlx::Sqlite;
    use sqlx::Transaction;
    use sqlx::sqlite::SqlitePoolOptions;
    use tracing::subscriber::DefaultGuard;
    use tracing::subscriber::set_default;
    use tracing_subscriber::fmt;
    use tracing_subscriber::layer::SubscriberExt as _;

    use crate::domain::model::episode::Episode;
    use crate::domain::model::episode::EpisodeIndex;
    use crate::domain::model::season::Season;
    use crate::domain::model::season::SeasonIndex;
    use crate::domain::model::series::Series;
    use crate::domain::model::video::ScanType;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::series_repository::SeriesFilter;
    use crate::domain::port::series_repository::SeriesRepository as _;

    use super::SqlxSeriesRepository;

    /// Append-only sink that lets a test read back the lines [`fmt`] emits.
    #[derive(Clone)]
    struct CaptureWriter {
        /// Buffer shared with the test that asserts on the captured output.
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "only the raw `write` and `flush` are meaningful for an in-memory capture buffer"
    )]
    impl stdio::Write for CaptureWriter {
        fn flush(&mut self) -> stdio::Result<()> {
            Ok(())
        }

        fn write(&mut self, buf: &[u8]) -> stdio::Result<usize> {
            let mut buffer = self.buffer.lock().unwrap_or_else(PoisonError::into_inner);
            buffer.extend_from_slice(buf);
            Ok(buf.len())
        }
    }

    /// Hand every formatted event to a fresh clone of the shared buffer.
    #[expect(
        clippy::missing_trait_methods,
        reason = "the default `make_writer_for` already routes through `make_writer`"
    )]
    impl<'writer> fmt::MakeWriter<'writer> for CaptureWriter {
        type Writer = Self;

        fn make_writer(&'writer self) -> Self::Writer {
            self.clone()
        }
    }

    /// Install a capturing subscriber on the current thread and return the
    /// shared buffer plus the guard that keeps it active.
    fn capture_logs() -> (Arc<Mutex<Vec<u8>>>, DefaultGuard) {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::registry().with(
            fmt::layer().with_ansi(false).with_writer(CaptureWriter {
                buffer: Arc::clone(&buffer),
            }),
        );
        let guard = set_default(subscriber);
        (buffer, guard)
    }

    /// Read the captured bytes back as a lossy UTF-8 string.
    fn captured_logs(buffer: &Mutex<Vec<u8>>) -> String {
        let bytes = buffer.lock().unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(bytes.as_slice()).into_owned()
    }

    /// Assert the captured output is exactly one `output_validation_failed`
    /// event with `reason`, carrying neither a stored value nor an `sqlx`
    /// detail.
    fn assert_output_validation_failed(logs: &str, reason: &str) {
        assert_eq!(
            logs.matches("WARN").count(),
            1,
            "rejected stored data must emit exactly one warn event: {logs}"
        );
        assert!(
            logs.contains("event=\"output_validation_failed\""),
            "the event must be output_validation_failed: {logs}"
        );
        assert!(
            logs.contains(&format!("reason=\"{reason}\"")),
            "the event must carry the stable reason `{reason}`: {logs}"
        );
        assert!(
            !logs.contains("sqlx"),
            "the event must not carry the sqlx detail: {logs}"
        );
    }

    /// Open a migrated in-memory database and begin one transaction.
    async fn begin_transaction() -> Result<Transaction<'static, Sqlite>, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        pool.begin().await
    }

    /// Seed a credential and a standard user so foreign keys resolve.
    async fn seed_user(transaction: &mut Transaction<'static, Sqlite>) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (1, 'hash')")
            .execute(&mut **transaction)
            .await?;
        sqlx::query(
            "INSERT INTO user (user_id, role, username, email, credential_id)
             VALUES (1, 'STANDARD', 'tester', NULL, 1)",
        )
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed a video file owned by the seeded user.
    async fn seed_video_file(
        transaction: &mut Transaction<'static, Sqlite>,
        file_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, integrity_hash)
             VALUES (?1, ?2, 1, 0, 'video/mp4', ?3)",
        )
        .bind(file_id)
        .bind(format!("{file_id}.mp4"))
        .bind(format!("{file_id:064x}"))
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed a progressive video of `width` pixels on `file_id`.
    async fn seed_video(
        transaction: &mut Transaction<'static, Sqlite>,
        video_id: i64,
        file_id: i64,
        width: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO video (
                video_id, duration_ms, codec, frame_count, width, height, color_id, scan_type, file_id
             )
             VALUES (?1, 1000.0, 'H264', 30, ?2, 10, 'YCbCr', 'PROGRESSIVE', ?3)",
        )
        .bind(video_id)
        .bind(width)
        .bind(file_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed a show with a lowercase country of origin.
    async fn seed_series(
        transaction: &mut Transaction<'static, Sqlite>,
        series_id: i64,
        name: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO series (
                series_id, name, plot, country_of_origin, first_air_date
             )
             VALUES (?1, ?2, 'A plot', 'us', '1990-04-08')",
        )
        .bind(series_id)
        .bind(name)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed a genre.
    async fn seed_genre(
        transaction: &mut Transaction<'static, Sqlite>,
        genre_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO genre (genre_id) VALUES (?1)")
            .bind(genre_id)
            .execute(&mut **transaction)
            .await?;
        Ok(())
    }

    /// Attach `genre_id` to `series_id`.
    async fn seed_series_genre(
        transaction: &mut Transaction<'static, Sqlite>,
        series_id: i64,
        genre_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO series_genre (series_id, genre_id) VALUES (?1, ?2)")
            .bind(series_id)
            .bind(genre_id)
            .execute(&mut **transaction)
            .await?;
        Ok(())
    }

    /// Seed a season of `series_id` at `season_index`.
    async fn seed_season(
        transaction: &mut Transaction<'static, Sqlite>,
        season_id: i64,
        series_id: i64,
        season_index: i64,
        name: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO season (season_id, series_id, season_index, name)
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(season_id)
        .bind(series_id)
        .bind(season_index)
        .bind(name)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed an episode of `season_id` at `episode_index`, backed by `video_id`.
    async fn seed_episode(
        transaction: &mut Transaction<'static, Sqlite>,
        episode_id: i64,
        season_id: i64,
        episode_index: i64,
        name: &str,
        video_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO episode (
                episode_id, season_id, episode_index, name, plot, air_date, video_id
             )
             VALUES (?1, ?2, ?3, ?4, 'Episode plot', '1991-04-08', ?5)",
        )
        .bind(episode_id)
        .bind(season_id)
        .bind(episode_index)
        .bind(name)
        .bind(video_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn search_hydrates_seasons_episodes_and_their_videos() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video_file(&mut transaction, 11).await?;
        seed_video_file(&mut transaction, 12).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_video(&mut transaction, 21, 11, 1280).await?;
        seed_video(&mut transaction, 22, 12, 720).await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_series(&mut transaction, 2, "Northern Exposure").await?;
        seed_genre(&mut transaction, "DRAMA").await?;
        seed_genre(&mut transaction, "MYSTERY").await?;
        seed_series_genre(&mut transaction, 1, "MYSTERY").await?;
        seed_series_genre(&mut transaction, 1, "DRAMA").await?;
        seed_season(&mut transaction, 40, 1, 1, Some("Season One")).await?;
        seed_season(&mut transaction, 41, 1, 2, None).await?;
        // Insert the second episode first so the order can only come from the
        // episode index, not the storage order.
        seed_episode(
            &mut transaction,
            60,
            40,
            2,
            "Zen, or the Skill to Catch a Killer",
            21,
        )
        .await?;
        seed_episode(&mut transaction, 61, 40, 1, "Pilot", 20).await?;
        seed_episode(&mut transaction, 62, 41, 1, "May the Giant Be with You", 22).await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let series = repository.search(&SeriesFilter::default()).await?;

        // Assert
        assert_eq!(series.len(), 2, "every show must be returned");
        let first = series.first().ok_or("the first show must be found")?;
        assert_eq!(first.id(), 1);
        assert_eq!(first.name(), "Twin Peaks");
        assert_eq!(first.plot(), "A plot");
        assert_eq!(
            first.country_of_origin(),
            "US",
            "the insert trigger must uppercase the country of origin"
        );
        assert_eq!(
            first.first_air_date(),
            NaiveDate::from_ymd_opt(1990, 4, 8).unwrap_or_default()
        );
        assert_eq!(
            first.genre_ids(),
            ["DRAMA", "MYSTERY"],
            "the genres must be ordered by identifier"
        );
        let seasons: Vec<_> = first.seasons().iter().map(Season::index).collect();
        assert_eq!(
            seasons
                .iter()
                .copied()
                .map(SeasonIndex::value)
                .collect::<Vec<_>>(),
            vec![1, 2],
            "the seasons must be ordered by index"
        );
        let first_season = first
            .seasons()
            .first()
            .ok_or("the first season must be found")?;
        assert_eq!(first_season.name(), Some("Season One"));
        let episode_indexes: Vec<_> = first_season
            .episodes()
            .iter()
            .map(Episode::index)
            .map(EpisodeIndex::value)
            .collect();
        assert_eq!(
            episode_indexes,
            vec![1, 2],
            "the episodes of a season must be ordered by index"
        );
        let pilot = first_season
            .episodes()
            .first()
            .ok_or("the first episode must be found")?;
        assert_eq!(pilot.name(), "Pilot");
        assert_eq!(pilot.plot(), "Episode plot");
        assert_eq!(
            pilot.air_date(),
            NaiveDate::from_ymd_opt(1991, 4, 8).unwrap_or_default()
        );
        assert_eq!(pilot.video().video_id(), 20);
        assert_eq!(pilot.video().file_id(), 10);
        assert_eq!(pilot.video().codec(), "H264");
        assert_eq!(pilot.video().width(), 1920);
        assert_eq!(pilot.video().height(), 10);
        assert_eq!(pilot.video().scan_type(), ScanType::Progressive);
        assert!((pilot.video().duration_ms() - 1000.0).abs() < f64::EPSILON);
        let second_season = first
            .seasons()
            .get(1)
            .ok_or("the second season must be found")?;
        assert_eq!(second_season.name(), None);
        assert_eq!(second_season.episodes().len(), 1);
        let second = series.get(1).ok_or("the second show must be found")?;
        assert_eq!(second.name(), "Northern Exposure");
        assert!(
            second.seasons().is_empty(),
            "a show without a season must report none"
        );
        assert!(second.genre_ids().is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn search_returns_a_season_that_holds_no_episode() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_season(&mut transaction, 40, 1, 1, None).await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let series = repository.search(&SeriesFilter::default()).await?;

        // Assert
        let seasons = series
            .first()
            .map(Series::seasons)
            .ok_or("the show must be hydrated")?;
        assert_eq!(seasons.len(), 1);
        let season = seasons.first().ok_or("the season must be hydrated")?;
        assert!(
            season.episodes().is_empty(),
            "a season without an episode holds none"
        );
        Ok(())
    }

    #[tokio::test]
    async fn search_filters_by_name_and_identifier() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_series(&mut transaction, 2, "Northern Exposure").await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let by_name = repository
            .search(&SeriesFilter {
                name: Some("Northern Exposure".to_owned()),
                ..SeriesFilter::default()
            })
            .await?;
        let by_id = repository
            .search(&SeriesFilter {
                id: Some(1),
                ..SeriesFilter::default()
            })
            .await?;

        // Assert
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name.first().map(Series::id), Some(2));
        assert_eq!(by_id.len(), 1);
        assert_eq!(by_id.first().map(Series::name), Some("Twin Peaks"));
        Ok(())
    }

    #[tokio::test]
    async fn search_returns_series_ordered_by_identifier() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_series(&mut transaction, 3, "Third").await?;
        seed_series(&mut transaction, 1, "First").await?;
        seed_series(&mut transaction, 2, "Second").await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let series = repository.search(&SeriesFilter::default()).await?;

        // Assert
        let ids: Vec<_> = series.iter().map(Series::id).collect();
        assert_eq!(ids, vec![1, 2, 3]);
        Ok(())
    }

    #[tokio::test]
    async fn search_addresses_the_largest_u32_episode_index() -> Result<(), Box<dyn Error>> {
        // Arrange: the largest index the schema and the domain both admit.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_season(&mut transaction, 40, 1, 0, None).await?;
        seed_episode(&mut transaction, 60, 40, 4_294_967_295, "The Last", 20).await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let series = repository.search(&SeriesFilter::default()).await?;

        // Assert
        let index = series
            .first()
            .and_then(|show| show.seasons().first())
            .and_then(|season| season.episodes().first())
            .map(Episode::index)
            .map(EpisodeIndex::value);
        assert_eq!(index, Some(4_294_967_295));
        Ok(())
    }

    #[tokio::test]
    async fn search_rejects_an_out_of_range_season_index() -> Result<(), Box<dyn Error>> {
        // Arrange: the range check is probed by relaxing it for this seed.
        let mut transaction = begin_transaction().await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&mut *transaction)
            .await?;
        seed_season(&mut transaction, 40, 1, 4_294_967_296, None).await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);
        let (buffer, _capture) = capture_logs();

        // Act
        let result = repository.search(&SeriesFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        assert_output_validation_failed(&captured_logs(&buffer), "season_index_out_of_range");
        Ok(())
    }

    #[tokio::test]
    async fn search_rejects_an_out_of_range_episode_index() -> Result<(), Box<dyn Error>> {
        // Arrange: the range check is probed by relaxing it for this seed.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_season(&mut transaction, 40, 1, 0, None).await?;
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&mut *transaction)
            .await?;
        seed_episode(&mut transaction, 60, 40, 4_294_967_296, "Too Far", 20).await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);
        let (buffer, _capture) = capture_logs();

        // Act
        let result = repository.search(&SeriesFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        assert_output_validation_failed(&captured_logs(&buffer), "episode_index_out_of_range");
        Ok(())
    }

    #[tokio::test]
    async fn search_rejects_a_stored_video_the_domain_cannot_hold() -> Result<(), Box<dyn Error>> {
        // Arrange: the width check is probed by relaxing it for this seed.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_season(&mut transaction, 40, 1, 0, None).await?;
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&mut *transaction)
            .await?;
        seed_video(&mut transaction, 20, 10, 0).await?;
        seed_episode(&mut transaction, 60, 40, 1, "Pilot", 20).await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);
        let (buffer, _capture) = capture_logs();

        // Act
        let result = repository.search(&SeriesFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        assert_output_validation_failed(&captured_logs(&buffer), "invalid_video");
        Ok(())
    }
}
