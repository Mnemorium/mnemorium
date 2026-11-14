#![cfg(feature = "test-seed")]

//! Test-only seed data, compiled only with the `test-seed` feature.
//!
//! The scripts under `test/seed/sql` mirror the migration mechanism: they are
//! embedded at compile time and applied once through a second `sqlx` migrator
//! with its own history table. The demo identities and the media bytes are
//! created here because SQL cannot reproduce an Argon2 hash bound to the live
//! instance pepper, nor materialise the files a seeded medium points at.
//!
//! The fixed identifiers below are shared with the SQL scripts, which name
//! them in their header comment: files `9001`-`9007`, images `9001`-`9003`,
//! videos `9001`-`9003`, audio `9001` and the accounts `9001`-`9002`.
//!
//! The volume library the black-box suites measure is generated into
//! `test/seed/sql/0005`-`0007` by `script/generate_perf_seed.sh`. Its `file`
//! rows name paths under `perf/`; this module writes one hidden hardlink base
//! per media type under `perf/.base/` and grounds every row's path to it, so
//! the startup scan reports each file `Untouched`.

use std::{
    fs as blocking_fs, io,
    path::{Path, PathBuf},
    time::{Duration, UNIX_EPOCH},
};

use anyhow::Context as _;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use tokio::{fs, task::spawn_blocking};
use tracing::info;

use crate::domain::port::PasswordHasher as _;
use crate::infrastructure::outbound::argon2::Argon2PasswordHasher;

/// The fixed password every seeded demo account shares.
///
/// This is test data, not an operational secret: it is compiled only under the
/// `test-seed` feature and the first run prints it so an operator can sign in.
const SEED_PASSWORD: &str = "C0rrect!Horse";

/// The demo accounts.
const SEED_USERS: [SeedUser; 2] = [
    SeedUser {
        credential_id: 9001,
        email: "alice@example.com",
        role: "STANDARD",
        user_id: 9001,
        username: "alice",
    },
    SeedUser {
        credential_id: 9002,
        email: "bobby@example.com",
        role: "STANDARD",
        user_id: 9002,
        username: "bobby",
    },
];

/// The media files, one per medium the scripts create.
const SEED_FILES: [SeedFile; 7] = [
    SeedFile {
        bytes: include_bytes!("../../../test/seed/media/cover.png"),
        file_id: 9001,
        mime_type_id: "image/png",
        owner_id: 0,
        relative_path: "seed/cover.png",
    },
    SeedFile {
        bytes: include_bytes!("../../../test/seed/media/gallery.png"),
        file_id: 9002,
        mime_type_id: "image/png",
        owner_id: 0,
        relative_path: "seed/gallery.png",
    },
    SeedFile {
        bytes: include_bytes!("../../../test/seed/media/playlist.png"),
        file_id: 9003,
        mime_type_id: "image/png",
        owner_id: 0,
        relative_path: "seed/playlist.png",
    },
    SeedFile {
        bytes: include_bytes!("../../../test/seed/media/clip.mp4"),
        file_id: 9004,
        mime_type_id: "video/mp4",
        owner_id: 0,
        relative_path: "seed/clip.mp4",
    },
    SeedFile {
        bytes: include_bytes!("../../../test/seed/media/episode.mp4"),
        file_id: 9005,
        mime_type_id: "video/mp4",
        owner_id: 0,
        relative_path: "seed/episode.mp4",
    },
    SeedFile {
        bytes: include_bytes!("../../../test/seed/media/gallery.mp4"),
        file_id: 9006,
        mime_type_id: "video/mp4",
        owner_id: 0,
        relative_path: "seed/gallery.mp4",
    },
    SeedFile {
        bytes: include_bytes!("../../../test/seed/media/track.flac"),
        file_id: 9007,
        mime_type_id: "audio/flac",
        owner_id: 9001,
        relative_path: "seed/track.flac",
    },
];

