//! Shared read path of the Asset bounded context: resolve a stored asset, decide
//! whether the caller may read it, and hand the use case what it needs.
//!
//! Every asset route goes through this module so the file, image and content
//! representations cannot drift on authorization (`API-044`): an unreadable
//! asset and an unknown asset are one outcome, and an unreadable asset's bytes
//! are never read.

use tracing::warn;

use crate::domain::alias::NumericID;
use crate::domain::model::asset::AssetContext;
use crate::domain::model::audio::Audio;
use crate::domain::model::file::File;
use crate::domain::model::gallery::Gallery;
use crate::domain::model::gallery_item::GalleryItemMedia;
use crate::domain::model::image::Image;
use crate::domain::model::permission::{Action, Global};
use crate::domain::model::scope::Scope;
use crate::domain::model::video::Video;
use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::audio_repository::{AudioFilter, AudioRepository as _};
use crate::domain::port::error::RepositoryError;
use crate::domain::port::file_repository::{FileFilter, FileRepository as _};
use crate::domain::port::gallery_repository::{
    GalleryFilter, GalleryItemFilter, GalleryRepository as _,
};
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::image_repository::{ImageFilter, ImageRepository as _};
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::movie_repository::MovieRepository as _;
use crate::domain::port::music_album_repository::MusicAlbumRepository as _;
use crate::domain::port::music_playlist_repository::{
    MusicPlaylistFilter, MusicPlaylistRepository as _,
};
use crate::domain::port::permission_repository::{PermissionFilter, PermissionRepository as _};
use crate::domain::port::series_repository::SeriesRepository as _;
use crate::domain::port::user_repository::{UserFilter, UserRepository as _};
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::port::video_repository::{VideoFilter, VideoRepository as _};
use crate::domain::service::authorization::{Authorization, Decision};

/// Why an asset could not be accessed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum AssetAccessError {
    /// The asset does not exist, or the caller may not read it.
    #[error("the asset does not exist")]
    NoSuchAsset,
    /// The caller does not exist.
    #[error("the caller does not exist")]
    NoSuchCaller,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

impl AssetAccessError {
    /// Map a repository failure onto the access error.
    fn repository(error: RepositoryError) -> Self {
        Self::Unknown(error.into())
    }
}

/// A stored file the caller may read, with the media rows backing it.
pub(crate) struct AccessedFile {
    /// Audio row backing the file, when one exists.
    pub audio: Option<Audio>,
    /// The file itself.
    pub file: File,
    /// Image row backing the file, when one exists.
    pub image: Option<Image>,
    /// Video row backing the file, when one exists.
    pub video: Option<Video>,
}

