use std::collections::HashMap;

use sqlx::{QueryBuilder, Sqlite, Transaction};

use crate::domain::alias::NumericID;
use crate::domain::model::music_album::{MusicAlbum, ProductionType};
use crate::domain::model::music_medium::{MediumIndex, MusicMedium};
use crate::domain::model::music_recording::{MusicRecording, TrackIndex};
use crate::domain::port::error::RepositoryError;
use crate::domain::port::music_album_repository::{MusicAlbumFilter, MusicAlbumRepository};

use super::error_mapping::stored_data_invalid;
use super::model::music_album::MusicAlbumRow;
use super::query_filter::push_filter;

/// Repository querying music albums, their mediums and the recordings they
/// sequence, backed by a `SQLite` transaction.
pub(crate) struct SqlxMusicAlbumRepository<'transaction> {
    /// Transaction the repository reads from.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxMusicAlbumRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub(crate) fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }
}

/// A medium being hydrated: its surrogate identifier, domain identity and the
/// recordings read so far.
type HydratedMedium = (NumericID, MediumIndex, Vec<MusicRecording>);

impl SqlxMusicAlbumRepository<'_> {
    /// Find the stored medium at `medium_index` of `album_id`.
    async fn album_medium(
        &mut self,
        album_id: NumericID,
        medium_index: u8,
    ) -> Result<Option<MusicMedium>, RepositoryError> {
        let album = self
            .search(&MusicAlbumFilter {
                id: Some(album_id),
                ..MusicAlbumFilter::default()
            })
            .await?
            .into_iter()
            .next();
        Ok(album.and_then(|found| {
            found
                .mediums()
                .iter()
                .find(|medium| medium.index().index() == medium_index)
                .cloned()
        }))
    }

    /// Read the recording placed at `(medium_id, track_index)`.
    async fn recording_at(
        &mut self,
        medium_id: NumericID,
        track_index: u8,
        recording_id: NumericID,
    ) -> Result<MusicRecording, RepositoryError> {
        let row = sqlx::query_as::<_, RecordingRow>(
            "SELECT
                music_recording.music_recording_id,
                music_recording.name,
                music_recording.isrc_code,
                music_recording.audio_id,
                file.file_id
             FROM music_recording
             JOIN audio ON audio.audio_id = music_recording.audio_id
             JOIN file ON file.file_id = audio.file_id
             WHERE music_recording.music_recording_id = ?1
               AND music_recording.music_recording_id IN (
                   SELECT music_recording_id
                   FROM music_track
                   WHERE music_medium_id = ?2 AND track_index = ?3
               )",
        )
        .bind(recording_id)
        .bind(medium_id)
        .bind(i64::from(track_index))
        .fetch_optional(&mut **self.transaction)
        .await?
        .ok_or_else(|| stored_data_invalid("missing_recording"))?;

        MusicRecording::try_new(
            row.music_recording_id,
            row.name,
            row.isrc_code,
            row.audio_id,
            row.file_id,
            None,
            TrackIndex::MediumIndex(track_index),
        )
        .map_err(|_| stored_data_invalid("invalid_recording"))
    }

    /// Remove every genre link of `album_id` and link `genre_ids` instead.
    ///
    /// The genre vocabulary is open, so a genre the catalogue does not know yet
    /// is inserted before it is linked.
    async fn replace_genres(
        &mut self,
        album_id: NumericID,
        genre_ids: &[String],
    ) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM music_album_genre WHERE music_album_id = ?")
            .bind(album_id)
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
            sqlx::query("INSERT INTO music_album_genre (music_album_id, genre_id) VALUES (?1, ?2)")
                .bind(album_id)
                .bind(&stored_genre)
                .execute(&mut **self.transaction)
                .await?;
        }
        Ok(())
    }

    /// Insert or retype the medium at `medium.index()` of `album_id`.
    async fn upsert_medium(
        &mut self,
        album_id: NumericID,
        medium: MediumIndex,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO music_medium (music_album_id, type, medium_index)
             VALUES (?1, ?2, ?3)
             ON CONFLICT (music_album_id, medium_index) DO UPDATE SET
                type = excluded.type",
        )
        .bind(album_id)
        .bind(medium.as_wire())
        .bind(i64::from(medium.index()))
        .execute(&mut **self.transaction)
        .await?;
        Ok(())
    }
}

