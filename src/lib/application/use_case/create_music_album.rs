use crate::domain::model::{Action, ChildKind};

use crate::application::authorization::authorize;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tracing::warn;

use crate::application::port::{
    CreateMusicAlbumCommand, CreateMusicAlbumError, CreateMusicAlbumResponse,
    CreateMusicAlbumUseCase,
};
use crate::domain::model::{MusicAlbum, ResourceKind, ResourceRef, Visibility};
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, MusicAlbumFilter, MusicAlbumRepository as _,
    MusicGroupFilter, MusicGroupRepository as _, NewResource, RepositoryError,
    ResourceRepository as _, UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _,
    UserUnitOfWork,
};
use crate::domain::service::{NamePolicy, NamePolicyError};

/// The genre an album carries when the request names none.
const DEFAULT_GENRE: &str = "UNKNOWN";

/// Use case implementation for creating a music album.
pub(crate) struct CreateMusicAlbum<F> {
    /// Factory opening the unit of work wrapping the creation.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> CreateMusicAlbum<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> CreateMusicAlbumUseCase for CreateMusicAlbum<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: CreateMusicAlbumCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<CreateMusicAlbumResponse, CreateMusicAlbumError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| CreateMusicAlbumError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| CreateMusicAlbumError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(CreateMusicAlbumError::NoSuchCaller)?;

                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::Catalogue, 0),
                    Action::CreateChild,
                    Some(ChildKind::MusicAlbum),
                )
                .await
                .map_err(|_| CreateMusicAlbumError::Forbidden)?;

                let name =
                    NamePolicy::validate(command.name().to_owned()).map_err(
                        |error| match error {
                            NamePolicyError::Empty => {
                                warn!(
                                    target: "security",
                                    event = "input_validation_failed",
                                    field = %"name",
                                    reason = %"empty",
                                    "rejected a client-supplied value"
                                );
                                CreateMusicAlbumError::InvalidName
                            }
                            NamePolicyError::TooLong => {
                                warn!(
                                    target: "security",
                                    event = "input_validation_failed",
                                    field = %"name",
                                    reason = %"too_long",
                                    "rejected a client-supplied value"
                                );
                                CreateMusicAlbumError::NameTooLong
                            }
                        },
                    )?;

                if command.genres().iter().any(String::is_empty) {
                    warn!(
                        target: "security",
                        event = "input_validation_failed",
                        field = %"genres",
                        reason = %"empty",
                        "rejected a client-supplied value"
                    );
                    return Err(CreateMusicAlbumError::InvalidGenre);
                }
                let genres = if command.genres().is_empty() {
                    vec![DEFAULT_GENRE.to_owned()]
                } else {
                    command.genres().to_vec()
                };

                if unit_of_work
                    .music_groups()
                    .search(&MusicGroupFilter {
                        id: Some(command.music_group_id()),
                        ..MusicGroupFilter::default()
                    })
                    .await
                    .map_err(|error| CreateMusicAlbumError::Unknown(error.into()))?
                    .is_empty()
                {
                    return Err(CreateMusicAlbumError::NoSuchGroup);
                }

                let existing = unit_of_work
                    .music_albums()
                    .search(&MusicAlbumFilter {
                        music_group_id: Some(command.music_group_id()),
                        name: Some(name.clone()),
                        ..MusicAlbumFilter::default()
                    })
                    .await
                    .map_err(|error| CreateMusicAlbumError::Unknown(error.into()))?
                    .into_iter()
                    .next();
                if let Some(album) = existing {
                    return Ok(CreateMusicAlbumResponse::new(album, false));
                }

                let album = MusicAlbum::try_new(
                    0,
                    name,
                    command.release_date(),
                    command.production_type(),
                    genres,
                    command.music_group_id(),
                    None,
                    command.cover_image_id(),
                    Vec::new(),
                )
                .map_err(|_| CreateMusicAlbumError::InvalidName)?;

                let stored = unit_of_work
                    .music_albums()
                    .create(album)
                    .await
                    .map_err(|error| {
                        if matches!(
                            error,
                            RepositoryError::AlreadyExist
                                | RepositoryError::Conflict
                                | RepositoryError::DataIntegrityViolation
                        ) {
                            CreateMusicAlbumError::Conflict
                        } else {
                            CreateMusicAlbumError::Unknown(error.into())
                        }
                    })?;

                unit_of_work
                    .resources()
                    .create(NewResource {
                        kind: ResourceKind::MusicAlbum,
                        entity_id: stored.id(),
                        owner_id: None,
                        auth_parent: Some(ResourceRef::new(ResourceKind::Catalogue, 0)),
                        visibility: Visibility::Public,
                    })
                    .await
                    .map_err(|error| CreateMusicAlbumError::Unknown(error.into()))?;

                Ok(CreateMusicAlbumResponse::new(stored, true))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| CreateMusicAlbumError::Unknown(error.into()))?;
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
    use crate::domain::NumericID;

    use crate::domain::model::{MusicGroup, Role, User};

    use crate::domain::port::{
        MockMusicAlbumRepository, MockMusicGroupRepository, MockUserRepository,
    };

    use crate::test_helpers::*;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = CreateMusicAlbum<TestFactory>;

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
            resources: no_resources(),
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    /// Expect the group load to find the default group.
    fn expect_group(groups: &mut MockMusicGroupRepository) -> Result<(), Box<dyn Error>> {
        let group = MusicGroup::try_new(0, "Unknown".to_owned(), None)?;
        groups
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(0))
            .return_once(move |_| {
                let found = group.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
        Ok(())
    }

    #[tokio::test]
    async fn create_music_album_defaults_the_genre_and_reports_created()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the identity is free.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        expect_group(&mut groups)?;
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search()
            .times(1)
            .withf(|filter| {
                filter.name.as_deref() == Some("Kind of Blue") && filter.music_group_id == Some(0)
            })
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let stored = sample_album(7, Vec::new())?;
        albums
            .expect_create()
            .times(1)
            .withf(|album| {
                album.name() == "Kind of Blue"
                    && album.genre_ids() == ["UNKNOWN".to_owned()]
                    && album.music_group_id() == 0
            })
            .return_once(move |_| {
                let created = stored.clone();
                Box::pin(async move { Ok(created) })
            });
        let harness = harness_with(users, albums, groups);
        let use_case: UseCase = CreateMusicAlbum::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(CreateMusicAlbumCommand::new(
                1,
                "  Kind of Blue  ".to_owned(),
                0,
                None,
                None,
                Vec::new(),
                None,
            ))
            .await?;

        // Assert
        assert!(response.created());
        assert_eq!(response.album().id(), 7);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_music_album_missing_cover_is_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange: the cover identifier does not resolve, so the insert violates
        // the cover foreign key.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        expect_group(&mut groups)?;
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        albums
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::DataIntegrityViolation) }));
        let harness = harness_with(users, albums, groups);
        let use_case: UseCase = CreateMusicAlbum::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMusicAlbumCommand::new(
                1,
                "Kind of Blue".to_owned(),
                0,
                None,
                None,
                Vec::new(),
                Some(9),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateMusicAlbumError::Conflict)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_music_album_returns_the_existing_album() -> Result<(), Box<dyn Error>> {
        // Arrange: the identity already exists.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        expect_group(&mut groups)?;
        let existing = sample_album(7, Vec::new())?;
        let mut albums = MockMusicAlbumRepository::new();
        albums.expect_search().times(1).return_once(move |_| {
            let found = existing.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
        albums.expect_create().times(0);
        let harness = harness_with(users, albums, groups);
        let use_case: UseCase = CreateMusicAlbum::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(CreateMusicAlbumCommand::new(
                1,
                "Album 7".to_owned(),
                0,
                None,
                None,
                vec!["JAZZ".to_owned()],
                None,
            ))
            .await?;

        // Assert
        assert!(!response.created());
        assert_eq!(response.album().id(), 7);
        Ok(())
    }

    #[tokio::test]
    async fn create_music_album_unknown_group_is_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        groups
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockMusicAlbumRepository::new(), groups);
        let use_case: UseCase = CreateMusicAlbum::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMusicAlbumCommand::new(
                1,
                "Kind of Blue".to_owned(),
                404,
                None,
                None,
                Vec::new(),
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateMusicAlbumError::NoSuchGroup)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_music_album_empty_name_is_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let harness = harness_with(
            users,
            MockMusicAlbumRepository::new(),
            MockMusicGroupRepository::new(),
        );
        let use_case: UseCase = CreateMusicAlbum::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMusicAlbumCommand::new(
                1,
                "   ".to_owned(),
                0,
                None,
                None,
                Vec::new(),
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateMusicAlbumError::InvalidName)));
        Ok(())
    }
    #[tokio::test]
    async fn create_music_album_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(
            users,
            MockMusicAlbumRepository::new(),
            MockMusicGroupRepository::new(),
        );
        let use_case: UseCase = CreateMusicAlbum::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMusicAlbumCommand::new(
                999,
                "Kind of Blue".to_owned(),
                0,
                None,
                None,
                Vec::new(),
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateMusicAlbumError::NoSuchCaller)));
        Ok(())
    }

    #[tokio::test]
    async fn create_music_album_rejects_an_empty_genre() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let harness = harness_with(
            users,
            MockMusicAlbumRepository::new(),
            MockMusicGroupRepository::new(),
        );
        let use_case: UseCase = CreateMusicAlbum::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMusicAlbumCommand::new(
                1,
                "Kind of Blue".to_owned(),
                0,
                None,
                None,
                vec![String::new()],
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateMusicAlbumError::InvalidGenre)));
        Ok(())
    }

    #[tokio::test]
    async fn create_music_album_dependency_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange: the group load fails at the port.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        groups
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::Unavailable) }));
        let harness = harness_with(users, MockMusicAlbumRepository::new(), groups);
        let use_case: UseCase = CreateMusicAlbum::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMusicAlbumCommand::new(
                1,
                "Kind of Blue".to_owned(),
                0,
                None,
                None,
                Vec::new(),
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateMusicAlbumError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_music_album_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        expect_group(&mut groups)?;
        let mut albums = MockMusicAlbumRepository::new();
        albums
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        albums.expect_create().times(1).returning(|_| {
            let created = sample_album(7, Vec::new()).map_err(|_| RepositoryError::OperationFailed);
            Box::pin(async move { created })
        });
        let harness = harness_with(users, albums, groups);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = CreateMusicAlbum::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMusicAlbumCommand::new(
                1,
                "Kind of Blue".to_owned(),
                0,
                None,
                None,
                Vec::new(),
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateMusicAlbumError::Unknown(_))));
        Ok(())
    }
}
