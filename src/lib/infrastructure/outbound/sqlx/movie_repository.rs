use sqlx::{QueryBuilder, Sqlite, Transaction};

use crate::domain::alias::NumericID;
use crate::domain::model::movie::Movie;
use crate::domain::model::video::Video;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::movie_repository::{MovieFilter, MovieRepository};

use super::error_mapping::stored_data_invalid;
use super::model::movie::MovieRow;
use super::model::scope::parse_scope;
use super::model::video::parse_scan_type;
use super::query_filter::push_filter;

/// Repository querying films and the videos that back them, backed by a
/// `SQLite` transaction.
pub(crate) struct SqlxMovieRepository<'transaction> {
    /// Transaction the repository reads from.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxMovieRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub(crate) fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }

    /// Remove every genre link of `movie_id` and link `genre_ids` instead.
    ///
    /// The genre vocabulary is open, so a genre the catalogue does not know yet
    /// is inserted before it is linked.
    async fn replace_genres(
        &mut self,
        movie_id: NumericID,
        genre_ids: &[String],
    ) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM movie_genre WHERE movie_id = ?")
            .bind(movie_id)
            .execute(&mut **self.transaction)
            .await?;
        for genre_id in genre_ids {
            // The genre table uppercases its identifier through a trigger, so the
            // link is written in the stored vocabulary rather than the caller's.
            let stored_genre = genre_id.to_ascii_uppercase();
            sqlx::query("INSERT OR IGNORE INTO genre (genre_id) VALUES (?)")
                .bind(&stored_genre)
                .execute(&mut **self.transaction)
                .await?;
            sqlx::query("INSERT INTO movie_genre (movie_id, genre_id) VALUES (?1, ?2)")
                .bind(movie_id)
                .bind(&stored_genre)
                .execute(&mut **self.transaction)
                .await?;
        }
        Ok(())
    }
}

