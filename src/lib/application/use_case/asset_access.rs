//! Shared read path of the Asset bounded context: resolve a stored asset, decide
//! whether the caller may read it, and hand the use case what it needs.
//!
//! Every asset route goes through this module so the file, image and content
//! representations cannot drift on authorization (`API-044`): an unreadable
//! asset and an unknown asset are one outcome, and an unreadable asset's bytes
//! are never read.

use crate::application::security_event;
use crate::domain::alias::NumericID;
use crate::domain::model::asset::AssetContext;
use crate::domain::model::audio::Audio;
use crate::domain::model::file::File;
use crate::domain::model::gallery::Gallery;
use crate::domain::model::gallery_item::GalleryItemMedia;
use crate::domain::model::image::Image;
use crate::domain::model::permission::Global;
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
use crate::domain::port::music_playlist_repository::{
    MusicPlaylistFilter, MusicPlaylistRepository as _,
};
use crate::domain::port::permission_repository::{PermissionFilter, PermissionRepository as _};
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

/// The medium a file backs, as a gallery item or a playlist cover names it.
#[derive(Debug, Clone, Copy)]
enum Media {
    /// A typed image.
    Image(NumericID),
    /// A typed video.
    Video(NumericID),
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

    let (media, scope) = match (image.as_ref(), video.as_ref()) {
        (Some(image_row), _) => (Some(Media::Image(image_row.image_id())), image_row.scope()),
        (None, Some(video_row)) => (Some(Media::Video(video_row.video_id())), video_row.scope()),
        (None, None) => (None, None),
    };
    let decision = match (media, scope) {
        (Some(_), Some(Scope::Library)) => {
            Authorization::decide_asset(&caller, &permissions, AssetContext::Library, &Global)
        }
        (Some(Media::Image(image_id)), Some(Scope::User)) => {
            if let Some(gallery) =
                gallery_of(unit_of_work, GalleryItemMedia::Image(image_id)).await?
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
                        cover_image_id: Some(image_id),
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
        (Some(Media::Video(video_id)), Some(Scope::User)) => {
            if let Some(gallery) =
                gallery_of(unit_of_work, GalleryItemMedia::Video(video_id)).await?
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
        _ => Authorization::decide_orphan_asset(&caller, &file),
    };
    if let Decision::Deny(denial) = decision {
        security_event::authorization_failed(
            caller.id(),
            denial.action().verb(),
            denial.action().target(),
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
