/// The audio repository adapter.
mod audio_repository;
/// The credential repository adapter.
mod credential_repository;
mod error_mapping;
/// The file repository adapter.
mod file_repository;
/// The gallery repository adapter.
mod gallery_repository;
/// The grant repository adapter.
mod grant_repository;
/// The image repository adapter.
mod image_repository;
/// The mime type repository adapter.
mod mime_type_repository;
/// The SQL row models the repositories decode into.
pub(crate) mod model;
/// The movie repository adapter.
mod movie_repository;
/// The music album repository adapter.
mod music_album_repository;
/// The music group repository adapter.
mod music_group_repository;
/// The music playlist repository adapter.
mod music_playlist_repository;
/// The shared SQL filter and sort clause builders.
mod query_filter;
/// The resource repository adapter.
mod resource_repository;
/// The series repository adapter.
mod series_repository;
/// The Root Admin setup-grant repository adapter.
mod setup_grant_repository;
/// The `SQLite` unit of work and its factory.
mod unit_of_work;
/// The upload repository adapter.
mod upload_repository;
/// The user repository adapter.
mod user_repository;
/// The video repository adapter.
mod video_repository;

pub(crate) use self::audio_repository::SqlxAudioRepository;
pub(crate) use self::credential_repository::SqlxCredentialRepository;
pub(crate) use self::file_repository::SqlxFileRepository;
pub(crate) use self::gallery_repository::SqlxGalleryRepository;
pub(crate) use self::grant_repository::SqlxGrantRepository;
pub(crate) use self::image_repository::SqlxImageRepository;
pub(crate) use self::mime_type_repository::SqlxMimeTypeRepository;
pub(crate) use self::movie_repository::SqlxMovieRepository;
pub(crate) use self::music_album_repository::SqlxMusicAlbumRepository;
pub(crate) use self::music_group_repository::SqlxMusicGroupRepository;
pub(crate) use self::music_playlist_repository::SqlxMusicPlaylistRepository;
pub(crate) use self::resource_repository::SqlxResourceRepository;
pub(crate) use self::series_repository::SqlxSeriesRepository;
pub(crate) use self::setup_grant_repository::SqlxSetupGrantRepository;
pub(crate) use self::unit_of_work::SqlxUnitOfWorkFactory;
pub(crate) use self::upload_repository::SqlxUploadRepository;
pub(crate) use self::user_repository::SqlxUserRepository;
pub(crate) use self::video_repository::SqlxVideoRepository;

