use std::collections::HashMap;

use sqlx::QueryBuilder;
use sqlx::Sqlite;
use sqlx::Transaction;

use crate::domain::alias::NumericID;
use crate::domain::model::music_album::MusicAlbum;
use crate::domain::model::music_album::ProductionType;
use crate::domain::model::music_medium::MusicMedium;
use crate::domain::model::music_recording::MusicRecording;
use crate::domain::model::music_track::MusicTrack;
use crate::domain::model::music_track::MusicTrackPosition;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::music_album_repository::MusicAlbumFilter;
use crate::domain::port::music_album_repository::MusicAlbumRepository;
use crate::domain::port::music_album_repository::MusicTrackFilter;

use super::model::music_album::MusicAlbumRow;
use super::model::music_track::MusicTrackRow;
use super::query_filter::push_filter;

/// Repository querying music albums and their tracks, backed by a `SQLite`
/// transaction.
pub struct SqlxMusicAlbumRepository<'transaction> {
    /// Transaction the repository reads from.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxMusicAlbumRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }
}

/// A medium being hydrated: its identifier, index and type, with the tracks
/// read so far.
type HydratedMedium = (NumericID, u32, String, Vec<MusicRecording>);

impl MusicAlbumRepository for SqlxMusicAlbumRepository<'_> {
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
                music_recording.first_release_date,
                music_recording.isrc_code,
                music_recording.audio_id,
                file.file_id
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
        if let Some(name) = filter.name.as_deref() {
            push_filter(&mut first, &mut builder, "music_album.name = ", name);
        }

        // The album, its medium and its tracks make one aggregate read, so the
        // order fixes both the collection order and each medium's play order. A
        // stable album identifier also makes the caller's offset/limit
        // pagination deterministic (`ListAlbums` pages in memory).
        builder.push(
            " ORDER BY music_album.music_album_id, music_medium.medium_index, music_track.track_index, music_track.music_track_id",
        );

        let rows = builder
            .build_query_as::<MusicAlbumRow>()
            .fetch_all(&mut **self.transaction)
            .await?;

        // The rows repeat an album once per track and arrive ordered by album,
        // then medium index, then track index, so one pass groups them without
        // re-sorting.
        let mut mediums: HashMap<NumericID, Vec<HydratedMedium>> = HashMap::new();
        let mut album_ids: Vec<NumericID> = Vec::new();

        for row in &rows {
            if album_ids.last() != Some(&row.music_album_id) {
                album_ids.push(row.music_album_id);
            }
            let Some(medium_id) = row.music_medium_id else {
                continue;
            };
            let medium_index = row
                .medium_index
                .ok_or(RepositoryError::DataIntegrityViolation)?;
            let medium_type = row
                .medium_type
                .clone()
                .ok_or(RepositoryError::DataIntegrityViolation)?;
            let album_mediums = mediums.entry(row.music_album_id).or_default();
            if album_mediums.last().map(|medium| medium.0) != Some(medium_id) {
                album_mediums.push((medium_id, medium_index, medium_type, Vec::new()));
            }
            let Some(recording_id) = row.music_recording_id else {
                continue;
            };
            let recording = MusicRecording::try_new(
                recording_id,
                row.recording_name.clone(),
                row.first_release_date
                    .ok_or(RepositoryError::DataIntegrityViolation)?,
                row.isrc_code.clone(),
                row.audio_id
                    .ok_or(RepositoryError::DataIntegrityViolation)?,
                row.file_id.ok_or(RepositoryError::DataIntegrityViolation)?,
                None,
            )
            .map_err(|_| RepositoryError::DataIntegrityViolation)?;
            if let Some(medium) = mediums
                .get_mut(&row.music_album_id)
                .and_then(|hydrated| hydrated.last_mut())
            {
                medium.3.push(recording);
            }
        }

        let mut albums = Vec::with_capacity(album_ids.len());
        for album_id in album_ids {
            let row = rows
                .iter()
                .find(|row| row.music_album_id == album_id)
                .ok_or(RepositoryError::DataIntegrityViolation)?;
            let album_mediums = mediums
                .remove(&album_id)
                .unwrap_or_default()
                .into_iter()
                .map(|(medium_id, medium_index, medium_type, tracks)| {
                    MusicMedium::new(medium_id, Some(medium_index), medium_type, tracks)
                })
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
                    None,
                    album_mediums,
                )
                .map_err(|_| RepositoryError::DataIntegrityViolation)?,
            );
        }
        Ok(albums)
    }

    async fn search_tracks(
        &mut self,
        filter: &MusicTrackFilter,
    ) -> Result<Vec<MusicTrack>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT
                music_track.music_track_id,
                music_track.music_medium_id,
                music_track.music_recording_id,
                music_track.track_index
            FROM music_track
            JOIN music_medium
                ON music_medium.music_medium_id = music_track.music_medium_id",
        );
        let mut first = true;

        if let Some(id) = filter.id {
            push_filter(
                &mut first,
                &mut builder,
                "music_track.music_track_id = ",
                id,
            );
        }
        if let Some(medium_id) = filter.medium_id {
            push_filter(
                &mut first,
                &mut builder,
                "music_medium.music_medium_id = ",
                medium_id,
            );
        }

        // Ascending medium index then track index is the album's play order;
        // the track identifier breaks a duplicate-position tie deterministically.
        builder.push(
            " ORDER BY music_medium.medium_index, music_track.track_index, music_track.music_track_id",
        );

        let rows = builder
            .build_query_as::<MusicTrackRow>()
            .fetch_all(&mut **self.transaction)
            .await?;

        rows.into_iter()
            .map(|row| {
                MusicTrack::try_new(
                    row.music_track_id,
                    MusicTrackPosition::Medium(row.music_medium_id),
                    row.music_recording_id,
                    row.track_index,
                )
                .map_err(|_| RepositoryError::DataIntegrityViolation)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use sqlx::Sqlite;
    use sqlx::Transaction;
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::domain::model::music_album::MusicAlbum;
    use crate::domain::model::music_album::ProductionType;
    use crate::domain::model::music_recording::MusicRecording;
    use crate::domain::model::music_track::MusicTrack;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::music_album_repository::MusicAlbumFilter;
    use crate::domain::port::music_album_repository::MusicAlbumRepository as _;
    use crate::domain::port::music_album_repository::MusicTrackFilter;

    use super::SqlxMusicAlbumRepository;

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

    /// Seed the audio backing a recording, on the seeded `2.0` channel.
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

    /// Seed a recording on `audio_id`; `name` may be absent.
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

    /// Seed a music album with an optional stored production type.
    async fn seed_album(
        transaction: &mut Transaction<'static, Sqlite>,
        album_id: i64,
        name: &str,
        production_type: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO music_album (music_album_id, name, release_date, production_type)
             VALUES (?1, ?2, '1959-08-17', ?3)",
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

    /// Seed a medium of `album_id` at `medium_index`.
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

    #[tokio::test]
    async fn search_returns_albums_with_genres_and_their_tracks() -> Result<(), Box<dyn Error>> {
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
        seed_track(&mut transaction, 50, 40, 30, 1).await?;
        seed_track(&mut transaction, 51, 40, 31, 2).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let albums = repository.search(&MusicAlbumFilter::default()).await?;

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
        assert_eq!(medium.id(), 40);
        assert_eq!(medium.medium_index(), 1);
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
        assert_eq!(first_track.file_id(), 10);
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

    #[tokio::test]
    async fn search_returns_albums_ordered_by_identifier() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_album(&mut transaction, 3, "Third", None).await?;
        seed_album(&mut transaction, 1, "First", None).await?;
        seed_album(&mut transaction, 2, "Second", None).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let albums = repository.search(&MusicAlbumFilter::default()).await?;

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
        let albums = repository.search(&MusicAlbumFilter::default()).await?;

        // Assert
        assert_eq!(
            albums.first().map(MusicAlbum::production_type),
            Some(None),
            "an unmapped stored token must be reported as an absent production type"
        );
        Ok(())
    }

    #[tokio::test]
    async fn search_tracks_returns_the_mediums_tracks_in_play_order() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.mp3").await?;
        seed_file(&mut transaction, 11, 1, "b.mp3").await?;
        seed_audio(&mut transaction, 20, 10, "a.mp3").await?;
        seed_audio(&mut transaction, 21, 11, "b.mp3").await?;
        seed_recording(&mut transaction, 30, Some("So What"), 20).await?;
        seed_recording(&mut transaction, 31, None, 21).await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        seed_album(&mut transaction, 2, "Sketches", None).await?;
        seed_medium(&mut transaction, 40, 1, 1).await?;
        seed_medium(&mut transaction, 41, 2, 1).await?;
        seed_track(&mut transaction, 50, 40, 30, 2).await?;
        seed_track(&mut transaction, 51, 40, 31, 1).await?;
        seed_track(&mut transaction, 52, 41, 30, 1).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let tracks = repository
            .search_tracks(&MusicTrackFilter {
                medium_id: Some(40),
                ..MusicTrackFilter::default()
            })
            .await?;

        // Assert
        let ids: Vec<_> = tracks.iter().map(MusicTrack::id).collect();
        assert_eq!(ids, vec![51, 50], "tracks must be ordered by track index");
        let first = tracks.first().ok_or("the first track must be found")?;
        assert_eq!(first.medium_id(), Some(40));
        assert_eq!(first.recording_id(), 31);
        assert_eq!(first.track_index(), 1);
        let second = tracks.get(1).ok_or("the second track must be found")?;
        assert_eq!(second.recording_id(), 30);
        Ok(())
    }

    #[tokio::test]
    async fn search_tracks_orders_across_mediums() -> Result<(), Box<dyn Error>> {
        // Arrange: a single album holds one medium (`uq_music_medium_music_album_id`),
        // so the collection spanning several mediums comes from several albums.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.mp3").await?;
        seed_file(&mut transaction, 11, 1, "b.mp3").await?;
        seed_audio(&mut transaction, 20, 10, "a.mp3").await?;
        seed_audio(&mut transaction, 21, 11, "b.mp3").await?;
        seed_recording(&mut transaction, 30, Some("First"), 20).await?;
        seed_recording(&mut transaction, 31, Some("Second"), 21).await?;
        seed_album(&mut transaction, 1, "Later", None).await?;
        seed_album(&mut transaction, 2, "Earlier", None).await?;
        seed_medium(&mut transaction, 40, 1, 2).await?;
        seed_medium(&mut transaction, 41, 2, 1).await?;
        seed_track(&mut transaction, 50, 40, 30, 1).await?;
        seed_track(&mut transaction, 51, 41, 31, 1).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let tracks = repository
            .search_tracks(&MusicTrackFilter::default())
            .await?;

        // Assert
        let ids: Vec<_> = tracks.iter().map(MusicTrack::id).collect();
        assert_eq!(
            ids,
            vec![51, 50],
            "mediums must be ordered by ascending medium index"
        );
        Ok(())
    }

    #[tokio::test]
    async fn search_tracks_excludes_playlist_attached_rows() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 1).await?;
        seed_file(&mut transaction, 10, 1, "a.mp3").await?;
        seed_audio(&mut transaction, 20, 10, "a.mp3").await?;
        seed_recording(&mut transaction, 30, Some("So What"), 20).await?;
        // A track attached to a playlist rather than a medium is out of scope.
        sqlx::query(
            "INSERT INTO music_playlist (music_playlist_id, name, user_id) VALUES (1, 'Mix', 1)",
        )
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO music_track (
                music_track_id, track_index, music_playlist_id, music_medium_id, music_recording_id
             )
             VALUES (60, 0, 1, NULL, 30)",
        )
        .execute(&mut *transaction)
        .await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let tracks = repository
            .search_tracks(&MusicTrackFilter::default())
            .await?;

        // Assert
        assert!(
            tracks.is_empty(),
            "a track attached to a playlist, not a medium, is out of scope"
        );
        Ok(())
    }

    #[tokio::test]
    async fn search_tracks_returns_empty_for_unknown_medium() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let tracks = repository
            .search_tracks(&MusicTrackFilter {
                medium_id: Some(999),
                ..MusicTrackFilter::default()
            })
            .await?;

        // Assert
        assert!(tracks.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn search_tracks_rejects_a_negative_track_index() -> Result<(), Box<dyn Error>> {
        // Arrange: `chk_music_track_track_index` admits any non-negative index,
        // so the domain guard is probed by relaxing the check for this seed.
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
        let result = repository
            .search_tracks(&MusicTrackFilter {
                medium_id: Some(40),
                ..MusicTrackFilter::default()
            })
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn search_reports_a_medium_that_holds_no_track() -> Result<(), Box<dyn Error>> {
        // Arrange: `uq_music_medium_music_album_id` allows an album one medium,
        // and a medium may hold no track yet.
        let mut transaction = begin_transaction().await?;
        seed_album(&mut transaction, 1, "Kind of Blue", None).await?;
        seed_medium(&mut transaction, 40, 1, 1).await?;
        let mut repository = SqlxMusicAlbumRepository::new(&mut transaction);

        // Act
        let albums = repository.search(&MusicAlbumFilter::default()).await?;

        // Assert
        let album = albums.first().ok_or("the album must be found")?;
        let medium = album.mediums().first().ok_or("the medium must be found")?;
        assert_eq!(medium.id(), 40);
        assert!(
            medium.tracks().is_empty(),
            "a medium without a track holds none"
        );
        Ok(())
    }

    #[tokio::test]
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
        Ok(())
    }
}