/// Resolve `file_id` and decide whether the caller may read it.
pub(crate) async fn access_file<U>(
    unit_of_work: &mut U,
    caller_id: NumericID,
    file_id: NumericID,
) -> Result<AccessedFile, AssetAccessError>
where
    U: AssetUnitOfWork + IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    let caller = unit_of_work
        .users()
        .search(&UserFilter {
            id: Some(caller_id),
            ..UserFilter::default()
        })
        .await
        .map_err(AssetAccessError::repository)?
        .into_iter()
        .next()
        .ok_or(AssetAccessError::NoSuchCaller)?;
    let permissions = unit_of_work
        .permissions()
        .search(&PermissionFilter {
            user_id: Some(caller.id()),
            ..PermissionFilter::default()
        })
        .await
        .map_err(AssetAccessError::repository)?;

    let file = unit_of_work
        .files()
        .search(&FileFilter {
            id: Some(file_id),
            ..FileFilter::default()
        })
        .await
        .map_err(AssetAccessError::repository)?
        .into_iter()
        .next()
        .ok_or(AssetAccessError::NoSuchAsset)?;
    let image = unit_of_work
        .images()
        .search(&ImageFilter {
            file_id: Some(file_id),
            ..ImageFilter::default()
        })
        .await
        .map_err(AssetAccessError::repository)?
        .into_iter()
        .next();
    let audio = unit_of_work
        .audios()
        .search(&AudioFilter {
            file_id: Some(file_id),
            ..AudioFilter::default()
        })
        .await
        .map_err(AssetAccessError::repository)?
        .into_iter()
        .next();
    let video = unit_of_work
        .videos()
        .search(&VideoFilter {
            file_id: Some(file_id),
            ..VideoFilter::default()
        })
        .await
        .map_err(AssetAccessError::repository)?
        .into_iter()
        .next();

    let decision = match (image.as_ref(), video.as_ref(), audio.as_ref()) {
        (Some(image_row), _, _) => match image_row.scope() {
            Some(Scope::Library) => {
                Authorization::decide_asset(&caller, &permissions, AssetContext::Library, &Global)
            }
            Some(Scope::User) => {
                if let Some(gallery) =
                    gallery_of(unit_of_work, GalleryItemMedia::Image(image_row.image_id())).await?
                {
                    Authorization::decide_asset(
                        &caller,
                        &permissions,
                        AssetContext::gallery(gallery.gallery_id()),
                        &gallery,
                    )
                } else {
                    let cover = unit_of_work
                        .music_playlists()
                        .search(&MusicPlaylistFilter {
                            cover_image_id: Some(image_row.image_id()),
                            ..MusicPlaylistFilter::default()
                        })
                        .await
                        .map_err(AssetAccessError::repository)?
                        .into_iter()
                        .next();
                    match cover {
                        Some(playlist) => Authorization::decide_asset(
                            &caller,
                            &permissions,
                            AssetContext::playlist(playlist.id()),
                            &playlist,
                        ),
                        None => Authorization::decide_orphan_asset(&caller, &file),
                    }
                }
            }
            None => Authorization::decide_orphan_asset(&caller, &file),
        },
        (None, Some(video_row), _) => match video_row.scope() {
            Some(Scope::Library) => {
                Authorization::decide_asset(&caller, &permissions, AssetContext::Library, &Global)
            }
            Some(Scope::User) => {
                if let Some(gallery) =
                    gallery_of(unit_of_work, GalleryItemMedia::Video(video_row.id())).await?
                {
                    Authorization::decide_asset(
                        &caller,
                        &permissions,
                        AssetContext::gallery(gallery.gallery_id()),
                        &gallery,
                    )
                } else {
                    Authorization::decide_orphan_asset(&caller, &file)
                }
            }
            None => {
                let movie = unit_of_work
                    .movies()
                    .find_by_video(video_row.id())
                    .await
                    .map_err(AssetAccessError::repository)?;
                if let Some(attached_movie) = movie {
                    Authorization::decide(
                        &caller,
                        &permissions,
                        Action::LibraryRead,
                        &attached_movie,
                    )
                } else if unit_of_work
                    .series()
                    .find_episode_by_video(video_row.id())
                    .await
                    .map_err(AssetAccessError::repository)?
                    .is_some()
                {
                    Authorization::decide_library_surface(&caller, &permissions)
                } else {
                    Authorization::decide_orphan_asset(&caller, &file)
                }
            }
        },
        (None, None, Some(audio_row)) => {
            let album = unit_of_work
                .music_albums()
                .find_album_by_audio(audio_row.id())
                .await
                .map_err(AssetAccessError::repository)?;
            match album {
                Some(attached_album) => Authorization::decide(
                    &caller,
                    &permissions,
                    Action::LibraryRead,
                    &attached_album,
                ),
                None => Authorization::decide_orphan_asset(&caller, &file),
            }
        }
        (None, None, None) => Authorization::decide_orphan_asset(&caller, &file),
    };
    if let Decision::Deny(denial) = decision {
        warn!(
            target: "security",
            event = "authz_failed",
            actor = caller.id(),
            action = %denial.action().verb(),
            target = %denial.action().target(),
            "denied an action"
        );
        return Err(AssetAccessError::NoSuchAsset);
    }

    Ok(AccessedFile {
        audio,
        file,
        image,
        video,
    })
}

/// Resolve `image_id` and decide whether the caller may read its file.
pub(crate) async fn access_image<U>(
    unit_of_work: &mut U,
    caller_id: NumericID,
    image_id: NumericID,
) -> Result<(Image, AccessedFile), AssetAccessError>
where
    U: AssetUnitOfWork + IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    let image = unit_of_work
        .images()
        .search(&ImageFilter {
            image_id: Some(image_id),
            ..ImageFilter::default()
        })
        .await
        .map_err(AssetAccessError::repository)?
        .into_iter()
        .next()
        .ok_or(AssetAccessError::NoSuchAsset)?;
    let accessed = access_file(unit_of_work, caller_id, image.file_id()).await?;
    Ok((image, accessed))
}

