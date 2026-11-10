use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tracing::{info, warn};

use crate::application::port::{DeleteUserCommand, DeleteUserError, DeleteUserUseCase};
use crate::application::use_case::{media_path, staged_path};
use crate::domain::model::{Action, AssetScope, GalleryItemMedia, Origin, Protected as _};
use crate::domain::port::{
    AssetUnitOfWork, AudioFilter, AudioRepository as _, CredentialFilter,
    CredentialRepository as _, EpisodeFilter, FileFilter, FileRepository as _, FileStorage,
    GalleryFilter, GalleryItemFilter, GalleryRepository as _, IdentityUnitOfWork, ImageFilter,
    ImageRepository as _, LibraryUnitOfWork, MovieFilter, MovieRepository as _, MusicAlbumFilter,
    MusicAlbumRepository as _, MusicPlaylistFilter, MusicPlaylistRepository as _, PermissionFilter,
    PermissionRepository as _, RepositoryError, SeriesRepository as _, UnitOfWork as _,
    UnitOfWorkFactory, UploadFilter, UploadRepository as _, UserFilter, UserRepository as _,
    UserUnitOfWork, VideoFilter, VideoRepository as _,
};
use crate::domain::service::{Authorization, Decision};

/// Use case implementation for deleting a user account and the data it owns.
pub(crate) struct DeleteUser<F, S> {
    /// Storage adapter removing the bytes of the files the deletion removed.
    file_storage: Arc<S>,
    /// Factory opening the unit of work wrapping the deletion.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, S: FileStorage> DeleteUser<F, S> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>, file_storage: Arc<S>) -> Self {
        Self {
            file_storage,
            unit_of_work_factory,
        }
    }
}

