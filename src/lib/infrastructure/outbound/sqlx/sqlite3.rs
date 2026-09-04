use std::fs::File;
use std::path::Path;

use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use tracing::warn;

/// Initializes the `SQLite` database connection pool, creating the database file
/// when it does not exist.
///
/// # Errors
///
/// Returns an error when the database file cannot be created, when the
/// connection to the database cannot be established, or when applying
/// migrations fails.
pub async fn init_db(db_file: &str, max_conn: u32) -> Result<SqlitePool, sqlx::Error> {
    if !Path::new(db_file).exists() {
        File::create(db_file)?;
        warn!("sqlite database file {db_file} did not exist; created it");
    }

    let conn_str = format!("sqlite:{db_file}");

    let pool = SqlitePoolOptions::new()
        .max_connections(max_conn)
        .connect(&conn_str)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use tempfile::tempdir;

    use super::init_db;

    #[tokio::test]
    async fn init_db_missing_file_creates_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let db_path = tmp.path().join("test.db");
        let db_file = db_path.to_string_lossy();

        // Act
        let pool = init_db(&db_file, 1).await?;

        // Assert
        assert!(
            db_path.exists(),
            "database file should exist after init_db when the path was missing"
        );

        pool.close().await;
        Ok(())
    }
}