/// Size of the bulk image base, in bytes.
///
/// Shared with `script/generate_perf_seed.sh`, whose `file` rows carry this
/// value. A mismatch makes the startup scan re-probe every bulk file.
const PERF_IMAGE_BYTES: u64 = 1024;
/// Size of the bulk video base, in bytes.
const PERF_VIDEO_BYTES: u64 = 2048;
/// Size of the bulk audio base, in bytes.
const PERF_AUDIO_BYTES: u64 = 4096;
/// Size of the multi-megabyte stream base the read-asset flow measures.
const PERF_STREAM_BYTES: u64 = 4_194_304;
/// Modification instant every bulk file, and its base, shares.
///
/// Whole seconds on purpose: `modified_at` is stored as text without
/// sub-second precision, so equality must not depend on it.
const PERF_MODIFIED_AT_EPOCH_S: u64 = 1_577_836_800;

/// The bulk file bases, one per media kind plus the stream asset.
///
/// The hidden directory matters: the startup walk skips entries whose name
/// starts with a dot, so a base is never registered as a library file.
const PERF_BASES: [PerfBase; 4] = [
    PerfBase {
        name: "image.png",
        size_bytes: PERF_IMAGE_BYTES,
        seed: include_bytes!("../../../test/seed/media/cover.png"),
    },
    PerfBase {
        name: "video.mp4",
        size_bytes: PERF_VIDEO_BYTES,
        seed: include_bytes!("../../../test/seed/media/clip.mp4"),
    },
    PerfBase {
        name: "audio.flac",
        size_bytes: PERF_AUDIO_BYTES,
        seed: include_bytes!("../../../test/seed/media/track.flac"),
    },
    PerfBase {
        name: "stream.mp4",
        size_bytes: PERF_STREAM_BYTES,
        seed: include_bytes!("../../../test/seed/media/audiovisual.mp4"),
    },
];

/// A hardlink base the bulk files share.
struct PerfBase {
    /// File name under `perf/.base/`.
    name: &'static str,
    /// Seed bytes repeated to fill `size_bytes`.
    seed: &'static [u8],
    /// Exact byte length the generated rows declare.
    size_bytes: u64,
}

/// A demo account the seed creates.
struct SeedUser {
    /// Identifier of the `credential` row.
    credential_id: i64,
    /// Contact address; `None` when the account has no email.
    email: &'static str,
    /// Role the account holds.
    role: &'static str,
    /// Identifier of the `user` row.
    user_id: i64,
    /// Login name.
    username: &'static str,
}

/// One media file the seed writes into the library root.
struct SeedFile {
    /// File content, embedded so the seeded image needs no source tree.
    bytes: &'static [u8],
    /// Identifier of the `file` row the scripts reference.
    file_id: i64,
    /// MIME type of the bytes.
    mime_type_id: &'static str,
    /// Identifier of the user that owns the file.
    owner_id: i64,
    /// Path relative to the library root.
    relative_path: &'static str,
}

/// Apply the complete test seed to `pool`.
///
/// The accounts and file rows are created first, because the SQL scripts refer
/// to the identifiers they own. Applying the seed twice is a no-op: every
/// insert tolerates an existing row and the scripts run once through
/// `_sqlx_seeds`.
///
/// # Errors
///
/// Returns an error when a file cannot be written, when an identity cannot be
/// hashed, or when the scripts fail. The library root is created when absent:
/// a normal build still never creates the operator's mount, but a seeded run
/// owns the directory it is about to populate.
pub(crate) async fn apply(
    pool: &SqlitePool,
    pepper: &str,
    library_root: &Path,
) -> anyhow::Result<()> {
    fs::create_dir_all(library_root).await?;

    let accounts = seed_identities(pool, pepper).await?;
    seed_files(pool, library_root).await?;
    // The hardlink bases are written before the migrator so a row can only be
    // created once its bytes are available. Grounding the visible links waits
    // for the migrator: the rows are what name them.
    prepare_perf_bases(library_root).await?;

    // The scripts mirror the migrations: embedded at compile time and applied
    // once through a migrator with its own history table.
    let mut migrator = sqlx::migrate!("./test/seed/sql");
    migrator.dangerous_set_table_name("_sqlx_seeds");
    migrator.run(pool).await?;

    // A row without its file is deleted by the startup scan, and a file
    // without its row is registered and probed. Both are paid for on every
    // `test-seed` boot, so the seed leaves neither state behind.
    seed_perf_links(pool, library_root).await?;

    info!("test seed data applied");
    if accounts > 0 {
        show_demo_accounts();
    }
    Ok(())
}

