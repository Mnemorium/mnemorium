use sqlx::QueryBuilder;
use sqlx::Sqlite;
use sqlx::Transaction;

use crate::domain::model::movie::Movie;
use crate::domain::model::video::Video;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::movie_repository::MovieFilter;
use crate::domain::port::movie_repository::MovieRepository;

use super::error_mapping::stored_data_invalid;
use super::model::movie::MovieRow;
use super::model::video::parse_scan_type;
use super::query_filter::push_filter;

/// Repository querying films and the videos that back them, backed by a
/// `SQLite` transaction.
pub struct SqlxMovieRepository<'transaction> {
    /// Transaction the repository reads from.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxMovieRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }
}

impl MovieRepository for SqlxMovieRepository<'_> {
    async fn search(&mut self, filter: &MovieFilter) -> Result<Vec<Movie>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT
                movie.movie_id,
                movie.name,
                movie.plot,
                movie.country_of_origin,
                movie.release_date,
                movie.cover_file_id,
                COALESCE((
                    SELECT GROUP_CONCAT(genre_id, ',')
                    FROM (
                        SELECT genre_id
                        FROM movie_genre
                        WHERE movie_genre.movie_id = movie.movie_id
                        ORDER BY genre_id
                    )
                ), '') AS genres,
                video.video_id,
                video.duration_ms,
                video.codec,
                video.frame_count,
                video.width,
                video.height,
                video.color_id,
                video.scan_type,
                video.file_id
            FROM movie
            INNER JOIN video ON video.video_id = movie.video_id",
        );
        let mut first = true;

        if let Some(id) = filter.id {
            push_filter(&mut first, &mut builder, "movie.movie_id = ", id);
        }
        if let Some(name) = filter.name.as_deref() {
            push_filter(&mut first, &mut builder, "movie.name = ", name);
        }

        // A stable identifier order makes the caller's offset/limit pagination
        // deterministic; the use cases page in memory.
        builder.push(" ORDER BY movie.movie_id");

        let rows = builder
            .build_query_as::<MovieRow>()
            .fetch_all(&mut **self.transaction)
            .await?;

        rows.iter()
            .map(|row| {
                let scan_type = parse_scan_type(&row.scan_type)
                    .ok_or_else(|| stored_data_invalid("unknown_scan_type"))?;
                let video = Video::try_new(
                    row.video_id,
                    row.duration_ms,
                    row.codec.clone(),
                    row.frame_count,
                    row.width,
                    row.height,
                    row.color_id.clone(),
                    scan_type,
                    row.file_id,
                )
                .map_err(|_| stored_data_invalid("invalid_video"))?;
                Movie::try_new(
                    row.movie_id,
                    row.name.clone(),
                    row.plot.clone(),
                    row.country_of_origin.clone(),
                    row.release_date,
                    row.genres
                        .split(',')
                        .filter(|genre| !genre.is_empty())
                        .map(str::to_owned)
                        .collect(),
                    row.cover_file_id,
                    video,
                )
                .map_err(|_| stored_data_invalid("invalid_movie"))
            })
            .collect()
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

    use crate::domain::model::movie::Movie;
    use crate::domain::model::video::ScanType;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::movie_repository::MovieFilter;
    use crate::domain::port::movie_repository::MovieRepository as _;

    use super::SqlxMovieRepository;

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

    /// Seed a film backed by `video_id`, with a lowercase country of origin.
    async fn seed_movie(
        transaction: &mut Transaction<'static, Sqlite>,
        movie_id: i64,
        name: &str,
        video_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO movie (
                movie_id, name, plot, country_of_origin, release_date, video_id
             )
             VALUES (?1, ?2, 'A plot', 'us', '1959-08-17', ?3)",
        )
        .bind(movie_id)
        .bind(name)
        .bind(video_id)
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

    /// Attach `genre_id` to `movie_id`.
    async fn seed_movie_genre(
        transaction: &mut Transaction<'static, Sqlite>,
        movie_id: i64,
        genre_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO movie_genre (movie_id, genre_id) VALUES (?1, ?2)")
            .bind(movie_id)
            .bind(genre_id)
            .execute(&mut **transaction)
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn search_returns_movies_with_genres_and_their_video() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video_file(&mut transaction, 11).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_video(&mut transaction, 21, 11, 1280).await?;
        seed_movie(&mut transaction, 1, "Some Like It Hot", 20).await?;
        seed_movie(&mut transaction, 2, "Sunset Boulevard", 21).await?;
        seed_genre(&mut transaction, "COMEDY").await?;
        seed_genre(&mut transaction, "ROMANCE").await?;
        seed_movie_genre(&mut transaction, 1, "ROMANCE").await?;
        seed_movie_genre(&mut transaction, 1, "COMEDY").await?;
        let mut repository = SqlxMovieRepository::new(&mut transaction);

        // Act
        let movies = repository.search(&MovieFilter::default()).await?;

        // Assert
        assert_eq!(movies.len(), 2, "every film must be returned");
        let first = movies.first().ok_or("the first film must be found")?;
        assert_eq!(first.id(), 1);
        assert_eq!(first.name(), "Some Like It Hot");
        assert_eq!(first.plot(), "A plot");
        assert_eq!(
            first.country_of_origin(),
            "US",
            "the insert trigger must uppercase the country of origin"
        );
        assert_eq!(
            first.release_date(),
            NaiveDate::from_ymd_opt(1959, 8, 17).unwrap_or_default()
        );
        assert_eq!(
            first.genre_ids(),
            ["COMEDY", "ROMANCE"],
            "the genres must be ordered by identifier"
        );
        let video = first.video();
        assert_eq!(video.video_id(), 20);
        assert_eq!(video.file_id(), 10);
        assert_eq!(video.codec(), "H264");
        assert_eq!(video.width(), 1920);
        assert_eq!(video.height(), 10);
        assert_eq!(video.scan_type(), ScanType::Progressive);
        assert!((video.duration_ms() - 1000.0).abs() < f64::EPSILON);
        let second = movies.get(1).ok_or("the second film must be found")?;
        assert_eq!(second.id(), 2);
        assert!(
            second.genre_ids().is_empty(),
            "a film without a genre must report none"
        );
        assert_eq!(second.video().width(), 1280);
        Ok(())
    }

    #[tokio::test]
    async fn search_projects_the_cover_file() -> Result<(), Box<dyn Error>> {
        // Arrange: one covered film and one without artwork.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video_file(&mut transaction, 11).await?;
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, integrity_hash)
             VALUES (12, '12.png', 1, 0, 'image/png', ?1)",
        )
        .bind(format!("{:064x}", 12i64))
        .execute(&mut *transaction)
        .await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_video(&mut transaction, 21, 11, 1280).await?;
        seed_movie(&mut transaction, 1, "Some Like It Hot", 20).await?;
        seed_movie(&mut transaction, 2, "Sunset Boulevard", 21).await?;
        sqlx::query("UPDATE movie SET cover_file_id = 12 WHERE movie_id = 1")
            .execute(&mut *transaction)
            .await?;
        let mut repository = SqlxMovieRepository::new(&mut transaction);

        // Act
        let movies = repository.search(&MovieFilter::default()).await?;

        // Assert
        let covered = movies.first().ok_or("the covered film must be found")?;
        assert_eq!(covered.cover_file_id(), Some(12));
        let uncovered = movies.get(1).ok_or("the uncovered film must be found")?;
        assert_eq!(uncovered.cover_file_id(), None);
        Ok(())
    }

    #[tokio::test]
    async fn search_filters_by_name_and_identifier() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video_file(&mut transaction, 11).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_video(&mut transaction, 21, 11, 1280).await?;
        seed_movie(&mut transaction, 1, "Some Like It Hot", 20).await?;
        seed_movie(&mut transaction, 2, "Sunset Boulevard", 21).await?;
        let mut repository = SqlxMovieRepository::new(&mut transaction);

        // Act
        let by_name = repository
            .search(&MovieFilter {
                name: Some("Sunset Boulevard".to_owned()),
                ..MovieFilter::default()
            })
            .await?;
        let by_id = repository
            .search(&MovieFilter {
                id: Some(1),
                ..MovieFilter::default()
            })
            .await?;

        // Assert
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name.first().map(Movie::id), Some(2));
        assert_eq!(by_id.len(), 1);
        assert_eq!(by_id.first().map(Movie::name), Some("Some Like It Hot"));
        Ok(())
    }

    #[tokio::test]
    async fn search_returns_movies_ordered_by_identifier() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video_file(&mut transaction, 11).await?;
        seed_video_file(&mut transaction, 12).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_video(&mut transaction, 21, 11, 1920).await?;
        seed_video(&mut transaction, 22, 12, 1920).await?;
        seed_movie(&mut transaction, 3, "Third", 20).await?;
        seed_movie(&mut transaction, 1, "First", 21).await?;
        seed_movie(&mut transaction, 2, "Second", 22).await?;
        let mut repository = SqlxMovieRepository::new(&mut transaction);

        // Act
        let movies = repository.search(&MovieFilter::default()).await?;

        // Assert
        let ids: Vec<_> = movies.iter().map(Movie::id).collect();
        assert_eq!(ids, vec![1, 2, 3]);
        Ok(())
    }

    #[tokio::test]
    async fn search_returns_empty_for_unknown_name() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_movie(&mut transaction, 1, "Some Like It Hot", 20).await?;
        let mut repository = SqlxMovieRepository::new(&mut transaction);

        // Act
        let movies = repository
            .search(&MovieFilter {
                name: Some("absent".to_owned()),
                ..MovieFilter::default()
            })
            .await?;

        // Assert
        assert!(movies.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn search_rejects_a_stored_video_the_domain_cannot_hold() -> Result<(), Box<dyn Error>> {
        // Arrange: the width check is probed by relaxing it for this seed.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&mut *transaction)
            .await?;
        seed_video(&mut transaction, 20, 10, 0).await?;
        seed_movie(&mut transaction, 1, "Some Like It Hot", 20).await?;
        let mut repository = SqlxMovieRepository::new(&mut transaction);
        let (buffer, _capture) = capture_logs();

        // Act
        let result = repository.search(&MovieFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        assert_output_validation_failed(&captured_logs(&buffer), "invalid_video");
        Ok(())
    }

    #[tokio::test]
    async fn search_rejects_an_unknown_stored_scan_type() -> Result<(), Box<dyn Error>> {
        // Arrange: the scan-type check admits only the known tokens, so the
        // defensive mapping is probed by relaxing it for this seed.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_movie(&mut transaction, 1, "Some Like It Hot", 20).await?;
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&mut *transaction)
            .await?;
        sqlx::query("UPDATE video SET scan_type = 'BOGUS' WHERE video_id = 20")
            .execute(&mut *transaction)
            .await?;
        let mut repository = SqlxMovieRepository::new(&mut transaction);
        let (buffer, _capture) = capture_logs();

        // Act
        let result = repository.search(&MovieFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        assert_output_validation_failed(&captured_logs(&buffer), "unknown_scan_type");
        Ok(())
    }

    #[tokio::test]
    async fn search_rejects_a_film_whose_stored_name_is_invalid() -> Result<(), Box<dyn Error>> {
        // Arrange: `movie.name` has no non-empty check, so the domain guard is
        // probed by seeding an empty name.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_movie(&mut transaction, 1, "", 20).await?;
        let mut repository = SqlxMovieRepository::new(&mut transaction);
        let (buffer, _capture) = capture_logs();

        // Act
        let result = repository.search(&MovieFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        assert_output_validation_failed(&captured_logs(&buffer), "invalid_movie");
        Ok(())
    }
}
