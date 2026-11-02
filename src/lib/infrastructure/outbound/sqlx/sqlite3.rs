use std::fs::File;
use std::path::Path;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqliteJournalMode;
use sqlx::sqlite::SqlitePoolOptions;
use tracing::warn;

/// Time a connection waits for a lock held by another connection.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
/// Upper bound of the write-ahead log file, in bytes (67 108 864; 64 MiB).
///
/// The value is a string because the consumer is a SQL pragma.
const WAL_SIZE_LIMIT_BYTES: &str = "67108864";

/// Initializes the `SQLite` database connection pool, creating the database file
/// when it does not exist.
///
/// # Errors
///
/// Returns an error when the database file cannot be created, when the
/// connection to the database cannot be established, or when applying
/// migrations fails.
pub async fn init_db(db_file: &str, max_connections: u32) -> Result<SqlitePool, sqlx::Error> {
    if !Path::new(db_file).exists() {
        File::create(db_file)?;
        warn!("sqlite database file {db_file} did not exist; created it");
    }

    // Write-ahead logging lets a read transaction (for example the one hashing
    // a staged upload) coexist with the single concurrent writer instead of
    // blocking it, which is what makes a pool larger than one connection
    // useful. The busy timeout bounds how long a connection waits for a lock
    // instead of failing immediately.
    //
    // `sqlx` 0.9 enables foreign-key enforcement by default; the explicit call
    // states the dependency the schema relies on, and `init_db` is asserted to
    // report `PRAGMA foreign_keys = 1` below.
    //
    // Recursive triggers make the implicit delete of an `INSERT OR REPLACE`
    // fire the `BEFORE DELETE` guard, so the seeded default gallery cannot be
    // recreated through a conflicting replace.
    let options = SqliteConnectOptions::new()
        .filename(db_file)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(BUSY_TIMEOUT)
        .pragma("journal_size_limit", WAL_SIZE_LIMIT_BYTES)
        .foreign_keys(true)
        .pragma("recursive_triggers", "ON");

    let pool = SqlitePoolOptions::new()
        .max_connections(max_connections)
        .connect_with(options)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use chrono::Utc;
    use tempfile::tempdir;
    use tokio::sync::Barrier;
    use tokio::task::JoinSet;

    use sqlx::SqlitePool;
    use sqlx::sqlite::SqlitePoolOptions;

    use super::init_db;
    use crate::domain::model::file::File;
    use crate::domain::port::asset_unit_of_work::AssetUnitOfWork as _;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::file_repository::FileFilter;
    use crate::domain::port::file_repository::FileRepository as _;
    use crate::domain::port::unit_of_work::UnitOfWork as _;
    use crate::domain::port::unit_of_work::UnitOfWorkFactory as _;
    use crate::infrastructure::outbound::sqlx::unit_of_work::SqlxUnitOfWorkFactory;
    use crate::test_helpers::DIGEST;

    /// A second digest, distinct from [`DIGEST`], for the second seeded file.
    const DIGEST_ALT: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

    /// `SQLite` extended result code for a writer that found the lock held
    /// (`SQLITE_BUSY`).
    const SQLITE_BUSY: &str = "5";

    /// How many units of work contend for the single `SQLite` writer.
    const WRITERS: usize = 8;

    #[tokio::test]
    async fn init_db_missing_file_creates_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let db_path = tmp.path().join("test.db");

        // Act
        let pool = init_db(db_path.to_string_lossy().as_ref(), 1).await?;

        // Assert
        assert!(
            db_path.exists(),
            "database file should exist after init_db when the path was missing"
        );

        pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn init_db_enables_wal_and_honours_max_connections() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let db_path = tmp.path().join("test.db");

        // Act
        let pool = init_db(db_path.to_string_lossy().as_ref(), 3).await?;

        // Assert
        assert_eq!(
            pool.options().get_max_connections(),
            3,
            "the pool must honour the configured maximum"
        );
        let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            journal_mode, "wal",
            "init_db must enable write-ahead logging"
        );
        let size_limit: i64 = sqlx::query_scalar("PRAGMA journal_size_limit")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            size_limit, 67_108_864,
            "init_db must bound the size of the write-ahead log"
        );
        let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            foreign_keys, 1,
            "init_db must enforce foreign-key constraints"
        );
        let recursive_triggers: i64 = sqlx::query_scalar("PRAGMA recursive_triggers")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            recursive_triggers, 1,
            "init_db must enable recursive triggers so REPLACE fires delete guards"
        );

        pool.close().await;
        Ok(())
    }

    /// Open a migrated in-memory database, one connection per call, so each
    /// test runs against a fresh database (TEST-033). The two pragmas mirror
    /// [`init_db`]: foreign-key enforcement backs the cascade tests and
    /// recursive triggers make the implicit delete of an `INSERT OR REPLACE`
    /// fire the default-gallery guard.
    async fn in_memory_pool() -> Result<SqlitePool, Box<dyn Error>> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::raw_sql("PRAGMA foreign_keys = ON; PRAGMA recursive_triggers = ON;")
            .execute(&pool)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(pool)
    }

    /// Seed one owner, one image and one video, each backed by its own file.
    async fn seed_media(pool: &SqlitePool) -> Result<(), Box<dyn Error>> {
        sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (1, 'hash')")
            .execute(pool)
            .await?;
        sqlx::query(
            "INSERT INTO user (user_id, role, username, email, credential_id)
             VALUES (1, 'STANDARD', 'tester', NULL, 1)",
        )
        .execute(pool)
        .await?;
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, size_bytes, modified_at, origin, integrity_hash)
             VALUES (1, 'a.png', 1, 0, 'image/png', 1024, '2026-10-06 00:00:00', 'UPLOADED', ?1)",
        )
        .bind(DIGEST)
        .execute(pool)
        .await?;
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, size_bytes, modified_at, origin, integrity_hash)
             VALUES (2, 'b.mp4', 1, 0, 'video/mp4', 1024, '2026-10-06 00:00:00', 'UPLOADED', ?1)",
        )
        .bind(DIGEST_ALT)
        .execute(pool)
        .await?;
        sqlx::query(
            "INSERT INTO image (image_id, name, width_px, height_px, orientation, created_at, file_id)
             VALUES (1, 'a.png', 10, 10, 'SQUARE', '2026-10-06', 1)",
        )
        .execute(pool)
        .await?;
        sqlx::query(
            "INSERT INTO video (video_id, duration_ms, codec, frame_count, width, height, color_id, scan_type, file_id)
             VALUES (1, 1000.0, 'H264', 30, 10, 10, 'YCbCr', 'PROGRESSIVE', 2)",
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Insert a private gallery owned by the seeded user.
    async fn seed_gallery(pool: &SqlitePool, gallery_id: i64) -> Result<(), Box<dyn Error>> {
        sqlx::query(
            "INSERT INTO gallery (gallery_id, name, created_at, last_modified_at, is_public, user_id)
             VALUES (?1, ?2, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 0, 1)",
        )
        .bind(gallery_id)
        .bind(format!("gallery {gallery_id}"))
        .execute(pool)
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn gallery_item_rejects_neither_media() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_gallery(&pool, 1).await?;

        // Act
        let result = sqlx::query(
            "INSERT INTO gallery_item (gallery_item_id, gallery_id, item_index)
             VALUES (1, 1, 0)",
        )
        .execute(&pool)
        .await
        .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn gallery_item_rejects_both_media() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_gallery(&pool, 1).await?;

        // Act
        let result = sqlx::query(
            "INSERT INTO gallery_item (gallery_item_id, gallery_id, image_id, video_id, item_index)
             VALUES (1, 1, 1, 1, 0)",
        )
        .execute(&pool)
        .await
        .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn gallery_item_rejects_negative_index() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_gallery(&pool, 1).await?;

        // Act
        let result = sqlx::query(
            "INSERT INTO gallery_item (gallery_item_id, gallery_id, image_id, item_index)
             VALUES (1, 1, 1, -1)",
        )
        .execute(&pool)
        .await
        .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn gallery_item_rejects_duplicate_index_within_a_gallery() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_gallery(&pool, 1).await?;
        sqlx::query(
            "INSERT INTO gallery_item (gallery_item_id, gallery_id, image_id, item_index)
             VALUES (1, 1, 1, 0)",
        )
        .execute(&pool)
        .await?;

        // Act
        let result = sqlx::query(
            "INSERT INTO gallery_item (gallery_item_id, gallery_id, video_id, item_index)
             VALUES (2, 1, 1, 0)",
        )
        .execute(&pool)
        .await
        .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn gallery_item_rejects_media_already_assigned() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_gallery(&pool, 1).await?;
        seed_gallery(&pool, 2).await?;
        sqlx::query(
            "INSERT INTO gallery_item (gallery_item_id, gallery_id, image_id, item_index)
             VALUES (1, 1, 1, 0)",
        )
        .execute(&pool)
        .await?;

        // Act
        let result = sqlx::query(
            "INSERT INTO gallery_item (gallery_item_id, gallery_id, image_id, item_index)
             VALUES (2, 2, 1, 0)",
        )
        .execute(&pool)
        .await
        .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn deleting_gallery_cascades_its_items() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_gallery(&pool, 1).await?;
        sqlx::query(
            "INSERT INTO gallery_item (gallery_item_id, gallery_id, image_id, item_index)
             VALUES (1, 1, 1, 0)",
        )
        .execute(&pool)
        .await?;

        // Act
        sqlx::query("DELETE FROM gallery WHERE gallery_id = 1")
            .execute(&pool)
            .await?;

        // Assert
        let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM gallery_item")
            .fetch_one(&pool)
            .await?;
        assert_eq!(remaining, 0, "deleting a gallery must cascade its items");
        Ok(())
    }

    #[tokio::test]
    async fn deleting_image_cascades_its_item() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_gallery(&pool, 1).await?;
        sqlx::query(
            "INSERT INTO gallery_item (gallery_item_id, gallery_id, image_id, item_index)
             VALUES (1, 1, 1, 0)",
        )
        .execute(&pool)
        .await?;

        // Act
        sqlx::query("DELETE FROM image WHERE image_id = 1")
            .execute(&pool)
            .await?;

        // Assert
        let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM gallery_item")
            .fetch_one(&pool)
            .await?;
        assert_eq!(remaining, 0, "deleting an image must cascade its item");
        Ok(())
    }

    #[tokio::test]
    async fn deleting_video_cascades_its_item() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_gallery(&pool, 1).await?;
        sqlx::query(
            "INSERT INTO gallery_item (gallery_item_id, gallery_id, video_id, item_index)
             VALUES (1, 1, 1, 0)",
        )
        .execute(&pool)
        .await?;

        // Act
        sqlx::query("DELETE FROM video WHERE video_id = 1")
            .execute(&pool)
            .await?;

        // Assert
        let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM gallery_item")
            .fetch_one(&pool)
            .await?;
        assert_eq!(remaining, 0, "deleting a video must cascade its item");
        Ok(())
    }

    #[tokio::test]
    async fn deleting_file_referenced_by_media_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;

        // Act
        let result = sqlx::query("DELETE FROM file WHERE file_id = 1")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn default_gallery_is_seeded_public_and_ownerless() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;

        // Act
        let (name, is_public, user_id): (String, bool, Option<i64>) =
            sqlx::query_as("SELECT name, is_public, user_id FROM gallery WHERE gallery_id = 0")
                .fetch_one(&pool)
                .await?;

        // Assert
        assert_eq!(name, "Default");
        assert!(is_public, "the default gallery must be public");
        assert_eq!(user_id, None, "the default gallery must be ownerless");
        Ok(())
    }

    #[tokio::test]
    async fn default_gallery_cannot_be_deleted() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;

        // Act
        let result = sqlx::query("DELETE FROM gallery WHERE gallery_id = 0")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(result, Err(RepositoryError::Conflict)));
        Ok(())
    }

    #[tokio::test]
    async fn default_gallery_cannot_be_renamed() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;

        // Act
        let result = sqlx::query("UPDATE gallery SET name = 'Other' WHERE gallery_id = 0")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(result, Err(RepositoryError::Conflict)));
        Ok(())
    }

    #[tokio::test]
    async fn default_gallery_cannot_change_owner() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;

        // Act
        let result = sqlx::query("UPDATE gallery SET user_id = 1 WHERE gallery_id = 0")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(result, Err(RepositoryError::Conflict)));
        Ok(())
    }

    #[tokio::test]
    async fn default_gallery_cannot_be_rekeyed() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;

        // Act
        let result = sqlx::query("UPDATE gallery SET gallery_id = 5 WHERE gallery_id = 0")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(result, Err(RepositoryError::Conflict)));
        Ok(())
    }

    #[tokio::test]
    async fn default_gallery_cannot_be_replaced() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;

        // Act
        let result = sqlx::query(
            "INSERT OR REPLACE INTO gallery
             (gallery_id, name, created_at, last_modified_at, is_public, user_id)
             VALUES (0, 'Replaced', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 1, NULL)",
        )
        .execute(&pool)
        .await
        .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(result, Err(RepositoryError::Conflict)));
        Ok(())
    }

    #[tokio::test]
    async fn default_gallery_last_modified_at_can_be_touched() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;

        // Act
        let affected = sqlx::query(
            "UPDATE gallery SET last_modified_at = '2099-01-01 00:00:00' WHERE gallery_id = 0",
        )
        .execute(&pool)
        .await?
        .rows_affected();

        // Assert
        assert_eq!(affected, 1, "touching last_modified_at must be allowed");
        Ok(())
    }

    #[tokio::test]
    async fn genre_insert_normalises_genre_id_uppercase() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;

        // Act
        sqlx::query("INSERT INTO genre (genre_id) VALUES ('rock')")
            .execute(&pool)
            .await?;

        // Assert
        let stored: String = sqlx::query_scalar("SELECT genre_id FROM genre")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            stored, "ROCK",
            "the insert trigger must normalise genre_id to uppercase"
        );
        Ok(())
    }

    #[tokio::test]
    async fn genre_update_normalises_genre_id_uppercase() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        sqlx::query("INSERT INTO genre (genre_id) VALUES ('JAZZ')")
            .execute(&pool)
            .await?;

        // Act
        sqlx::query("UPDATE genre SET genre_id = 'jazz' WHERE genre_id = 'JAZZ'")
            .execute(&pool)
            .await?;

        // Assert
        let stored: String = sqlx::query_scalar("SELECT genre_id FROM genre")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            stored, "JAZZ",
            "the update trigger must normalise genre_id to uppercase"
        );
        Ok(())
    }

    #[tokio::test]
    async fn genre_update_normalises_referenced_genre_id() -> Result<(), Box<dyn Error>> {
        // Arrange: a genre referenced by a music album.
        let pool = in_memory_pool().await?;
        sqlx::query(
            "INSERT INTO music_album (music_album_id, name, release_date)
             VALUES (1, 'Album', '2026-01-01')",
        )
        .execute(&pool)
        .await?;
        sqlx::query("INSERT INTO genre (genre_id) VALUES ('ROCK')")
            .execute(&pool)
            .await?;
        sqlx::query("INSERT INTO music_album_genre (music_album_id, genre_id) VALUES (1, 'ROCK')")
            .execute(&pool)
            .await?;

        // Act
        sqlx::query("UPDATE genre SET genre_id = 'rock' WHERE genre_id = 'ROCK'")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from)?;

        // Assert: the parent keeps its normalised key and the child still resolves.
        let stored: String = sqlx::query_scalar("SELECT genre_id FROM genre")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            stored, "ROCK",
            "the update must keep the genre key uppercase"
        );
        let children: i64 =
            sqlx::query_scalar("SELECT count(*) FROM music_album_genre WHERE genre_id = 'ROCK'")
                .fetch_one(&pool)
                .await?;
        assert_eq!(
            children, 1,
            "the child row must still resolve to its parent"
        );
        Ok(())
    }

    #[tokio::test]
    async fn genre_update_colliding_with_existing_key_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        sqlx::query("INSERT INTO genre (genre_id) VALUES ('ROCK')")
            .execute(&pool)
            .await?;
        sqlx::query("INSERT INTO genre (genre_id) VALUES ('POP')")
            .execute(&pool)
            .await?;

        // Act: the normalising update collides with the existing uppercase row.
        let result = sqlx::query("UPDATE genre SET genre_id = 'rock' WHERE genre_id = 'POP'")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    /// Seed a show with a lowercase country of origin.
    async fn seed_series(pool: &SqlitePool, series_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO series (series_id, name, plot, country_of_origin, first_air_date)
             VALUES (?1, ?2, 'A plot', 'us', '1990-04-08')",
        )
        .bind(series_id)
        .bind(format!("series {series_id}"))
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Seed a season of `series_id` at `season_index`.
    async fn seed_season(
        pool: &SqlitePool,
        season_id: i64,
        series_id: i64,
        season_index: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO season (season_id, series_id, season_index) VALUES (?1, ?2, ?3)")
            .bind(season_id)
            .bind(series_id)
            .bind(season_index)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Seed an episode of `season_id` at `episode_index`, backed by `video_id`.
    async fn seed_episode(
        pool: &SqlitePool,
        episode_id: i64,
        season_id: i64,
        episode_index: i64,
        video_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO episode (
                episode_id, season_id, episode_index, name, plot, air_date, video_id
             )
             VALUES (?1, ?2, ?3, ?4, 'A plot', '1991-04-08', ?5)",
        )
        .bind(episode_id)
        .bind(season_id)
        .bind(episode_index)
        .bind(format!("episode {episode_id}"))
        .bind(video_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn series_insert_normalises_country_of_origin_uppercase() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;

        // Act
        seed_series(&pool, 1).await?;

        // Assert
        let stored: String = sqlx::query_scalar("SELECT country_of_origin FROM series")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            stored, "US",
            "the insert trigger must normalise country_of_origin to uppercase"
        );
        Ok(())
    }

    #[tokio::test]
    async fn series_update_normalises_country_of_origin_uppercase() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_series(&pool, 1).await?;

        // Act
        sqlx::query("UPDATE series SET country_of_origin = 'ca' WHERE series_id = 1")
            .execute(&pool)
            .await?;

        // Assert
        let stored: String = sqlx::query_scalar("SELECT country_of_origin FROM series")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            stored, "CA",
            "the update trigger must normalise country_of_origin to uppercase"
        );
        Ok(())
    }

    #[tokio::test]
    async fn season_rejects_an_index_above_the_u32_range() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_series(&pool, 1).await?;

        // Act
        let result = seed_season(&pool, 40, 1, 4_294_967_296)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn season_accepts_the_largest_u32_index() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_series(&pool, 1).await?;

        // Act
        seed_season(&pool, 40, 1, 4_294_967_295).await?;

        // Assert
        let stored: i64 = sqlx::query_scalar("SELECT season_index FROM season")
            .fetch_one(&pool)
            .await?;
        assert_eq!(stored, 4_294_967_295);
        Ok(())
    }

    #[tokio::test]
    async fn episode_rejects_an_index_above_the_u32_range() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_series(&pool, 1).await?;
        seed_season(&pool, 40, 1, 1).await?;

        // Act
        let result = seed_episode(&pool, 60, 40, 4_294_967_296, 1)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn season_rejects_a_duplicate_index_within_a_series() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_series(&pool, 1).await?;
        seed_season(&pool, 40, 1, 1).await?;

        // Act
        let result = seed_season(&pool, 41, 1, 1)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn episode_rejects_a_duplicate_index_within_a_season() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_series(&pool, 1).await?;
        seed_season(&pool, 40, 1, 1).await?;
        seed_episode(&pool, 60, 40, 1, 1).await?;
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, size_bytes, modified_at, origin, integrity_hash)
             VALUES (3, 'c.mp4', 1, 0, 'video/mp4', 1024, '2026-10-06 00:00:00', 'UPLOADED', ?1)",
        )
        .bind(format!("{:064x}", 3i32))
        .execute(&pool)
        .await?;
        sqlx::query(
            "INSERT INTO video (video_id, duration_ms, codec, frame_count, width, height, color_id, scan_type, file_id)
             VALUES (2, 1000.0, 'H264', 30, 10, 10, 'YCbCr', 'PROGRESSIVE', 3)",
        )
        .execute(&pool)
        .await?;

        // Act
        let result = seed_episode(&pool, 61, 40, 1, 2)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn episode_rejects_a_video_already_assigned() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_series(&pool, 1).await?;
        seed_season(&pool, 40, 1, 1).await?;
        seed_season(&pool, 41, 1, 2).await?;
        seed_episode(&pool, 60, 40, 1, 1).await?;

        // Act
        let result = seed_episode(&pool, 61, 41, 1, 1)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn deleting_series_cascades_its_seasons_and_episodes() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_series(&pool, 1).await?;
        seed_season(&pool, 40, 1, 1).await?;
        seed_episode(&pool, 60, 40, 1, 1).await?;

        // Act
        sqlx::query("DELETE FROM series WHERE series_id = 1")
            .execute(&pool)
            .await?;

        // Assert
        let seasons: i64 = sqlx::query_scalar("SELECT count(*) FROM season")
            .fetch_one(&pool)
            .await?;
        let episodes: i64 = sqlx::query_scalar("SELECT count(*) FROM episode")
            .fetch_one(&pool)
            .await?;
        assert_eq!(seasons, 0, "deleting a show must cascade its seasons");
        assert_eq!(episodes, 0, "deleting a show must cascade its episodes");
        Ok(())
    }

    /// Insert one film, show, season and album, each naming `cover_image_id`
    /// as its cover, in dependency order.
    async fn seed_covered_owners(
        pool: &SqlitePool,
        cover_image_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO movie (
                movie_id, name, plot, country_of_origin, release_date, video_id, cover_image_id
             )
             VALUES (1, 'Film', 'A plot', 'US', '1990-04-08', 1, ?1)",
        )
        .bind(cover_image_id)
        .execute(pool)
        .await?;
        sqlx::query(
            "INSERT INTO series (
                series_id, name, plot, country_of_origin, first_air_date, cover_image_id
             )
             VALUES (1, 'Show', 'A plot', 'US', '1990-04-08', ?1)",
        )
        .bind(cover_image_id)
        .execute(pool)
        .await?;
        sqlx::query(
            "INSERT INTO season (season_id, series_id, season_index, cover_image_id)
             VALUES (40, 1, 1, ?1)",
        )
        .bind(cover_image_id)
        .execute(pool)
        .await?;
        sqlx::query(
            "INSERT INTO music_album (music_album_id, name, release_date, cover_image_id)
             VALUES (1, 'Album', '1990-04-08', ?1)",
        )
        .bind(cover_image_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Add the seeded image to a private gallery owned by the seeded user.
    async fn seed_gallery_image(pool: &SqlitePool, image_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO gallery (
                gallery_id, name, created_at, last_modified_at, is_public, user_id
             )
             VALUES (2, 'gallery 2', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 0, 1)",
        )
        .execute(pool)
        .await?;
        sqlx::query(
            "INSERT INTO gallery_item (gallery_item_id, gallery_id, image_id, item_index)
             VALUES (1, 2, ?1, 0)",
        )
        .bind(image_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn library_cover_accepts_a_library_image_on_every_owner() -> Result<(), Box<dyn Error>> {
        // Arrange: image 1 starts unattached.
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;

        // Act
        seed_covered_owners(&pool, 1).await?;

        // Assert: every owner stores the image as its cover and the first
        // attachment marked it LIBRARY.
        let covers: Vec<Option<i64>> = vec![
            sqlx::query_scalar("SELECT cover_image_id FROM movie")
                .fetch_one(&pool)
                .await?,
            sqlx::query_scalar("SELECT cover_image_id FROM series")
                .fetch_one(&pool)
                .await?,
            sqlx::query_scalar("SELECT cover_image_id FROM season")
                .fetch_one(&pool)
                .await?,
            sqlx::query_scalar("SELECT cover_image_id FROM music_album")
                .fetch_one(&pool)
                .await?,
        ];
        let scope: Option<String> =
            sqlx::query_scalar("SELECT scope FROM image WHERE image_id = 1")
                .fetch_one(&pool)
                .await?;
        assert_eq!(
            covers,
            vec![Some(1); 4],
            "every library owner must store the image as its cover"
        );
        assert_eq!(scope.as_deref(), Some("LIBRARY"));
        Ok(())
    }

    #[tokio::test]
    async fn cover_rejects_a_missing_image() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;

        // Act: image 99 does not exist.
        let result = sqlx::query(
            "INSERT INTO movie (
                movie_id, name, plot, country_of_origin, release_date, video_id, cover_image_id
             )
             VALUES (1, 'Film', 'A plot', 'US', '1990-04-08', 1, 99)",
        )
        .execute(&pool)
        .await
        .map_err(RepositoryError::from);

        // Assert: the foreign key refuses the dangling reference.
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn library_cover_rejects_a_user_image() -> Result<(), Box<dyn Error>> {
        // Arrange: a gallery item owns image 1, so it is USER.
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;

        // Act
        seed_gallery_image(&pool, 1).await?;
        let result = sqlx::query(
            "INSERT INTO movie (
                movie_id, name, plot, country_of_origin, release_date, video_id, cover_image_id
             )
             VALUES (1, 'Film', 'A plot', 'US', '1990-04-08', 1, 1)",
        )
        .execute(&pool)
        .await
        .map_err(RepositoryError::from);

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "a library cover must reject a USER image as a conflict"
        );
        let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM movie")
            .fetch_one(&pool)
            .await?;
        assert_eq!(stored, 0, "a rejected cover must store nothing");
        Ok(())
    }

    #[tokio::test]
    async fn gallery_rejects_a_library_image() -> Result<(), Box<dyn Error>> {
        // Arrange: image 1 is a library cover, so it is LIBRARY.
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_covered_owners(&pool, 1).await?;

        // Act
        let result = seed_gallery_image(&pool, 1)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "gallery media must reject a LIBRARY image as a conflict"
        );
        Ok(())
    }

    #[tokio::test]
    async fn playlist_cover_rejects_a_library_image() -> Result<(), Box<dyn Error>> {
        // Arrange: image 1 is a library cover, so it is LIBRARY.
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_covered_owners(&pool, 1).await?;

        // Act
        let result = sqlx::query(
            "INSERT INTO music_playlist (music_playlist_id, name, user_id, cover_image_id)
             VALUES (1, 'Mix', 1, 1)",
        )
        .execute(&pool)
        .await
        .map_err(RepositoryError::from);

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "a playlist cover must reject a LIBRARY image as a conflict"
        );
        Ok(())
    }

    #[tokio::test]
    async fn gallery_rejects_a_library_video() -> Result<(), Box<dyn Error>> {
        // Arrange: video 1 is marked LIBRARY, which gallery media may not use.
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_gallery(&pool, 1).await?;
        sqlx::query("UPDATE video SET scope = 'LIBRARY' WHERE video_id = 1")
            .execute(&pool)
            .await?;

        // Act
        let result = sqlx::query(
            "INSERT INTO gallery_item (gallery_item_id, gallery_id, video_id, item_index)
             VALUES (1, 1, 1, 0)",
        )
        .execute(&pool)
        .await
        .map_err(RepositoryError::from);

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "gallery media must reject a LIBRARY video as a conflict"
        );
        Ok(())
    }

    #[tokio::test]
    async fn cover_update_rejects_a_user_image() -> Result<(), Box<dyn Error>> {
        // Arrange: a film without artwork, and image 1 used as gallery media.
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_gallery_image(&pool, 1).await?;
        sqlx::query(
            "INSERT INTO movie (
                movie_id, name, plot, country_of_origin, release_date, video_id
             )
             VALUES (1, 'Film', 'A plot', 'US', '1990-04-08', 1)",
        )
        .execute(&pool)
        .await?;

        // Act
        let result = sqlx::query("UPDATE movie SET cover_image_id = 1 WHERE movie_id = 1")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "an update to a USER image cover must be rejected as a conflict"
        );
        let stored: Option<i64> = sqlx::query_scalar("SELECT cover_image_id FROM movie")
            .fetch_one(&pool)
            .await?;
        assert_eq!(stored, None, "the rejected update must store nothing");
        Ok(())
    }

    #[tokio::test]
    async fn library_cover_is_shared_by_two_albums() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;

        // Act
        sqlx::query(
            "INSERT INTO music_album (music_album_id, name, release_date, cover_image_id)
             VALUES (1, 'First', '1990-04-08', 1)",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "INSERT INTO music_album (music_album_id, name, release_date, cover_image_id)
             VALUES (2, 'Second', '1990-04-08', 1)",
        )
        .execute(&pool)
        .await?;

        // Assert
        let covers: Vec<Option<i64>> =
            sqlx::query_scalar("SELECT cover_image_id FROM music_album ORDER BY music_album_id")
                .fetch_all(&pool)
                .await?;
        assert_eq!(
            covers,
            vec![Some(1), Some(1)],
            "a LIBRARY image may back several covers"
        );
        Ok(())
    }

    #[tokio::test]
    async fn second_playlist_cover_for_a_user_image_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        sqlx::query(
            "INSERT INTO music_playlist (music_playlist_id, name, user_id, cover_image_id)
             VALUES (1, 'Mix one', 1, 1)",
        )
        .execute(&pool)
        .await?;

        // Act
        let result = sqlx::query(
            "INSERT INTO music_playlist (music_playlist_id, name, user_id, cover_image_id)
             VALUES (2, 'Mix two', 1, 1)",
        )
        .execute(&pool)
        .await
        .map_err(RepositoryError::from);

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::AlreadyExist)),
            "a USER image must not back two playlist covers"
        );
        Ok(())
    }

    #[tokio::test]
    async fn playlist_and_gallery_cannot_share_a_user_image() -> Result<(), Box<dyn Error>> {
        // Arrange: a playlist cover owns image 1.
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        sqlx::query(
            "INSERT INTO music_playlist (music_playlist_id, name, user_id, cover_image_id)
             VALUES (1, 'Mix', 1, 1)",
        )
        .execute(&pool)
        .await?;

        // Act
        let result = seed_gallery_image(&pool, 1)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "a USER image must not have two referrers"
        );
        Ok(())
    }

    #[tokio::test]
    async fn asset_scope_rejects_an_unknown_value() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;

        // Act
        let result = sqlx::query("UPDATE image SET scope = 'GALLERY' WHERE image_id = 1")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn asset_scope_is_immutable_once_set() -> Result<(), Box<dyn Error>> {
        // Arrange: the first attachment marks image 1 USER.
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_gallery_image(&pool, 1).await?;

        // Act
        let result = sqlx::query("UPDATE image SET scope = 'LIBRARY' WHERE image_id = 1")
            .execute(&pool)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "an asset scope must not change once set"
        );
        let scope: Option<String> =
            sqlx::query_scalar("SELECT scope FROM image WHERE image_id = 1")
                .fetch_one(&pool)
                .await?;
        assert_eq!(scope.as_deref(), Some("USER"));
        Ok(())
    }

    #[tokio::test]
    async fn deleting_season_cascades_its_episodes() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = in_memory_pool().await?;
        seed_media(&pool).await?;
        seed_series(&pool, 1).await?;
        seed_season(&pool, 40, 1, 1).await?;
        seed_episode(&pool, 60, 40, 1, 1).await?;

        // Act
        sqlx::query("DELETE FROM season WHERE season_id = 40")
            .execute(&pool)
            .await?;

        // Assert
        let episodes: i64 = sqlx::query_scalar("SELECT count(*) FROM episode")
            .fetch_one(&pool)
            .await?;
        assert_eq!(episodes, 0, "deleting a season must cascade its episodes");
        Ok(())
    }

    /// Open a migrated on-disk database with `connections` pooled connections.
    ///
    /// The write lock is observable only across two connections and only
    /// outside `:memory:`, where each connection would get its own database.
    /// The `init_db` tests above open an on-disk database for the same reason.
    ///
    /// The unit-of-work lock intent is asserted here rather than beside the
    /// factory: `TEST-048` keeps the factory file free of test constructs, and
    /// this module already owns the datastore configuration the intent relies
    /// on.
    async fn disk_pool(
        connections: u32,
    ) -> Result<(tempfile::TempDir, SqlitePool), Box<dyn Error>> {
        let tmp = tempdir()?;
        let database = tmp.path().join("test.db");
        let pool = init_db(database.to_string_lossy().as_ref(), connections).await?;
        Ok((tmp, pool))
    }

    /// Ask a second connection for the write lock without waiting for it.
    ///
    /// Succeeds when the lock is free; the probe then releases it, so the
    /// connection returns to the pool outside any transaction.
    async fn try_write_lock(pool: &SqlitePool) -> Result<(), sqlx::Error> {
        let mut connection = pool.acquire().await?;
        sqlx::query("PRAGMA busy_timeout = 0")
            .execute(&mut *connection)
            .await?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *connection)
            .await?;
        sqlx::query("ROLLBACK").execute(&mut *connection).await?;
        Ok(())
    }

    /// The extended result code a failing statement carried, when it has one.
    fn sqlite_code(error: sqlx::Error) -> Option<String> {
        if let sqlx::Error::Database(database) = error {
            database.code().map(std::borrow::Cow::into_owned)
        } else {
            None
        }
    }

    #[tokio::test]
    async fn begin_write_holds_the_write_lock_before_the_first_statement()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let (_tmp, pool) = disk_pool(2).await?;
        let unit_of_work = SqlxUnitOfWorkFactory::new(pool.clone())
            .begin_write()
            .await?;

        // Act
        let blocked = try_write_lock(&pool).await;

        // Assert
        let code = blocked.err().and_then(sqlite_code);
        assert_eq!(
            code.as_deref(),
            Some(SQLITE_BUSY),
            "a write-intent unit of work must hold the write lock from BEGIN"
        );
        unit_of_work.rollback().await?;
        pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn begin_leaves_the_write_lock_to_the_write_intent_opener() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let (_tmp, pool) = disk_pool(2).await?;
        let unit_of_work = SqlxUnitOfWorkFactory::new(pool.clone()).begin().await?;

        // Act
        let locked = try_write_lock(&pool).await;

        // Assert
        assert!(
            locked.is_ok(),
            "a read-only unit of work must not hold the write lock"
        );
        unit_of_work.rollback().await?;
        pool.close().await;
        Ok(())
    }

    /// One unit of work that reads before it writes, then commits.
    ///
    /// This is the shape of every writing use case: the ingest scan searches the
    /// row it is about to insert, so the transaction holds a snapshot before its
    /// first write. The barrier releases the writers together so their windows
    /// overlap; it waits before `begin`, because a write-intent transaction holds
    /// the writer lock for its whole body.
    async fn read_then_write(
        factory: &SqlxUnitOfWorkFactory,
        path: String,
        barrier: Arc<Barrier>,
    ) -> Result<(), String> {
        barrier.wait().await;
        let mut unit_of_work = factory
            .begin_write()
            .await
            .map_err(|error| error.to_string())?;
        let existing = unit_of_work
            .files()
            .search(&FileFilter {
                path: Some(path.clone()),
                ..FileFilter::default()
            })
            .await
            .map_err(|error| error.to_string())?;
        if !existing.is_empty() {
            return Err(format!("{path} was registered before the round began"));
        }
        let file = File::try_new(
            0,
            path,
            1,
            false,
            "image/png".to_owned(),
            Utc::now().date_naive(),
            1024,
            Utc::now().naive_utc(),
            None,
            None,
        )
        .map_err(|error| error.to_string())?;
        unit_of_work
            .files()
            .create(file)
            .await
            .map_err(|error| error.to_string())?;
        unit_of_work
            .commit()
            .await
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    /// Every failure the joined tasks reported, in completion order.
    async fn joined_failures(tasks: &mut JoinSet<Result<(), String>>) -> Vec<String> {
        let mut failures = Vec::new();
        while let Some(joined) = tasks.join_next().await {
            match joined {
                Ok(Ok(())) => {}
                Ok(Err(message)) => failures.push(message),
                Err(error) => failures.push(error.to_string()),
            }
        }
        failures
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn concurrent_read_then_write_units_of_work_all_commit() -> Result<(), Box<dyn Error>> {
        // Arrange
        let (_tmp, pool) = disk_pool(u32::try_from(WRITERS)?).await?;
        seed_media(&pool).await?;
        let factory = Arc::new(SqlxUnitOfWorkFactory::new(pool.clone()));
        let barrier = Arc::new(Barrier::new(WRITERS));

        // Act
        let mut tasks = (0..WRITERS)
            .map(|index| {
                let task_factory = Arc::clone(&factory);
                let task_barrier = Arc::clone(&barrier);
                let path = format!("concurrent-{index}.png");
                async move { read_then_write(&task_factory, path, task_barrier).await }
            })
            .collect::<JoinSet<Result<(), String>>>();
        let failures = joined_failures(&mut tasks).await;
        let persisted: i64 =
            sqlx::query_scalar("SELECT count(*) FROM file WHERE path LIKE 'concurrent-%'")
                .fetch_one(&pool)
                .await?;

        // Assert
        assert!(
            failures.is_empty(),
            "concurrent units of work reported {} failure(s): {}",
            failures.len(),
            failures.join("; ")
        );
        assert_eq!(
            persisted,
            i64::try_from(WRITERS)?,
            "every concurrent unit of work must commit its own file"
        );
        pool.close().await;
        Ok(())
    }
}