/// Create the demo accounts and their credentials, returning how many users
/// were inserted.
async fn seed_identities(pool: &SqlitePool, pepper: &str) -> anyhow::Result<u64> {
    let hasher = Argon2PasswordHasher::new(pepper.as_bytes().to_vec());
    let mut created = 0u64;

    for account in &SEED_USERS {
        let password_hash = hasher.hash_password(SEED_PASSWORD).await?;
        sqlx::query(
            "INSERT INTO credential (credential_id, password_hash)
             VALUES (?1, ?2)
             ON CONFLICT DO NOTHING",
        )
        .bind(account.credential_id)
        .bind(&password_hash)
        .execute(pool)
        .await?;

        let inserted = sqlx::query(
            "INSERT INTO user (user_id, role, username, email, credential_id)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT DO NOTHING",
        )
        .bind(account.user_id)
        .bind(account.role)
        .bind(account.username)
        .bind(account.email)
        .bind(account.credential_id)
        .execute(pool)
        .await?
        .rows_affected();

        created = created.saturating_add(inserted);

        // Every account is a resource under the instance root, and the Admin
        // role grant gives the seeded Admin owner-level reach over it
        // (`REQ-AUTHORIZATION-019`, `-035`).
        sqlx::query(
            "INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
             VALUES ('USER', ?1, NULL, 1, 'PRIVATE')
             ON CONFLICT DO NOTHING",
        )
        .bind(account.user_id)
        .execute(pool)
        .await?;
        sqlx::query(
            "INSERT INTO grant (
                 principal_kind, principal_id, principal_role, resource_id,
                 role, child_kind, effect
             )
             SELECT 'ROLE', NULL, 'ADMIN', resource_id, 'OWNER', 'ALL', 'ALLOW'
             FROM resource WHERE kind = 'USER' AND entity_id = ?1
             ON CONFLICT DO NOTHING",
        )
        .bind(account.user_id)
        .execute(pool)
        .await?;
    }

    Ok(created)
}

/// Write the media files under the library root and record their `file` rows.
async fn seed_files(pool: &SqlitePool, library_root: &Path) -> anyhow::Result<()> {
    let directory = library_root.join("seed");
    fs::create_dir_all(&directory).await?;
    let uploaded_at = Utc::now().date_naive();

    for media in &SEED_FILES {
        let absolute = library_root.join(media.relative_path);
        if !absolute.exists() {
            fs::write(&absolute, media.bytes).await?;
        }

        // The scan compares the record against the file's size and
        // modification instant, so both are read after the bytes are in place.
        let metadata = fs::metadata(&absolute).await?;
        let size_bytes = i64::try_from(metadata.len())?;
        let modified_at = DateTime::<Utc>::from(metadata.modified()?).naive_utc();

        sqlx::query(
            "INSERT INTO file (
                file_id, path, user_id, is_public, mime_type_id, uploaded_at,
                size_bytes, modified_at, origin, integrity_hash
             )
             VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6, ?7, 'SCANNED', NULL)
             ON CONFLICT DO NOTHING",
        )
        .bind(media.file_id)
        .bind(media.relative_path)
        .bind(media.owner_id)
        .bind(media.mime_type_id)
        .bind(uploaded_at)
        .bind(size_bytes)
        .bind(modified_at)
        .execute(pool)
        .await?;

        // A seeded file is a resource owned by the account it names, so the
        // read is decided by the grant on it (`REQ-AUTHORIZATION-025`).
        sqlx::query(
            "INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
             VALUES ('FILE', ?1, ?2, NULL, 'PRIVATE')
             ON CONFLICT DO NOTHING",
        )
        .bind(media.file_id)
        .bind(media.owner_id)
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Write the hardlink bases the bulk file rows point at.
///
/// Each base is filled to its declared size and stamped with the shared
/// modification instant. The base directory is hidden, so the startup walk
/// skips it and only the visible links are reconciled.
async fn prepare_perf_bases(library_root: &Path) -> anyhow::Result<()> {
    let directory: PathBuf = library_root.join("perf/.base");
    fs::create_dir_all(&directory).await?;
    let modified = UNIX_EPOCH
        .checked_add(Duration::from_secs(PERF_MODIFIED_AT_EPOCH_S))
        .context("the perf base instant is representable")?;

    for base in &PERF_BASES {
        let path = directory.join(base.name);
        let length = usize::try_from(base.size_bytes)?;
        let needs_write =
            blocking_fs::metadata(&path).map_or(true, |metadata| metadata.len() != base.size_bytes);
        if needs_write {
            let mut bytes = Vec::with_capacity(length);
            while bytes.len() < length {
                let remaining = length.saturating_sub(bytes.len());
                let take = remaining.min(base.seed.len());
                let piece = base
                    .seed
                    .get(..take)
                    .context("the base seed slice is in range")?;
                bytes.extend_from_slice(piece);
            }
            fs::write(&path, &bytes).await?;
        }

        // The write resets the instant to "now", so stamp it afterwards on
        // every run, not only when the bytes changed.
        let stamped = path.clone();
        spawn_blocking(move || -> io::Result<()> {
            let file = blocking_fs::File::options().write(true).open(&stamped)?;
            file.set_times(blocking_fs::FileTimes::new().set_modified(modified))
        })
        .await
        .map_err(|error| anyhow::anyhow!("the base stamp task failed: {error}"))??;
    }

    Ok(())
}

