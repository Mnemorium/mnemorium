use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tracing::warn;

use crate::application::port::{
    UpdateMusicAlbumCommand, UpdateMusicAlbumError, UpdateMusicAlbumResponse,
    UpdateMusicAlbumUseCase,
};
use crate::domain::model::{Action, MusicAlbum};
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, MusicAlbumFilter, MusicAlbumRepository as _,
    MusicGroupFilter, MusicGroupRepository as _, PermissionFilter, PermissionRepository as _,
    RepositoryError, UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _,
    UserUnitOfWork,
};
use crate::domain::service::{Authorization, Decision, NamePolicy, NamePolicyError};

/// Use case implementation for updating a music album.
pub(crate) struct UpdateMusicAlbum<F> {
    /// Factory opening the unit of work wrapping the update.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> UpdateMusicAlbum<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> UpdateMusicAlbumUseCase for UpdateMusicAlbum<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: UpdateMusicAlbumCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<UpdateMusicAlbumResponse, UpdateMusicAlbumError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| UpdateMusicAlbumError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| UpdateMusicAlbumError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(UpdateMusicAlbumError::NoSuchCaller)?;

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| UpdateMusicAlbumError::Unknown(error.into()))?;

                let album = unit_of_work
                    .music_albums()
                    .search(&MusicAlbumFilter {
                        id: Some(command.album_id()),
                        ..MusicAlbumFilter::default()
                    })
                    .await
                    .map_err(|error| UpdateMusicAlbumError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(UpdateMusicAlbumError::NoSuchAlbum)?;

                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::LibraryRead, &album)
                {
                    warn!(
                        target: "security",
                        event = "authz_failed",
                        actor = caller.id(),
                        action = %denial.action().verb(),
                        target = %denial.action().target(),
                        "denied an action"
                    );
                    return Err(UpdateMusicAlbumError::NoSuchAlbum);
                }
                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::LibraryManage, &album)
                {
                    warn!(
                        target: "security",
                        event = "authz_failed",
                        actor = caller.id(),
                        action = %denial.action().verb(),
                        target = %denial.action().target(),
                        "denied an action"
                    );
                    return Err(if denial.conceals() {
                        UpdateMusicAlbumError::NoSuchAlbum
                    } else {
                        UpdateMusicAlbumError::Forbidden
                    });
                }

                let name = match command.name() {
                    Some(requested) => {
                        NamePolicy::validate(requested.to_owned()).map_err(|error| match error {
                            NamePolicyError::Empty => {
                                warn!(
                                    target: "security",
                                    event = "input_validation_failed",
                                    field = %"name",
                                    reason = %"empty",
                                    "rejected a client-supplied value"
                                );
                                UpdateMusicAlbumError::InvalidName
                            }
                            NamePolicyError::TooLong => {
                                warn!(
                                    target: "security",
                                    event = "input_validation_failed",
                                    field = %"name",
                                    reason = %"too_long",
                                    "rejected a client-supplied value"
                                );
                                UpdateMusicAlbumError::NameTooLong
                            }
                        })?
                    }
                    None => album.name().to_owned(),
                };
                let music_group_id = match command.music_group_id() {
                    Some(group_id) => {
                        if unit_of_work
                            .music_groups()
                            .search(&MusicGroupFilter {
                                id: Some(group_id),
                                ..MusicGroupFilter::default()
                            })
                            .await
                            .map_err(|error| UpdateMusicAlbumError::Unknown(error.into()))?
                            .is_empty()
                        {
                            return Err(UpdateMusicAlbumError::NoSuchGroup);
                        }
                        group_id
                    }
                    None => album.music_group_id(),
                };

                if command.name().is_some() || command.music_group_id().is_some() {
                    let collisions = unit_of_work
                        .music_albums()
                        .search(&MusicAlbumFilter {
                            music_group_id: Some(music_group_id),
                            name: Some(name.clone()),
                            ..MusicAlbumFilter::default()
                        })
                        .await
                        .map_err(|error| UpdateMusicAlbumError::Unknown(error.into()))?;
                    if collisions
                        .iter()
                        .any(|collision| collision.id() != album.id())
                    {
                        return Err(UpdateMusicAlbumError::Conflict);
                    }
                }

                let genres = match command.genres() {
                    Some(requested) => {
                        if requested.iter().any(String::is_empty) {
                            warn!(
                                target: "security",
                                event = "input_validation_failed",
                                field = %"genres",
                                reason = %"empty",
                                "rejected a client-supplied value"
                            );
                            return Err(UpdateMusicAlbumError::InvalidGenre);
                        }
                        requested.to_vec()
                    }
                    None => album.genre_ids().to_vec(),
                };
                let release_date = command.release_date().resolve(album.release_date());
                let production_type = command.production_type().resolve(album.production_type());
                let cover_image_id = command.cover_image_id().resolve(album.cover_image_id());

                let updated = MusicAlbum::try_new(
                    album.id(),
                    name,
                    release_date,
                    production_type,
                    genres,
                    music_group_id,
                    album.music_group_ids().map(<[_]>::to_vec),
                    cover_image_id,
                    album.mediums().to_vec(),
                )
                .map_err(|_| UpdateMusicAlbumError::InvalidName)?;

                let stored = unit_of_work
                    .music_albums()
                    .save(updated)
                    .await
                    .map_err(|error| {
                        if matches!(
                            error,
                            RepositoryError::AlreadyExist
                                | RepositoryError::Conflict
                                | RepositoryError::DataIntegrityViolation
                        ) {
                            UpdateMusicAlbumError::Conflict
                        } else {
                            UpdateMusicAlbumError::Unknown(error.into())
                        }
                    })?;

                Ok(UpdateMusicAlbumResponse::new(stored))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| UpdateMusicAlbumError::Unknown(error.into()))?;
                    Ok(value)
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
    use crate::application::FieldUpdate;

    use crate::domain::NumericID;

    use crate::domain::model::{MusicGroup, Permission, Role, Scope, User};

    use crate::domain::port::{
        MockMusicAlbumRepository, MockMusicGroupRepository, MockPermissionRepository,
        MockUserRepository,
    };

    use crate::test_helpers::*;
    use chrono::NaiveDate;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = UpdateMusicAlbum<TestFactory>;

    /// Expect the caller load to find an administrator.
    fn expect_admin(
        users: &mut MockUserRepository,
        caller_id: NumericID,
    ) -> Result<(), Box<dyn Error>> {
        let caller = User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
        users
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(caller_id))
            .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
        Ok(())
    }

    /// Build a Library test harness around the given mocks.
    fn harness_with(
        users: MockUserRepository,
        albums: MockMusicAlbumRepository,
        groups: MockMusicGroupRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            music_albums: albums,
            music_groups: groups,
            permissions: no_permissions(),
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    /// Expect the album load to find `album`.
    fn expect_album(albums: &mut MockMusicAlbumRepository, album: MusicAlbum) {
        let album_id = album.id();
        albums
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(album_id))
            .return_once(move |_| {
                let found = album.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
    }

    #[tokio::test]
    async fn update_music_album_changes_the_date_and_genres() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_album(7, Vec::new())?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, stored);
        albums
            .expect_save()
            .times(1)
            .withf(|album| {
                album.id() == 7
                    && album.genre_ids() == ["JAZZ".to_owned()]
                    && album.release_date() == NaiveDate::from_ymd_opt(1959, 8, 17)
            })
            .returning(|album| Box::pin(async move { Ok(album) }));
        let harness = harness_with(users, albums, MockMusicGroupRepository::new());
        let use_case: UseCase = UpdateMusicAlbum::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdateMusicAlbumCommand::new(
                1,
                7,
                None,
                None,
                FieldUpdate::Set(album_release_date()),
                FieldUpdate::Absent,
                Some(vec!["JAZZ".to_owned()]),
                FieldUpdate::Absent,
            ))
            .await?;

        // Assert
        assert_eq!(response.album().genre_ids(), ["JAZZ".to_owned()]);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_music_album_missing_cover_is_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange: the replacement cover does not resolve, so the update
        // violates the cover foreign key.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_album(7, Vec::new())?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, stored);
        albums
            .expect_save()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::DataIntegrityViolation) }));
        let harness = harness_with(users, albums, MockMusicGroupRepository::new());
        let use_case: UseCase = UpdateMusicAlbum::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateMusicAlbumCommand::new(
                1,
                7,
                None,
                None,
                FieldUpdate::Absent,
                FieldUpdate::Absent,
                None,
                FieldUpdate::Set(9),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateMusicAlbumError::Conflict)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_music_album_clears_an_optional_value() -> Result<(), Box<dyn Error>> {
        // Arrange: the album carries a release date.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_album(7, Vec::new())?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, stored);
        albums
            .expect_save()
            .times(1)
            .withf(|album| album.release_date().is_none())
            .returning(|album| Box::pin(async move { Ok(album) }));
        let harness = harness_with(users, albums, MockMusicGroupRepository::new());
        let use_case: UseCase = UpdateMusicAlbum::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdateMusicAlbumCommand::new(
                1,
                7,
                None,
                None,
                FieldUpdate::Clear,
                FieldUpdate::Absent,
                None,
                FieldUpdate::Absent,
            ))
            .await?;

        // Assert
        assert!(response.album().release_date().is_none());
        Ok(())
    }

    #[tokio::test]
    async fn update_music_album_group_collision_is_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange: another album already carries the requested identity.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_album(7, Vec::new())?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, stored);
        let mut groups = MockMusicGroupRepository::new();
        let group = MusicGroup::try_new(4, "The Group".to_owned(), None)?;
        groups
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(4))
            .return_once(move |_| {
                let found = group.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
        albums
            .expect_search()
            .times(1)
            .withf(|filter| filter.music_group_id == Some(4))
            .returning(|_| {
                let collision = sample_album(8, Vec::new()).ok();
                Box::pin(async move { Ok(collision.into_iter().collect()) })
            });
        let harness = harness_with(users, albums, groups);
        let use_case: UseCase = UpdateMusicAlbum::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateMusicAlbumCommand::new(
                1,
                7,
                Some("Album 8".to_owned()),
                Some(4),
                FieldUpdate::Absent,
                FieldUpdate::Absent,
                None,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateMusicAlbumError::Conflict)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_music_album_unknown_group_is_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_album(7, Vec::new())?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, stored);
        let mut groups = MockMusicGroupRepository::new();
        groups
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, albums, groups);
        let use_case: UseCase = UpdateMusicAlbum::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateMusicAlbumCommand::new(
                1,
                7,
                None,
                Some(404),
                FieldUpdate::Absent,
                FieldUpdate::Absent,
                None,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateMusicAlbumError::NoSuchGroup)));
        Ok(())
    }

    #[tokio::test]
    async fn update_music_album_empty_name_is_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_album(7, Vec::new())?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, stored);
        let harness = harness_with(users, albums, MockMusicGroupRepository::new());
        let use_case: UseCase = UpdateMusicAlbum::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateMusicAlbumCommand::new(
                1,
                7,
                Some("   ".to_owned()),
                None,
                FieldUpdate::Absent,
                FieldUpdate::Absent,
                None,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateMusicAlbumError::InvalidName)));
        Ok(())
    }

    #[tokio::test]
    async fn update_music_album_unknown_album_returns_no_such_album() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, albums, MockMusicGroupRepository::new());
        let use_case: UseCase = UpdateMusicAlbum::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateMusicAlbumCommand::new(
                1,
                404,
                None,
                None,
                FieldUpdate::Absent,
                FieldUpdate::Absent,
                None,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateMusicAlbumError::NoSuchAlbum)));
        Ok(())
    }

    #[tokio::test]
    async fn update_music_album_dependency_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange: the album load fails at the port.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::Unavailable) }));
        let harness = harness_with(users, albums, MockMusicGroupRepository::new());
        let use_case: UseCase = UpdateMusicAlbum::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateMusicAlbumCommand::new(
                1,
                7,
                None,
                None,
                FieldUpdate::Absent,
                FieldUpdate::Absent,
                None,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateMusicAlbumError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_music_album_standard_user_is_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let stored = sample_album(7, Vec::new())?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, stored);
        let harness = harness_with(users, albums, MockMusicGroupRepository::new());
        let use_case: UseCase = UpdateMusicAlbum::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateMusicAlbumCommand::new(
                1,
                7,
                Some("Renamed".to_owned()),
                None,
                FieldUpdate::Absent,
                FieldUpdate::Absent,
                None,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateMusicAlbumError::Forbidden)));
        Ok(())
    }

    #[tokio::test]
    async fn update_music_album_hidden_album_is_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange: the caller is shut out of the library entirely.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let stored = sample_album(7, Vec::new())?;
        let mut albums = MockMusicAlbumRepository::new();
        expect_album(&mut albums, stored);
        let mut permissions = MockPermissionRepository::new();
        expect_permissions(
            &mut permissions,
            1,
            vec![Permission::try_new(1, Action::LibraryRead, Scope::None)?],
        );
        let unit_of_work = TestUnitOfWork {
            music_albums: albums,
            permissions,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
        let use_case: UseCase = UpdateMusicAlbum::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateMusicAlbumCommand::new(
                1,
                7,
                Some("Renamed".to_owned()),
                None,
                FieldUpdate::Absent,
                FieldUpdate::Absent,
                None,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateMusicAlbumError::NoSuchAlbum)));
        Ok(())
    }
}
