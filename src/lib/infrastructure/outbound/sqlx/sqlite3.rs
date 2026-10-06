use std::fs::File;
use std::path::Path;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqliteJournalMode;
use sqlx::sqlite::SqlitePoolOptions;
use tracing::warn;

use crate::domain::model::sqlite3::Sqlite3;

/// Time a connection waits for a lock held by another connection, in seconds.
const BUSY_TIMEOUT_SECONDS: u64 = 5;
/// Upper bound of the write-ahead log file, in bytes (64 MiB).
const WAL_SIZE_LIMIT_BYTES: &str = "67108864";

/// Initializes the `SQLite` database connection pool, creating the database file
/// when it does not exist.
///
/// # Errors
///
/// Returns an error when the database file cannot be created, when the
/// connection to the database cannot be established, or when applying
/// migrations fails.
pub async fn init_db(settings: &Sqlite3) -> Result<SqlitePool, sqlx::Error> {
    let db_file = settings.path();

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
        .busy_timeout(Duration::from_secs(BUSY_TIMEOUT_SECONDS))
        .pragma("journal_size_limit", WAL_SIZE_LIMIT_BYTES)
        .foreign_keys(true)
        .pragma("recursive_triggers", "ON");

    let pool = SqlitePoolOptions::new()
        .max_connections(settings.max_connections())
        .connect_with(options)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use tempfile::tempdir;

    use sqlx::SqlitePool;

    use super::init_db;
    use crate::domain::model::sqlite3::Sqlite3;
    use crate::domain::port::error::RepositoryError;

    /// A 64-character hexadecimal digest accepted by `chk_file_integrity_hash`.
    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    /// A second digest, distinct from [`DIGEST`], for the second seeded file.
    const DIGEST_ALT: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

    #[tokio::test]
    async fn init_db_missing_file_creates_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let db_path = tmp.path().join("test.db");
        let settings = Sqlite3::try_new(db_path.to_string_lossy().into_owned(), 1)?;

        // Act
        let pool = init_db(&settings).await?;

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
        let settings = Sqlite3::try_new(db_path.to_string_lossy().into_owned(), 3)?;

        // Act
        let pool = init_db(&settings).await?;

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

    /// Build a migrated on-disk database through [`init_db`] so the tests
    /// exercise the same connection options the server uses.
    async fn migrated_pool() -> Result<(tempfile::TempDir, SqlitePool), Box<dyn Error>> {
        let tmp = tempdir()?;
        let db_path = tmp.path().join("test.db");
        let settings = Sqlite3::try_new(db_path.to_string_lossy().into_owned(), 1)?;
        let pool = init_db(&settings).await?;
        Ok((tmp, pool))
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
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, integrity_hash)
             VALUES (1, 'a.png', 1, 0, 'image/png', ?1)",
        )
        .bind(DIGEST)
        .execute(pool)
        .await?;
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, integrity_hash)
             VALUES (2, 'b.mp4', 1, 0, 'video/mp4', ?1)",
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
        let (_tmp, pool) = migrated_pool().await?;
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
        let (_tmp, pool) = migrated_pool().await?;
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
        let (_tmp, pool) = migrated_pool().await?;
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
        let (_tmp, pool) = migrated_pool().await?;
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
        let (_tmp, pool) = migrated_pool().await?;
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
        let (_tmp, pool) = migrated_pool().await?;
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
        let (_tmp, pool) = migrated_pool().await?;
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
        let (_tmp, pool) = migrated_pool().await?;
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
        let (_tmp, pool) = migrated_pool().await?;
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
        let (_tmp, pool) = migrated_pool().await?;

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
        let (_tmp, pool) = migrated_pool().await?;

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
        let (_tmp, pool) = migrated_pool().await?;

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
        let (_tmp, pool) = migrated_pool().await?;
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
        let (_tmp, pool) = migrated_pool().await?;

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
        let (_tmp, pool) = migrated_pool().await?;

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
        let (_tmp, pool) = migrated_pool().await?;

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
}