/// Hardlink every bulk `file` row to its base, idempotently.
///
/// The rows are the authority on which paths exist: the generator derives
/// them arithmetically, and this step only grounds the paths they name. An
/// existing link whose size or instant drifted from the base is relinked,
/// because the scan compares exactly those two values.
async fn seed_perf_links(pool: &SqlitePool, library_root: &Path) -> anyhow::Result<()> {
    let rows: Vec<(String, String, i64)> =
        sqlx::query_as("SELECT path, mime_type_id, size_bytes FROM file WHERE path LIKE 'perf/%'")
            .fetch_all(pool)
            .await?;
    if rows.is_empty() {
        return Ok(());
    }

    let root = library_root.to_path_buf();
    let modified = UNIX_EPOCH
        .checked_add(Duration::from_secs(PERF_MODIFIED_AT_EPOCH_S))
        .context("the perf base instant is representable")?;
    spawn_blocking(move || -> anyhow::Result<()> {
        let base_directory = root.join("perf/.base");
        for row in &rows {
            let relative = &row.0;
            let expected = u64::try_from(row.2)?;
            let base = match (row.1.as_str(), expected) {
                ("image/png", _) => "image.png",
                ("audio/flac", _) => "audio.flac",
                ("video/mp4", PERF_STREAM_BYTES) => "stream.mp4",
                ("video/mp4", _) => "video.mp4",
                _ => continue,
            };
            let link = root.join(relative);
            let matches = blocking_fs::metadata(&link).is_ok_and(|metadata| {
                metadata.len() == expected
                    && metadata.modified().is_ok_and(|instant| instant == modified)
            });
            if matches {
                continue;
            }
            if let Some(parent) = link.parent() {
                blocking_fs::create_dir_all(parent)?;
            }
            match blocking_fs::remove_file(&link) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            blocking_fs::hard_link(base_directory.join(base), &link)?;
            let grounded = blocking_fs::metadata(&link).is_ok_and(|metadata| {
                metadata.len() == expected
                    && metadata.modified().is_ok_and(|instant| instant == modified)
            });
            if !grounded {
                anyhow::bail!(
                    "the seeded file {} does not match its base after a hardlink",
                    link.display()
                );
            }
        }
        Ok(())
    })
    .await
    .map_err(|error| anyhow::anyhow!("the perf link task failed: {error}"))??;

    Ok(())
}

