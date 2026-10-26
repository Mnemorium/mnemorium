use std::mem::take;

use chrono::NaiveDate;
use sqlx::QueryBuilder;
use sqlx::Sqlite;
use sqlx::Transaction;

use crate::domain::alias::NumericID;
use crate::domain::model::music_playlist::MusicPlaylist;
use crate::domain::model::music_recording::MusicRecording;
use crate::domain::model::music_recording::TrackIndex;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::music_playlist_repository::MusicPlaylistFilter;
use crate::domain::port::music_playlist_repository::MusicPlaylistRepository;

use super::error_mapping::stored_data_invalid;
use super::model::music_playlist::MusicPlaylistRow;
use super::query_filter::push_filter;

/// Repository persisting music playlists and the recordings they sequence,
/// backed by a `SQLite` transaction.
pub struct SqlxMusicPlaylistRepository<'transaction> {
    /// Transaction the repository reads from and writes to.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxMusicPlaylistRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }
}

impl MusicPlaylistRepository for SqlxMusicPlaylistRepository<'_> {
    async fn add_track(
        &mut self,
        playlist_id: NumericID,
        recording_id: NumericID,
    ) -> Result<Option<MusicRecording>, RepositoryError> {
        // The placement insert validates the recording and appends at the next
        // index atomically. No row is inserted when the recording does not
        // exist, which reports the missing recording as a value.
        let inserted = sqlx::query_scalar::<_, i64>(
            "INSERT INTO music_track (track_index, music_playlist_id, music_recording_id)
             SELECT COALESCE((
                 SELECT MAX(track_index) + 1
                 FROM music_track
                 WHERE music_playlist_id = ?1
             ), 0), ?1, music_recording_id
             FROM music_recording
             WHERE music_recording_id = ?2
             RETURNING track_index",
        )
        .bind(playlist_id)
        .bind(recording_id)
        .fetch_optional(&mut **self.transaction)
        .await?;
        let Some(track_index_raw) = inserted else {
            return Ok(None);
        };
        let track_index = u32::try_from(track_index_raw)
            .map_err(|_| stored_data_invalid("track_index_out_of_range"))?;

        let row = sqlx::query_as::<_, RecordingRow>(
            "SELECT
                music_recording.music_recording_id,
                music_recording.name,
                music_recording.first_release_date,
                music_recording.isrc_code,
                music_recording.audio_id,
                file.file_id
             FROM music_recording
             JOIN audio ON audio.audio_id = music_recording.audio_id
             JOIN file ON file.file_id = audio.file_id
             WHERE music_recording.music_recording_id = ?",
        )
        .bind(recording_id)
        .fetch_optional(&mut **self.transaction)
        .await?
        .ok_or_else(|| stored_data_invalid("missing_recording"))?;

        MusicRecording::try_new(
            row.music_recording_id,
            row.name,
            row.first_release_date,
            row.isrc_code,
            row.audio_id,
            row.file_id,
            None,
            TrackIndex::PlaylistIndex(track_index),
        )
        .map_err(|_| stored_data_invalid("invalid_recording"))
        .map(Some)
    }

    async fn create(&mut self, playlist: MusicPlaylist) -> Result<MusicPlaylist, RepositoryError> {
        let row = sqlx::query_as::<_, MusicPlaylistRow>(
            "INSERT INTO music_playlist (name, created_at, last_modified_at, user_id, is_public)
             VALUES (?1, ?2, ?3, ?4, ?5)
             RETURNING
                music_playlist_id,
                name,
                created_at,
                last_modified_at,
                user_id,
                is_public,
                NULL AS music_recording_id,
                NULL AS recording_name,
                NULL AS first_release_date,
                NULL AS isrc_code,
                NULL AS audio_id,
                NULL AS file_id,
                NULL AS track_index",
        )
        .bind(playlist.name())
        .bind(playlist.created_at())
        .bind(playlist.last_modified_at())
        .bind(playlist.user_id())
        .bind(playlist.is_public())
        .fetch_one(&mut **self.transaction)
        .await?;

        Ok(domain_playlist(row, Vec::new()))
    }

    async fn delete(&mut self, id: NumericID) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM music_playlist WHERE music_playlist_id = ?")
            .bind(id)
            .execute(&mut **self.transaction)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn remove_track(
        &mut self,
        playlist_id: NumericID,
        track_index: u32,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query(
            "DELETE FROM music_track
             WHERE music_playlist_id = ?1 AND track_index = ?2",
        )
        .bind(playlist_id)
        .bind(i64::from(track_index))
        .execute(&mut **self.transaction)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn replace_tracks(
        &mut self,
        playlist_id: NumericID,
        order: &[(u32, NumericID)],
    ) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM music_track WHERE music_playlist_id = ?")
            .bind(playlist_id)
            .execute(&mut **self.transaction)
            .await?;
        for &(track_index, recording_id) in order {
            sqlx::query(
                "INSERT INTO music_track (track_index, music_playlist_id, music_recording_id)
                 VALUES (?1, ?2, ?3)",
            )
            .bind(i64::from(track_index))
            .bind(playlist_id)
            .bind(recording_id)
            .execute(&mut **self.transaction)
            .await?;
        }
        Ok(())
    }

    async fn save(&mut self, playlist: MusicPlaylist) -> Result<MusicPlaylist, RepositoryError> {
        sqlx::query(
            "INSERT INTO music_playlist (
                music_playlist_id, name, created_at, last_modified_at, user_id, is_public
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (music_playlist_id) DO UPDATE SET
                name = excluded.name,
                created_at = excluded.created_at,
                last_modified_at = excluded.last_modified_at,
                user_id = excluded.user_id,
                is_public = excluded.is_public",
        )
        .bind(playlist.id())
        .bind(playlist.name())
        .bind(playlist.created_at())
        .bind(playlist.last_modified_at())
        .bind(playlist.user_id())
        .bind(playlist.is_public())
        .execute(&mut **self.transaction)
        .await?;

        self.search(&MusicPlaylistFilter {
            id: Some(playlist.id()),
            ..MusicPlaylistFilter::default()
        })
        .await?
        .into_iter()
        .next()
        .ok_or(RepositoryError::OperationFailed)
    }

    async fn search(
        &mut self,
        filter: &MusicPlaylistFilter,
    ) -> Result<Vec<MusicPlaylist>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT
                music_playlist.music_playlist_id,
                music_playlist.name,
                music_playlist.created_at,
                music_playlist.last_modified_at,
                music_playlist.user_id,
                music_playlist.is_public,
                music_track.track_index,
                music_recording.music_recording_id,
                music_recording.name AS recording_name,
                music_recording.first_release_date,
                music_recording.isrc_code,
                music_recording.audio_id,
                file.file_id
            FROM music_playlist
            LEFT JOIN music_track
                ON music_track.music_playlist_id = music_playlist.music_playlist_id
            LEFT JOIN music_recording
                ON music_recording.music_recording_id = music_track.music_recording_id
            LEFT JOIN audio ON audio.audio_id = music_recording.audio_id
            LEFT JOIN file ON file.file_id = audio.file_id",
        );
        let mut first = true;

        if let Some(id) = filter.id {
            push_filter(
                &mut first,
                &mut builder,
                "music_playlist.music_playlist_id = ",
                id,
            );
        }
        if let Some(is_public) = filter.is_public {
            push_filter(
                &mut first,
                &mut builder,
                "music_playlist.is_public = ",
                is_public,
            );
        }
        if let Some(name) = filter.name.as_deref() {
            push_filter(&mut first, &mut builder, "music_playlist.name = ", name);
        }
        if let Some(user_id) = filter.user_id {
            push_filter(
                &mut first,
                &mut builder,
                "music_playlist.user_id = ",
                user_id,
            );
        }

        // A stable playlist order makes the caller's offset/limit pagination
        // deterministic (`ListPlaylists` pages in memory), and the track index
        // fixes each playlist's play order.
        builder.push(" ORDER BY music_playlist.music_playlist_id, music_track.track_index");

        let rows = builder
            .build_query_as::<MusicPlaylistRow>()
            .fetch_all(&mut **self.transaction)
            .await?;

        let mut playlist_ids: Vec<NumericID> = Vec::new();
        let mut tracks: Vec<Vec<MusicRecording>> = Vec::new();
        for row in &rows {
            if playlist_ids.last() != Some(&row.music_playlist_id) {
                playlist_ids.push(row.music_playlist_id);
                tracks.push(Vec::new());
            }
            let Some(recording_id) = row.music_recording_id else {
                continue;
            };
            let track_index_raw = row
                .track_index
                .ok_or_else(|| stored_data_invalid("missing_track_index"))?;
            let track_index = u32::try_from(track_index_raw)
                .map_err(|_| stored_data_invalid("track_index_out_of_range"))?;
            let recording = MusicRecording::try_new(
                recording_id,
                row.recording_name.clone(),
                row.first_release_date
                    .ok_or_else(|| stored_data_invalid("missing_first_release_date"))?,
                row.isrc_code.clone(),
                row.audio_id
                    .ok_or_else(|| stored_data_invalid("missing_audio_id"))?,
                row.file_id
                    .ok_or_else(|| stored_data_invalid("missing_file_id"))?,
                None,
                TrackIndex::PlaylistIndex(track_index),
            )
            .map_err(|_| stored_data_invalid("invalid_recording"))?;
            if let Some(playlist_tracks) = tracks.last_mut() {
                playlist_tracks.push(recording);
            }
        }

        let mut playlists = Vec::with_capacity(playlist_ids.len());
        for (index, playlist_id) in playlist_ids.into_iter().enumerate() {
            let row = rows
                .iter()
                .find(|row| row.music_playlist_id == playlist_id)
                .ok_or_else(|| stored_data_invalid("missing_playlist_row"))?;
            playlists.push(domain_playlist(
                row.clone(),
                tracks.get_mut(index).map_or_else(Vec::new, take),
            ));
        }
        Ok(playlists)
    }
}

