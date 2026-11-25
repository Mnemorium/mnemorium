use crate::application::authorization::authorize;
use crate::application::cover_is_viewable;
use crate::domain::model::ChildKind;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tracing::warn;

use crate::application::port::{
    CreateMusicGroupCommand, CreateMusicGroupError, CreateMusicGroupResponse,
    CreateMusicGroupUseCase,
};
use crate::domain::model::{Action, MusicGroup, ResourceKind, ResourceRef, Visibility};
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, MusicGroupFilter, MusicGroupRepository as _,
    NewResource, RepositoryError, ResourceRepository as _, UnitOfWork as _, UnitOfWorkFactory,
    UserFilter, UserRepository as _, UserUnitOfWork,
};
use crate::domain::service::{NamePolicy, NamePolicyError};

/// Use case implementation for creating a music group.
pub(crate) struct CreateMusicGroup<F> {
    /// Factory opening the unit of work wrapping the creation.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> CreateMusicGroup<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> CreateMusicGroupUseCase for CreateMusicGroup<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "RepositoryError is non_exhaustive, so a catch-all arm is required"
    )]
    fn execute<'future>(
        &'future self,
        command: CreateMusicGroupCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<CreateMusicGroupResponse, CreateMusicGroupError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| CreateMusicGroupError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| CreateMusicGroupError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(CreateMusicGroupError::NoSuchCaller)?;

                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::Catalogue, 0),
                    Action::CreateChild,
                    Some(ChildKind::MusicGroup),
                )
                .await
                .map_err(|_| CreateMusicGroupError::Forbidden)?;

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
                                CreateMusicGroupError::InvalidName
                            }
                            NamePolicyError::SurroundingWhitespace => {
                                warn!(
                                    target: "security",
                                    event = "input_validation_failed",
                                    field = %"name",
                                    reason = %"surrounding_whitespace",
                                    "rejected a client-supplied value"
                                );
                                CreateMusicGroupError::InvalidName
                            }
                            NamePolicyError::TooLong => {
                                warn!(
                                    target: "security",
                                    event = "input_validation_failed",
                                    field = %"name",
                                    reason = %"too_long",
                                    "rejected a client-supplied value"
                                );
                                CreateMusicGroupError::NameTooLong
                            }
                        },
                    )?;

                let existing = unit_of_work
                    .music_groups()
                    .search(&MusicGroupFilter {
                        stage_name: Some(name.clone()),
                        ..MusicGroupFilter::default()
                    })
                    .await
                    .map_err(|error| CreateMusicGroupError::Unknown(error.into()))?;
                if !existing.is_empty() {
                    return Err(CreateMusicGroupError::NameAlreadyExists);
                }

                if !cover_is_viewable(&mut unit_of_work, &caller, command.cover_image_id()).await {
                    return Err(CreateMusicGroupError::InvalidCover);
                }

                let group = MusicGroup::try_new(0, name, command.cover_image_id())
                    .map_err(|_| CreateMusicGroupError::InvalidName)?;
                let created = unit_of_work
                    .music_groups()
                    .create(group)
                    .await
                    .map_err(|error| match error {
                        // A concurrent insert won the race for the unique name
                        // (`STY-RUST-014`).
                        RepositoryError::AlreadyExist => CreateMusicGroupError::NameAlreadyExists,
                        RepositoryError::Conflict | RepositoryError::DataIntegrityViolation => {
                            CreateMusicGroupError::InvalidCover
                        }
                        other => CreateMusicGroupError::Unknown(other.into()),
                    })?;

                unit_of_work
                    .resources()
                    .create(NewResource {
                        kind: ResourceKind::MusicGroup,
                        entity_id: created.id(),
                        owner_id: None,
                        auth_parent: Some(ResourceRef::new(ResourceKind::Catalogue, 0)),
                        visibility: Visibility::Public,
                    })
                    .await
                    .map_err(|error| CreateMusicGroupError::Unknown(error.into()))?;

                Ok(CreateMusicGroupResponse::new(created))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| CreateMusicGroupError::Unknown(error.into()))?;
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

    use crate::domain::model::{Role, User};

    use crate::domain::port::{MockMusicGroupRepository, MockUserRepository};

    use crate::test_helpers::*;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = CreateMusicGroup<TestFactory>;

    /// Build a group named `name` with an optional cover.
    fn group(
        id: NumericID,
        name: &str,
        cover_image_id: Option<NumericID>,
    ) -> Result<MusicGroup, Box<dyn Error>> {
        Ok(MusicGroup::try_new(id, name.to_owned(), cover_image_id)?)
    }

    #[tokio::test]
    async fn create_music_group_surrounding_whitespace_returns_invalid_name()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: NumericID|
              -> Result<(), Box<dyn Error>> {
            let caller =
                User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        groups.expect_search().times(0);
        groups.expect_create().times(0);
        let harness = (move |users: MockUserRepository,
                             groups: MockMusicGroupRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                music_groups: groups,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, groups);
        let use_case: UseCase = CreateMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMusicGroupCommand::new(
                1,
                " The Group ".to_owned(),
                Some(19),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateMusicGroupError::InvalidName)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_music_group_duplicate_name_returns_name_already_exists()
    -> Result<(), Box<dyn Error>> {
        // Arrange: a group already carries the name.
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: NumericID|
              -> Result<(), Box<dyn Error>> {
            let caller =
                User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        let existing = group(3, "The Group", None)?;
        groups.expect_search().times(1).return_once(move |_| {
            let found = vec![existing.clone()];
            Box::pin(async move { Ok(found) })
        });
        let harness = (move |users: MockUserRepository,
                             groups: MockMusicGroupRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                music_groups: groups,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, groups);
        let use_case: UseCase = CreateMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMusicGroupCommand::new(
                1,
                "The Group".to_owned(),
                None,
            ))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(CreateMusicGroupError::NameAlreadyExists)
        ));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_music_group_empty_name_returns_invalid_name() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: NumericID|
              -> Result<(), Box<dyn Error>> {
            let caller =
                User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let harness = (move |users: MockUserRepository,
                             groups: MockMusicGroupRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                music_groups: groups,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, MockMusicGroupRepository::new());
        let use_case: UseCase = CreateMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMusicGroupCommand::new(1, "   ".to_owned(), None))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateMusicGroupError::InvalidName)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_music_group_too_long_name_returns_name_too_long() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: NumericID|
              -> Result<(), Box<dyn Error>> {
            let caller =
                User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let harness = (move |users: MockUserRepository,
                             groups: MockMusicGroupRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                music_groups: groups,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, MockMusicGroupRepository::new());
        let use_case: UseCase = CreateMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMusicGroupCommand::new(1, "x".repeat(101), None))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateMusicGroupError::NameTooLong)));
        Ok(())
    }

    #[tokio::test]
    async fn create_music_group_user_cover_returns_invalid_cover() -> Result<(), Box<dyn Error>> {
        // Arrange: the repository reports the USER image as a trigger conflict.
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: NumericID|
              -> Result<(), Box<dyn Error>> {
            let caller =
                User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        groups
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        groups
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::Conflict) }));
        let harness = (move |users: MockUserRepository,
                             groups: MockMusicGroupRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                music_groups: groups,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, groups);
        let use_case: UseCase = CreateMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMusicGroupCommand::new(
                1,
                "The Group".to_owned(),
                Some(19),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateMusicGroupError::InvalidCover)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_music_group_unknown_cover_returns_invalid_cover() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the named identifier resolves to no image, so the cover rule
        // refuses before the repository is reached.
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: NumericID|
              -> Result<(), Box<dyn Error>> {
            let caller =
                User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        groups
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        groups.expect_create().times(0);
        let harness = (move |users: MockUserRepository,
                             groups: MockMusicGroupRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                music_groups: groups,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work).with_unknown_images()
        })(users, groups);
        let use_case: UseCase = CreateMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMusicGroupCommand::new(
                1,
                "The Group".to_owned(),
                Some(19),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateMusicGroupError::InvalidCover)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_music_group_concurrent_duplicate_returns_name_already_exists()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the pre-check misses, but the unique constraint wins the race.
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: NumericID|
              -> Result<(), Box<dyn Error>> {
            let caller =
                User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        groups
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        groups
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::AlreadyExist) }));
        let harness = (move |users: MockUserRepository,
                             groups: MockMusicGroupRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                music_groups: groups,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, groups);
        let use_case: UseCase = CreateMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMusicGroupCommand::new(
                1,
                "The Group".to_owned(),
                None,
            ))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(CreateMusicGroupError::NameAlreadyExists)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn create_music_group_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = (move |users: MockUserRepository,
                             groups: MockMusicGroupRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                music_groups: groups,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, MockMusicGroupRepository::new());
        let use_case: UseCase = CreateMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMusicGroupCommand::new(
                999,
                "The Group".to_owned(),
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateMusicGroupError::NoSuchCaller)));
        Ok(())
    }

    #[tokio::test]
    async fn create_music_group_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut MockUserRepository,
               caller_id: NumericID|
              -> Result<(), Box<dyn Error>> {
            let caller =
                User::try_new(caller_id, "admin".to_owned(), None, caller_id, Role::Admin)?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        groups
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let stored = group(7, "The Group", None)?;
        groups.expect_create().times(1).return_once(move |_| {
            let created = stored.clone();
            Box::pin(async move { Ok(created) })
        });
        let harness = (move |users: MockUserRepository,
                             groups: MockMusicGroupRepository|
              -> GalleryFactoryHarness {
            let unit_of_work = TestUnitOfWork {
                music_groups: groups,
                users,
                ..TestUnitOfWork::default()
            };
            GalleryFactoryHarness::new(unit_of_work)
        })(users, groups);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = CreateMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMusicGroupCommand::new(
                1,
                "The Group".to_owned(),
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateMusicGroupError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
}
