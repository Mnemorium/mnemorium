use sqlx::QueryBuilder;
use sqlx::Sqlite;
use sqlx::Transaction;

use crate::domain::model::music_album::ProductionType;
use crate::domain::model::music_track::MusicTrack;
use crate::domain::model::music_track::MusicTrackPosition;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::music_album_repository::MusicAlbumFilter;
use crate::domain::port::music_album_repository::MusicAlbumRepository;
use crate::domain::port::music_album_repository::MusicAlbumSummary;
use crate::domain::port::music_album_repository::MusicTrackDetail;
use crate::domain::port::music_album_repository::MusicTrackFilter;

use super::model::music_album::MusicAlbumSummary as SqlxMusicAlbumSummary;
use super::model::music_track::MusicTrackDetail as SqlxMusicTrackDetail;
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

impl MusicAlbumRepository for SqlxMusicAlbumRepository<'_> {
    async fn search(
        &mut self,
        filter: &MusicAlbumFilter,
    ) -> Result<Vec<MusicAlbumSummary>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT
                music_album.music_album_id,
                music_album.name,
                music_album.release_date,
                music_album.production_type,
                (
                    SELECT music_medium.music_medium_id
                    FROM music_medium
                    WHERE music_medium.music_album_id = music_album.music_album_id
                ) AS music_medium_id,
                COALESCE((
                    SELECT GROUP_CONCAT(genre_id, ',')
                    FROM (
                        SELECT genre_id
                        FROM music_album_genre
                        WHERE music_album_genre.music_album_id = music_album.music_album_id
                        ORDER BY genre_id
                    )
                ), '') AS genres,
                (
                    SELECT COUNT(*)
                    FROM music_track
                    JOIN music_medium
                        ON music_medium.music_medium_id = music_track.music_medium_id
                    WHERE music_medium.music_album_id = music_album.music_album_id
                ) AS track_count
            FROM music_album",
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

        // A stable order makes the caller's offset/limit pagination deterministic
        // (`ListAlbums` pages the returned collection in memory).
        builder.push(" ORDER BY music_album.music_album_id");

        let rows = builder
            .build_query_as::<SqlxMusicAlbumSummary>()
            .fetch_all(&mut **self.transaction)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let genres = row
                    .genres
                    .split(',')
                    .filter(|genre| !genre.is_empty())
                    .map(str::to_owned)
                    .collect();
                let production_type = row
                    .production_type
                    .as_deref()
                    .and_then(ProductionType::from_wire);
                MusicAlbumSummary::new(
                    row.music_album_id,
                    row.music_medium_id,
                    row.name,
                    row.release_date,
                    production_type,
                    genres,
                    usize::try_from(row.track_count).unwrap_or_default(),
                )
            })
            .collect())
    }

    async fn search_tracks(
        &mut self,
        filter: &MusicTrackFilter,
    ) -> Result<Vec<MusicTrackDetail>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT
                music_track.music_track_id,
                music_track.music_recording_id,
                music_track.track_index,
                music_medium.music_album_id,
                music_medium.music_medium_id,
                music_medium.medium_index,
                music_recording.first_release_date,
                music_recording.isrc_code,
                music_recording.name AS name,
                file.file_id
            FROM music_track
            JOIN music_medium
                ON music_medium.music_medium_id = music_track.music_medium_id
            JOIN music_recording
                ON music_recording.music_recording_id = music_track.music_recording_id
            JOIN audio ON audio.audio_id = music_recording.audio_id
            JOIN file ON file.file_id = audio.file_id",
        );
        let mut first = true;

        if let Some(medium_id) = filter.music_medium_id {
            push_filter(
                &mut first,
                &mut builder,
                "music_medium.music_medium_id = ",
                medium_id,
            );
        }
        if let Some(track_id) = filter.music_track_id {
            push_filter(
                &mut first,
                &mut builder,
                "music_track.music_track_id = ",
                track_id,
            );
        }

        // Ascending medium index then track index is the album's play order;
        // the track identifier breaks a duplicate-position tie deterministically.
        builder.push(
            " ORDER BY music_medium.medium_index, music_track.track_index, music_track.music_track_id",
        );

        let rows = builder
            .build_query_as::<SqlxMusicTrackDetail>()
            .fetch_all(&mut **self.transaction)
            .await?;

        rows.into_iter()
            .map(|row| {
                let track = MusicTrack::try_new(
                    row.music_track_id,
                    MusicTrackPosition::Medium(row.music_medium_id),
                    row.music_recording_id,
                    row.track_index,
                )
                .map_err(|_| RepositoryError::DataIntegrityViolation)?;
                Ok(MusicTrackDetail::new(
                    track,
                    row.music_album_id,
                    row.medium_index,
                    row.file_id,
                    row.name,
                    row.isrc_code,
                    row.first_release_date,
                ))
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

    use crate::domain::model::music_album::ProductionType;
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

    /// Seed a playlist-attached track, owned by `user_id`'s playlist.
    #[expect(
        clippy::single_call_fn,
        reason = "the out-of-scope seed is named so the test states what it excludes"
    )]
    async fn seed_playlist_track(
        transaction: &mut Transaction<'static, Sqlite>,
        track_id: i64,
        recording_id: i64,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO music_playlist (music_playlist_id, name, user_id) VALUES (1, 'Mix', ?1)",
        )
        .bind(user_id)
        .execute(&mut **transaction)
        .await?;
        sqlx::query(
            "INSERT INTO music_track (
                music_track_id, track_index, music_playlist_id, music_medium_id, music_recording_id
             )
             VALUES (?1, 0, 1, NULL, ?2)",
        )
        .bind(track_id)
        .bind(recording_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn search_returns_albums_with_genres_and_track_count() -> Result<(), Box<dyn Error>> {
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
        assert_eq!(first.genres(), ["BLUES", "JAZZ"]);
        assert_eq!(first.music_medium_id(), Some(40));
        assert_eq!(first.track_count(), 2);
        let second = albums.get(1).ok_or("the second album must be found")?;
        assert_eq!(second.id(), 2);
        assert_eq!(second.production_type(), None);
        assert_eq!(
            second.music_medium_id(),
            None,
            "an album without a medium must report no medium"
        );
        assert!(second.genres().is_empty());
        assert_eq!(second.track_count(), 0);
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
        assert_eq!(by_name.first().map(super::MusicAlbumSummary::id), Some(2));
        assert_eq!(by_id.len(), 1);
        assert_eq!(
            by_id.first().map(super::MusicAlbumSummary::name),
            Some("Kind of Blue")
        );
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
        let ids: Vec<_> = albums.iter().map(super::MusicAlbumSummary::id).collect();
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
            albums
                .first()
                .map(super::MusicAlbumSummary::production_type),
            Some(None),
            "an unmapped stored token must be reported as an absent production type"
        );
        Ok(())
    }

    #[tokio::test]
    async fn search_tracks_scopes_to_the_album_and_reports_details() -> Result<(), Box<dyn Error>> {
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
                music_medium_id: Some(40),
                ..MusicTrackFilter::default()
            })
            .await?;

        // Assert
        let ids: Vec<_> = tracks
            .iter()
            .map(|detail| detail.track().music_track_id())
            .collect();
        assert_eq!(ids, vec![51, 50], "tracks must be ordered by track index");
        let first = tracks.first().ok_or("the first track must be found")?;
        assert_eq!(first.album_id(), 1);
        assert_eq!(first.medium_index(), 1);
        assert_eq!(first.file_id(), 11);
        assert_eq!(
            first.name(),
            None,
            "a nameless recording has no display name"
        );
        assert_eq!(first.isrc_code(), None);
        let second = tracks.get(1).ok_or("the second track must be found")?;
        assert_eq!(second.file_id(), 10);
        assert_eq!(second.name(), Some("So What"));
        assert_eq!(second.track().recording_id(), 30);
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
        let ids: Vec<_> = tracks
            .iter()
            .map(|detail| detail.track().music_track_id())
            .collect();
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
        seed_playlist_track(&mut transaction, 60, 30, 1).await?;
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
                music_medium_id: Some(999),
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
                music_medium_id: Some(40),
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
}