impl MusicAlbumRepository for SqlxMusicAlbumRepository<'_> {
    async fn create(&mut self, album: MusicAlbum) -> Result<MusicAlbum, RepositoryError> {
        let album_id = sqlx::query_scalar::<_, NumericID>(
            "INSERT INTO music_album (
                name, release_date, production_type, cover_image_id, music_group_id
             )
             VALUES (?1, ?2, ?3, ?4, ?5)
             RETURNING music_album_id",
        )
        .bind(album.name())
        .bind(album.release_date())
        .bind(album.production_type().map(ProductionType::as_wire))
        .bind(album.cover_image_id())
        .bind(album.music_group_id())
        .fetch_one(&mut **self.transaction)
        .await?;

        self.replace_genres(album_id, album.genre_ids()).await?;

        self.search(&MusicAlbumFilter {
            id: Some(album_id),
            ..MusicAlbumFilter::default()
        })
        .await?
        .into_iter()
        .next()
        .ok_or(RepositoryError::OperationFailed)
    }

    async fn has_medium_placement(
        &mut self,
        recording_id: NumericID,
    ) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT 1
             FROM music_track
             WHERE music_recording_id = ? AND music_medium_id IS NOT NULL",
        )
        .bind(recording_id)
        .fetch_optional(&mut **self.transaction)
        .await?
        .is_some())
    }

    async fn place_track(
        &mut self,
        album_id: NumericID,
        medium_index: u8,
        track_index: u8,
        recording_id: NumericID,
    ) -> Result<Option<MusicRecording>, RepositoryError> {
        let medium_id = sqlx::query_scalar::<_, NumericID>(
            "SELECT music_medium_id
             FROM music_medium
             WHERE music_album_id = ?1 AND medium_index = ?2",
        )
        .bind(album_id)
        .bind(i64::from(medium_index))
        .fetch_optional(&mut **self.transaction)
        .await?;
        let Some(destination_medium_id) = medium_id else {
            return Ok(None);
        };

        let recording_exists = sqlx::query_scalar::<_, i64>(
            "SELECT 1 FROM music_recording WHERE music_recording_id = ?",
        )
        .bind(recording_id)
        .fetch_optional(&mut **self.transaction)
        .await?
        .is_some();
        if !recording_exists {
            return Ok(None);
        }

        let occupant = sqlx::query_scalar::<_, NumericID>(
            "SELECT music_recording_id
             FROM music_track
             WHERE music_medium_id = ?1 AND track_index = ?2",
        )
        .bind(destination_medium_id)
        .bind(i64::from(track_index))
        .fetch_optional(&mut **self.transaction)
        .await?;
        if let Some(occupying_recording) = occupant {
            if occupying_recording == recording_id {
                let recording = self
                    .recording_at(destination_medium_id, track_index, recording_id)
                    .await?;
                return Ok(Some(recording));
            }
            return Err(RepositoryError::Conflict);
        }

        sqlx::query(
            "DELETE FROM music_track
             WHERE music_recording_id = ? AND music_medium_id IS NOT NULL",
        )
        .bind(recording_id)
        .execute(&mut **self.transaction)
        .await?;
        sqlx::query(
            "INSERT INTO music_track (track_index, music_medium_id, music_recording_id)
             VALUES (?1, ?2, ?3)",
        )
        .bind(i64::from(track_index))
        .bind(destination_medium_id)
        .bind(recording_id)
        .execute(&mut **self.transaction)
        .await?;

        let recording = self
            .recording_at(destination_medium_id, track_index, recording_id)
            .await?;
        Ok(Some(recording))
    }

    async fn place_tracks(
        &mut self,
        album_id: NumericID,
        placements: &[(u8, u8, NumericID)],
    ) -> Result<(), RepositoryError> {
        if placements.is_empty() {
            return Ok(());
        }

        // Removing every named recording's previous placement first lets two
        // recordings swap addresses without colliding with each other.
        let mut delete = QueryBuilder::<Sqlite>::new(
            "DELETE FROM music_track
             WHERE music_medium_id IS NOT NULL AND music_recording_id IN (",
        );
        let mut separated = delete.separated(", ");
        for &(_, _, recording_id) in placements {
            separated.push_bind(recording_id);
        }
        separated.push_unseparated(")");
        delete.build().execute(&mut **self.transaction).await?;

        for &(medium_index, track_index, recording_id) in placements {
            let medium_id = sqlx::query_scalar::<_, NumericID>(
                "SELECT music_medium_id
                 FROM music_medium
                 WHERE music_album_id = ?1 AND medium_index = ?2",
            )
            .bind(album_id)
            .bind(i64::from(medium_index))
            .fetch_optional(&mut **self.transaction)
            .await?
            .ok_or_else(|| stored_data_invalid("missing_medium"))?;
            sqlx::query(
                "INSERT INTO music_track (track_index, music_medium_id, music_recording_id)
                 VALUES (?1, ?2, ?3)",
            )
            .bind(i64::from(track_index))
            .bind(medium_id)
            .bind(recording_id)
            .execute(&mut **self.transaction)
            .await?;
        }
        Ok(())
    }

    async fn put_medium(
        &mut self,
        album_id: NumericID,
        medium: MediumIndex,
    ) -> Result<Option<MusicMedium>, RepositoryError> {
        let album_exists =
            sqlx::query_scalar::<_, i64>("SELECT 1 FROM music_album WHERE music_album_id = ?")
                .bind(album_id)
                .fetch_optional(&mut **self.transaction)
                .await?
                .is_some();
        if !album_exists {
            return Ok(None);
        }

        self.upsert_medium(album_id, medium).await?;
        self.album_medium(album_id, medium.index()).await
    }

    async fn replace_mediums(
        &mut self,
        album_id: NumericID,
        mediums: &[MediumIndex],
    ) -> Result<(), RepositoryError> {
        for medium in mediums {
            self.upsert_medium(album_id, *medium).await?;
        }

        // A stored medium outside the requested structure is removed only when it
        // sequences nothing, so an apply never discards a track placement.
        let mut delete = QueryBuilder::<Sqlite>::new(
            "DELETE FROM music_medium
             WHERE music_album_id = ",
        );
        delete.push_bind(album_id);
        if !mediums.is_empty() {
            delete.push(" AND medium_index NOT IN (");
            let mut separated = delete.separated(", ");
            for medium in mediums {
                separated.push_bind(i64::from(medium.index()));
            }
            separated.push_unseparated(")");
        }
        delete.push(
            " AND NOT EXISTS (
                SELECT 1
                FROM music_track
                WHERE music_track.music_medium_id = music_medium.music_medium_id
             )",
        );
        delete.build().execute(&mut **self.transaction).await?;
        Ok(())
    }

    async fn save_recording(
        &mut self,
        recording: MusicRecording,
    ) -> Result<Option<MusicRecording>, RepositoryError> {
        let result = sqlx::query(
            "UPDATE music_recording
             SET name = ?1, isrc_code = ?2
             WHERE music_recording_id = ?3",
        )
        .bind(recording.name())
        .bind(recording.isrc_code())
        .bind(recording.id())
        .execute(&mut **self.transaction)
        .await?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        Ok(Some(recording))
    }

    async fn search(
        &mut self,
        filter: &MusicAlbumFilter,
    ) -> Result<Vec<MusicAlbum>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT
                music_album.music_album_id,
                music_album.name,
                music_album.release_date,
                music_album.production_type,
                music_album.cover_image_id,
                music_album.music_group_id,
                COALESCE((
                    SELECT GROUP_CONCAT(genre_id, ',')
                    FROM (
                        SELECT genre_id
                        FROM music_album_genre
                        WHERE music_album_genre.music_album_id = music_album.music_album_id
                        ORDER BY genre_id
                    )
                ), '') AS genres,
                music_medium.music_medium_id,
                music_medium.medium_index,
                music_medium.type AS medium_type,
                music_recording.music_recording_id,
                music_recording.name AS recording_name,
                music_recording.isrc_code,
                music_recording.audio_id,
                file.file_id,
                music_track.track_index
            FROM music_album
            LEFT JOIN music_medium
                ON music_medium.music_album_id = music_album.music_album_id
            LEFT JOIN music_track
                ON music_track.music_medium_id = music_medium.music_medium_id
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
                "music_album.music_album_id = ",
                id,
            );
        }
        if let Some(music_group_id) = filter.music_group_id {
            push_filter(
                &mut first,
                &mut builder,
                "music_album.music_group_id = ",
                music_group_id,
            );
        }
        if let Some(name) = filter.name.as_deref() {
            push_filter(&mut first, &mut builder, "music_album.name = ", name);
        }

        // The album, its mediums and their recording occurrences make one
        // aggregate read, so the order fixes both the medium collection order and
        // each medium's play order. A stable album identifier also makes the
        // caller's offset/limit pagination deterministic (`ListAlbums` pages in
        // memory).
        builder.push(
            " ORDER BY music_album.music_album_id, music_medium.medium_index, music_track.track_index",
        );

        let rows = builder
            .build_query_as::<MusicAlbumRow>()
            .fetch_all(&mut **self.transaction)
            .await?;

        // The rows repeat an album once per track and arrive ordered by album,
        // then medium index, then track index, so one pass groups them without
        // re-sorting. The medium surrogate identifier keys the group; the domain
        // identity is the validated `MediumIndex`.
        let mut mediums: HashMap<NumericID, Vec<HydratedMedium>> = HashMap::new();
        let mut album_ids: Vec<NumericID> = Vec::new();

        for row in &rows {
            if album_ids.last() != Some(&row.music_album_id) {
                album_ids.push(row.music_album_id);
            }
            let Some(medium_id) = row.music_medium_id else {
                continue;
            };
            let medium_index_raw = row
                .medium_index
                .ok_or_else(|| stored_data_invalid("missing_medium_index"))?;
            let medium_index = u8::try_from(medium_index_raw)
                .map_err(|_| stored_data_invalid("medium_index_out_of_range"))?;
            let medium_type = row
                .medium_type
                .clone()
                .ok_or_else(|| stored_data_invalid("missing_medium_type"))?;
            let medium_identity = MediumIndex::from_wire(&medium_type, medium_index)
                .ok_or_else(|| stored_data_invalid("unknown_medium_type"))?;
            let album_mediums = mediums.entry(row.music_album_id).or_default();
            if album_mediums.last().map(|medium| medium.0) != Some(medium_id) {
                album_mediums.push((medium_id, medium_identity, Vec::new()));
            }
            let Some(recording_id) = row.music_recording_id else {
                continue;
            };
            let track_index_raw = row
                .track_index
                .ok_or_else(|| stored_data_invalid("missing_track_index"))?;
            let track_index = u8::try_from(track_index_raw)
                .map_err(|_| stored_data_invalid("track_index_out_of_range"))?;
            let recording = MusicRecording::try_new(
                recording_id,
                row.recording_name.clone(),
                row.isrc_code.clone(),
                row.audio_id
                    .ok_or_else(|| stored_data_invalid("missing_audio_id"))?,
                row.file_id
                    .ok_or_else(|| stored_data_invalid("missing_file_id"))?,
                None,
                TrackIndex::MediumIndex(track_index),
            )
            .map_err(|_| stored_data_invalid("invalid_recording"))?;
            if let Some(medium) = mediums
                .get_mut(&row.music_album_id)
                .and_then(|hydrated| hydrated.last_mut())
            {
                medium.2.push(recording);
            }
        }

        let mut albums = Vec::with_capacity(album_ids.len());
        for album_id in album_ids {
            let row = rows
                .iter()
                .find(|row| row.music_album_id == album_id)
                .ok_or_else(|| stored_data_invalid("missing_album_row"))?;
            let album_mediums = mediums
                .remove(&album_id)
                .unwrap_or_default()
                .into_iter()
                .map(|(_, index, tracks)| MusicMedium::new(index, tracks))
                .collect();
            albums.push(
                MusicAlbum::try_new(
                    album_id,
                    row.name.clone(),
                    row.release_date,
                    row.production_type
                        .as_deref()
                        .and_then(ProductionType::from_wire),
                    row.genres
                        .split(',')
                        .filter(|genre| !genre.is_empty())
                        .map(str::to_owned)
                        .collect(),
                    row.music_group_id,
                    None,
                    row.cover_image_id,
                    album_mediums,
                )
                .map_err(|_| stored_data_invalid("invalid_album"))?,
            );
        }
        Ok(albums)
    }

    async fn update(&mut self, album: MusicAlbum) -> Result<Option<MusicAlbum>, RepositoryError> {
        let result = sqlx::query(
            "UPDATE music_album
             SET name = ?1,
                 release_date = ?2,
                 production_type = ?3,
                 cover_image_id = ?4,
                 music_group_id = ?5
             WHERE music_album_id = ?6",
        )
        .bind(album.name())
        .bind(album.release_date())
        .bind(album.production_type().map(ProductionType::as_wire))
        .bind(album.cover_image_id())
        .bind(album.music_group_id())
        .bind(album.id())
        .execute(&mut **self.transaction)
        .await?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }

        self.replace_genres(album.id(), album.genre_ids()).await?;
        self.search(&MusicAlbumFilter {
            id: Some(album.id()),
            ..MusicAlbumFilter::default()
        })
        .await?
        .into_iter()
        .next()
        .map(Some)
        .ok_or(RepositoryError::OperationFailed)
    }
}