use std::fs::File;
use std::path::Path;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
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
pub(crate) async fn init_db(
    db_file: &str,
    max_connections: u32,
) -> Result<SqlitePool, sqlx::Error> {
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

/// Fixtures the `SQLite` adapter tests share (TEST-044).
#[cfg(test)]
mod test_helpers {
    use std::error::Error;

    use crate::domain::NumericID;
    use crate::domain::model::{File, ResourceKind};
    use crate::domain::port::FileFilter;
    use crate::domain::port::{
        AssetUnitOfWork as _, FileRepository as _, RepositoryError, UnitOfWork as _,
        UnitOfWorkFactory as _,
    };
    use crate::infrastructure::outbound::sqlx::{SqlxUnitOfWorkFactory, init_db};
    use crate::test_helpers::DIGEST;
    use chrono::{NaiveDateTime, Utc};
    use rstest::fixture;
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::{Sqlite, SqlitePool, Transaction};
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::sync::Barrier;
    use tokio::task::JoinSet;

    /// A second digest, distinct from [`DIGEST`], for a second seeded file.
    pub(crate) const DIGEST_ALT: &str =
        "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

    /// Open a migrated in-memory database — one `:memory:` connection per call so
    /// each test runs against a fresh datastore (TEST-033), capped at a single
    /// connection (TEST-034) — and return the transaction the repository under
    /// test operates on.
    ///
    /// # Errors
    ///
    /// Returns an error when the in-memory database cannot be opened or when
    /// applying the migrations fails.
    pub(crate) async fn begin_transaction() -> Result<Transaction<'static, Sqlite>, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        pool.begin().await
    }

    pub(crate) async fn email_is_accepted(email: &str) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_credential_v2(&mut transaction, 1).await?;

        // Act
        let result = sqlx::query(
            "INSERT INTO user (user_id, credential_id, email, role, username)
             VALUES (1, 1, ?1, 'STANDARD', 'erin')",
        )
        .bind(email)
        .execute(&mut *transaction)
        .await;

        // Assert
        assert!(result.is_ok(), "{email:?} was rejected: {result:?}");
        Ok(())
    }

    pub(crate) async fn email_is_rejected(email: &str) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_credential_v2(&mut transaction, 1).await?;

        // Act
        let result = sqlx::query(
            "INSERT INTO user (user_id, credential_id, email, role, username)
             VALUES (1, 1, ?1, 'STANDARD', 'erin')",
        )
        .bind(email)
        .execute(&mut *transaction)
        .await;

        // Assert
        let mapped = result.map_err(RepositoryError::from);
        assert!(
            matches!(mapped, Err(RepositoryError::DataIntegrityViolation)),
            "{email:?} was stored"
        );
        Ok(())
    }

    pub(crate) async fn seed_album(
        transaction: &mut Transaction<'static, Sqlite>,
        album_id: i64,
        name: &str,
        production_type: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO music_album (music_album_id, name, release_date, production_type, music_group_id)
             VALUES (?1, ?2, '1959-08-17', ?3, 0)",
        )
        .bind(album_id)
        .bind(name)
        .bind(production_type)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_album_genre(
        transaction: &mut Transaction<'static, Sqlite>,
        album_id: i64,
        genre_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO music_album_genre (music_album_id, genre_id) VALUES (?1, ?2)")
            .bind(album_id)
            .bind(genre_id)
            .execute(&mut **transaction)
            .await?;
        Ok(())
    }

    pub(crate) async fn seed_album_placement(
        transaction: &mut Transaction<'static, Sqlite>,
        recording_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO music_album (music_album_id, name, release_date, production_type, music_group_id)
             VALUES (1, 'Kind of Blue', '1959-08-17', NULL, 0)",
        )
        .execute(&mut **transaction)
        .await?;
        sqlx::query(
            "INSERT INTO music_medium (music_medium_id, music_album_id, type, medium_index)
             VALUES (40, 1, 'CD', 1)",
        )
        .execute(&mut **transaction)
        .await?;
        sqlx::query(
            "INSERT INTO music_track (track_index, music_playlist_id, music_medium_id, music_recording_id)
             VALUES (1, NULL, 40, ?1)",
        )
        .bind(recording_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_audio(
        transaction: &mut Transaction<'static, Sqlite>,
        audio_id: i64,
        file_id: i64,
        name: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO audio (
                audio_id, name, duration_ms, codec, sample_rate_hz,
                bit_depth, block_size, file_id
             )
             VALUES (?1, ?2, 1000.0, 'MP3', 44100.0, 16, 16, ?3)",
        )
        .bind(audio_id)
        .bind(name)
        .bind(file_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_covered_group(
        transaction: &mut Transaction<'static, Sqlite>,
        cover_image_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO music_group (stage_name, cover_image_id) VALUES ('The Band', ?1)")
            .bind(cover_image_id)
            .execute(&mut **transaction)
            .await?;
        Ok(())
    }

    pub(crate) async fn seed_covered_owners(
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
            "INSERT INTO music_album (music_album_id, name, release_date, cover_image_id, music_group_id)
             VALUES (1, 'Album', '1990-04-08', ?1, 0)",
        )
        .bind(cover_image_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_credential(
        transaction: &mut Transaction<'static, Sqlite>,
        id: i64,
        password_hash: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (?1, ?2)")
            .bind(id)
            .bind(password_hash)
            .execute(&mut **transaction)
            .await?;
        Ok(())
    }

    pub(crate) async fn seed_credential_v2(
        transaction: &mut Transaction<'static, Sqlite>,
        id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (?1, ?2)")
            .bind(id)
            .bind(format!("hash-{id}"))
            .execute(&mut **transaction)
            .await?;
        Ok(())
    }

    pub(crate) async fn seed_episode(
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

    pub(crate) async fn seed_episode_backed_by(
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

    pub(crate) async fn seed_episode_v2(
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

    pub(crate) async fn seed_file(
        transaction: &mut Transaction<'static, Sqlite>,
        file_id: i64,
        user_id: i64,
        path: &str,
        mime_type_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, size_bytes, modified_at, origin, integrity_hash)
             VALUES (?1, ?2, ?3, 0, ?4, 1024, '2026-10-06 00:00:00', 'UPLOADED', ?5)",
        )
        .bind(file_id)
        .bind(path)
        .bind(user_id)
        .bind(mime_type_id)
        .bind(format!("{file_id:064x}"))
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_file_v2(
        transaction: &mut Transaction<'static, Sqlite>,
        file_id: i64,
        user_id: i64,
        path: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, size_bytes, modified_at, origin, integrity_hash)
             VALUES (?1, ?2, ?3, 0, 'audio/mpeg', 1024, '2026-10-06 00:00:00', 'UPLOADED', ?4)",
        )
        .bind(file_id)
        .bind(path)
        .bind(user_id)
        .bind(format!("{file_id:064x}"))
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_file_v3(
        transaction: &mut Transaction<'static, Sqlite>,
        file_id: i64,
        path: &str,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, size_bytes, modified_at)
             VALUES (?1, ?2, ?3, 0, 'video/mp4', 10, '2026-09-26 10:00:00')",
        )
        .bind(file_id)
        .bind(path)
        .bind(user_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_file_v4(
        transaction: &mut Transaction<'static, Sqlite>,
        file_id: i64,
        user_id: i64,
        mime_type_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO file (
                file_id, path, user_id, is_public, mime_type_id, size_bytes, modified_at,
                integrity_hash
             )
             VALUES (?1, ?2, ?3, 0, ?4, 1024, '2026-10-06 00:00:00', ?5)",
        )
        .bind(file_id)
        .bind(format!("{file_id}.mp3"))
        .bind(user_id)
        .bind(mime_type_id)
        .bind(format!("{file_id:064x}"))
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_gallery(
        pool: &SqlitePool,
        gallery_id: i64,
    ) -> Result<(), Box<dyn Error>> {
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

    pub(crate) async fn seed_gallery_image(
        pool: &SqlitePool,
        image_id: i64,
    ) -> Result<(), sqlx::Error> {
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

    pub(crate) async fn seed_gallery_media(
        transaction: &mut Transaction<'static, Sqlite>,
        gallery_id: i64,
        gallery_item_id: i64,
        image_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO gallery (gallery_id, name, created_at, last_modified_at, is_public, user_id)
             VALUES (?1, 'Holiday', '2026-10-06 00:00:00', '2026-10-06 00:00:00', 1, 1)",
        )
        .bind(gallery_id)
        .execute(&mut **transaction)
        .await?;
        sqlx::query(
            "INSERT INTO gallery_item (gallery_item_id, gallery_id, image_id, item_index)
             VALUES (?1, ?2, ?3, 0)",
        )
        .bind(gallery_item_id)
        .bind(gallery_id)
        .bind(image_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_gallery_v2(
        transaction: &mut Transaction<'static, Sqlite>,
        gallery_id: i64,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO gallery (gallery_id, name, created_at, last_modified_at, is_public, user_id)
             VALUES (?1, ?2, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 0, ?3)",
        )
        .bind(gallery_id)
        .bind(format!("gallery {gallery_id}"))
        .bind(user_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_genre(
        transaction: &mut Transaction<'static, Sqlite>,
        genre_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO genre (genre_id) VALUES (?1)")
            .bind(genre_id)
            .execute(&mut **transaction)
            .await?;
        Ok(())
    }

    pub(crate) async fn seed_image(
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

    pub(crate) async fn seed_image_file(
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

    pub(crate) async fn seed_image_v2(
        transaction: &mut Transaction<'static, Sqlite>,
        image_id: i64,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, size_bytes, modified_at, origin, integrity_hash)
             VALUES (?1, ?2, ?3, 0, 'image/png', 1024, '2026-10-06 00:00:00', 'UPLOADED', ?4)",
        )
        .bind(image_id)
        .bind(format!("{image_id}.png"))
        .bind(user_id)
        .bind(format!("{image_id:064x}"))
        .execute(&mut **transaction)
        .await?;
        sqlx::query(
            "INSERT INTO image (image_id, name, width_px, height_px, orientation, created_at, file_id)
             VALUES (?1, ?2, 10, 10, 'SQUARE', '2026-10-06', ?1)",
        )
        .bind(image_id)
        .bind(format!("{image_id}.png"))
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_image_v3(
        transaction: &mut Transaction<'static, Sqlite>,
        image_id: i64,
        file_id: i64,
        scope: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO image (
                image_id, name, width_px, height_px, orientation, created_at, scope, file_id
             )
             VALUES (?1, ?2, 10, 10, 'SQUARE', '2026-10-06', ?3, ?4)",
        )
        .bind(image_id)
        .bind(format!("cover-{image_id}.png"))
        .bind(scope)
        .bind(file_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_image_v4(
        transaction: &mut Transaction<'static, Sqlite>,
        image_id: i64,
        file_id: i64,
        name: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO image (image_id, name, width_px, height_px, orientation, created_at, file_id)
             VALUES (?1, ?2, 10, 10, 'SQUARE', '2026-10-06', ?3)",
        )
        .bind(image_id)
        .bind(name)
        .bind(file_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }
    pub(crate) async fn seed_media(pool: &SqlitePool) -> Result<(), Box<dyn Error>> {
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

    pub(crate) async fn seed_medium(
        transaction: &mut Transaction<'static, Sqlite>,
        medium_id: i64,
        album_id: i64,
        medium_index: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO music_medium (music_medium_id, music_album_id, type, medium_index)
             VALUES (?1, ?2, 'CD', ?3)",
        )
        .bind(medium_id)
        .bind(album_id)
        .bind(medium_index)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_movie(
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

    pub(crate) async fn seed_movie_genre(
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

    pub(crate) async fn seed_one_recording(
        transaction: &mut Transaction<'static, Sqlite>,
        recording_id: i64,
        name: &str,
    ) -> Result<(), sqlx::Error> {
        seed_file_v2(transaction, recording_id, 1, &format!("{name}.mp3")).await?;
        seed_audio(transaction, recording_id, recording_id, name).await?;
        seed_recording(transaction, recording_id, Some(name), recording_id).await
    }

    pub(crate) async fn seed_placement(
        transaction: &mut Transaction<'static, Sqlite>,
        playlist_id: i64,
        recording_id: i64,
        track_index: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO music_track (track_index, music_playlist_id, music_medium_id, music_recording_id)
             VALUES (?1, ?2, NULL, ?3)",
        )
        .bind(track_index)
        .bind(playlist_id)
        .bind(recording_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_playlist(
        transaction: &mut Transaction<'static, Sqlite>,
        playlist_id: i64,
        name: &str,
        user_id: i64,
        is_public: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO music_playlist (
                music_playlist_id, name, user_id, is_public
             )
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(playlist_id)
        .bind(name)
        .bind(user_id)
        .bind(is_public)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_playlist_v2(
        transaction: &mut Transaction<'static, Sqlite>,
        playlist_id: i64,
        name: &str,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO music_playlist (music_playlist_id, name, user_id) VALUES (?1, ?2, ?3)",
        )
        .bind(playlist_id)
        .bind(name)
        .bind(user_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_recording(
        transaction: &mut Transaction<'static, Sqlite>,
        recording_id: i64,
        name: Option<&str>,
        audio_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO music_recording (
                music_recording_id, name, isrc_code, audio_id
             )
             VALUES (?1, ?2, NULL, ?3)",
        )
        .bind(recording_id)
        .bind(name)
        .bind(audio_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_resource(
        transaction: &mut Transaction<'static, Sqlite>,
        entity_id: NumericID,
    ) -> Result<NumericID, sqlx::Error> {
        let (resource_id,) = sqlx::query_as::<_, (NumericID,)>(
            "INSERT INTO resource (kind, entity_id, owner_id, visibility)
             VALUES (?1, ?2, NULL, 'PRIVATE')
             RETURNING resource_id",
        )
        .bind(ResourceKind::Gallery.as_wire())
        .bind(entity_id)
        .fetch_one(&mut **transaction)
        .await?;
        Ok(resource_id)
    }

    pub(crate) async fn seed_season(
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

    pub(crate) async fn seed_season_v2(
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

    pub(crate) async fn seed_setup_grant(
        transaction: &mut Transaction<'static, Sqlite>,
        nonce: &str,
        expires_at: NaiveDateTime,
        consumed_at: Option<NaiveDateTime>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO root_admin_setup (nonce, expires_at, consumed_at)
             VALUES (?1, ?2, ?3)",
        )
        .bind(nonce)
        .bind(expires_at)
        .bind(consumed_at)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_series(pool: &SqlitePool, series_id: i64) -> Result<(), sqlx::Error> {
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

    pub(crate) async fn seed_series_genre(
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

    pub(crate) async fn seed_series_v2(
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

    pub(crate) async fn seed_track(
        transaction: &mut Transaction<'static, Sqlite>,
        track_id: i64,
        medium_id: i64,
        recording_id: i64,
        track_index: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO music_track (
                music_track_id, track_index, music_playlist_id, music_medium_id, music_recording_id
             )
             VALUES (?1, ?2, NULL, ?3, ?4)",
        )
        .bind(track_id)
        .bind(track_index)
        .bind(medium_id)
        .bind(recording_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_user(
        transaction: &mut Transaction<'static, Sqlite>,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (?1, ?2)")
            .bind(user_id)
            .bind(format!("hash-{user_id}"))
            .execute(&mut **transaction)
            .await?;
        sqlx::query(
            "INSERT INTO user (user_id, role, username, email, credential_id)
             VALUES (?1, 'STANDARD', ?2, NULL, ?1)",
        )
        .bind(user_id)
        .bind(format!("user{user_id}"))
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_user_v2(
        transaction: &mut Transaction<'static, Sqlite>,
    ) -> Result<(), sqlx::Error> {
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

    pub(crate) async fn seed_user_v3(
        transaction: &mut Transaction<'static, Sqlite>,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (?1, ?2)")
            .bind(user_id)
            .bind(format!("hash-{user_id}"))
            .execute(&mut **transaction)
            .await?;
        sqlx::query(
            "INSERT INTO user (user_id, role, username, email, credential_id)
             VALUES (?1, 'STANDARD', ?2, NULL, ?1)",
        )
        .bind(user_id)
        .bind(format!("user{user_id}"))
        .execute(&mut **transaction)
        .await?;
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, size_bytes, modified_at, origin, integrity_hash)
             VALUES (?1, ?2, ?3, 0, 'image/png', 1024, '2026-10-06 00:00:00', 'UPLOADED', ?4)",
        )
        .bind(user_id)
        .bind(format!("cover-{user_id}.png"))
        .bind(user_id)
        .bind(format!("{user_id:064x}"))
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(crate) async fn seed_video(
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

    pub(crate) async fn seed_video_file(
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
    pub(crate) async fn seed_video_v3(
        transaction: &mut Transaction<'static, Sqlite>,
        video_id: i64,
        file_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO video (
                video_id, duration_ms, codec, frame_count, width, height, color_id, scan_type, file_id
             )
             VALUES (?1, 1000.0, 'H264', 30, 10, 10, 'YCbCr', 'PROGRESSIVE', ?2)",
        )
        .bind(video_id)
        .bind(file_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Open a migrated in-memory database, one connection per call, so each
    /// test runs against a fresh database (TEST-033). The two pragmas mirror
    /// [`init_db`]: foreign-key enforcement backs the cascade tests and
    /// recursive triggers make the implicit delete of an `INSERT OR REPLACE`
    /// fire the default-gallery guard.
    #[fixture]
    pub(crate) async fn in_memory_pool() -> Result<SqlitePool, Box<dyn Error>> {
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

    /// Open a migrated on-disk database with `connections` pooled connections.
    ///
    /// The write lock is observable only across two connections and only
    /// outside `:memory:`, where each connection would get its own database.
    /// The `init_db` tests above open an on-disk database for the same reason.
    ///
    /// The unit-of-work lock intent is asserted here rather than beside the
    /// factory: `TEST-046` keeps the factory file free of test constructs, and
    /// this module already owns the datastore configuration the intent relies
    /// on.
    pub(crate) async fn disk_pool(
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
    pub(crate) async fn try_write_lock(pool: &SqlitePool) -> Result<(), sqlx::Error> {
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
    pub(crate) fn sqlite_code(error: sqlx::Error) -> Option<String> {
        if let sqlx::Error::Database(database) = error {
            database.code().map(std::borrow::Cow::into_owned)
        } else {
            None
        }
    }

    /// One unit of work that reads before it writes, then commits.
    ///
    /// This is the shape of every writing use case: the ingest scan searches the
    /// row it is about to insert, so the transaction holds a snapshot before its
    /// first write. The barrier releases the writers together so their windows
    /// overlap; it waits before `begin`, because a write-intent transaction holds
    /// the writer lock for its whole body.
    pub(crate) async fn read_then_write(
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
    pub(crate) async fn joined_failures(tasks: &mut JoinSet<Result<(), String>>) -> Vec<String> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::error::Error;

    use tempfile::tempdir;

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
}