/// Resolve `audio_id` and decide whether the caller may read its file.
pub(crate) async fn access_audio<U>(
    unit_of_work: &mut U,
    caller_id: NumericID,
    audio_id: NumericID,
) -> Result<(Audio, AccessedFile), AssetAccessError>
where
    U: AssetUnitOfWork + IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    let audio = unit_of_work
        .audios()
        .search(&AudioFilter {
            id: Some(audio_id),
            ..AudioFilter::default()
        })
        .await
        .map_err(AssetAccessError::repository)?
        .into_iter()
        .next()
        .ok_or(AssetAccessError::NoSuchAsset)?;
    let accessed = access_file(unit_of_work, caller_id, audio.file_id()).await?;
    Ok((audio, accessed))
}

/// Resolve `video_id` and decide whether the caller may read its file.
pub(crate) async fn access_video<U>(
    unit_of_work: &mut U,
    caller_id: NumericID,
    video_id: NumericID,
) -> Result<(Video, AccessedFile), AssetAccessError>
where
    U: AssetUnitOfWork + IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    let video = unit_of_work
        .videos()
        .search(&VideoFilter {
            id: Some(video_id),
            ..VideoFilter::default()
        })
        .await
        .map_err(AssetAccessError::repository)?
        .into_iter()
        .next()
        .ok_or(AssetAccessError::NoSuchAsset)?;
    let accessed = access_file(unit_of_work, caller_id, video.file_id()).await?;
    Ok((video, accessed))
}

