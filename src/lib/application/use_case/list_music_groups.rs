use crate::application::authorization::allows;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{
    ListMusicGroupsCommand, ListMusicGroupsError, ListMusicGroupsItem, ListMusicGroupsResponse,
    ListMusicGroupsUseCase,
};
use crate::domain::model::{Action, ResourceKind, ResourceRef};
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, MusicGroupFilter, MusicGroupRepository as _,
    UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork,
};

/// Use case implementation for listing the music groups the caller may see.
pub(crate) struct ListMusicGroups<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> ListMusicGroups<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> ListMusicGroupsUseCase for ListMusicGroups<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: ListMusicGroupsCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ListMusicGroupsResponse, ListMusicGroupsError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| ListMusicGroupsError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| ListMusicGroupsError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ListMusicGroupsError::NoSuchCaller)?;

                let mut groups = unit_of_work
                    .music_groups()
                    .search(&MusicGroupFilter {
                        stage_name: command.name().map(str::to_owned),
                        ..MusicGroupFilter::default()
                    })
                    .await
                    .map_err(|error| ListMusicGroupsError::Unknown(error.into()))?;

                let mut visible = Vec::with_capacity(groups.len());
                for group in groups {
                    if allows(
                        &mut unit_of_work.resources(),
                        &caller,
                        ResourceRef::new(ResourceKind::MusicGroup, group.id()),
                        Action::Discover,
                        None,
                    )
                    .await
                    {
                        visible.push(group);
                    }
                }
                groups = visible;

                let total = groups.len();
                let offset = command.offset().unwrap_or(0);
                let page = groups
                    .into_iter()
                    .skip(offset)
                    .take(command.limit().unwrap_or(usize::MAX))
                    .map(|group| {
                        ListMusicGroupsItem::new(
                            group.id(),
                            group.stage_name().to_owned(),
                            group.cover_image_id(),
                        )
                    })
                    .collect();

                Ok(ListMusicGroupsResponse::new(page, total))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| ListMusicGroupsError::Unknown(error.into()))?;
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
    type UseCase = ListMusicGroups<TestFactory>;

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
            resources: no_resources(),
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    #[tokio::test]
    async fn list_music_groups_returns_the_page() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        let first = group(1, "The Group", Some(19))?;
        let second = group(2, "Another Group", None)?;
        groups
            .expect_search()
            .times(1)
            .withf(|filter| filter.stage_name.is_none())
            .return_once(move |_| {
                let found = vec![first.clone(), second.clone()];
                Box::pin(async move { Ok(found) })
            });
        let harness = harness_with(users, groups);
        let use_case: UseCase = ListMusicGroups::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListMusicGroupsCommand::new(1, None, None, None))
            .await?;

        // Assert
        assert_eq!(
            response,
            ListMusicGroupsResponse::new(
                vec![
                    ListMusicGroupsItem::new(1, "The Group".to_owned(), Some(19)),
                    ListMusicGroupsItem::new(2, "Another Group".to_owned(), None),
                ],
                2,
            )
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_music_groups_forwards_the_filter_and_paginates() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        let first = group(1, "Group 1", None)?;
        let second = group(2, "Group 2", None)?;
        let third = group(3, "Group 3", None)?;
        groups
            .expect_search()
            .times(1)
            .withf(|filter| filter.stage_name.as_deref() == Some("Group"))
            .return_once(move |_| {
                let found = vec![first.clone(), second.clone(), third.clone()];
                Box::pin(async move { Ok(found) })
            });
        let harness = harness_with(users, groups);
        let use_case: UseCase = ListMusicGroups::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListMusicGroupsCommand::new(
                1,
                Some("Group".to_owned()),
                Some(1),
                Some(1),
            ))
            .await?;

        // Assert
        assert_eq!(response.total(), 3);
        assert_eq!(
            response.groups().first().map(ListMusicGroupsItem::group_id),
            Some(2)
        );
        Ok(())
    }
    #[tokio::test]
    async fn list_music_groups_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockMusicGroupRepository::new());
        let use_case: UseCase = ListMusicGroups::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListMusicGroupsCommand::new(999, None, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(ListMusicGroupsError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_music_groups_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        groups
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, groups);
        let use_case: UseCase = ListMusicGroups::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListMusicGroupsCommand::new(1, None, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(ListMusicGroupsError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn list_music_groups_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut groups = MockMusicGroupRepository::new();
        groups
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, groups);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = ListMusicGroups::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListMusicGroupsCommand::new(1, None, None, None))
            .await;

        // Assert
        assert!(matches!(result, Err(ListMusicGroupsError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }
}