/// One recording and the file backing its audio.
#[derive(Debug, sqlx::FromRow)]
struct RecordingRow {
    /// Identifier of the backing audio.
    audio_id: NumericID,
    /// Identifier of the backing file.
    file_id: NumericID,
    /// ISRC code of the recording, when set.
    isrc_code: Option<String>,
    /// Unique identifier of the recording.
    music_recording_id: NumericID,
    /// Name of the recording, when set.
    name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
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
        if ours.iter().any(|line| line.contains("VINYL")) {
            return Err("the event must not carry the stored value".to_owned());
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

    /// Seed the audio backing a recording.
    async fn seed_audio(
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

    /// Seed a recording on `audio_id`; `name` may be absent.
    async fn seed_recording(
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

    /// Seed a music album with an optional stored production type.
    async fn seed_album(
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

    /// Attach `genre_id` to `album_id`.
    async fn seed_album_genre(
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

    /// Seed a `CD` medium of `album_id` at `medium_index`.
    async fn seed_medium(
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

    /// Seed an album track on `medium_id` at `track_index`.
    async fn seed_track(
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

    /// Seed a playlist owned by `user_id`.
    async fn seed_playlist(
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

    /// Search the albums the case seeded, excluding the seeded bucket (album 0).
    async fn search_seeded(
        repository: &mut SqlxMusicAlbumRepository<'_>,
        filter: &MusicAlbumFilter,
    ) -> Result<Vec<MusicAlbum>, RepositoryError> {
        Ok(repository
            .search(filter)
            .await?
            .into_iter()
            .filter(|album| album.id() != 0)
            .collect())
    }

    /// The medium index of a recording expected to be placed in a medium.
    fn medium_index(recording: &MusicRecording) -> Option<u8> {
        recording.index().medium_index()
    }

    #[tokio::test]
    async fn search_returns_albums_with_genres_and_their_ordered_tracks()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.mp3").await?;
        seed_file(&mut transaction, 11, 1, "b.mp3").await?;
        seed_audio(&mut transaction, 20, 10, "a.mp3").await?;
        seed_audio(&mut transaction, 21, 11, "b.mp3").await?;
        seed_recording(&mut transaction, 30, Some("So What"), 20).await?;
        seed_recording(&mut transaction, 31, Some("Blue in Green"), 21).await?;
        seed_album(&mut transaction, 1, "Kind of Blue", Some("STUDIO")).await?;
        seed_album(&mut transaction, 2, "Sketches", None).await?;
        seed_genre(&mut transaction, "JAZZ").await?;
        seed_genre(&mut transaction, "BLUES").await?;
        seed_album_genre(&mut transaction, 1, "JAZZ").await?;
        seed_album_genre(&mut transaction, 1, "BLUES").await?;
        seed_medium(&mut transaction, 40, 1, 1).await?;
        seed_track(&mut transaction, 51, 40, 31, 2).await?;
        seed_track(&mut transaction, 50, 40, 30, 1).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let albums = search_seeded(&mut repository, &MusicAlbumFilter::default()).await?;

        // Assert
        assert_eq!(albums.len(), 2, "every album must be returned");
        let first = albums.first().ok_or("the first album must be found")?;
        assert_eq!(first.id(), 1);
        assert_eq!(first.name(), "Kind of Blue");
        assert_eq!(first.production_type(), Some(ProductionType::Studio));
        assert_eq!(first.genre_ids(), ["BLUES", "JAZZ"]);
        let medium = first
            .mediums()
            .first()
            .ok_or("the album's medium must be hydrated")?;
        assert_eq!(medium.index(), MediumIndex::Cd(1));
        let titles: Vec<_> = medium
            .tracks()
            .iter()
            .filter_map(MusicRecording::name)
            .collect();
        assert_eq!(
            titles,
            vec!["So What", "Blue in Green"],
            "the medium's tracks must be in play order"
        );
        let first_track = medium
            .tracks()
            .first()
            .ok_or("the first track must be found")?;
        assert_eq!(medium_index(first_track), Some(1));
        assert_eq!(first_track.file_id(), 10);
        let second_track = medium
            .tracks()
            .get(1)
            .ok_or("the second track must be found")?;
        assert_eq!(medium_index(second_track), Some(2));
        let second = albums.get(1).ok_or("the second album must be found")?;
        assert_eq!(second.id(), 2);
        assert_eq!(second.production_type(), None);
        assert!(
            second.mediums().is_empty(),
            "an album without a medium must report no medium"
        );
        assert!(second.genre_ids().is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn search_returns_every_medium_of_an_album_in_index_order() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.mp3").await?;
        seed_file(&mut transaction, 11, 1, "b.mp3").await?;
        seed_audio(&mut transaction, 20, 10, "a.mp3").await?;
        seed_audio(&mut transaction, 21, 11, "b.mp3").await?;
        seed_recording(&mut transaction, 30, Some("First"), 20).await?;
        seed_recording(&mut transaction, 31, Some("Second"), 21).await?;
        seed_album(&mut transaction, 1, "Double Album", None).await?;
        seed_medium(&mut transaction, 40, 1, 2).await?;
        seed_medium(&mut transaction, 41, 1, 1).await?;
        seed_track(&mut transaction, 50, 40, 30, 1).await?;
        seed_track(&mut transaction, 51, 41, 31, 1).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let albums = search_seeded(&mut repository, &MusicAlbumFilter::default()).await?;

        // Assert
        let album = albums.first().ok_or("the album must be found")?;
        let indexes: Vec<_> = album
            .mediums()
            .iter()
            .map(|medium| medium.index().index())
            .collect();
        assert_eq!(indexes, vec![1, 2], "mediums must be ordered by index");
        let titles: Vec<_> = album
            .mediums()
            .iter()
            .flat_map(MusicMedium::tracks)
            .filter_map(MusicRecording::name)
            .collect();
        assert_eq!(titles, vec!["Second", "First"]);
        Ok(())
    }

    #[tokio::test]
    async fn seeding_rejects_two_medium_placements_of_one_recording() -> Result<(), Box<dyn Error>>
    {
        // Arrange: one recording placed twice in one medium.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.mp3").await?;
        seed_audio(&mut transaction, 20, 10, "a.mp3").await?;
        seed_recording(&mut transaction, 30, Some("Reprise"), 20).await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        seed_medium(&mut transaction, 40, 1, 1).await?;
        seed_track(&mut transaction, 50, 40, 30, 1).await?;

        // Act: a recording has one medium placement at most.
        let second = seed_track(&mut transaction, 51, 40, 30, 2).await;

        // Assert
        assert!(
            second.is_err(),
            "the partial unique index must refuse a second medium placement"
        );
        Ok(())
    }

    #[tokio::test]
    async fn search_filters_by_name_and_identifier() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        seed_album(&mut transaction, 2, "Sketches", None).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let by_name = repository
            .search(&MusicAlbumFilter {
                name: Some("Sketches".to_owned()),
                ..MusicAlbumFilter::default()
            })
            .await?;
        let by_id = repository
            .search(&MusicAlbumFilter {
                id: Some(1),
                ..MusicAlbumFilter::default()
            })
            .await?;

        // Assert
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name.first().map(MusicAlbum::id), Some(2));
        assert_eq!(by_id.len(), 1);
        assert_eq!(by_id.first().map(MusicAlbum::name), Some("Kind of Blue"));
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

    #[tokio::test]
    async fn search_projects_the_cover_image() -> Result<(), Box<dyn Error>> {
        // Arrange: one covered album and one without artwork.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, size_bytes, modified_at, origin, integrity_hash)
             VALUES (12, '12.png', 1, 0, 'image/png', 1024, '2026-10-06 00:00:00', 'UPLOADED', ?1)",
        )
        .bind(format!("{:064x}", 12i64))
        .execute(&mut *transaction)
        .await?;
        seed_image(&mut transaction, 12, 12).await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        seed_album(&mut transaction, 2, "Sketches", None).await?;
        sqlx::query("UPDATE music_album SET cover_image_id = 12 WHERE music_album_id = 1")
            .execute(&mut *transaction)
            .await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let albums = search_seeded(&mut repository, &MusicAlbumFilter::default()).await?;

        // Assert
        let covered = albums.first().ok_or("the covered album must be found")?;
        assert_eq!(covered.cover_image_id(), Some(12));
        let uncovered = albums.get(1).ok_or("the uncovered album must be found")?;
        assert_eq!(uncovered.cover_image_id(), None);
        Ok(())
    }

    #[tokio::test]
    async fn search_projects_the_music_group() -> Result<(), Box<dyn Error>> {
        // Arrange: an album in a group other than the seeded bucket's.
        let mut transaction = begin_transaction().await?;
        sqlx::query("INSERT INTO music_group (music_group_id, stage_name) VALUES (5, 'Group')")
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "INSERT INTO music_album (music_album_id, name, release_date, production_type, music_group_id)
             VALUES (1, 'Kind of Blue', '1959-08-17', NULL, 5)",
        )
        .execute(&mut *transaction)
        .await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let albums = search_seeded(&mut repository, &MusicAlbumFilter::default()).await?;

        // Assert
        let album = albums.first().ok_or("the album must be found")?;
        assert_eq!(album.music_group_id(), 5);
        Ok(())
    }

    #[tokio::test]
    async fn search_projects_an_album_without_a_release_date() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        sqlx::query(
            "INSERT INTO music_album (music_album_id, name, release_date, production_type, music_group_id)
             VALUES (1, 'Undated', NULL, NULL, 0)",
        )
        .execute(&mut *transaction)
        .await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let albums = search_seeded(&mut repository, &MusicAlbumFilter::default()).await?;

        // Assert
        let album = albums.first().ok_or("the album must be found")?;
        assert_eq!(album.release_date(), None);
        Ok(())
    }

    #[tokio::test]
    async fn search_reads_a_digital_medium() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_album(&mut transaction, 1, "Digital", None).await?;
        sqlx::query(
            "INSERT INTO music_medium (music_medium_id, music_album_id, type, medium_index)
             VALUES (40, 1, 'DIGITAL', 1)",
        )
        .execute(&mut *transaction)
        .await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let albums = search_seeded(&mut repository, &MusicAlbumFilter::default()).await?;

        // Assert
        let medium = albums
            .first()
            .and_then(|album| album.mediums().first())
            .ok_or("the digital medium must be hydrated")?;
        assert_eq!(medium.index(), MediumIndex::Digital(1));
        assert_eq!(medium.index().as_wire(), "DIGITAL");
        Ok(())
    }

    #[tokio::test]
    async fn search_reads_a_multi_disc_compact_disc_release() -> Result<(), Box<dyn Error>> {
        // Arrange: a release with one CD per reported position.
        let mut transaction = begin_transaction().await?;
        seed_album(&mut transaction, 1, "Double CD", None).await?;
        seed_medium(&mut transaction, 40, 1, 1).await?;
        seed_medium(&mut transaction, 41, 1, 2).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let albums = search_seeded(&mut repository, &MusicAlbumFilter::default()).await?;

        // Assert
        let indexes: Vec<_> = albums
            .first()
            .ok_or("the album must be found")?
            .mediums()
            .iter()
            .map(MusicMedium::index)
            .collect();
        assert_eq!(indexes, vec![MediumIndex::Cd(1), MediumIndex::Cd(2)]);
        Ok(())
    }

    #[tokio::test]
    async fn search_returns_the_unknown_bucket() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let albums = repository.search(&MusicAlbumFilter::default()).await?;

        // Assert
        let bucket = albums.first().ok_or("the bucket must be found")?;
        assert_eq!(bucket.id(), 0);
        assert_eq!(bucket.name(), "Unknown");
        assert_eq!(bucket.music_group_id(), 0);
        assert_eq!(bucket.release_date(), None);
        assert_eq!(bucket.genre_ids(), ["UNKNOWN"]);
        let medium = bucket
            .mediums()
            .first()
            .ok_or("the bucket medium is seeded")?;
        assert_eq!(medium.index(), MediumIndex::Digital(1));
        assert!(medium.tracks().is_empty(), "the bucket medium starts empty");
        Ok(())
    }

    #[tokio::test]
    async fn search_returns_albums_ordered_by_identifier() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_album(&mut transaction, 3, "Third", None).await?;
        seed_album(&mut transaction, 1, "First", None).await?;
        seed_album(&mut transaction, 2, "Second", None).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let albums = search_seeded(&mut repository, &MusicAlbumFilter::default()).await?;

        // Assert
        let ids: Vec<_> = albums.iter().map(MusicAlbum::id).collect();
        assert_eq!(ids, vec![1, 2, 3]);
        Ok(())
    }

    #[tokio::test]
    async fn search_returns_empty_for_unknown_name() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let albums = repository
            .search(&MusicAlbumFilter {
                name: Some("absent".to_owned()),
                ..MusicAlbumFilter::default()
            })
            .await?;

        // Assert
        assert!(albums.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn search_maps_a_stored_production_type_outside_the_list_to_none()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the check constraint admits only the known tokens, so the
        // defensive mapping is probed by relaxing it for this seed.
        let mut transaction = begin_transaction().await?;
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&mut *transaction)
            .await?;
        seed_album(&mut transaction, 1, "Mystery", Some("BOGUS")).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let albums = search_seeded(&mut repository, &MusicAlbumFilter::default()).await?;

        // Assert
        assert_eq!(
            albums.first().map(MusicAlbum::production_type),
            Some(None),
            "an unmapped stored token must be reported as an absent production type"
        );
        Ok(())
    }