/// Print the demo credentials once, when an account was actually created.
#[expect(
    clippy::print_stdout,
    reason = "the seeded password is fixed test data, not an operational secret, and the operator needs it to sign in"
)]
fn show_demo_accounts() {
    println!(
        "Test seed applied; the demo accounts alice and bobby share the password '{SEED_PASSWORD}'"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use chrono::NaiveDateTime;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::error::Error;
    use tempfile::tempdir;

    /// Open a migrated in-memory database with the Root Admin the runtime
    /// creates before the seed runs.
    async fn migrated_pool() -> Result<SqlitePool, Box<dyn Error>> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::raw_sql("PRAGMA foreign_keys = ON; PRAGMA recursive_triggers = ON;")
            .execute(&pool)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (0, 'root')")
            .execute(&pool)
            .await?;
        sqlx::query(
            "INSERT INTO user (user_id, role, username, email, credential_id)
             VALUES (0, 'ADMIN', 'root', NULL, 0)",
        )
        .execute(&pool)
        .await?;
        Ok(pool)
    }

    #[tokio::test]
    async fn seed_creates_the_demo_library_and_is_idempotent() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = migrated_pool().await?;
        let library = tempdir()?;

        // Act
        apply(&pool, "pepper", library.path()).await?;
        let first: i64 = sqlx::query_scalar("SELECT count(*) FROM gallery")
            .fetch_one(&pool)
            .await?;
        apply(&pool, "pepper", library.path()).await?;
        let second: i64 = sqlx::query_scalar("SELECT count(*) FROM gallery")
            .fetch_one(&pool)
            .await?;

        // Assert
        assert_eq!(first, second, "applying the seed twice must insert nothing");
        assert!(library.path().join("seed/cover.png").exists());
        assert!(library.path().join("seed/track.flac").exists());
        let users: i64 = sqlx::query_scalar("SELECT count(*) FROM user WHERE user_id >= 9001")
            .fetch_one(&pool)
            .await?;
        let movie: String = sqlx::query_scalar("SELECT name FROM movie WHERE movie_id = 9001")
            .fetch_one(&pool)
            .await?;
        let album: String =
            sqlx::query_scalar("SELECT name FROM music_album WHERE music_album_id = 9001")
                .fetch_one(&pool)
                .await?;
        assert_eq!(users, 2);
        assert_eq!(movie, "Seed Movie");
        assert_eq!(album, "Seed Album");
        Ok(())
    }

    /// Every `file` row the seed leaves behind must resolve to a file whose
    /// size and modification instant the scan compares equal.
    ///
    /// This is the invariant that keeps a `test-seed` boot from deleting the
    /// bulk library: a missing path deletes the row, and a size or instant
    /// drift re-probes it on every start.
    #[tokio::test]
    async fn seeded_files_resolve_on_disk_with_matching_metadata() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = migrated_pool().await?;
        let library = tempdir()?;
        apply(&pool, "pepper", library.path()).await?;

        // Act
        let rows: Vec<(String, i64, NaiveDateTime)> =
            sqlx::query_as("SELECT path, size_bytes, modified_at FROM file")
                .fetch_all(&pool)
                .await?;
        let perf: i64 = sqlx::query_scalar("SELECT count(*) FROM file WHERE path LIKE 'perf/%'")
            .fetch_one(&pool)
            .await?;

        // Assert
        assert!(perf > 0, "the volume library must be seeded");
        assert!(!rows.is_empty());
        assert_files_match(library.path(), &rows)?;
        Ok(())
    }

    /// Assert every recorded path exists with the recorded metadata.
    fn assert_files_match(
        root: &Path,
        rows: &[(String, i64, NaiveDateTime)],
    ) -> Result<(), Box<dyn Error>> {
        for row in rows {
            let path = &row.0;
            let metadata = fs::metadata(root.join(path))?;
            let actual_size = i64::try_from(metadata.len())?;
            let actual_modified = DateTime::<Utc>::from(metadata.modified()?).naive_utc();
            assert_eq!(actual_size, row.1, "{path} changed size");
            assert_eq!(actual_modified, row.2, "{path} changed instant");
        }
        Ok(())
    }

    #[tokio::test]
    async fn seeded_accounts_authenticate_with_the_shared_password() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = migrated_pool().await?;
        let library = tempdir()?;
        apply(&pool, "pepper", library.path()).await?;

        // Act
        let hash: String =
            sqlx::query_scalar("SELECT password_hash FROM credential WHERE credential_id = 9001")
                .fetch_one(&pool)
                .await?;
        let hasher = Argon2PasswordHasher::new(b"pepper".to_vec());
        let verified = hasher.verify_password(SEED_PASSWORD, &hash).await?;

        // Assert
        assert!(verified, "the seeded demo account must accept its password");
        Ok(())
    }

    #[tokio::test]
    async fn seed_creates_a_missing_library_root() -> Result<(), Box<dyn Error>> {
        // Arrange
        let pool = migrated_pool().await?;
        let missing = tempdir()?.path().join("absent");

        // Act
        apply(&pool, "pepper", &missing).await?;

        // Assert
        assert!(missing.is_dir(), "the seed must create the library root");
        assert!(missing.join("seed/cover.png").exists());
        Ok(())
    }
}