/// One recording and the file backing its audio.
#[derive(Debug, sqlx::FromRow)]
struct RecordingRow {
    /// Identifier of the backing audio.
    audio_id: NumericID,
    /// Identifier of the backing file.
    file_id: NumericID,
    /// Date of the first release of the recording.
    first_release_date: NaiveDate,
    /// ISRC code of the recording, when set.
    isrc_code: Option<String>,
    /// Unique identifier of the recording.
    music_recording_id: NumericID,
    /// Name of the recording, when set.
    name: Option<String>,
}

/// Map a persisted playlist row and its hydrated tracks back to the domain
/// model.
fn domain_playlist(row: MusicPlaylistRow, tracks: Vec<MusicRecording>) -> MusicPlaylist {
    MusicPlaylist::new(
        row.music_playlist_id,
        row.name,
        row.created_at,
        row.last_modified_at,
        row.user_id,
        row.is_public,
        tracks,
    )
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::single_call_fn,
        reason = "named fixtures keep the tests readable"
    )]

    use std::error::Error;
    use std::io as stdio;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::PoisonError;

    use chrono::NaiveDateTime;
    use sqlx::Sqlite;
    use sqlx::Transaction;
    use sqlx::sqlite::SqlitePoolOptions;
    use tracing::subscriber::DefaultGuard;
    use tracing::subscriber::set_default;
    use tracing_subscriber::fmt;
    use tracing_subscriber::layer::SubscriberExt as _;

    use crate::domain::model::music_playlist::MusicPlaylist;
    use crate::domain::model::music_recording::MusicRecording;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::music_playlist_repository::MusicPlaylistFilter;
    use crate::domain::port::music_playlist_repository::MusicPlaylistRepository as _;

    use super::SqlxMusicPlaylistRepository;

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
    /// event with `reason`.
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

    /// Seed an audio file owned by `user_id`.
    async fn seed_file(
        transaction: &mut Transaction<'static, Sqlite>,
        file_id: i64,
        user_id: i64,
        path: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, integrity_hash)
             VALUES (?1, ?2, ?3, 0, 'audio/mpeg', ?4)",
        )
        .bind(file_id)
        .bind(path)
        .bind(user_id)
        .bind(format!("{file_id:064x}"))
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed the audio backing a recording.
    async fn seed_audio(
        transaction: &mut Transaction<'static, Sqlite>,
        audio_id: i64,
        file_id: i64,
        name: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO audio (
                audio_id, name, duration_ms, codec, sample_rate_hz, audio_channel_id,
                bit_depth, block_size, file_id
             )
             VALUES (?1, ?2, 1000.0, 'MP3', 44100.0, '2.0', 16, 16, ?3)",
        )
        .bind(audio_id)
        .bind(name)
        .bind(file_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed a recording on `audio_id`.
    async fn seed_recording(
        transaction: &mut Transaction<'static, Sqlite>,
        recording_id: i64,
        name: Option<&str>,
        audio_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO music_recording (
                music_recording_id, name, first_release_date, isrc_code, audio_id
             )
             VALUES (?1, ?2, '1959-08-17', NULL, ?3)",
        )
        .bind(recording_id)
        .bind(name)
        .bind(audio_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed a playlist owned by `user_id`.
    async fn seed_playlist(
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

    /// Seed a playlist placement directly, bypassing the repository guards.
    async fn seed_placement(
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

    /// Seed an album, one medium and one placement on it.
    async fn seed_album_placement(
        transaction: &mut Transaction<'static, Sqlite>,
        recording_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO music_album (music_album_id, name, release_date, production_type)
             VALUES (1, 'Kind of Blue', '1959-08-17', NULL)",
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

    /// Build a domain playlist with no tracks.
    fn playlist(name: &str, user_id: i64, is_public: bool) -> MusicPlaylist {
        MusicPlaylist::new(
            0,
            name.to_owned(),
            NaiveDateTime::default(),
            NaiveDateTime::default(),
            user_id,
            is_public,
            Vec::new(),
        )
    }

    /// Seed one recording backed by its own file and audio.
    async fn seed_one_recording(
        transaction: &mut Transaction<'static, Sqlite>,
        recording_id: i64,
        name: &str,
    ) -> Result<(), sqlx::Error> {
        seed_file(transaction, recording_id, 1, &format!("{name}.mp3")).await?;
        seed_audio(transaction, recording_id, recording_id, name).await?;
        seed_recording(transaction, recording_id, Some(name), recording_id).await
    }

    #[tokio::test]
    async fn create_round_trips_and_returns_empty_tracks() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxMusicPlaylistRepository::new(&mut transaction);

        // Act
        let created = repository.create(playlist("Mix", 1, false)).await?;
        let found = repository
            .search(&MusicPlaylistFilter {
                id: Some(created.id()),
                ..MusicPlaylistFilter::default()
            })
            .await?;

        // Assert
        assert_ne!(created.id(), 0, "a new playlist must receive an identifier");
        assert_eq!(created.name(), "Mix");
        assert_eq!(created.user_id(), 1);
        assert!(!created.is_public());
        assert!(created.tracks().is_empty());
        assert_eq!(found.first(), Some(&created));
        Ok(())
    }

    #[tokio::test]
    async fn create_rejects_a_duplicate_name_for_the_same_owner() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxMusicPlaylistRepository::new(&mut transaction);
        repository.create(playlist("Mix", 1, false)).await?;

        // Act
        let duplicate = repository.create(playlist("Mix", 1, true)).await;

        // Assert
        assert!(matches!(duplicate, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn create_allows_the_same_name_for_different_owners() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_user(&mut transaction, 2).await?;
        let mut repository = SqlxMusicPlaylistRepository::new(&mut transaction);

        // Act
        let first = repository.create(playlist("Mix", 1, false)).await?;
        let second = repository.create(playlist("Mix", 2, false)).await?;

        // Assert
        assert_ne!(first.id(), second.id());
        Ok(())
    }

    #[tokio::test]
    async fn search_returns_an_empty_playlist_with_no_tracks() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_playlist(&mut transaction, 7, "Mix", 1, false).await?;
        let mut repository = SqlxMusicPlaylistRepository::new(&mut transaction);

        // Act
        let playlists = repository.search(&MusicPlaylistFilter::default()).await?;

        // Assert
        let found = playlists.first().ok_or("the playlist must be found")?;
        assert_eq!(found.id(), 7);
        assert!(found.tracks().is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn search_hydrates_tracks_in_index_order() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_one_recording(&mut transaction, 30, "First").await?;
        seed_one_recording(&mut transaction, 31, "Second").await?;
        seed_one_recording(&mut transaction, 32, "Third").await?;
        seed_playlist(&mut transaction, 7, "Mix", 1, false).await?;
        seed_placement(&mut transaction, 7, 32, 2).await?;
        seed_placement(&mut transaction, 7, 30, 0).await?;
        seed_placement(&mut transaction, 7, 31, 1).await?;
        let mut repository = SqlxMusicPlaylistRepository::new(&mut transaction);

        // Act
        let playlists = repository.search(&MusicPlaylistFilter::default()).await?;

        // Assert
        let tracks = playlists
            .first()
            .map(MusicPlaylist::tracks)
            .ok_or("the playlist must be found")?;
        let names: Vec<_> = tracks.iter().filter_map(MusicRecording::name).collect();
        assert_eq!(names, vec!["First", "Second", "Third"]);
        let indexes: Vec<_> = tracks
            .iter()
            .filter_map(|track| track.index().playlist_index())
            .collect();
        assert_eq!(indexes, vec![0, 1, 2]);
        assert_eq!(tracks.first().map(MusicRecording::file_id), Some(30));
        Ok(())
    }

    #[tokio::test]
    async fn search_applies_every_filter_conjunctively() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_user(&mut transaction, 2).await?;
        seed_playlist(&mut transaction, 1, "Trip", 1, true).await?;
        seed_playlist(&mut transaction, 2, "Trip", 2, false).await?;
        seed_playlist(&mut transaction, 3, "Other", 1, true).await?;
        let mut repository = SqlxMusicPlaylistRepository::new(&mut transaction);

        // Act
        let by_name = repository
            .search(&MusicPlaylistFilter {
                name: Some("Trip".to_owned()),
                ..MusicPlaylistFilter::default()
            })
            .await?;
        let by_public = repository
            .search(&MusicPlaylistFilter {
                is_public: Some(true),
                ..MusicPlaylistFilter::default()
            })
            .await?;
        let by_owner = repository
            .search(&MusicPlaylistFilter {
                user_id: Some(2),
                ..MusicPlaylistFilter::default()
            })
            .await?;
        let conjunctive = repository
            .search(&MusicPlaylistFilter {
                name: Some("Trip".to_owned()),
                user_id: Some(1),
                ..MusicPlaylistFilter::default()
            })
            .await?;

        // Assert
        assert_eq!(by_name.len(), 2);
        assert_eq!(by_public.len(), 2);
        assert!(by_public.iter().all(MusicPlaylist::is_public));
        assert_eq!(by_owner.first().map(MusicPlaylist::id), Some(2));
        assert_eq!(conjunctive.first().map(MusicPlaylist::id), Some(1));
        Ok(())
    }

    #[tokio::test]
    async fn search_excludes_a_medium_attached_row() -> Result<(), Box<dyn Error>> {
        // Arrange: the recording is sequenced by an album medium, not a playlist.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_one_recording(&mut transaction, 30, "So What").await?;
        seed_album_placement(&mut transaction, 30).await?;
        seed_playlist(&mut transaction, 7, "Mix", 1, false).await?;
        let mut repository = SqlxMusicPlaylistRepository::new(&mut transaction);

        // Act
        let playlists = repository.search(&MusicPlaylistFilter::default()).await?;

        // Assert
        assert!(
            playlists
                .first()
                .map(MusicPlaylist::tracks)
                .is_some_and(<[MusicRecording]>::is_empty),
            "a medium-attached placement must not reach a playlist"
        );
        Ok(())
    }

    #[tokio::test]
    async fn search_rejects_a_track_index_outside_the_u32_range() -> Result<(), Box<dyn Error>> {
        // Arrange: the range check is probed by relaxing it for this seed.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_one_recording(&mut transaction, 30, "So What").await?;
        seed_playlist(&mut transaction, 7, "Mix", 1, false).await?;
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&mut *transaction)
            .await?;
        seed_placement(&mut transaction, 7, 30, 4_294_967_296).await?;
        let mut repository = SqlxMusicPlaylistRepository::new(&mut transaction);
        let (buffer, _capture) = capture_logs();

        // Act
        let result = repository.search(&MusicPlaylistFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        assert_output_validation_failed(&captured_logs(&buffer), "track_index_out_of_range");
        Ok(())
    }

    #[tokio::test]
    async fn seeding_rejects_a_playlist_track_index_above_the_u32_range()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the schema itself must reject the row, without relaxing the
        // check the repository read-side guard is probed against elsewhere.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_one_recording(&mut transaction, 30, "So What").await?;
        seed_playlist(&mut transaction, 7, "Mix", 1, false).await?;

        // Act
        let result = sqlx::query(
            "INSERT INTO music_track (track_index, music_playlist_id, music_medium_id, music_recording_id)
             VALUES (4294967296, 7, NULL, 30)",
        )
        .execute(&mut *transaction)
        .await
        .map_err(RepositoryError::from);

        // Assert: chk_music_track_playlist_track_index.
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn seeding_rejects_a_track_placed_in_both_containers() -> Result<(), Box<dyn Error>> {
        // Arrange: the recording, the album medium and the playlist all exist,
        // so only the one-container check can reject the row.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_one_recording(&mut transaction, 30, "So What").await?;
        seed_album_placement(&mut transaction, 30).await?;
        seed_playlist(&mut transaction, 7, "Mix", 1, false).await?;

        // Act
        let result = sqlx::query(
            "INSERT INTO music_track (track_index, music_playlist_id, music_medium_id, music_recording_id)
             VALUES (0, 7, 40, 30)",
        )
        .execute(&mut *transaction)
        .await
        .map_err(RepositoryError::from);

        // Assert: chk_music_track_one_container, isolated from
        // uq_music_track_medium_track_index (40, 1),
        // uq_music_track_playlist_track_index and
        // uq_music_track_playlist_recording, each of which is free here.
        assert!(
            matches!(result, Err(RepositoryError::DataIntegrityViolation)),
            "a track placed in both containers must be rejected: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn seeding_rejects_a_track_placed_in_neither_container() -> Result<(), Box<dyn Error>> {
        // Arrange: both container columns are NULL and every other constraint
        // is satisfied, so only the one-container check can reject the row.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_one_recording(&mut transaction, 30, "So What").await?;

        // Act
        let result = sqlx::query(
            "INSERT INTO music_track (track_index, music_playlist_id, music_medium_id, music_recording_id)
             VALUES (0, NULL, NULL, 30)",
        )
        .execute(&mut *transaction)
        .await
        .map_err(RepositoryError::from);

        // Assert: chk_music_track_one_container. No partial unique
        // index applies because each is restricted to rows where its container
        // column is set.
        assert!(
            matches!(result, Err(RepositoryError::DataIntegrityViolation)),
            "a track placed in no container must be rejected: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn search_rejects_a_recording_whose_stored_isrc_code_is_malformed()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_one_recording(&mut transaction, 30, "So What").await?;
        sqlx::query("UPDATE music_recording SET isrc_code = 'SHORT' WHERE music_recording_id = 30")
            .execute(&mut *transaction)
            .await?;
        seed_playlist(&mut transaction, 7, "Mix", 1, false).await?;
        seed_placement(&mut transaction, 7, 30, 0).await?;
        let mut repository = SqlxMusicPlaylistRepository::new(&mut transaction);
        let (buffer, _capture) = capture_logs();

        // Act
        let result = repository.search(&MusicPlaylistFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        assert_output_validation_failed(&captured_logs(&buffer), "invalid_recording");
        Ok(())
    }

    #[tokio::test]
    async fn add_track_appends_at_the_next_index() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_one_recording(&mut transaction, 30, "First").await?;
        seed_one_recording(&mut transaction, 31, "Second").await?;
        let mut repository = SqlxMusicPlaylistRepository::new(&mut transaction);
        let created = repository.create(playlist("Mix", 1, false)).await?;

        // Act
        let first = repository.add_track(created.id(), 30).await?;
        let second = repository.add_track(created.id(), 31).await?;

        // Assert
        assert_eq!(
            first
                .as_ref()
                .and_then(|track| track.index().playlist_index()),
            Some(0),
            "the first track must receive index zero"
        );
        assert_eq!(
            second
                .as_ref()
                .and_then(|track| track.index().playlist_index()),
            Some(1),
            "a later track must append at the next index"
        );
        assert_eq!(
            second.and_then(|track| track.name().map(str::to_owned)),
            Some("Second".to_owned())
        );
        Ok(())
    }

    #[tokio::test]
    async fn add_track_reports_an_unknown_recording_as_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxMusicPlaylistRepository::new(&mut transaction);
        let created = repository.create(playlist("Mix", 1, false)).await?;

        // Act
        let result = repository.add_track(created.id(), 999).await?;

        // Assert
        assert!(result.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn add_track_rejects_a_duplicate_recording() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_one_recording(&mut transaction, 30, "First").await?;
        let mut repository = SqlxMusicPlaylistRepository::new(&mut transaction);
        let created = repository.create(playlist("Mix", 1, false)).await?;
        repository.add_track(created.id(), 30).await?;

        // Act
        let duplicate = repository.add_track(created.id(), 30).await;

        // Assert: uq_music_track_playlist_recording rejects a second occurrence.
        assert!(matches!(duplicate, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn add_track_allows_the_same_recording_in_two_playlists() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_one_recording(&mut transaction, 30, "First").await?;
        let mut repository = SqlxMusicPlaylistRepository::new(&mut transaction);
        let first = repository.create(playlist("First", 1, false)).await?;
        let second = repository.create(playlist("Second", 1, false)).await?;
        repository.add_track(first.id(), 30).await?;

        // Act
        let result = repository.add_track(second.id(), 30).await?;

        // Assert
        assert!(result.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn remove_track_leaves_the_remaining_indices_unchanged() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_one_recording(&mut transaction, 30, "First").await?;
        seed_one_recording(&mut transaction, 31, "Second").await?;
        let mut repository = SqlxMusicPlaylistRepository::new(&mut transaction);
        let created = repository.create(playlist("Mix", 1, false)).await?;
        repository.add_track(created.id(), 30).await?;
        repository.add_track(created.id(), 31).await?;

        // Act
        let removed = repository.remove_track(created.id(), 0).await?;
        let missing = repository.remove_track(created.id(), 0).await?;
        let found = repository
            .search(&MusicPlaylistFilter {
                id: Some(created.id()),
                ..MusicPlaylistFilter::default()
            })
            .await?;

        // Assert
        assert!(removed);
        assert!(!missing, "a missing placement must report false");
        let indexes: Vec<_> = found
            .first()
            .map(MusicPlaylist::tracks)
            .unwrap_or_default()
            .iter()
            .filter_map(|track| track.index().playlist_index())
            .collect();
        assert_eq!(indexes, vec![1], "the remaining track keeps its index");
        Ok(())
    }

    #[tokio::test]
    async fn replace_tracks_reindexes_to_a_contiguous_sequence() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_one_recording(&mut transaction, 30, "First").await?;
        seed_one_recording(&mut transaction, 31, "Second").await?;
        let mut repository = SqlxMusicPlaylistRepository::new(&mut transaction);
        let created = repository.create(playlist("Mix", 1, false)).await?;
        repository.add_track(created.id(), 30).await?;
        repository.add_track(created.id(), 31).await?;

        // Act
        repository
            .replace_tracks(created.id(), &[(0, 31), (1, 30)])
            .await?;
        let found = repository
            .search(&MusicPlaylistFilter {
                id: Some(created.id()),
                ..MusicPlaylistFilter::default()
            })
            .await?;

        // Assert
        let tracks = found.first().map(MusicPlaylist::tracks).unwrap_or_default();
        let ids: Vec<_> = tracks.iter().map(MusicRecording::id).collect();
        assert_eq!(ids, vec![31, 30]);
        let indexes: Vec<_> = tracks
            .iter()
            .filter_map(|track| track.index().playlist_index())
            .collect();
        assert_eq!(indexes, vec![0, 1]);
        Ok(())
    }

    #[tokio::test]
    async fn save_persists_last_modified_at_and_metadata() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        let mut repository = SqlxMusicPlaylistRepository::new(&mut transaction);
        let mut created = repository.create(playlist("Mix", 1, false)).await?;
        let touched = NaiveDateTime::parse_from_str("2030-01-02 03:04:05", "%Y-%m-%d %H:%M:%S")?;

        // Act
        created.set_name("Renamed".to_owned());
        created.set_is_public(true);
        created.touch(touched);
        let saved = repository.save(created).await?;
        let found = repository
            .search(&MusicPlaylistFilter {
                id: Some(saved.id()),
                ..MusicPlaylistFilter::default()
            })
            .await?;

        // Assert
        assert_eq!(saved.name(), "Renamed");
        assert!(saved.is_public());
        assert_eq!(saved.last_modified_at(), touched);
        assert_eq!(
            found.first().map(MusicPlaylist::last_modified_at),
            Some(touched)
        );
        Ok(())
    }

    #[tokio::test]
    async fn delete_removes_the_placements_and_keeps_the_recording() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_one_recording(&mut transaction, 30, "First").await?;
        let mut repository = SqlxMusicPlaylistRepository::new(&mut transaction);
        let created = repository.create(playlist("Mix", 1, false)).await?;
        repository.add_track(created.id(), 30).await?;

        // Act
        let deleted = repository.delete(created.id()).await?;
        let missing = repository.delete(created.id()).await?;
        let placements: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM music_track WHERE music_playlist_id = ?")
                .bind(created.id())
                .fetch_one(&mut *transaction)
                .await?;
        let recordings: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM music_recording")
            .fetch_one(&mut *transaction)
            .await?;

        // Assert
        assert!(deleted);
        assert!(!missing, "a missing playlist must report false");
        assert_eq!(placements, 0, "deleting a playlist must cascade its tracks");
        assert_eq!(recordings, 1, "the shared recording must survive");
        Ok(())
    }
}