    #[tokio::test]
    async fn search_reports_a_medium_that_holds_no_track() -> Result<(), Box<dyn Error>> {
        // Arrange: a medium may sequence no recording yet.
        let mut transaction = begin_transaction().await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        seed_medium(&mut transaction, 40, 1, 1).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let albums = search_seeded(&mut repository, &MusicAlbumFilter::default()).await?;

        // Assert
        let album = albums.first().ok_or("the album must be found")?;
        let medium = album.mediums().first().ok_or("the medium must be found")?;
        assert_eq!(medium.index(), MediumIndex::Cd(1));
        assert!(
            medium.tracks().is_empty(),
            "a medium without a track holds none"
        );
        Ok(())
    }

    #[tokio::test]
    async fn search_excludes_a_playlist_attached_recording() -> Result<(), Box<dyn Error>> {
        // Arrange: the recording is sequenced by a playlist, not by a medium.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.mp3").await?;
        seed_audio(&mut transaction, 20, 10, "a.mp3").await?;
        seed_recording(&mut transaction, 30, Some("So What"), 20).await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        seed_medium(&mut transaction, 40, 1, 1).await?;
        seed_playlist(&mut transaction, 1, "Mix", 1).await?;
        sqlx::query(
            "INSERT INTO music_track (
                music_track_id, track_index, music_playlist_id, music_medium_id, music_recording_id
             )
             VALUES (60, 1, 1, NULL, 30)",
        )
        .execute(&mut *transaction)
        .await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let albums = search_seeded(&mut repository, &MusicAlbumFilter::default()).await?;

