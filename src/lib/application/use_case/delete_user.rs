use crate::application::authorization::authorize;
use crate::domain::model::{ResourceKind, ResourceRef};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tracing::info;

use crate::application::port::{DeleteUserCommand, DeleteUserError, DeleteUserUseCase};
use crate::application::use_case::{media_path, staged_path};
use crate::domain::model::{Action, AssetScope, GalleryItemMedia, Origin, Role};
use crate::domain::port::{
    AssetUnitOfWork, AudioFilter, AudioRepository as _, CredentialFilter,
    CredentialRepository as _, EpisodeFilter, FileFilter, FileRepository as _, FileStorage,
    GalleryFilter, GalleryItemFilter, GalleryRepository as _, IdentityUnitOfWork, ImageFilter,
    ImageRepository as _, LibraryUnitOfWork, MovieFilter, MovieRepository as _, MusicAlbumFilter,
    MusicAlbumRepository as _, MusicPlaylistFilter, MusicPlaylistRepository as _, RepositoryError,
    ResourceFilter, ResourceRepository as _, SeriesRepository as _, UnitOfWork as _,
    UnitOfWorkFactory, UploadFilter, UploadRepository as _, UserFilter, UserRepository as _,
    UserUnitOfWork, VideoFilter, VideoRepository as _,
};

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
                // Only the root administers Admin accounts, and an Admin may
                // not delete itself (`REQ-USER-021`).
                if target.role() == Role::Admin && !caller.is_root_admin() {
                    return Err(DeleteUserError::TargetNotModifiable);
                }
                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::User, target.id()),
                    Action::Delete,
                    None,
                )
                .await
                .map_err(|_| DeleteUserError::NotAdmin)?;

                let mut removed_paths = Vec::new();

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
                        // The resource follows the file: its owner becomes the
                        // Root Admin and its authorization parent is reparented
                        // off the deleted account's tree, or deleting that
                        // account would cascade the resource away with it.
                        let root_resource_id = unit_of_work
                            .resources()
                            .search(
                                &ResourceFilter {
                                    entity_id: Some(0),
                                    kind: Some(ResourceKind::User),
                                    ..ResourceFilter::default()
                                },
                                &caller,
                            )
                            .await
                            .map_err(|error| DeleteUserError::Unknown(error.into()))?
                            .first()
                            .map(|access| access.resource().resource_id());
                        unit_of_work
                            .resources()
                            .reassign_owner(
                                ResourceRef::new(ResourceKind::File, file.id()),
                                Some(0),
                                root_resource_id,
                            )
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

                // The identity's resource outlives the row it addresses unless
                // this unit of work removes it: a later account reuses the freed
                // identifier and collides with the stale resource (`PERS-020`).
                unit_of_work
                    .resources()
                    .delete(ResourceRef::new(ResourceKind::User, target.id()))
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
        ChunkBitmap, File, Gallery, Image, IntegrityHash, MusicPlaylist, Orientation, Role, Upload,
        User,
    };
    use crate::domain::port::{
        MockAudioRepository, MockFileRepository, MockFileStorage, MockImageRepository,
        MockUserRepository, MockVideoRepository,
    };
    use crate::test_helpers::*;
    use chrono::{NaiveDate, NaiveDateTime};
    use std::error::Error;
    use std::sync::atomic::{AtomicBool, Ordering};

    type UseCase = DeleteUser<TestFactory, MockFileStorage>;

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

    #[tokio::test]
    async fn delete_user_admin_deletes_a_standard_user() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(7, Role::Admin)?;
        let target = user(8, Role::Standard)?;
        let harness = harness(
            (move |caller: &User, target: &User| -> TestUnitOfWork {
                let mut unit_of_work = (move |caller: &User, target: &User| -> TestUnitOfWork {
                    let mut users = MockUserRepository::new();
                    (move |users: &mut MockUserRepository, caller: User, target: Option<User>| {
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
                    })(&mut users, caller.clone(), Some(target.clone()));
                    TestUnitOfWork {
                        resources: (move || -> crate::domain::port::MockResourceRepository {
                            let mut resources =
                                (move || -> crate::domain::port::MockResourceRepository {
                                    let mut resources =
                                        crate::domain::port::MockResourceRepository::new();
                                    resources.expect_search().returning(|filter, caller| {
                                        let user_id = caller.id();
                                        let resource = crate::domain::model::Resource::from_stored(
                                            1,
                                            filter.kind.unwrap_or(
                                                crate::domain::model::ResourceKind::Library,
                                            ),
                                            filter.entity_id.unwrap_or_default(),
                                            Some(user_id),
                                            None,
                                            crate::domain::model::Visibility::Private,
                                        );
                                        let grant = crate::domain::model::Grant::from_stored(
                                            1,
                                            crate::domain::model::Principal::User(user_id),
                                            1,
                                            crate::domain::model::ResourceRole::Downloader,
                                            crate::domain::model::ChildKind::All,
                                            crate::domain::model::GrantEffect::Allow,
                                        );
                                        Box::pin(async move {
                                            Ok(vec![crate::domain::model::ResourceAccess::new(
                                                resource,
                                                Vec::new(),
                                                vec![grant],
                                                Vec::new(),
                                            )])
                                        })
                                    });
                                    resources
            .expect_search_many()
            .returning(|addresses, caller| {
                let user_id = caller.id();
                let found = addresses
                    .iter()
                    .map(|address| {
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            address.kind(),
                            address.id(),
                            Some(user_id),
                            None,
                            crate::domain::model::Visibility::Private,
                        );
                        let grant = crate::domain::model::Grant::from_stored(
                            1,
                            crate::domain::model::Principal::User(user_id),
                            1,
                            crate::domain::model::ResourceRole::Downloader,
                            crate::domain::model::ChildKind::All,
                            crate::domain::model::GrantEffect::Allow,
                        );
                        crate::domain::model::ResourceAccess::new(resource, Vec::new(), vec![grant], Vec::new())
                    })
                    .collect::<Vec<_>>();
                Box::pin(async move { Ok(found) })
            });
                                    resources.expect_create().returning(|new_resource| {
                                        let resource = crate::domain::model::Resource::from_stored(
                                            1,
                                            new_resource.kind,
                                            new_resource.entity_id,
                                            new_resource.owner_id,
                                            None,
                                            new_resource.visibility,
                                        );
                                        Box::pin(async move { Ok(resource) })
                                    });
                                    resources
                                })();
                            resources
                                .expect_delete()
                                .returning(|_| Box::pin(async { Ok(true) }));
                            resources
                        })(),
                        users,
                        ..TestUnitOfWork::default()
                    }
                })(caller, target);
                (move |unit_of_work: &mut TestUnitOfWork| {
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
                })(&mut unit_of_work);
                (move |unit_of_work: &mut TestUnitOfWork, target: &User| {
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
                })(&mut unit_of_work, target);
                unit_of_work
            })(&caller, &target),
            MockFileStorage::new(),
        );

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

    // proves: SCN-USER-017-B — Business rule
    #[tokio::test]
    async fn delete_user_removes_an_uploaded_file_with_no_media_row() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let caller = user(7, Role::Admin)?;
        let target = user(8, Role::Standard)?;
        let owned = File::try_new(
            1,
            "files/1_orphan.png".to_owned(),
            8,
            false,
            "image/png".to_owned(),
            NaiveDate::default(),
            8,
            NaiveDateTime::default(),
            Some(IntegrityHash::try_new(DIGEST.to_owned())?),
            Some(Origin::Uploaded),
        )?;
        let mut unit_of_work = (move |caller: &User, target: &User| -> TestUnitOfWork {
            let mut users = MockUserRepository::new();
            (move |users: &mut MockUserRepository, caller: User, target: Option<User>| {
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
            })(&mut users, caller.clone(), Some(target.clone()));
            TestUnitOfWork {
                users,
                ..TestUnitOfWork::default()
            }
        })(&caller, &target);
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
        (move |files: &mut MockFileRepository, owned: File| {
            files.expect_search().times(1).returning(move |filter| {
                assert_eq!(filter.user_id, Some(8));
                let found = vec![owned.clone()];
                Box::pin(async move { Ok(found) })
            });
            files
                .expect_delete()
                .times(1)
                .withf(|filter| filter.id == Some(1))
                .returning(|_| Box::pin(async { Ok(true) }));
        })(&mut unit_of_work.files, owned);
        unit_of_work.images = {
            let mut images = MockImageRepository::new();
            images
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            images
        };
        unit_of_work.videos = {
            let mut videos = MockVideoRepository::new();
            videos
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            videos
        };
        unit_of_work.audios = {
            let mut audios = MockAudioRepository::new();
            audios
                .expect_search()
                .times(1)
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));
            audios
        };
        (move |unit_of_work: &mut TestUnitOfWork, target: &User| {
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
        })(&mut unit_of_work, &target);
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_delete_upload_file()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        let harness = harness(unit_of_work, file_storage);

        // Act
        let result = harness
            .use_case
            .execute(DeleteUserCommand::new(caller.id(), target.id()))
            .await;

        // Assert: the orphan file row is removed and its bytes are queued.
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_removes_its_resource() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(7, Role::Admin)?;
        let target = user(8, Role::Standard)?;
        let mut unit_of_work = (move |caller: &User, target: &User| -> TestUnitOfWork {
            let mut users = MockUserRepository::new();
            (move |users: &mut MockUserRepository, caller: User, target: Option<User>| {
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
            })(&mut users, caller.clone(), Some(target.clone()));
            TestUnitOfWork {
                users,
                ..TestUnitOfWork::default()
            }
        })(&caller, &target);
        (move |unit_of_work: &mut TestUnitOfWork| {
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
        })(&mut unit_of_work);
        (move |unit_of_work: &mut TestUnitOfWork, target: &User| {
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
        })(&mut unit_of_work, &target);
        let mut resources = (move || -> crate::domain::port::MockResourceRepository {
            let mut resources = crate::domain::port::MockResourceRepository::new();
            resources.expect_search().returning(|filter, caller| {
                let user_id = caller.id();
                let resource = crate::domain::model::Resource::from_stored(
                    1,
                    filter
                        .kind
                        .unwrap_or(crate::domain::model::ResourceKind::Library),
                    filter.entity_id.unwrap_or_default(),
                    Some(user_id),
                    None,
                    crate::domain::model::Visibility::Private,
                );
                let grant = crate::domain::model::Grant::from_stored(
                    1,
                    crate::domain::model::Principal::User(user_id),
                    1,
                    crate::domain::model::ResourceRole::Downloader,
                    crate::domain::model::ChildKind::All,
                    crate::domain::model::GrantEffect::Allow,
                );
                Box::pin(async move {
                    Ok(vec![crate::domain::model::ResourceAccess::new(
                        resource,
                        Vec::new(),
                        vec![grant],
                        Vec::new(),
                    )])
                })
            });
            resources
                .expect_search_many()
                .returning(|addresses, caller| {
                    let user_id = caller.id();
                    let found = addresses
                        .iter()
                        .map(|address| {
                            let resource = crate::domain::model::Resource::from_stored(
                                1,
                                address.kind(),
                                address.id(),
                                Some(user_id),
                                None,
                                crate::domain::model::Visibility::Private,
                            );
                            let grant = crate::domain::model::Grant::from_stored(
                                1,
                                crate::domain::model::Principal::User(user_id),
                                1,
                                crate::domain::model::ResourceRole::Downloader,
                                crate::domain::model::ChildKind::All,
                                crate::domain::model::GrantEffect::Allow,
                            );
                            crate::domain::model::ResourceAccess::new(
                                resource,
                                Vec::new(),
                                vec![grant],
                                Vec::new(),
                            )
                        })
                        .collect::<Vec<_>>();
                    Box::pin(async move { Ok(found) })
                });
            resources.expect_create().returning(|new_resource| {
                let resource = crate::domain::model::Resource::from_stored(
                    1,
                    new_resource.kind,
                    new_resource.entity_id,
                    new_resource.owner_id,
                    None,
                    new_resource.visibility,
                );
                Box::pin(async move { Ok(resource) })
            });
            resources
        })();
        resources
            .expect_delete()
            .times(1)
            .withf(|address| address.kind() == ResourceKind::User && address.id() == 8)
            .returning(|_| Box::pin(async { Ok(true) }));
        unit_of_work.resources = resources;
        let harness = harness(unit_of_work, MockFileStorage::new());

        // Act
        let result = harness
            .use_case
            .execute(DeleteUserCommand::new(caller.id(), target.id()))
            .await;

        // Assert: a freed identifier must not leave a stale USER resource.
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_clears_a_colliding_digest_on_transfer() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(0, Role::Admin)?;
        let target = user(8, Role::Standard)?;
        let mut unit_of_work = (move |caller: &User, target: &User| -> TestUnitOfWork {
            let mut users = MockUserRepository::new();
            (move |users: &mut MockUserRepository, caller: User, target: Option<User>| {
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
            })(&mut users, caller.clone(), Some(target.clone()));
            TestUnitOfWork {
                users,
                ..TestUnitOfWork::default()
            }
        })(&caller, &target);
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
        let owned = File::try_new(
            1,
            "files/1_artwork.png".to_owned(),
            8,
            false,
            "image/png".to_owned(),
            NaiveDate::default(),
            0,
            NaiveDateTime::default(),
            Some(IntegrityHash::try_new(DIGEST.to_owned())?),
            Some(Origin::Uploaded),
        )?;
        let root_file = File::try_new(
            2,
            "files/2_artwork.png".to_owned(),
            0,
            false,
            "image/png".to_owned(),
            NaiveDate::default(),
            0,
            NaiveDateTime::default(),
            Some(IntegrityHash::try_new(DIGEST.to_owned())?),
            Some(Origin::Uploaded),
        )?;
        (move |files: &mut MockFileRepository, owned: File, root_file: File| {
            files.expect_search().times(2).returning(move |filter| {
                let found = if filter.user_id == Some(8) && filter.integrity_hash.is_none() {
                    vec![owned.clone()]
                } else if filter.user_id == Some(0) && filter.integrity_hash.is_some() {
                    vec![root_file.clone()]
                } else {
                    Vec::new()
                };
                Box::pin(async move { Ok(found) })
            });
        })(&mut unit_of_work.files, owned, root_file);
        let mut images = MockImageRepository::new();
        images.expect_search().times(1).returning(|_| {
            let image = Image::try_new(
                1,
                "artwork.png".to_owned(),
                64,
                64,
                Orientation::Square,
                NaiveDate::default(),
                1,
                Some(AssetScope::Library),
            );
            Box::pin(async move { Ok(image.into_iter().collect()) })
        });
        unit_of_work.images = images;
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
            .files
            .expect_clear_integrity_hash()
            .times(1)
            .withf(|id| *id == 1)
            .returning(|_| Box::pin(async { Ok(true) }));
        unit_of_work
            .files
            .expect_reassign_owner()
            .times(1)
            .withf(|id, owner| *id == 1 && *owner == 0)
            .returning(|_, _| Box::pin(async { Ok(true) }));
        (move |unit_of_work: &mut TestUnitOfWork, target: &User| {
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
        })(&mut unit_of_work, &target);
        let mut resources = (move || -> crate::domain::port::MockResourceRepository {
            let mut resources = crate::domain::port::MockResourceRepository::new();
            resources.expect_search().returning(|filter, caller| {
                let user_id = caller.id();
                let resource = crate::domain::model::Resource::from_stored(
                    1,
                    filter
                        .kind
                        .unwrap_or(crate::domain::model::ResourceKind::Library),
                    filter.entity_id.unwrap_or_default(),
                    Some(user_id),
                    None,
                    crate::domain::model::Visibility::Private,
                );
                let grant = crate::domain::model::Grant::from_stored(
                    1,
                    crate::domain::model::Principal::User(user_id),
                    1,
                    crate::domain::model::ResourceRole::Downloader,
                    crate::domain::model::ChildKind::All,
                    crate::domain::model::GrantEffect::Allow,
                );
                Box::pin(async move {
                    Ok(vec![crate::domain::model::ResourceAccess::new(
                        resource,
                        Vec::new(),
                        vec![grant],
                        Vec::new(),
                    )])
                })
            });
            resources
                .expect_search_many()
                .returning(|addresses, caller| {
                    let user_id = caller.id();
                    let found = addresses
                        .iter()
                        .map(|address| {
                            let resource = crate::domain::model::Resource::from_stored(
                                1,
                                address.kind(),
                                address.id(),
                                Some(user_id),
                                None,
                                crate::domain::model::Visibility::Private,
                            );
                            let grant = crate::domain::model::Grant::from_stored(
                                1,
                                crate::domain::model::Principal::User(user_id),
                                1,
                                crate::domain::model::ResourceRole::Downloader,
                                crate::domain::model::ChildKind::All,
                                crate::domain::model::GrantEffect::Allow,
                            );
                            crate::domain::model::ResourceAccess::new(
                                resource,
                                Vec::new(),
                                vec![grant],
                                Vec::new(),
                            )
                        })
                        .collect::<Vec<_>>();
                    Box::pin(async move { Ok(found) })
                });
            resources.expect_create().returning(|new_resource| {
                let resource = crate::domain::model::Resource::from_stored(
                    1,
                    new_resource.kind,
                    new_resource.entity_id,
                    new_resource.owner_id,
                    None,
                    new_resource.visibility,
                );
                Box::pin(async move { Ok(resource) })
            });
            resources
        })();
        resources
            .expect_reassign_owner()
            .times(1)
            .withf(|address, owner, _| {
                address.kind() == ResourceKind::File && address.id() == 1 && *owner == Some(0)
            })
            .returning(|_, _, _| Box::pin(async { Ok(true) }));
        resources
            .expect_delete()
            .times(1)
            .withf(|address| address.kind() == ResourceKind::User && address.id() == 8)
            .returning(|_| Box::pin(async { Ok(true) }));
        unit_of_work.resources = resources;
        let harness = harness(unit_of_work, MockFileStorage::new());

        // Act
        let result = harness
            .use_case
            .execute(DeleteUserCommand::new(caller.id(), target.id()))
            .await;

        // Assert: the transferred row must not collide on the Root Admin's digest.
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_root_deletes_an_admin() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(0, Role::Admin)?;
        let target = user(8, Role::Admin)?;
        let harness = harness(
            (move |caller: &User, target: &User| -> TestUnitOfWork {
                let mut unit_of_work = (move |caller: &User, target: &User| -> TestUnitOfWork {
                    let mut users = MockUserRepository::new();
                    (move |users: &mut MockUserRepository, caller: User, target: Option<User>| {
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
                    })(&mut users, caller.clone(), Some(target.clone()));
                    TestUnitOfWork {
                        resources: (move || -> crate::domain::port::MockResourceRepository {
                            let mut resources =
                                (move || -> crate::domain::port::MockResourceRepository {
                                    let mut resources =
                                        crate::domain::port::MockResourceRepository::new();
                                    resources.expect_search().returning(|filter, caller| {
                                        let user_id = caller.id();
                                        let resource = crate::domain::model::Resource::from_stored(
                                            1,
                                            filter.kind.unwrap_or(
                                                crate::domain::model::ResourceKind::Library,
                                            ),
                                            filter.entity_id.unwrap_or_default(),
                                            Some(user_id),
                                            None,
                                            crate::domain::model::Visibility::Private,
                                        );
                                        let grant = crate::domain::model::Grant::from_stored(
                                            1,
                                            crate::domain::model::Principal::User(user_id),
                                            1,
                                            crate::domain::model::ResourceRole::Downloader,
                                            crate::domain::model::ChildKind::All,
                                            crate::domain::model::GrantEffect::Allow,
                                        );
                                        Box::pin(async move {
                                            Ok(vec![crate::domain::model::ResourceAccess::new(
                                                resource,
                                                Vec::new(),
                                                vec![grant],
                                                Vec::new(),
                                            )])
                                        })
                                    });
                                    resources
            .expect_search_many()
            .returning(|addresses, caller| {
                let user_id = caller.id();
                let found = addresses
                    .iter()
                    .map(|address| {
                        let resource = crate::domain::model::Resource::from_stored(
                            1,
                            address.kind(),
                            address.id(),
                            Some(user_id),
                            None,
                            crate::domain::model::Visibility::Private,
                        );
                        let grant = crate::domain::model::Grant::from_stored(
                            1,
                            crate::domain::model::Principal::User(user_id),
                            1,
                            crate::domain::model::ResourceRole::Downloader,
                            crate::domain::model::ChildKind::All,
                            crate::domain::model::GrantEffect::Allow,
                        );
                        crate::domain::model::ResourceAccess::new(resource, Vec::new(), vec![grant], Vec::new())
                    })
                    .collect::<Vec<_>>();
                Box::pin(async move { Ok(found) })
            });
                                    resources.expect_create().returning(|new_resource| {
                                        let resource = crate::domain::model::Resource::from_stored(
                                            1,
                                            new_resource.kind,
                                            new_resource.entity_id,
                                            new_resource.owner_id,
                                            None,
                                            new_resource.visibility,
                                        );
                                        Box::pin(async move { Ok(resource) })
                                    });
                                    resources
                                })();
                            resources
                                .expect_delete()
                                .returning(|_| Box::pin(async { Ok(true) }));
                            resources
                        })(),
                        users,
                        ..TestUnitOfWork::default()
                    }
                })(caller, target);
                (move |unit_of_work: &mut TestUnitOfWork| {
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
                })(&mut unit_of_work);
                (move |unit_of_work: &mut TestUnitOfWork, target: &User| {
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
                })(&mut unit_of_work, target);
                unit_of_work
            })(&caller, &target),
            MockFileStorage::new(),
        );

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
    async fn delete_user_admin_cannot_delete_another_admin() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(7, Role::Admin)?;
        let target = user(8, Role::Admin)?;
        let harness = harness(
            (move |caller: &User, target: &User| -> TestUnitOfWork {
                let mut users = MockUserRepository::new();
                (move |users: &mut MockUserRepository, caller: User, target: Option<User>| {
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
                })(&mut users, caller.clone(), Some(target.clone()));
                TestUnitOfWork {
                    users,
                    ..TestUnitOfWork::default()
                }
            })(&caller, &target),
            MockFileStorage::new(),
        );

        // Act
        let result = harness
            .use_case
            .execute(DeleteUserCommand::new(caller.id(), target.id()))
            .await;

        // Assert: only the root administers Admin accounts.
        assert!(matches!(result, Err(DeleteUserError::TargetNotModifiable)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        assert!(!harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn delete_user_admin_cannot_delete_itself() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(7, Role::Admin)?;
        let harness = harness(
            (move |caller: &User, target: &User| -> TestUnitOfWork {
                let mut users = MockUserRepository::new();
                (move |users: &mut MockUserRepository, caller: User, target: Option<User>| {
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
                })(&mut users, caller.clone(), Some(target.clone()));
                TestUnitOfWork {
                    users,
                    ..TestUnitOfWork::default()
                }
            })(&caller, &caller),
            MockFileStorage::new(),
        );

        // Act
        let result = harness
            .use_case
            .execute(DeleteUserCommand::new(caller.id(), caller.id()))
            .await;

        // Assert
        assert!(matches!(result, Err(DeleteUserError::TargetNotModifiable)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        assert!(!harness.committed.load(Ordering::SeqCst));
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

    // proves: SCN-USER-015-C — Recovery
    #[tokio::test]
    async fn delete_user_identity_removal_failure_rolls_back() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(7, Role::Admin)?;
        let target = user(8, Role::Standard)?;
        let mut unit_of_work = (move |caller: &User, target: &User| -> TestUnitOfWork {
            let mut users = MockUserRepository::new();
            (move |users: &mut MockUserRepository, caller: User, target: Option<User>| {
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
            })(&mut users, caller.clone(), Some(target.clone()));
            TestUnitOfWork {
                users,
                ..TestUnitOfWork::default()
            }
        })(&caller, &target);
        (move |unit_of_work: &mut TestUnitOfWork| {
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
        })(&mut unit_of_work);
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
        let mut unit_of_work = (move |caller: &User, target: &User| -> TestUnitOfWork {
            let mut users = MockUserRepository::new();
            (move |users: &mut MockUserRepository, caller: User, target: Option<User>| {
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
            })(&mut users, caller.clone(), Some(target.clone()));
            TestUnitOfWork {
                users,
                ..TestUnitOfWork::default()
            }
        })(&caller, &target);
        unit_of_work
            .files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        (move |unit_of_work: &mut TestUnitOfWork, target: &User| {
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
        })(&mut unit_of_work, &target);
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
    // proves: SCN-USER-015-C — Recovery
    #[tokio::test]
    async fn delete_user_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(7, Role::Admin)?;
        let target = user(8, Role::Standard)?;
        let unit_of_work = (move |caller: &User, target: &User| -> TestUnitOfWork {
            let mut unit_of_work = (move |caller: &User, target: &User| -> TestUnitOfWork {
                let mut users = MockUserRepository::new();
                (move |users: &mut MockUserRepository, caller: User, target: Option<User>| {
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
                })(&mut users, caller.clone(), Some(target.clone()));
                TestUnitOfWork {
                    users,
                    ..TestUnitOfWork::default()
                }
            })(caller, target);
            (move |unit_of_work: &mut TestUnitOfWork| {
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
            })(&mut unit_of_work);
            (move |unit_of_work: &mut TestUnitOfWork, target: &User| {
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
            })(&mut unit_of_work, target);
            unit_of_work
        })(&caller, &target);
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