/// Return the gallery a gallery item attaches `media` to, if any.
async fn gallery_of<U>(
    unit_of_work: &mut U,
    media: GalleryItemMedia,
) -> Result<Option<Gallery>, AssetAccessError>
where
    U: LibraryUnitOfWork,
{
    let Some(item) = unit_of_work
        .galleries()
        .search_items(&GalleryItemFilter {
            media: Some(media),
            ..GalleryItemFilter::default()
        })
        .await
        .map_err(AssetAccessError::repository)?
        .into_iter()
        .next()
    else {
        return Ok(None);
    };
    let gallery_id = item.item().gallery_id();
    Ok(unit_of_work
        .galleries()
        .search(&GalleryFilter {
            gallery_id: Some(gallery_id),
            ..GalleryFilter::default()
        })
        .await
        .map_err(AssetAccessError::repository)?
        .into_iter()
        .next())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::file::File;
    use crate::domain::model::permission::{Permission, Scope as PermissionScope};
    use crate::domain::port::audio_repository::MockAudioRepository;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::file_repository::MockFileRepository;
    use crate::domain::port::image_repository::MockImageRepository;
    use crate::domain::port::movie_repository::MockMovieRepository;
    use crate::domain::port::music_album_repository::MockMusicAlbumRepository;
    use crate::domain::port::permission_repository::MockPermissionRepository;
    use crate::domain::port::series_repository::MockSeriesRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::domain::port::video_repository::MockVideoRepository;
    use crate::test_helpers::*;
    use chrono::{NaiveDate, NaiveDateTime};
    use std::error::Error;

    fn file(id: NumericID, owner_id: NumericID) -> Result<File, Box<dyn Error>> {
        Ok(File::try_new(
            id,
            format!("{id}.mp4"),
            owner_id,
            false,
            "video/mp4".to_owned(),
            NaiveDate::default(),
            10,
            NaiveDateTime::default(),
            None,
            None,
        )?)
    }

    fn video(id: NumericID, file_id: NumericID) -> Result<Video, Box<dyn Error>> {
        sample_video(id, file_id)
    }

    fn uow_with(
        users: MockUserRepository,
        permissions: MockPermissionRepository,
        files: MockFileRepository,
        images: MockImageRepository,
        audios: MockAudioRepository,
        videos: MockVideoRepository,
        movies: MockMovieRepository,
        albums: MockMusicAlbumRepository,
        series: MockSeriesRepository,
    ) -> TestUow {
        TestUow {
            audios,
            files,
            images,
            movies,
            music_albums: albums,
            permissions,
            series,
            users,
            videos,
            ..TestUow::default()
        }
    }

    fn library_read(scope: PermissionScope) -> Result<Vec<Permission>, Box<dyn Error>> {
        Ok(vec![Permission::try_new(9, Action::LibraryRead, scope)?])
    }

    #[tokio::test]
    async fn a_catalogue_video_follows_the_film_that_names_it() -> Result<(), Box<dyn Error>> {
        // Arrange: video 20 backs film 5.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut permissions = MockPermissionRepository::new();
        let granted = library_read(PermissionScope::All)?;
        permissions
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(granted) }));
        let mut files = MockFileRepository::new();
        let stored_file = file(10, 3)?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let mut images = MockImageRepository::new();
        images
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut audios = MockAudioRepository::new();
        audios
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut videos = MockVideoRepository::new();
        let stored_video = video(20, 10)?;
        videos.expect_search().times(2).returning(move |_| {
            let stored = stored_video.clone();
            Box::pin(async move { Ok(vec![stored]) })
        });
        let mut movies = MockMovieRepository::new();
        let movie = sample_movie(5)?;
        movies
            .expect_find_by_video()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(Some(movie)) }));
        let mut uow = uow_with(
            users,
            permissions,
            files,
            images,
            audios,
            videos,
            movies,
            MockMusicAlbumRepository::new(),
            MockSeriesRepository::new(),
        );

        // Act
        let accessed = access_video(&mut uow, 9, 20).await;

        // Assert
        assert!(accessed.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn a_catalogue_video_is_concealed_from_a_caller_without_library_read()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 9)?;
        let mut permissions = MockPermissionRepository::new();
        let denied = library_read(PermissionScope::None)?;
        permissions
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(denied) }));
        let mut files = MockFileRepository::new();
        let stored_file = file(10, 3)?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let mut images = MockImageRepository::new();
        images
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut audios = MockAudioRepository::new();
        audios
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut videos = MockVideoRepository::new();
        let stored_video = video(20, 10)?;
        videos.expect_search().times(2).returning(move |_| {
            let stored = stored_video.clone();
            Box::pin(async move { Ok(vec![stored]) })
        });
        let mut movies = MockMovieRepository::new();
        let movie = sample_movie(5)?;
        movies
            .expect_find_by_video()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(Some(movie)) }));
        let mut uow = uow_with(
            users,
            permissions,
            files,
            images,
            audios,
            videos,
            movies,
            MockMusicAlbumRepository::new(),
            MockSeriesRepository::new(),
        );

        // Act
        let result = access_video(&mut uow, 9, 20).await;

        // Assert
        assert!(matches!(result, Err(AssetAccessError::NoSuchAsset)));
        Ok(())
    }

    #[tokio::test]
    async fn a_repository_failure_is_an_unknown_access_error() -> Result<(), Box<dyn Error>> {
        // Arrange: the video lookup fails.
        let mut videos = MockVideoRepository::new();
        videos
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let mut uow = uow_with(
            MockUserRepository::new(),
            MockPermissionRepository::new(),
            MockFileRepository::new(),
            MockImageRepository::new(),
            MockAudioRepository::new(),
            videos,
            MockMovieRepository::new(),
            MockMusicAlbumRepository::new(),
            MockSeriesRepository::new(),
        );

        // Act
        let result = access_video(&mut uow, 9, 20).await;

        // Assert
        assert!(matches!(result, Err(AssetAccessError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn an_unattached_audio_is_readable_only_by_its_file_owner() -> Result<(), Box<dyn Error>>
    {
        // Arrange: audio 30 backs a file owned by caller 3, and no recording
        // places it in an album.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 3)?;
        let mut permissions = MockPermissionRepository::new();
        permissions
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut files = MockFileRepository::new();
        let stored_file = File::try_new(
            10,
            "10.mp3".to_owned(),
            3,
            false,
            "audio/mpeg".to_owned(),
            NaiveDate::default(),
            10,
            NaiveDateTime::default(),
            None,
            None,
        )?;
        files
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![stored_file]) }));
        let mut images = MockImageRepository::new();
        images
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut audios = MockAudioRepository::new();
        let stored_audio = Audio::try_new(
            30,
            "Track".to_owned(),
            None,
            "mp3".to_owned(),
            None,
            None,
            None,
            10,
        )?;
        audios.expect_search().times(2).returning(move |_| {
            let stored = stored_audio.clone();
            Box::pin(async move { Ok(vec![stored]) })
        });
        let mut videos = MockVideoRepository::new();
        videos
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_find_album_by_audio()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));
        let mut uow = uow_with(
            users,
            permissions,
            files,
            images,
            audios,
            videos,
            MockMovieRepository::new(),
            albums,
            MockSeriesRepository::new(),
        );

        // Act
        let accessed = access_audio(&mut uow, 3, 30).await;

        // Assert
        assert!(accessed.is_ok());
        Ok(())
    }
}