        // Assert
        let medium = albums
            .first()
            .and_then(|album| album.mediums().first())
            .ok_or("the album's medium must be hydrated")?;
        assert!(
            medium.tracks().is_empty(),
            "a playlist-attached row must not reach the album's medium"
        );
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn search_rejects_an_unknown_stored_medium_type() -> Result<(), Box<dyn Error>> {
        // Arrange: the stored-type check admits only `CD`, so the defensive
        // mapping is probed by relaxing it for this seed.
        let mut transaction = begin_transaction().await?;
        seed_album(&mut transaction, 1, "Mystery", None).await?;
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "INSERT INTO music_medium (music_medium_id, music_album_id, type, medium_index)
             VALUES (40, 1, 'VINYL', 1)",
        )
        .execute(&mut *transaction)
        .await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let result = repository.search(&MusicAlbumFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        logs_assert(|lines| assert_output_validation_failed(lines, "unknown_medium_type"));
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn search_rejects_an_out_of_range_medium_index() -> Result<(), Box<dyn Error>> {
        // Arrange: the range check is probed by relaxing it for this seed.
        let mut transaction = begin_transaction().await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&mut *transaction)
            .await?;
        seed_medium(&mut transaction, 40, 1, 256).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let result = repository.search(&MusicAlbumFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        logs_assert(|lines| assert_output_validation_failed(lines, "medium_index_out_of_range"));
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn search_rejects_an_out_of_range_track_index() -> Result<(), Box<dyn Error>> {
        // Arrange: the medium range check is probed by relaxing it for this seed.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.mp3").await?;
        seed_audio(&mut transaction, 20, 10, "a.mp3").await?;
        seed_recording(&mut transaction, 30, Some("So What"), 20).await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        seed_medium(&mut transaction, 40, 1, 1).await?;
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&mut *transaction)
            .await?;
        seed_track(&mut transaction, 50, 40, 30, 256).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let result = repository.search(&MusicAlbumFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        logs_assert(|lines| assert_output_validation_failed(lines, "track_index_out_of_range"));
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn search_rejects_a_recording_whose_stored_isrc_code_is_malformed()
    -> Result<(), Box<dyn Error>> {
        // Arrange: `music_recording.isrc_code` is `VARCHAR(12)` with only a
        // uniqueness constraint, so the domain guard is probed by seeding a
        // short code.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.mp3").await?;
        seed_audio(&mut transaction, 20, 10, "a.mp3").await?;
        seed_recording(&mut transaction, 30, Some("So What"), 20).await?;
        sqlx::query("UPDATE music_recording SET isrc_code = 'SHORT' WHERE music_recording_id = 30")
            .execute(&mut *transaction)
            .await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        seed_medium(&mut transaction, 40, 1, 1).await?;
        seed_track(&mut transaction, 50, 40, 30, 1).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let result = repository.search(&MusicAlbumFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        logs_assert(|lines| assert_output_validation_failed(lines, "invalid_recording"));
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn search_rejects_a_negative_medium_track_index() -> Result<(), Box<dyn Error>> {
        // Arrange: the medium range check is probed by relaxing it for this seed.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.mp3").await?;
        seed_audio(&mut transaction, 20, 10, "a.mp3").await?;
        seed_recording(&mut transaction, 30, Some("So What"), 20).await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        seed_medium(&mut transaction, 40, 1, 1).await?;
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&mut *transaction)
            .await?;
        seed_track(&mut transaction, 50, 40, 30, -1).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let result = repository.search(&MusicAlbumFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        logs_assert(|lines| assert_output_validation_failed(lines, "track_index_out_of_range"));
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn search_rejects_an_album_whose_stored_name_is_invalid() -> Result<(), Box<dyn Error>> {
        // Arrange: `music_album.name` has no non-empty check, so the domain
        // guard is probed by seeding an empty name.
        let mut transaction = begin_transaction().await?;
        seed_album(&mut transaction, 1, "", None).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let result = repository.search(&MusicAlbumFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        logs_assert(|lines| assert_output_validation_failed(lines, "invalid_album"));
        Ok(())
    }

    #[tokio::test]
    async fn search_uses_the_track_index_as_the_occurrence_address() -> Result<(), Box<dyn Error>> {
        // Arrange: two recordings on one medium, with a gap left by the index.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.mp3").await?;
        seed_file(&mut transaction, 11, 1, "b.mp3").await?;
        seed_audio(&mut transaction, 20, 10, "a.mp3").await?;
        seed_audio(&mut transaction, 21, 11, "b.mp3").await?;
        seed_recording(&mut transaction, 30, Some("One"), 20).await?;
        seed_recording(&mut transaction, 31, Some("Three"), 21).await?;
        seed_album(&mut transaction, 1, "Gapped", None).await?;
        seed_medium(&mut transaction, 40, 1, 1).await?;
        seed_track(&mut transaction, 50, 40, 30, 1).await?;
        seed_track(&mut transaction, 51, 40, 31, 3).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let albums = search_seeded(&mut repository, &MusicAlbumFilter::default()).await?;

        // Assert
        let tracks = albums
            .first()
            .and_then(|album| album.mediums().first())
            .map(MusicMedium::tracks)
            .ok_or("the medium's tracks must be hydrated")?;
        let indexes: Vec<_> = tracks.iter().filter_map(medium_index).collect();
        assert_eq!(indexes, vec![1, 3], "a removed track leaves its gap");
        Ok(())
    }

    #[tokio::test]
    async fn search_maps_a_playlist_recording_with_a_playlist_index() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the playlist placement is excluded from the album read, but
        // its own index shape stays representable by the domain value.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.mp3").await?;
        seed_audio(&mut transaction, 20, 10, "a.mp3").await?;
        seed_recording(&mut transaction, 30, Some("So What"), 20).await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        seed_medium(&mut transaction, 40, 1, 1).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let albums = search_seeded(&mut repository, &MusicAlbumFilter::default()).await?;

        // Assert
        let medium = albums
            .first()
            .and_then(|album| album.mediums().first())
            .ok_or("the medium must be hydrated")?;
        assert_eq!(medium.index(), MediumIndex::Cd(1));
        assert!(medium.tracks().is_empty());
        assert_eq!(TrackIndex::PlaylistIndex(7).playlist_index(), Some(7));
        assert_eq!(TrackIndex::MediumIndex(2).medium_index(), Some(2));
        Ok(())
    }

    #[tokio::test]
    async fn create_inserts_the_album_and_its_genres() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);
        let album = MusicAlbum::try_new(
            0,
            "Kind of Blue".to_owned(),
            None,
            Some(ProductionType::Studio),
            vec!["jazz".to_owned()],
            0,
            None,
            None,
            Vec::new(),
        )?;

        // Act
        let stored = repository.create(album).await?;

        // Assert
        assert_ne!(stored.id(), 0, "the repository assigns a fresh identity");
        assert_eq!(stored.name(), "Kind of Blue");
        assert_eq!(
            stored.genre_ids(),
            ["JAZZ"],
            "the genre link uses the stored uppercase vocabulary"
        );
        Ok(())
    }

    #[tokio::test]
    async fn create_rejects_a_duplicate_identity() -> Result<(), Box<dyn Error>> {
        // Arrange: the identity already exists.
        let mut transaction = begin_transaction().await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);
        let album = MusicAlbum::try_new(
            0,
            "Kind of Blue".to_owned(),
            None,
            None,
            Vec::new(),
            0,
            None,
            None,
            Vec::new(),
        )?;

        // Act
        let result = repository.create(album).await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn save_updates_the_album_and_replaces_its_genres() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_genre(&mut transaction, "JAZZ").await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        seed_album_genre(&mut transaction, 1, "JAZZ").await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);
        let updated = MusicAlbum::try_new(
            1,
            "Kind of Blue (Remastered)".to_owned(),
            None,
            Some(ProductionType::Studio),
            vec!["BLUES".to_owned()],
            0,
            None,
            None,
            Vec::new(),
        )?;

        // Act
        let found = repository.update(updated).await?;

        // Assert
        let stored = found.ok_or("the album must be found")?;
        assert_eq!(stored.name(), "Kind of Blue (Remastered)");
        assert_eq!(stored.genre_ids(), ["BLUES"]);
        Ok(())
    }

    #[tokio::test]
    async fn save_reports_an_unknown_album_as_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);
        let album = MusicAlbum::try_new(
            404,
            "Unknown".to_owned(),
            None,
            None,
            Vec::new(),
            0,
            None,
            None,
            Vec::new(),
        )?;

        // Act
        let stored = repository.update(album).await?;

        // Assert
        assert!(stored.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn put_medium_creates_then_retypes_and_keeps_the_tracks() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.mp3").await?;
        seed_audio(&mut transaction, 20, 10, "a.mp3").await?;
        seed_recording(&mut transaction, 30, Some("So What"), 20).await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;

        // Act: create the medium, then retype it.
        let created = {
            let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);
            repository.put_medium(1, MediumIndex::Cd(1)).await?
        };
        let medium_id = sqlx::query_scalar::<_, i64>(
            "SELECT music_medium_id
             FROM music_medium
             WHERE music_album_id = 1 AND medium_index = 1",
        )
        .fetch_one(&mut *transaction)
        .await?;
        seed_track(&mut transaction, 50, medium_id, 30, 1).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);
        let retyped = repository.put_medium(1, MediumIndex::Digital(1)).await?;

