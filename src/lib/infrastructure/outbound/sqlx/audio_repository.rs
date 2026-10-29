use sqlx::QueryBuilder;
use sqlx::Sqlite;
use sqlx::Transaction;

use crate::domain::model::audio::Audio;
use crate::domain::port::audio_repository::AudioFilter;
use crate::domain::port::audio_repository::AudioRepository;
use crate::domain::port::error::RepositoryError;

use super::model::audio::Audio as SqlxAudio;
use super::query_filter::push_filter;

/// Repository persisting audio rows, backed by a `SQLite` transaction.
pub struct SqlxAudioRepository<'transaction> {
    /// Transaction the repository reads from and writes to.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxAudioRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }
}

impl AudioRepository for SqlxAudioRepository<'_> {
    async fn create(&mut self, audio: Audio) -> Result<Audio, RepositoryError> {
        let row = sqlx::query_as::<_, SqlxAudio>(
            "INSERT INTO audio (
                name, duration_ms, codec, sample_rate_hz, bit_depth, block_size, file_id
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            RETURNING
                audio_id,
                name,
                duration_ms,
                codec,
                sample_rate_hz,
                bit_depth,
                block_size,
                file_id",
        )
        .bind(audio.name())
        .bind(audio.duration_ms())
        .bind(audio.codec())
        .bind(audio.sample_rate_hz())
        .bind(audio.bit_depth())
        .bind(audio.block_size())
        .bind(audio.file_id())
        .fetch_one(&mut **self.transaction)
        .await?;

        domain_audio(row)
    }

    async fn search(&mut self, filter: &AudioFilter) -> Result<Vec<Audio>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT
                audio_id,
                name,
                duration_ms,
                codec,
                sample_rate_hz,
                bit_depth,
                block_size,
                file_id
            FROM audio",
        );
        let mut first = true;

        if let Some(file_id) = filter.file_id {
            push_filter(&mut first, &mut builder, "file_id = ", file_id);
        }

        let rows = builder
            .build_query_as::<SqlxAudio>()
            .fetch_all(&mut **self.transaction)
            .await?;

        rows.into_iter().map(domain_audio).collect()
    }
}

/// Map a persisted audio row back to the domain model.
fn domain_audio(row: SqlxAudio) -> Result<Audio, RepositoryError> {
    Audio::try_new(
        row.audio_id,
        row.name,
        row.duration_ms,
        row.codec,
        row.sample_rate_hz,
        row.bit_depth,
        row.block_size,
        row.file_id,
    )
    .map_err(|_| RepositoryError::DataIntegrityViolation)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use rstest::rstest;
    use sqlx::Sqlite;
    use sqlx::Transaction;
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::domain::model::audio::Audio;
    use crate::domain::port::audio_repository::AudioFilter;
    use crate::domain::port::audio_repository::AudioRepository as _;
    use crate::domain::port::error::RepositoryError;

    use super::SqlxAudioRepository;

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
    async fn seed_user(
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

    /// Seed a file owned by `user_id`.
    async fn seed_file(
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

    /// Build a domain audio row.
    fn audio(
        file_id: i64,
        bit_depth: Option<i64>,
        block_size: Option<i64>,
    ) -> Result<Audio, Box<dyn Error>> {
        Ok(Audio::try_new(
            0,
            "track.mp3".to_owned(),
            1_000.0f64,
            "MP3".to_owned(),
            44_100.0f64,
            bit_depth,
            block_size,
            file_id,
        )?)
    }

    #[tokio::test]
    async fn create_assigns_identifier_and_round_trips() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "audio/mpeg").await?;
        let mut repository = SqlxAudioRepository::new(&mut transaction);

        // Act
        let persisted = repository.create(audio(10, Some(16), Some(1152))?).await?;
        let found = repository
            .search(&AudioFilter {
                file_id: Some(10),
                ..AudioFilter::default()
            })
            .await?;

        // Assert
        assert_ne!(
            persisted.id(),
            0,
            "a new audio must receive a real identifier"
        );
        assert_eq!(persisted.bit_depth(), Some(16));
        assert_eq!(persisted.block_size(), Some(1152));
        assert_eq!(found, vec![persisted]);
        Ok(())
    }

    #[tokio::test]
    async fn create_and_read_back_absent_measurements() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "audio/mpeg").await?;
        let mut repository = SqlxAudioRepository::new(&mut transaction);

        // Act
        let persisted = repository.create(audio(10, None, None)?).await?;

        // Assert: an unknown measurement is absent rather than fabricated.
        assert!(persisted.bit_depth().is_none());
        assert!(persisted.block_size().is_none());
        Ok(())
    }

    #[tokio::test]
    async fn create_duplicate_file_returns_already_exist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "audio/mpeg").await?;
        let mut repository = SqlxAudioRepository::new(&mut transaction);
        repository.create(audio(10, None, None)?).await?;

        // Act
        let result = repository.create(audio(10, None, None)?).await;

        // Assert: uq_audio_file_id, one audio row per file.
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn create_missing_file_returns_data_integrity_violation() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;

        // Act
        let mut repository = SqlxAudioRepository::new(&mut transaction);
        let result = repository.create(audio(999, None, None)?).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[rstest]
    #[case::bit_depth_not_positive(
        "INSERT INTO audio (audio_id, name, duration_ms, codec, sample_rate_hz, bit_depth, file_id)
         VALUES (1, 'a', 1.0, 'mp3', 44100.0, 0, 10)"
    )]
    #[case::block_size_not_positive(
        "INSERT INTO audio (audio_id, name, duration_ms, codec, sample_rate_hz, block_size, file_id)
         VALUES (1, 'a', 1.0, 'mp3', 44100.0, 0, 10)"
    )]
    #[tokio::test]
    async fn raw_insert_violating_a_check_returns_data_integrity_violation(
        #[case] statement: &'static str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "audio/mpeg").await?;

        // Act
        let result = sqlx::query(statement)
            .execute(&mut *transaction)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::DataIntegrityViolation)),
            "statement must violate a check constraint: {statement}"
        );
        Ok(())
    }
}
