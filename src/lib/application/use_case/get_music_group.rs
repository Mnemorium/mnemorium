use crate::application::authorization::authorize;
use crate::domain::model::{ResourceKind, ResourceRef};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{
    GetMusicGroupCommand, GetMusicGroupError, GetMusicGroupResponse, GetMusicGroupUseCase,
};
use crate::domain::model::Action;
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, MusicGroupFilter, MusicGroupRepository as _,
    UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork,
};

/// Use case implementation for fetching one music group.
pub(crate) struct GetMusicGroup<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetMusicGroup<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetMusicGroupUseCase for GetMusicGroup<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: GetMusicGroupCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetMusicGroupResponse, GetMusicGroupError>> + Send + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetMusicGroupError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| GetMusicGroupError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetMusicGroupError::NoSuchCaller)?;

                let group = unit_of_work
                    .music_groups()
                    .search(&MusicGroupFilter {
                        id: Some(command.group_id()),
                        ..MusicGroupFilter::default()
                    })
                    .await
                    .map_err(|error| GetMusicGroupError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetMusicGroupError::NoSuchGroup)?;
                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::MusicGroup, group.id()),
                    Action::View,
                    None,
                )
                .await
                .map_err(|_| GetMusicGroupError::NoSuchGroup)?;

                Ok(GetMusicGroupResponse::new(
                    group.stage_name().to_owned(),
                    group.id(),
                    group.cover_image_id(),
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetMusicGroupError::Unknown(error.into()))?;
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

    use crate::domain::model::MusicGroup;

    use crate::domain::port::{MockMusicGroupRepository, MockUserRepository, RepositoryError};

    use crate::test_helpers::*;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = GetMusicGroup<TestFactory>;

    /// Build a group named `name` with an optional cover.
    fn group(
        id: NumericID,
        name: &str,
        cover_image_id: Option<NumericID>,
    ) -> Result<MusicGroup, Box<dyn Error>> {
        Ok(MusicGroup::try_new(id, name.to_owned(), cover_image_id)?)
    }

    #[tokio::test]
    async fn get_music_group_returns_the_requested_group() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut crate::domain::port::MockUserRepository,
               caller_id: crate::domain::NumericID|
              -> Result<(), Box<dyn std::error::Error>> {
            let caller = crate::domain::model::User::try_new(
                caller_id,
                "caller".to_owned(),
                None,
                caller_id,
                crate::domain::model::Role::Standard,
            )?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        let expected = group(7, "The Group", Some(19))?;
        groups
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(7))
            .return_once(move |_| {
                let found = expected.clone();
                Box::pin(async move { Ok(vec![found]) })
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
        let use_case: UseCase = GetMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(GetMusicGroupCommand::new(1, 7)).await?;

        // Assert
        assert_eq!(
            response,
            GetMusicGroupResponse::new("The Group".to_owned(), 7, Some(19)),
            "the representation must carry the requested group"
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_music_group_unknown_group_returns_no_such_group() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut crate::domain::port::MockUserRepository,
               caller_id: crate::domain::NumericID|
              -> Result<(), Box<dyn std::error::Error>> {
            let caller = crate::domain::model::User::try_new(
                caller_id,
                "caller".to_owned(),
                None,
                caller_id,
                crate::domain::model::Role::Standard,
            )?;
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
        let use_case: UseCase = GetMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetMusicGroupCommand::new(1, 404)).await;

        // Assert
        assert!(matches!(result, Err(GetMusicGroupError::NoSuchGroup)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
    #[tokio::test]
    async fn get_music_group_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
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
        let use_case: UseCase = GetMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetMusicGroupCommand::new(999, 7)).await;

        // Assert
        assert!(matches!(result, Err(GetMusicGroupError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_music_group_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut crate::domain::port::MockUserRepository,
               caller_id: crate::domain::NumericID|
              -> Result<(), Box<dyn std::error::Error>> {
            let caller = crate::domain::model::User::try_new(
                caller_id,
                "caller".to_owned(),
                None,
                caller_id,
                crate::domain::model::Role::Standard,
            )?;
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
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
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
        let use_case: UseCase = GetMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetMusicGroupCommand::new(1, 7)).await;

        // Assert
        assert!(matches!(result, Err(GetMusicGroupError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_music_group_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        (move |users: &mut crate::domain::port::MockUserRepository,
               caller_id: crate::domain::NumericID|
              -> Result<(), Box<dyn std::error::Error>> {
            let caller = crate::domain::model::User::try_new(
                caller_id,
                "caller".to_owned(),
                None,
                caller_id,
                crate::domain::model::Role::Standard,
            )?;
            users
                .expect_search()
                .times(1)
                .withf(move |filter| filter.id == Some(caller_id))
                .return_once(move |_| Box::pin(async move { Ok(vec![caller]) }));
            Ok(())
        })(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        let stored = group(7, "The Group", None)?;
        groups.expect_search().times(1).return_once(move |_| {
            let found = stored.clone();
            Box::pin(async move { Ok(vec![found]) })
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
        let use_case: UseCase = GetMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetMusicGroupCommand::new(1, 7)).await;

        // Assert
        assert!(matches!(result, Err(GetMusicGroupError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
}
