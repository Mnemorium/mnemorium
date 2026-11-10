use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tracing::warn;

use crate::application::port::get_music_group::{
    GetMusicGroupCommand, GetMusicGroupError, GetMusicGroupResponse, GetMusicGroupUseCase,
};
use crate::domain::model::permission::Action;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::music_group_repository::MusicGroupRepository as _;
use crate::domain::port::permission_repository::{PermissionFilter, PermissionRepository as _};
use crate::domain::port::unit_of_work::{UnitOfWork as _, UnitOfWorkFactory};
use crate::domain::port::user_repository::{UserFilter, UserRepository as _};
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::authorization::{Authorization, Decision};

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

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| GetMusicGroupError::Unknown(error.into()))?;

                let group = unit_of_work
                    .music_groups()
                    .get(command.group_id())
                    .await
                    .map_err(|error| GetMusicGroupError::Unknown(error.into()))?
                    .ok_or(GetMusicGroupError::NoSuchGroup)?;
                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::LibraryRead, &group)
                {
                    warn!(
                        target: "security",
                        event = "authz_failed",
                        actor = caller.id(),
                        action = %denial.action().verb(),
                        target = %denial.action().target(),
                        "denied an action"
                    );
                    return Err(GetMusicGroupError::NoSuchGroup);
                }

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
    use crate::domain::alias::NumericID;
    use crate::domain::model::music_group::MusicGroup;
    use crate::domain::model::permission::{Permission, Scope};
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::music_group_repository::MockMusicGroupRepository;
    use crate::domain::port::permission_repository::MockPermissionRepository;
    use crate::domain::port::user_repository::MockUserRepository;
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

    /// Build a Library test harness around the given mocks.
    fn harness_with(
        users: MockUserRepository,
        groups: MockMusicGroupRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            music_groups: groups,
            permissions: no_permissions(),
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    #[tokio::test]
    async fn get_music_group_returns_the_requested_group() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        let expected = group(7, "The Group", Some(19))?;
        groups
            .expect_get()
            .times(1)
            .withf(|group_id| *group_id == 7)
            .return_once(move |_| {
                let found = expected.clone();
                Box::pin(async move { Ok(Some(found)) })
            });
        let harness = harness_with(users, groups);
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
        expect_caller(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        groups
            .expect_get()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));
        let harness = harness_with(users, groups);
        let use_case: UseCase = GetMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetMusicGroupCommand::new(1, 404)).await;

        // Assert
        assert!(matches!(result, Err(GetMusicGroupError::NoSuchGroup)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_music_group_none_override_returns_no_such_group() -> Result<(), Box<dyn Error>> {
        // Arrange: the caller is shut out of the library entirely.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        let hidden = group(7, "The Group", None)?;
        groups.expect_get().times(1).return_once(move |_| {
            let found = hidden.clone();
            Box::pin(async move { Ok(Some(found)) })
        });
        let mut permissions = MockPermissionRepository::new();
        expect_permissions(
            &mut permissions,
            1,
            vec![Permission::try_new(1, Action::LibraryRead, Scope::None)?],
        );
        let unit_of_work = TestUnitOfWork {
            music_groups: groups,
            permissions,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
        let use_case: UseCase = GetMusicGroup::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetMusicGroupCommand::new(1, 7)).await;

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
        let harness = harness_with(users, MockMusicGroupRepository::new());
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
        expect_caller(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        groups
            .expect_get()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, groups);
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
        expect_caller(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        let stored = group(7, "The Group", None)?;
        groups.expect_get().times(1).return_once(move |_| {
            let found = stored.clone();
            Box::pin(async move { Ok(Some(found)) })
        });
        let harness = harness_with(users, groups);
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