        // Assert
        let stored_created = created.ok_or("the album must be found")?;
        assert_eq!(stored_created.index(), MediumIndex::Cd(1));
        let stored_retyped = retyped.ok_or("the album must be found")?;
        assert_eq!(stored_retyped.index(), MediumIndex::Digital(1));
        assert_eq!(stored_retyped.tracks().len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn put_medium_reports_an_unknown_album_as_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let written = repository.put_medium(404, MediumIndex::Cd(1)).await?;

        // Assert
        assert!(written.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn place_track_moves_the_recording_between_mediums() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.mp3").await?;
        seed_audio(&mut transaction, 20, 10, "a.mp3").await?;
        seed_recording(&mut transaction, 30, Some("So What"), 20).await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        seed_medium(&mut transaction, 40, 1, 1).await?;
        seed_medium(&mut transaction, 41, 1, 2).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let placed = repository.place_track(1, 1, 1, 30).await?;
        let placed_elsewhere = repository.has_medium_placement(30).await?;
        let moved = repository.place_track(1, 2, 1, 30).await?;
        let unchanged = repository.place_track(1, 2, 1, 30).await?;

        // Assert
        assert_eq!(
            placed.as_ref().map(MusicRecording::index),
            Some(TrackIndex::MediumIndex(1))
        );
        assert!(placed_elsewhere);
        assert_eq!(
            moved.as_ref().map(MusicRecording::index),
            Some(TrackIndex::MediumIndex(1))
        );
        assert_eq!(unchanged.as_ref().map(MusicRecording::id), Some(30));
        Ok(())
    }

    #[tokio::test]
    async fn place_track_reports_an_occupied_destination_as_conflict() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the destination already holds another recording.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.mp3").await?;
        seed_file(&mut transaction, 11, 1, "b.mp3").await?;
        seed_audio(&mut transaction, 20, 10, "a.mp3").await?;
        seed_audio(&mut transaction, 21, 11, "b.mp3").await?;
        seed_recording(&mut transaction, 30, Some("So What"), 20).await?;
        seed_recording(&mut transaction, 31, Some("Blue in Green"), 21).await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        seed_medium(&mut transaction, 40, 1, 1).await?;
        seed_track(&mut transaction, 50, 40, 30, 1).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let result = repository.place_track(1, 1, 1, 31).await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::Conflict)));
        Ok(())
    }

    #[tokio::test]
    async fn place_track_reports_an_unknown_recording_as_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        seed_medium(&mut transaction, 40, 1, 1).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let placed = repository.place_track(1, 1, 1, 404).await?;

        // Assert
        assert!(placed.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn place_tracks_swaps_two_recordings() -> Result<(), Box<dyn Error>> {
        // Arrange: each recording already holds the other's destination.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.mp3").await?;
        seed_file(&mut transaction, 11, 1, "b.mp3").await?;
        seed_audio(&mut transaction, 20, 10, "a.mp3").await?;
        seed_audio(&mut transaction, 21, 11, "b.mp3").await?;
        seed_recording(&mut transaction, 30, Some("So What"), 20).await?;
        seed_recording(&mut transaction, 31, Some("Blue in Green"), 21).await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        seed_medium(&mut transaction, 40, 1, 1).await?;
        seed_track(&mut transaction, 50, 40, 30, 1).await?;
        seed_track(&mut transaction, 51, 40, 31, 2).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        repository
            .place_tracks(1, &[(1, 1, 31), (1, 2, 30)])
            .await?;

        // Assert
        let albums = search_seeded(&mut repository, &MusicAlbumFilter::default()).await?;
        let tracks = albums
            .first()
            .and_then(|album| album.mediums().first())
            .map(MusicMedium::tracks)
            .ok_or("the medium must be hydrated")?;
        assert_eq!(tracks.first().map(MusicRecording::id), Some(31));
        assert_eq!(tracks.get(1).map(MusicRecording::id), Some(30));
        Ok(())
    }

    #[tokio::test]
    async fn replace_mediums_keeps_a_medium_that_sequences_a_recording()
    -> Result<(), Box<dyn Error>> {
        // Arrange: medium 2 holds a recording and is outside the new structure.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.mp3").await?;
        seed_audio(&mut transaction, 20, 10, "a.mp3").await?;
        seed_recording(&mut transaction, 30, Some("So What"), 20).await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        seed_medium(&mut transaction, 40, 1, 1).await?;
        seed_medium(&mut transaction, 41, 1, 2).await?;
        seed_track(&mut transaction, 50, 41, 30, 1).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        repository
            .replace_mediums(1, &[MediumIndex::Digital(1)])
            .await?;

        // Assert
        let albums = search_seeded(&mut repository, &MusicAlbumFilter::default()).await?;
        let album = albums.first().ok_or("the album must be found")?;
        let indices: Vec<_> = album.mediums().iter().map(MusicMedium::index).collect();
        assert_eq!(indices, [MediumIndex::Digital(1), MediumIndex::Cd(2)]);
        Ok(())
    }

    #[tokio::test]
    async fn replace_mediums_removes_an_empty_medium_outside_the_structure()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        seed_medium(&mut transaction, 40, 1, 1).await?;
        seed_medium(&mut transaction, 41, 1, 2).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        repository
            .replace_mediums(1, &[MediumIndex::Digital(1)])
            .await?;

        // Assert
        let albums = search_seeded(&mut repository, &MusicAlbumFilter::default()).await?;
        let album = albums.first().ok_or("the album must be found")?;
        let indices: Vec<_> = album.mediums().iter().map(MusicMedium::index).collect();
        assert_eq!(indices, [MediumIndex::Digital(1)]);
        Ok(())
    }

    #[tokio::test]
    async fn save_recording_updates_the_name_and_isrc() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.mp3").await?;
        seed_audio(&mut transaction, 20, 10, "a.mp3").await?;
        seed_recording(&mut transaction, 30, Some("So What"), 20).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);
        let recording = MusicRecording::try_new(
            30,
            Some("So What (Remastered)".to_owned()),
            Some("USSM15900001".to_owned()),
            20,
            10,
            None,
            TrackIndex::MediumIndex(1),
        )?;

        // Act
        let saved = repository.save_recording(recording).await?;

        // Assert
        let stored = saved.ok_or("the recording must be found")?;
        assert_eq!(stored.name(), Some("So What (Remastered)"));
        assert_eq!(stored.isrc_code(), Some("USSM15900001"));
        Ok(())
    }

    #[tokio::test]
    async fn search_filters_by_music_group() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        sqlx::query("INSERT INTO music_group (music_group_id, stage_name) VALUES (4, 'The Group')")
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "INSERT INTO music_album (music_album_id, name, music_group_id)
             VALUES (2, 'Sketches', 4)",
        )
        .execute(&mut *transaction)
        .await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let albums = repository
            .search(&MusicAlbumFilter {
                music_group_id: Some(4),
                ..MusicAlbumFilter::default()
            })
            .await?;

        // Assert
        assert_eq!(albums.len(), 1);
        assert_eq!(albums.first().map(MusicAlbum::id), Some(2));
        Ok(())
    }
}
