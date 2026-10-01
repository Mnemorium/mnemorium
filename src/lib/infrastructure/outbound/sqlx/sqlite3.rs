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
    let options = SqliteConnectOptions::new()
        .filename(db_file)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(BUSY_TIMEOUT_SECONDS))
        .pragma("journal_size_limit", WAL_SIZE_LIMIT_BYTES);

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

    use super::init_db;
    use crate::domain::model::sqlite3::Sqlite3;

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

        pool.close().await;
        Ok(())
    }
}