impl<F, S> DeleteUserUseCase for DeleteUser<F, S>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork + IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
    S: FileStorage,
{
    fn execute<'future>(
        &'future self,
        command: DeleteUserCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), DeleteUserError>> + Send + 'future>> {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);
        let file_storage = Arc::clone(&self.file_storage);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| DeleteUserError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| DeleteUserError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(DeleteUserError::NoSuchCaller)?;

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| DeleteUserError::Unknown(error.into()))?;

                let target = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.user_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| DeleteUserError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(DeleteUserError::NoSuchUser)?;

                // The Root Admin's own account is never deletable. Checking the
                // identifier before the permission keeps the documented `403`
                // for every caller (`design.md`, decision 8).
                if target.id() == 0 {
                    return Err(DeleteUserError::TargetNotModifiable);
                }
                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::UserDelete, &target)
                {
                    warn!(
                        target: "security",
                        event = "authz_failed",
                        actor = caller.id(),
                        action = %denial.action().verb(),
                        target = %denial.action().target(),
                        "denied an action"
                    );
                    // The relation guard refuses an Admin target for every
                    // non-root caller; the scope decision refuses every other
                    // caller without `user.delete`.
                    return Err(
                        if target.is_privileged_account() && !caller.is_root_admin() {
                            DeleteUserError::TargetNotModifiable
                        } else {
                            DeleteUserError::NotAdmin
                        },
                    );
                }

                let mut removed_paths = Vec::new();

                unit_of_work
                    .permissions()
                    .delete(&PermissionFilter {
                        user_id: Some(target.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| DeleteUserError::Unknown(error.into()))?;

                // Owned galleries and playlists go first: once they are gone,
                // every remaining referrer of a media row is necessarily
                // someone else's (`design.md`, decision 4).
                let owned_galleries = unit_of_work
                    .galleries()
                    .search(&GalleryFilter {
                        user_id: Some(target.id()),
                        ..GalleryFilter::default()
                    })
                    .await
                    .map_err(|error| DeleteUserError::Unknown(error.into()))?;
                for gallery in owned_galleries {
                    unit_of_work
                        .galleries()
                        .delete(&GalleryFilter {
                            gallery_id: Some(gallery.gallery_id()),
                            ..GalleryFilter::default()
                        })
                        .await
                        .map_err(|error| DeleteUserError::Unknown(error.into()))?;
                }

                let owned_playlists = unit_of_work
                    .music_playlists()
                    .search(&MusicPlaylistFilter {
                        user_id: Some(target.id()),
                        ..MusicPlaylistFilter::default()
                    })
                    .await
                    .map_err(|error| DeleteUserError::Unknown(error.into()))?;
                for playlist in owned_playlists {
                    unit_of_work
                        .music_playlists()
                        .delete(&MusicPlaylistFilter {
                            id: Some(playlist.id()),
                            ..MusicPlaylistFilter::default()
                        })
                        .await
                        .map_err(|error| DeleteUserError::Unknown(error.into()))?;
                }

                let owned_uploads = unit_of_work
                    .uploads()
                    .search(&UploadFilter {
                        user_id: Some(target.id()),
                        ..UploadFilter::default()
                    })
                    .await
                    .map_err(|error| DeleteUserError::Unknown(error.into()))?;
                for upload in owned_uploads {
                    unit_of_work
                        .uploads()
                        .delete(&UploadFilter {
                            upload_id: Some(upload.upload_id()),
                            ..UploadFilter::default()
                        })
                        .await
                        .map_err(|error| DeleteUserError::Unknown(error.into()))?;
                    removed_paths.push(staged_path(upload.upload_id()));
                }

                let owned_files = unit_of_work
                    .files()
                    .search(&FileFilter {
                        user_id: Some(target.id()),
                        ..FileFilter::default()
                    })
                    .await
                    .map_err(|error| DeleteUserError::Unknown(error.into()))?;
                for file in owned_files {
                    let image_row = unit_of_work
                        .images()
                        .search(&ImageFilter {
                            file_id: Some(file.id()),
                            ..ImageFilter::default()
                        })
                        .await
                        .map_err(|error| DeleteUserError::Unknown(error.into()))?
                        .into_iter()
                        .next();
                    let video_row = unit_of_work
                        .videos()
                        .search(&VideoFilter {
                            file_id: Some(file.id()),
                            ..VideoFilter::default()
                        })
                        .await
                        .map_err(|error| DeleteUserError::Unknown(error.into()))?
                        .into_iter()
                        .next();
                    let audio_row = unit_of_work
                        .audios()
                        .search(&AudioFilter {
                            file_id: Some(file.id()),
                            ..AudioFilter::default()
                        })
                        .await
                        .map_err(|error| DeleteUserError::Unknown(error.into()))?
                        .into_iter()
                        .next();

                    // Classify the file: a media row another user's resource or
                    // the catalogue still references is shared; every other
                    // row is private to the deleted account (`design.md`,
                    // decision 5). The account's own galleries and playlists
                    // are already gone, so a surviving referrer belongs to
                    // another caller.
                    let mut shared = false;
                    if let Some(image) = image_row.as_ref() {
                        match image.scope() {
                            Some(AssetScope::Library) => shared = true,
                            Some(AssetScope::User) => {
                                let in_gallery = !unit_of_work
                                    .galleries()
                                    .search_items(&GalleryItemFilter {
                                        media: Some(GalleryItemMedia::Image(image.image_id())),
                                        ..GalleryItemFilter::default()
                                    })
                                    .await
                                    .map_err(|error| DeleteUserError::Unknown(error.into()))?
                                    .is_empty();
                                let as_cover = !unit_of_work
                                    .music_playlists()
                                    .search(&MusicPlaylistFilter {
                                        cover_image_id: Some(image.image_id()),
                                        ..MusicPlaylistFilter::default()
                                    })
                                    .await
                                    .map_err(|error| DeleteUserError::Unknown(error.into()))?
                                    .is_empty();
                                shared = in_gallery || as_cover;
                            }
                            None => {}
                        }
                    }
                    if !shared && let Some(video) = video_row.as_ref() {
                        match video.scope() {
                            Some(AssetScope::Library) => shared = true,
                            Some(AssetScope::User) => {
                                shared = !unit_of_work
                                    .galleries()
                                    .search_items(&GalleryItemFilter {
                                        media: Some(GalleryItemMedia::Video(video.id())),
                                        ..GalleryItemFilter::default()
                                    })
                                    .await
                                    .map_err(|error| DeleteUserError::Unknown(error.into()))?
                                    .is_empty();
                            }
                            None => {
                                let in_film = !unit_of_work
                                    .movies()
                                    .search(&MovieFilter {
                                        video_id: Some(video.id()),
                                        ..MovieFilter::default()
                                    })
                                    .await
                                    .map_err(|error| DeleteUserError::Unknown(error.into()))?
                                    .is_empty();
                                let in_episode = !unit_of_work
                                    .series()
                                    .search_episodes(&EpisodeFilter {
                                        video_id: Some(video.id()),
                                        ..EpisodeFilter::default()
                                    })
                                    .await
                                    .map_err(|error| DeleteUserError::Unknown(error.into()))?
                                    .is_empty();
                                shared = in_film || in_episode;
                            }
                        }
                    }
                    if !shared && let Some(audio) = audio_row.as_ref() {
                        shared = !unit_of_work
                            .music_albums()
                            .search(&MusicAlbumFilter {
                                audio_id: Some(audio.id()),
                                ..MusicAlbumFilter::default()
                            })
                            .await
                            .map_err(|error| DeleteUserError::Unknown(error.into()))?
                            .is_empty();
                    }

                    if shared {
                        // Transfer the file to the Root Admin. A digest the
                        // Root Admin already owns is cleared first, because
                        // `uq_file_user_id_integrity_hash` ignores a null
                        // digest (`design.md`, decision 6).
                        if let Some(digest) = file.integrity_hash() {
                            let collision = unit_of_work
                                .files()
                                .search(&FileFilter {
                                    integrity_hash: Some(digest.as_str().to_owned()),
                                    user_id: Some(0),
                                    ..FileFilter::default()
                                })
                                .await
                                .map_err(|error| DeleteUserError::Unknown(error.into()))?;
                            if !collision.is_empty() {
                                unit_of_work
                                    .files()
                                    .clear_integrity_hash(file.id())
                                    .await
                                    .map_err(|error| DeleteUserError::Unknown(error.into()))?;
                            }
                        }
                        unit_of_work
                            .files()
                            .reassign_owner(file.id(), 0)
                            .await
                            .map_err(|error| DeleteUserError::Unknown(error.into()))?;
                    } else {
                        if let Some(image) = image_row.as_ref() {
                            unit_of_work
                                .images()
                                .delete(&ImageFilter {
                                    image_id: Some(image.image_id()),
                                    ..ImageFilter::default()
                                })
                                .await
                                .map_err(|error| DeleteUserError::Unknown(error.into()))?;
                        }
                        if let Some(video) = video_row.as_ref() {
                            unit_of_work
                                .videos()
                                .delete(&VideoFilter {
                                    id: Some(video.id()),
                                    ..VideoFilter::default()
                                })
                                .await
                                .map_err(|error| DeleteUserError::Unknown(error.into()))?;
                        }
                        if let Some(audio) = audio_row.as_ref() {
                            unit_of_work
                                .audios()
                                .delete(&AudioFilter {
                                    id: Some(audio.id()),
                                    ..AudioFilter::default()
                                })
                                .await
                                .map_err(|error| DeleteUserError::Unknown(error.into()))?;
                        }
                        let deleted = unit_of_work
                            .files()
                            .delete(&FileFilter {
                                id: Some(file.id()),
                                ..FileFilter::default()
                            })
                            .await
                            .map_err(|error| DeleteUserError::Unknown(error.into()))?;
                        // A scanned file is the operator's content, so only a
                        // file the server stored queues its bytes for removal.
                        if deleted && file.origin() == Origin::Uploaded {
                            removed_paths.push(media_path(file.path()));
                        }
                    }
                }

                let deleted = unit_of_work
                    .users()
                    .delete(&UserFilter {
                        id: Some(target.id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| {
                        if matches!(error, RepositoryError::Conflict) {
                            DeleteUserError::TargetNotModifiable
                        } else {
                            DeleteUserError::Unknown(error.into())
                        }
                    })?;
                if !deleted {
                    return Err(DeleteUserError::NoSuchUser);
                }

                // `user.credential_id` references the credential, so the user
                // row goes first; the reverse order violates the foreign key.
                unit_of_work
                    .credentials()
                    .delete(&CredentialFilter {
                        id: Some(target.credential_id()),
                        ..CredentialFilter::default()
                    })
                    .await
                    .map_err(|error| DeleteUserError::Unknown(error.into()))?;

                Ok(removed_paths)
            }
            .await;

            match result {
                Ok(removed_paths) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| DeleteUserError::Unknown(error.into()))?;
                    info!(
                        target: "security",
                        event = "user_admin",
                        actor = command.caller_id(),
                        action = %"delete",
                        target = %"user",
                        "changed a user-administration object"
                    );
                    // The bytes are removed only once the rows are committed,
                    // and a failure never fails the request: the file-storage
                    // adapter logs the fault (`OBS-002`).
                    for path in removed_paths {
                        if file_storage.delete_upload_file(&path).await.is_err() {
                            // The adapter owns the failure log (`OBS-002`).
                        }
                    }
                    Ok(())
                }
                Err(error) => {
                    // The unit-of-work adapter owns the rollback-failure log (OBS-002).
                    let _rollback = unit_of_work.rollback().await;
                    Err(error)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::{
        Audio, ChunkBitmap, File, Gallery, GalleryItem, Image, IntegrityHash, MusicPlaylist,
        Orientation, Permission, Role, ScanType, Scope, Upload, User, Video,
    };
    use crate::domain::port::{
        GalleryItemDetail, MockFileStorage, MockPermissionRepository, MockUserRepository,
        StorageError,
    };
    use crate::test_helpers::*;
    use chrono::{NaiveDate, NaiveDateTime};
    use std::error::Error;
    use std::sync::atomic::{AtomicBool, Ordering};

    type UseCase = DeleteUser<TestFactory, MockFileStorage>;

    /// The media rows a shared-file scenario backs.
    #[derive(Clone)]
    enum SharedMedia {
        /// The file backs an audio referenced by an album.
        Audio(Audio),
        /// The file backs an image referenced by another gallery.
        Image(Image),
        /// The file backs a video referenced by a film.
        Video(Video),
    }

    /// The media rows a private-file scenario backs.
    enum PrivateMedia {
        /// The file backs an audio row.
        Audio(Audio),
        /// The file backs an image row.
        Image(Image),
        /// The file backs no media row.
        Orphan,
        /// The file backs a video row.
        Video(Video),
    }

    /// A use case under test together with its transaction-lifecycle flags.
    struct Harness {
        /// Set when the unit of work is committed.
        committed: Arc<AtomicBool>,
        /// Set when the unit of work is rolled back.
        rolled_back: Arc<AtomicBool>,
        /// The use case under test.
        use_case: UseCase,
    }

    fn harness(unit_of_work: TestUnitOfWork, file_storage: MockFileStorage) -> Harness {
        let harness = GalleryFactoryHarness::new(unit_of_work);
        Harness {
            committed: Arc::clone(&harness.committed),
            rolled_back: Arc::clone(&harness.rolled_back),
            use_case: DeleteUser::new(Arc::clone(&harness.factory), Arc::new(file_storage)),
        }
    }

    fn user(id: i64, role: Role) -> Result<User, Box<dyn Error>> {
        Ok(User::try_new(
            id,
            format!("user{id}"),
            None,
            id.max(1),
            role,
        )?)
    }

    fn file(id: i64, user_id: i64, path: &str) -> Result<File, Box<dyn Error>> {
        Ok(File::try_new(
            id,
            path.to_owned(),
            user_id,
            false,
            "image/png".to_owned(),
            NaiveDate::default(),
            10,
            NaiveDateTime::default(),
            Some(IntegrityHash::try_new(DIGEST.to_owned())?),
            Some(Origin::Uploaded),
        )?)
    }

    fn image(
        image_id: i64,
        file_id: i64,
        scope: Option<AssetScope>,
    ) -> Result<Image, Box<dyn Error>> {
        Ok(Image::try_new(
            image_id,
            "art".to_owned(),
            10,
            10,
            Orientation::Square,
            NaiveDate::default(),
            file_id,
            scope,
        )?)
    }

    fn video(
        video_id: i64,
        file_id: i64,
        scope: Option<AssetScope>,
    ) -> Result<Video, Box<dyn Error>> {
        Ok(Video::try_new(
            video_id,
            1_000.0,
            "h264".to_owned(),
            30,
            640,
            480,
            "sRGB".to_owned(),
            ScanType::Progressive,
            file_id,
            scope,
        )?)
    }

    fn audio(audio_id: i64, file_id: i64) -> Result<Audio, Box<dyn Error>> {
        Ok(Audio::try_new(
            audio_id,
            "track".to_owned(),
            Some(1_000.0),
            "flac".to_owned(),
            Some(44_100.0),
            Some(16),
            Some(4_096),
            file_id,
        )?)
    }

    fn upload(upload_id: i64, user_id: i64) -> Result<Upload, Box<dyn Error>> {
        Ok(Upload::try_new(
            upload_id,
            user_id,
            "song.flac".to_owned(),
            8,
            "audio/flac".to_owned(),
            4,
            IntegrityHash::try_new(DIGEST.to_owned())?,
            ChunkBitmap::from_bytes(vec![0], 2)?,
            false,
            false,
            None,
            NaiveDateTime::default(),
        )?)
    }

    fn gallery(gallery_id: i64, user_id: i64) -> Gallery {
        Gallery::new(
            gallery_id,
            Some(user_id),
            "Gallery".to_owned(),
            false,
            NaiveDateTime::default(),
            NaiveDateTime::default(),
        )
    }

    fn playlist(id: i64, user_id: i64) -> MusicPlaylist {
        MusicPlaylist::new(
            id,
            "Mix".to_owned(),
            NaiveDateTime::default(),
            NaiveDateTime::default(),
            user_id,
            false,
            None,
            Vec::new(),
        )
    }

    fn item_detail(
        gallery_item_id: i64,
        gallery_id: i64,
        image_id: i64,
        file_id: i64,
    ) -> Result<GalleryItemDetail, Box<dyn Error>> {
        Ok(GalleryItemDetail::new(
            GalleryItem::try_new(
                gallery_item_id,
                gallery_id,
                Some(image_id),
                None,
                gallery_item_id,
                NaiveDateTime::default(),
            )?,
            file_id,
            "item".to_owned(),
        ))
    }

    /// Expect the caller load and then the target load.
    fn expect_actor(users: &mut MockUserRepository, caller: User, target: Option<User>) {
        let caller_id = caller.id();
        users.expect_search().times(2).returning(move |filter| {
            let found = if filter.id == Some(caller_id) {
                Some(caller.clone())
            } else {
                target
                    .clone()
                    .filter(|candidate| filter.id == Some(candidate.id()))
            };
            Box::pin(async move { Ok(found.into_iter().collect()) })
        });
    }

    /// Expect the permission overrides of `caller_id` to be loaded empty.
    fn expect_permission_load(permissions: &mut MockPermissionRepository, caller_id: i64) {
        permissions
            .expect_search()
            .times(1)
            .withf(move |filter| filter.user_id == Some(caller_id))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
    }

    /// Expect the target's permission overrides to be removed.
    fn expect_permission_cleanup(permissions: &mut MockPermissionRepository, target_id: i64) {
        permissions
            .expect_delete()
            .times(1)
            .withf(move |filter| filter.user_id == Some(target_id))
            .returning(|_| Box::pin(async { Ok(true) }));
    }

    /// Expect every owned collection to resolve empty.
    fn expect_no_owned_content(unit_of_work: &mut TestUnitOfWork) {
        unit_of_work
            .galleries
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        unit_of_work
            .music_playlists
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        unit_of_work
            .uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        unit_of_work
            .files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
    }

    /// Expect the user row and its credential to be removed.
    fn expect_identity_removal(unit_of_work: &mut TestUnitOfWork, target: &User) {
        let target_id = target.id();
        unit_of_work
            .users
            .expect_delete()
            .times(1)
            .withf(move |filter| filter.id == Some(target_id))
            .returning(|_| Box::pin(async { Ok(true) }));
        let credential_id = target.credential_id();
        unit_of_work
            .credentials
            .expect_delete()
            .times(1)
            .withf(move |filter| filter.id == Some(credential_id))
            .returning(|_| Box::pin(async { Ok(true) }));
    }

    /// Build a unit of work with the caller, target and permission cleanup
    /// stubbed, leaving the owned collections and identity removal to the test.
    fn bare_unit_of_work(caller: &User, target: &User) -> TestUnitOfWork {
        let mut users = MockUserRepository::new();
        expect_actor(&mut users, caller.clone(), Some(target.clone()));
        let mut permissions = MockPermissionRepository::new();
        expect_permission_load(&mut permissions, caller.id());
        expect_permission_cleanup(&mut permissions, target.id());
        TestUnitOfWork {
            permissions,
            users,
            ..TestUnitOfWork::default()
        }
    }

    /// Build a unit of work for a deletion that removes no owned content.
    fn plain_unit_of_work(caller: &User, target: &User) -> TestUnitOfWork {
        let mut unit_of_work = bare_unit_of_work(caller, target);
        expect_no_owned_content(&mut unit_of_work);
        expect_identity_removal(&mut unit_of_work, target);
        unit_of_work
    }

    #[tokio::test]
    async fn delete_user_admin_deletes_a_standard_user() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(7, Role::Admin)?;
        let target = user(8, Role::Standard)?;
        let harness = harness(plain_unit_of_work(&caller, &target), MockFileStorage::new());

        // Act
        let result = harness
            .use_case
            .execute(DeleteUserCommand::new(caller.id(), target.id()))
            .await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_root_deletes_an_admin() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(0, Role::Admin)?;
        let target = user(8, Role::Admin)?;
        let harness = harness(plain_unit_of_work(&caller, &target), MockFileStorage::new());

        // Act
        let result = harness
            .use_case
            .execute(DeleteUserCommand::new(caller.id(), target.id()))
            .await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_standard_caller_returns_not_admin() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(3, Role::Standard)?;
        let target = user(8, Role::Standard)?;
        let mut users = MockUserRepository::new();
        expect_actor(&mut users, caller.clone(), Some(target.clone()));
        let mut permissions = MockPermissionRepository::new();
        expect_permission_load(&mut permissions, caller.id());
        let unit_of_work = TestUnitOfWork {
            permissions,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = harness(unit_of_work, MockFileStorage::new());

        // Act
        let result = harness
            .use_case
            .execute(DeleteUserCommand::new(caller.id(), target.id()))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteUserError::NotAdmin)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_standard_with_override_deletes_a_standard_user()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(3, Role::Standard)?;
        let target = user(8, Role::Standard)?;
        let mut users = MockUserRepository::new();
        expect_actor(&mut users, caller.clone(), Some(target.clone()));
        let mut permissions = MockPermissionRepository::new();
        let granted = Permission::try_new(caller.id(), Action::UserDelete, Scope::All)?;
        permissions
            .expect_search()
            .times(1)
            .withf(move |filter| filter.user_id == Some(3))
            .return_once(move |_| Box::pin(async move { Ok(vec![granted]) }));
        expect_permission_cleanup(&mut permissions, target.id());
        let mut unit_of_work = TestUnitOfWork {
            permissions,
            users,
            ..TestUnitOfWork::default()
        };
        expect_no_owned_content(&mut unit_of_work);
        expect_identity_removal(&mut unit_of_work, &target);
        let harness = harness(unit_of_work, MockFileStorage::new());

        // Act
        let result = harness
            .use_case
            .execute(DeleteUserCommand::new(caller.id(), target.id()))
            .await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_admin_target_from_an_admin_returns_target_not_modifiable()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(7, Role::Admin)?;
        let target = user(8, Role::Admin)?;
        let mut users = MockUserRepository::new();
        expect_actor(&mut users, caller.clone(), Some(target.clone()));
        let mut permissions = MockPermissionRepository::new();
        expect_permission_load(&mut permissions, caller.id());
        let unit_of_work = TestUnitOfWork {
            permissions,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = harness(unit_of_work, MockFileStorage::new());

        // Act
        let result = harness
            .use_case
            .execute(DeleteUserCommand::new(caller.id(), target.id()))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteUserError::TargetNotModifiable)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_admin_deleting_itself_returns_target_not_modifiable()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(7, Role::Admin)?;
        let target = user(7, Role::Admin)?;
        let mut users = MockUserRepository::new();
        expect_actor(&mut users, caller.clone(), Some(target.clone()));
        let mut permissions = MockPermissionRepository::new();
        expect_permission_load(&mut permissions, caller.id());
        let unit_of_work = TestUnitOfWork {
            permissions,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = harness(unit_of_work, MockFileStorage::new());

        // Act
        let result = harness
            .use_case
            .execute(DeleteUserCommand::new(caller.id(), target.id()))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteUserError::TargetNotModifiable)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_root_target_returns_target_not_modifiable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(0, Role::Admin)?;
        let target = user(0, Role::Admin)?;
        let mut users = MockUserRepository::new();
        expect_actor(&mut users, caller.clone(), Some(target.clone()));
        let mut permissions = MockPermissionRepository::new();
        expect_permission_load(&mut permissions, caller.id());
        let unit_of_work = TestUnitOfWork {
            permissions,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = harness(unit_of_work, MockFileStorage::new());

        // Act
        let result = harness
            .use_case
            .execute(DeleteUserCommand::new(caller.id(), target.id()))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteUserError::TargetNotModifiable)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_unknown_target_returns_no_such_user() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(7, Role::Admin)?;
        let mut users = MockUserRepository::new();
        expect_actor(&mut users, caller.clone(), None);
        let mut permissions = MockPermissionRepository::new();
        expect_permission_load(&mut permissions, caller.id());
        let unit_of_work = TestUnitOfWork {
            permissions,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = harness(unit_of_work, MockFileStorage::new());

        // Act
        let result = harness
            .use_case
            .execute(DeleteUserCommand::new(caller.id(), 999))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteUserError::NoSuchUser)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let unit_of_work = TestUnitOfWork {
            users,
            ..TestUnitOfWork::default()
        };
        let harness = harness(unit_of_work, MockFileStorage::new());

        // Act
        let result = harness
            .use_case
            .execute(DeleteUserCommand::new(999, 8))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteUserError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_identity_removal_failure_rolls_back() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(7, Role::Admin)?;
        let target = user(8, Role::Standard)?;
        let mut unit_of_work = bare_unit_of_work(&caller, &target);
        expect_no_owned_content(&mut unit_of_work);
        unit_of_work
            .users
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness(unit_of_work, MockFileStorage::new());

        // Act
        let result = harness
            .use_case
            .execute(DeleteUserCommand::new(caller.id(), target.id()))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteUserError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        assert!(!harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_removes_owned_galleries_playlists_and_uploads()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(7, Role::Admin)?;
        let target = user(8, Role::Standard)?;
        let mut unit_of_work = bare_unit_of_work(&caller, &target);
        unit_of_work
            .files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        expect_identity_removal(&mut unit_of_work, &target);
        let galleries = vec![gallery(2, target.id()), gallery(3, target.id())];
        unit_of_work
            .galleries
            .expect_search()
            .times(1)
            .withf(move |filter| filter.user_id == Some(8))
            .return_once(move |_| Box::pin(async move { Ok(galleries) }));
        unit_of_work
            .galleries
            .expect_delete()
            .times(2)
            .returning(|_| Box::pin(async { Ok(true) }));
        let playlists = vec![playlist(4, target.id())];
        unit_of_work
            .music_playlists
            .expect_search()
            .times(1)
            .withf(move |filter| filter.user_id == Some(8))
            .return_once(move |_| Box::pin(async move { Ok(playlists) }));
        unit_of_work
            .music_playlists
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let uploads = vec![upload(9, target.id())?];
        unit_of_work
            .uploads
            .expect_search()
            .times(1)
            .withf(move |filter| filter.user_id == Some(8))
            .return_once(move |_| Box::pin(async move { Ok(uploads) }));
        unit_of_work
            .uploads
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut storage = MockFileStorage::new();
        storage
            .expect_delete_upload_file()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        let harness = harness(unit_of_work, storage);

        // Act
        let result = harness
            .use_case
            .execute(DeleteUserCommand::new(caller.id(), target.id()))
            .await;

        // Assert: the ownership rows and the staged content are removed.
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    /// Expect every owned collection of the target to resolve empty.
    fn expect_empty_owned(unit_of_work: &mut TestUnitOfWork, target_id: i64) {
        unit_of_work
            .galleries
            .expect_search()
            .times(1)
            .withf(move |filter| filter.user_id == Some(target_id))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        unit_of_work
            .music_playlists
            .expect_search()
            .times(1)
            .withf(move |filter| filter.user_id == Some(target_id))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        unit_of_work
            .uploads
            .expect_search()
            .times(1)
            .withf(move |filter| filter.user_id == Some(target_id))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
    }

    /// Run a deletion over a single private file backing `media`, returning the
    /// harness and the result.
    async fn delete_file_assuming_private(media: PrivateMedia) -> Result<Harness, Box<dyn Error>> {
        let caller = user(7, Role::Admin)?;
        let target = user(8, Role::Standard)?;
        let mut unit_of_work = bare_unit_of_work(&caller, &target);
        expect_identity_removal(&mut unit_of_work, &target);
        expect_empty_owned(&mut unit_of_work, target.id());
        let stored = file(100, target.id(), "1_photo.png")?;
        let file_id = stored.id();
        unit_of_work
            .files
            .expect_search()
            .times(1)
            .withf(move |filter| filter.user_id == Some(8))
            .return_once(move |_| Box::pin(async move { Ok(vec![stored]) }));
        unit_of_work
            .files
            .expect_delete()
            .times(1)
            .withf(move |filter| filter.id == Some(file_id))
            .returning(|_| Box::pin(async { Ok(true) }));

        match media {
            PrivateMedia::Image(image_value) => {
                let image_id = image_value.image_id();
                unit_of_work
                    .images
                    .expect_search()
                    .times(1)
                    .return_once(move |_| Box::pin(async move { Ok(vec![image_value]) }));
                unit_of_work
                    .videos
                    .expect_search()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                unit_of_work
                    .audios
                    .expect_search()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                unit_of_work
                    .galleries
                    .expect_search_items()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                unit_of_work
                    .music_playlists
                    .expect_search()
                    .times(1)
                    .withf(move |filter| filter.cover_image_id == Some(image_id))
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                unit_of_work
                    .images
                    .expect_delete()
                    .times(1)
                    .withf(move |filter| filter.image_id == Some(image_id))
                    .returning(|_| Box::pin(async { Ok(true) }));
            }
            PrivateMedia::Video(video_value) => {
                let video_id = video_value.id();
                unit_of_work
                    .images
                    .expect_search()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                unit_of_work
                    .videos
                    .expect_search()
                    .times(1)
                    .return_once(move |_| Box::pin(async move { Ok(vec![video_value]) }));
                unit_of_work
                    .audios
                    .expect_search()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                unit_of_work
                    .movies
                    .expect_search()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                unit_of_work
                    .series
                    .expect_search_episodes()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                unit_of_work
                    .videos
                    .expect_delete()
                    .times(1)
                    .withf(move |filter| filter.id == Some(video_id))
                    .returning(|_| Box::pin(async { Ok(true) }));
            }
            PrivateMedia::Audio(audio_value) => {
                let audio_id = audio_value.id();
                unit_of_work
                    .images
                    .expect_search()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                unit_of_work
                    .videos
                    .expect_search()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                unit_of_work
                    .audios
                    .expect_search()
                    .times(1)
                    .return_once(move |_| Box::pin(async move { Ok(vec![audio_value]) }));
                unit_of_work
                    .music_albums
                    .expect_search()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                unit_of_work
                    .audios
                    .expect_delete()
                    .times(1)
                    .withf(move |filter| filter.id == Some(audio_id))
                    .returning(|_| Box::pin(async { Ok(true) }));
            }
            PrivateMedia::Orphan => {
                unit_of_work
                    .images
                    .expect_search()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                unit_of_work
                    .videos
                    .expect_search()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
                unit_of_work
                    .audios
                    .expect_search()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            }
        }

        let mut storage = MockFileStorage::new();
        storage
            .expect_delete_upload_file()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        let harness = harness(unit_of_work, storage);
        harness
            .use_case
            .execute(DeleteUserCommand::new(caller.id(), target.id()))
            .await?;
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(harness)
    }

    #[tokio::test]
    async fn delete_user_removes_a_private_image() -> Result<(), Box<dyn Error>> {
        // Act & Assert
        delete_file_assuming_private(PrivateMedia::Image(image(7, 100, Some(AssetScope::User))?))
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_removes_a_private_video() -> Result<(), Box<dyn Error>> {
        // Act & Assert
        delete_file_assuming_private(PrivateMedia::Video(video(7, 100, None)?)).await?;
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_removes_a_private_audio() -> Result<(), Box<dyn Error>> {
        // Act & Assert
        delete_file_assuming_private(PrivateMedia::Audio(audio(7, 100)?)).await?;
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_removes_an_orphan_file() -> Result<(), Box<dyn Error>> {
        // Act & Assert
        delete_file_assuming_private(PrivateMedia::Orphan).await?;
        Ok(())
    }

    /// Run a deletion over a single shared file backing `media`.
    async fn delete_file_assuming_shared(
        media: SharedMedia,
        collision: bool,
    ) -> Result<Harness, Box<dyn Error>> {
        let caller = user(7, Role::Admin)?;
        let target = user(8, Role::Standard)?;
        let mut unit_of_work = bare_unit_of_work(&caller, &target);
        expect_identity_removal(&mut unit_of_work, &target);
        expect_empty_owned(&mut unit_of_work, target.id());
        let stored = file(100, target.id(), "1_shared.png")?;
        let file_id = stored.id();
        unit_of_work
            .files
            .expect_search()
            .times(1)
            .withf(move |filter| filter.user_id == Some(8))
            .return_once(move |_| Box::pin(async move { Ok(vec![stored]) }));
        let root_copy = file(200, 0, "1_root.png")?;
        unit_of_work
            .files
            .expect_search()
            .times(1)
            .withf(|filter| filter.user_id == Some(0) && filter.integrity_hash.is_some())
            .return_once(move |_| {
                let found = if collision {
                    vec![root_copy]
                } else {
                    Vec::new()
                };
                Box::pin(async move { Ok(found) })
            });
        unit_of_work
            .files
            .expect_reassign_owner()
            .times(1)
            .withf(move |id, owner| *id == file_id && *owner == 0)
            .returning(|_, _| Box::pin(async { Ok(true) }));
        unit_of_work
            .files
            .expect_clear_integrity_hash()
            .times(usize::from(collision))
            .returning(|_| Box::pin(async { Ok(true) }));

        let (image_value, video_value, audio_value) = match media.clone() {
            SharedMedia::Image(value) => (Some(value), None, None),
            SharedMedia::Video(value) => (None, Some(value), None),
            SharedMedia::Audio(value) => (None, None, Some(value)),
        };
        unit_of_work
            .images
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(image_value.into_iter().collect()) }));
        unit_of_work
            .videos
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(video_value.into_iter().collect()) }));
        unit_of_work
            .audios
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(audio_value.into_iter().collect()) }));

        match media.clone() {
            SharedMedia::Image(value) => {
                let image_id = value.image_id();
                let detail = item_detail(1, 2, image_id, 100)?;
                unit_of_work
                    .galleries
                    .expect_search_items()
                    .times(1)
                    .withf(move |filter| filter.media == Some(GalleryItemMedia::Image(image_id)))
                    .return_once(move |_| {
                        let found = detail.clone();
                        Box::pin(async move { Ok(vec![found]) })
                    });
                unit_of_work
                    .music_playlists
                    .expect_search()
                    .times(1)
                    .withf(move |filter| filter.cover_image_id == Some(image_id))
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            }
            SharedMedia::Video(_) => {
                let film = sample_movie(5)?;
                unit_of_work
                    .movies
                    .expect_search()
                    .times(1)
                    .return_once(move |_| Box::pin(async move { Ok(vec![film]) }));
                unit_of_work
                    .series
                    .expect_search_episodes()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            }
            SharedMedia::Audio(_) => {
                let album = sample_album(5, Vec::new())?;
                unit_of_work
                    .music_albums
                    .expect_search()
                    .times(1)
                    .return_once(move |_| Box::pin(async move { Ok(vec![album]) }));
            }
        }

        // The private-file path must not run.
        unit_of_work.files.expect_delete().times(0);
        unit_of_work.images.expect_delete().times(0);
        unit_of_work.videos.expect_delete().times(0);
        unit_of_work.audios.expect_delete().times(0);
        let mut storage = MockFileStorage::new();
        storage.expect_delete_upload_file().times(0);

        let harness = harness(unit_of_work, storage);
        harness
            .use_case
            .execute(DeleteUserCommand::new(caller.id(), target.id()))
            .await?;
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(harness)
    }

    #[tokio::test]
    async fn delete_user_transfers_an_image_in_another_users_gallery() -> Result<(), Box<dyn Error>>
    {
        // Act & Assert
        delete_file_assuming_shared(
            SharedMedia::Image(image(7, 100, Some(AssetScope::User))?),
            false,
        )
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_transfers_shared_media_and_clears_a_colliding_digest()
    -> Result<(), Box<dyn Error>> {
        // Act & Assert
        delete_file_assuming_shared(
            SharedMedia::Image(image(7, 100, Some(AssetScope::User))?),
            true,
        )
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_transfers_a_video_backing_a_film() -> Result<(), Box<dyn Error>> {
        // Act & Assert
        let harness =
            delete_file_assuming_shared(SharedMedia::Video(video(7, 100, None)?), false).await?;
        // The transfer succeeds and no bytes are removed.
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_transfers_an_audio_backing_an_album() -> Result<(), Box<dyn Error>> {
        // Act & Assert
        delete_file_assuming_shared(SharedMedia::Audio(audio(7, 100)?), false).await?;
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_byte_removal_failure_still_succeeds() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(7, Role::Admin)?;
        let target = user(8, Role::Standard)?;
        let mut unit_of_work = bare_unit_of_work(&caller, &target);
        expect_identity_removal(&mut unit_of_work, &target);
        expect_empty_owned(&mut unit_of_work, target.id());
        let stored = file(100, target.id(), "1_photo.png")?;
        unit_of_work
            .files
            .expect_search()
            .times(1)
            .withf(move |filter| filter.user_id == Some(8))
            .return_once(move |_| Box::pin(async move { Ok(vec![stored]) }));
        unit_of_work
            .files
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        unit_of_work
            .images
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        unit_of_work
            .videos
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        unit_of_work
            .audios
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut storage = MockFileStorage::new();
        storage
            .expect_delete_upload_file()
            .times(1)
            .returning(|_| Box::pin(async { Err(StorageError::Unavailable) }));
        let harness = harness(unit_of_work, storage);

        // Act
        let result = harness
            .use_case
            .execute(DeleteUserCommand::new(caller.id(), target.id()))
            .await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(7, Role::Admin)?;
        let target = user(8, Role::Standard)?;
        let unit_of_work = plain_unit_of_work(&caller, &target);
        unit_of_work.commit_fails.store(true, Ordering::SeqCst);
        let harness = harness(unit_of_work, MockFileStorage::new());

        // Act
        let result = harness
            .use_case
            .execute(DeleteUserCommand::new(caller.id(), target.id()))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteUserError::Unknown(_))));
        Ok(())
    }
}