impl MovieRepository for SqlxMovieRepository<'_> {
    async fn create(&mut self, movie: Movie) -> Result<Movie, RepositoryError> {
        let movie_id = sqlx::query_scalar::<_, NumericID>(
            "INSERT INTO movie (
                name, plot, country_of_origin, release_date, video_id, cover_image_id
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             RETURNING movie_id",
        )
        .bind(movie.name())
        .bind(movie.plot())
        .bind(movie.country_of_origin())
        .bind(movie.release_date())
        .bind(movie.video_id())
        .bind(movie.cover_image_id())
        .fetch_one(&mut **self.transaction)
        .await?;

        self.replace_genres(movie_id, movie.genre_ids()).await?;

        self.search(&MovieFilter {
            id: Some(movie_id),
            ..MovieFilter::default()
        })
        .await?
        .into_iter()
        .next()
        .ok_or(RepositoryError::OperationFailed)
    }

    async fn search(&mut self, filter: &MovieFilter) -> Result<Vec<Movie>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT
                movie.movie_id,
                movie.name,
                movie.plot,
                movie.country_of_origin,
                movie.release_date,
                movie.cover_image_id,
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
                video.file_id,
                video.scope AS video_scope
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
        if let Some(video_id) = filter.video_id {
            push_filter(&mut first, &mut builder, "movie.video_id = ", video_id);
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
                let scope = row
                    .video_scope
                    .as_deref()
                    .map(|value| {
                        parse_scope(value).ok_or_else(|| stored_data_invalid("unknown_scope"))
                    })
                    .transpose()?;
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
                    scope,
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
                    row.cover_image_id,
                    video,
                )
                .map_err(|_| stored_data_invalid("invalid_movie"))
            })
            .collect()
    }

    async fn update(&mut self, movie: Movie) -> Result<Option<Movie>, RepositoryError> {
        let result = sqlx::query(
            "UPDATE movie
             SET name = ?1,
                 plot = ?2,
                 country_of_origin = ?3,
                 release_date = ?4,
                 cover_image_id = ?5
             WHERE movie_id = ?6",
        )
        .bind(movie.name())
        .bind(movie.plot())
        .bind(movie.country_of_origin())
        .bind(movie.release_date())
        .bind(movie.cover_image_id())
        .bind(movie.id())
        .execute(&mut **self.transaction)
        .await?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }

        self.replace_genres(movie.id(), movie.genre_ids()).await?;
        self.search(&MovieFilter {
            id: Some(movie.id()),
            ..MovieFilter::default()
        })
        .await?
        .into_iter()
        .next()
        .map(Some)
        .ok_or(RepositoryError::OperationFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::video::ScanType;
    use crate::test_helpers::*;
    use chrono::NaiveDate;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::error::Error;
    use tracing_test::traced_test;

    /// Assert the captured output is exactly one `output_validation_failed`
    /// event with `reason`, carrying neither a stored value nor an `sqlx`
    /// detail.
    fn assert_output_validation_failed(lines: &[&str], reason: &str) -> Result<(), String> {
        let ours: Vec<&str> = lines
            .iter()
            .copied()
            .filter(|line| line.contains("security:"))
            .collect();
        let warn_count = ours.iter().filter(|line| line.contains("WARN")).count();
        if warn_count != 1 {
            return Err(format!(
                "rejected stored data must emit exactly one warn event (found {warn_count})"
            ));
        }
        if !ours
            .iter()
            .any(|line| line.contains("event=\"output_validation_failed\""))
        {
            return Err("the event must be output_validation_failed".to_owned());
        }
        if !ours
            .iter()
            .any(|line| line.contains(&format!("reason=\"{reason}\"")))
        {
            return Err(format!("the event must carry the stable reason `{reason}`"));
        }
        if ours.iter().any(|line| line.contains("sqlx")) {
            return Err("the event must not carry the sqlx detail".to_owned());
        }
        Ok(())
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
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, size_bytes, modified_at, origin, integrity_hash)
             VALUES (?1, ?2, 1, 0, 'video/mp4', 1024, '2026-10-06 00:00:00', 'UPLOADED', ?3)",
        )
        .bind(file_id)
        .bind(format!("{file_id}.mp4"))
        .bind(format!("{file_id:064x}"))
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed an image row of `image_id` on `file_id`, in a square orientation.
    async fn seed_image(
        transaction: &mut Transaction<'static, Sqlite>,
        image_id: i64,
        file_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO image (image_id, name, width_px, height_px, orientation, created_at, file_id)
             VALUES (?1, ?2, 10, 10, 'SQUARE', '2026-10-06', ?3)",
        )
        .bind(image_id)
        .bind(format!("{image_id}.png"))
        .bind(file_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed an image file of `file_id`.
    async fn seed_image_file(
        transaction: &mut Transaction<'static, Sqlite>,
        file_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, size_bytes, modified_at, origin, integrity_hash)
             VALUES (?1, ?2, 1, 0, 'image/png', 1024, '2026-10-06 00:00:00', 'UPLOADED', ?3)",
        )
        .bind(file_id)
        .bind(format!("{file_id}.png"))
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

    /// Seed a show holding one season and an episode backed by `video_id`.
    async fn seed_episode_backed_by(
        transaction: &mut Transaction<'static, Sqlite>,
        video_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO series (series_id, name, plot) VALUES (1, 'Twin Peaks', 'A plot')",
        )
        .execute(&mut **transaction)
        .await?;
        sqlx::query("INSERT INTO season (season_id, series_id, season_index) VALUES (1, 1, 1)")
            .execute(&mut **transaction)
            .await?;
        sqlx::query(
            "INSERT INTO episode (episode_id, season_id, episode_index, name, plot, video_id)
             VALUES (1, 1, 1, 'Pilot', 'A plot', ?1)",
        )
        .bind(video_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn search_reports_an_absent_release_date_and_country_as_none()
    -> Result<(), Box<dyn Error>> {
        // Arrange: a film storing neither a release date nor a country.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        sqlx::query(
            "INSERT INTO movie (movie_id, name, plot, video_id)
             VALUES (1, 'Some Like It Hot', 'A plot', 20)",
        )
        .execute(&mut *transaction)
        .await?;
        let mut repository = SqlxMovieRepository::new(&mut transaction);

        // Act
        let movies = repository.search(&MovieFilter::default()).await?;

        // Assert
        let movie = movies.first().ok_or("the film must be found")?;
        assert_eq!(
            movie.release_date(),
            None,
            "a stored null must read as none"
        );
        assert_eq!(
            movie.country_of_origin(),
            None,
            "a stored null must read as none"
        );
        Ok(())
    }

    #[tokio::test]
    async fn insert_rejects_a_video_already_backing_an_episode() -> Result<(), Box<dyn Error>> {
        // Arrange: an episode already backs video 20.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_episode_backed_by(&mut transaction, 20).await?;

        // Act
        let result = sqlx::query(
            "INSERT INTO movie (movie_id, name, plot, video_id)
             VALUES (1, 'Some Like It Hot', 'A plot', 20)",
        )
        .execute(&mut *transaction)
        .await
        .map_err(RepositoryError::from);

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "a video backing an episode must be rejected as a conflict, got: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn update_rejects_a_video_already_backing_an_episode() -> Result<(), Box<dyn Error>> {
        // Arrange: film 1 backs video 21 and an episode backs video 20.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video_file(&mut transaction, 11).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_video(&mut transaction, 21, 11, 1920).await?;
        seed_episode_backed_by(&mut transaction, 20).await?;
        seed_movie(&mut transaction, 1, "Some Like It Hot", 21).await?;

        // Act
        let result = sqlx::query("UPDATE movie SET video_id = 20 WHERE movie_id = 1")
            .execute(&mut *transaction)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "moving a film onto an episode's video must be rejected as a conflict, got: {result:?}"
        );
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
            Some("US"),
            "the insert trigger must uppercase the country of origin"
        );
        assert_eq!(
            first.release_date(),
            Some(NaiveDate::from_ymd_opt(1959, 8, 17).unwrap_or_default())
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
    async fn search_projects_the_cover_image() -> Result<(), Box<dyn Error>> {
        // Arrange: one covered film and one without artwork.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video_file(&mut transaction, 11).await?;
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, size_bytes, modified_at, origin, integrity_hash)
             VALUES (12, '12.png', 1, 0, 'image/png', 1024, '2026-10-06 00:00:00', 'UPLOADED', ?1)",
        )
        .bind(format!("{:064x}", 12i64))
        .execute(&mut *transaction)
        .await?;
        seed_image(&mut transaction, 12, 12).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_video(&mut transaction, 21, 11, 1280).await?;
        seed_movie(&mut transaction, 1, "Some Like It Hot", 20).await?;
        seed_movie(&mut transaction, 2, "Sunset Boulevard", 21).await?;
        sqlx::query("UPDATE movie SET cover_image_id = 12 WHERE movie_id = 1")
            .execute(&mut *transaction)
            .await?;
        let mut repository = SqlxMovieRepository::new(&mut transaction);

        // Act
        let movies = repository.search(&MovieFilter::default()).await?;

        // Assert
        let covered = movies.first().ok_or("the covered film must be found")?;
        assert_eq!(covered.cover_image_id(), Some(12));
        let uncovered = movies.get(1).ok_or("the uncovered film must be found")?;
        assert_eq!(uncovered.cover_image_id(), None);
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
    #[traced_test]
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

        // Act
        let result = repository.search(&MovieFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        logs_assert(|lines| assert_output_validation_failed(lines, "invalid_video"));
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
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

        // Act
        let result = repository.search(&MovieFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        logs_assert(|lines| assert_output_validation_failed(lines, "unknown_scan_type"));
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn search_rejects_a_film_whose_stored_name_is_invalid() -> Result<(), Box<dyn Error>> {
        // Arrange: `movie.name` has no non-empty check, so the domain guard is
        // probed by seeding an empty name.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_movie(&mut transaction, 1, "", 20).await?;
        let mut repository = SqlxMovieRepository::new(&mut transaction);

        // Act
        let result = repository.search(&MovieFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        logs_assert(|lines| assert_output_validation_failed(lines, "invalid_movie"));
        Ok(())
    }

    #[tokio::test]
    async fn create_stores_the_film_with_its_genres_and_cover() -> Result<(), Box<dyn Error>> {
        // Arrange: a free video and a library cover, with two genre identifiers
        // the catalogue does not hold yet.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_image_file(&mut transaction, 12).await?;
        seed_image(&mut transaction, 12, 12).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        let movie = Movie::try_new(
            0,
            "Some Like It Hot".to_owned(),
            "A plot".to_owned(),
            Some("us".to_owned()),
            Some(NaiveDate::from_ymd_opt(1959, 8, 17).unwrap_or_default()),
            vec!["Drama".to_owned(), "MYSTERY".to_owned()],
            Some(12),
            sample_video(20, 10)?,
        )?;
        let mut repository = SqlxMovieRepository::new(&mut transaction);

        // Act
        let stored = repository.create(movie).await?;

        // Assert
        assert_ne!(stored.id(), 0, "the repository assigns an identity");
        assert_eq!(stored.name(), "Some Like It Hot");
        assert_eq!(stored.plot(), "A plot");
        assert_eq!(
            stored.country_of_origin(),
            Some("US"),
            "the insert trigger must uppercase the country of origin"
        );
        assert_eq!(
            stored.release_date(),
            Some(NaiveDate::from_ymd_opt(1959, 8, 17).unwrap_or_default())
        );
        assert_eq!(
            stored.genre_ids(),
            ["DRAMA", "MYSTERY"],
            "an unknown genre must be stored in the catalogue vocabulary and linked"
        );
        assert_eq!(stored.cover_image_id(), Some(12));
        assert_eq!(stored.video_id(), 20, "the film must read back its video");
        Ok(())
    }

    #[tokio::test]
    async fn create_rejects_a_video_already_backing_a_film() -> Result<(), Box<dyn Error>> {
        // Arrange: film 1 already backs video 20.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_movie(&mut transaction, 1, "Some Like It Hot", 20).await?;
        let movie = Movie::try_new(
            0,
            "Another Film".to_owned(),
            "A plot".to_owned(),
            None,
            None,
            Vec::new(),
            None,
            sample_video(20, 10)?,
        )?;
        let mut repository = SqlxMovieRepository::new(&mut transaction);

        // Act
        let result = repository.create(movie).await;

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::AlreadyExist)),
            "a video already backing a film must violate the unique key, got: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn create_rejects_a_video_already_backing_an_episode() -> Result<(), Box<dyn Error>> {
        // Arrange: an episode already backs video 20.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_episode_backed_by(&mut transaction, 20).await?;
        let movie = Movie::try_new(
            0,
            "Some Like It Hot".to_owned(),
            "A plot".to_owned(),
            None,
            None,
            Vec::new(),
            None,
            sample_video(20, 10)?,
        )?;
        let mut repository = SqlxMovieRepository::new(&mut transaction);

        // Act
        let result = repository.create(movie).await;

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "the cross-kind trigger must reject a video backing an episode, got: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn create_rejects_a_cover_that_is_not_a_library_image() -> Result<(), Box<dyn Error>> {
        // Arrange: image 12 is attached to a user asset, so it cannot become a
        // film cover.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_image_file(&mut transaction, 12).await?;
        seed_image(&mut transaction, 12, 12).await?;
        sqlx::query("UPDATE image SET scope = 'USER' WHERE image_id = 12")
            .execute(&mut *transaction)
            .await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        let movie = Movie::try_new(
            0,
            "Some Like It Hot".to_owned(),
            "A plot".to_owned(),
            None,
            None,
            Vec::new(),
            Some(12),
            sample_video(20, 10)?,
        )?;
        let mut repository = SqlxMovieRepository::new(&mut transaction);

        // Act
        let result = repository.create(movie).await;

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "a USER image must be rejected as a cover conflict, got: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn update_replaces_metadata_and_genres_but_not_the_video() -> Result<(), Box<dyn Error>> {
        // Arrange: film 1 backs video 20; the update names an unseeded video to
        // prove the adapter never rewrites it.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_movie(&mut transaction, 1, "Old Name", 20).await?;
        let updated = Movie::try_new(
            1,
            "New Name".to_owned(),
            "New plot".to_owned(),
            Some("GB".to_owned()),
            Some(NaiveDate::from_ymd_opt(1960, 1, 2).unwrap_or_default()),
            vec!["Science Fiction".to_owned()],
            None,
            sample_video(99, 99)?,
        )?;
        let mut repository = SqlxMovieRepository::new(&mut transaction);

        // Act
        let stored = repository.update(updated).await?;

        // Assert
        let persisted = stored.ok_or("the stored film must be returned")?;
        assert_eq!(persisted.name(), "New Name");
        assert_eq!(persisted.plot(), "New plot");
        assert_eq!(persisted.country_of_origin(), Some("GB"));
        assert_eq!(
            persisted.release_date(),
            Some(NaiveDate::from_ymd_opt(1960, 1, 2).unwrap_or_default())
        );
        assert_eq!(
            persisted.genre_ids(),
            ["SCIENCE FICTION"],
            "the genre links must be replaced"
        );
        assert_eq!(
            persisted.video_id(),
            20,
            "the update must never rewrite the backing video"
        );
        Ok(())
    }

    #[tokio::test]
    async fn update_returns_none_for_an_absent_film() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let updated = Movie::try_new(
            404,
            "New Name".to_owned(),
            "New plot".to_owned(),
            None,
            None,
            Vec::new(),
            None,
            sample_video(1, 1)?,
        )?;
        let mut repository = SqlxMovieRepository::new(&mut transaction);

        // Act
        let stored = repository.update(updated).await?;

        // Assert
        assert!(stored.is_none(), "an absent film must read as none");
        Ok(())
    }
}
