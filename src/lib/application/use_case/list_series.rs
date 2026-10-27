use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::list_series::ListSeriesCommand;
use crate::application::port::list_series::ListSeriesError;
use crate::application::port::list_series::ListSeriesItem;
use crate::application::port::list_series::ListSeriesResponse;
use crate::application::port::list_series::ListSeriesUseCase;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::series_repository::SeriesFilter;
use crate::domain::port::series_repository::SeriesRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for listing the shows the caller may see.
pub struct ListSeries<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> ListSeries<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> ListSeriesUseCase for ListSeries<F>
where
    F: UnitOfWorkFactory,
    F::Uow: LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: ListSeriesCommand,
    ) -> Pin<Box<dyn Future<Output = Result<ListSeriesResponse, ListSeriesError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| ListSeriesError::Unknown(error.into()))?;

            let result = async {
                unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| ListSeriesError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ListSeriesError::NoSuchCaller)?;

                let series = unit_of_work
                    .series()
                    .search(&SeriesFilter {
                        name: command.name().map(str::to_owned),
                        ..SeriesFilter::default()
                    })
                    .await
                    .map_err(|error| ListSeriesError::Unknown(error.into()))?;

                let total = series.len();
                let offset = command.offset().unwrap_or(0);
                let page = series
                    .into_iter()
                    .skip(offset)
                    .take(command.limit().unwrap_or(usize::MAX))
                    .map(|shown| ListSeriesItem::new(shown.id(), shown.name().to_owned()))
                    .collect();

                Ok(ListSeriesResponse::new(page, total))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| ListSeriesError::Unknown(error.into()))?;
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
    use std::error::Error;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use crate::application::port::list_series::ListSeriesCommand;
    use crate::application::port::list_series::ListSeriesError;
    use crate::application::port::list_series::ListSeriesItem;
    use crate::application::port::list_series::ListSeriesUseCase as _;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::series_repository::MockSeriesRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;
    use crate::test_helpers::expect_caller;
    use crate::test_helpers::sample_episode;
    use crate::test_helpers::sample_season;
    use crate::test_helpers::sample_series;

    use super::ListSeries;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = ListSeries<TestFactory>;

    /// Build a Library test harness around the given mocks.
    fn harness_with(
        users: MockUserRepository,
        series: MockSeriesRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            series,
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    #[tokio::test]
    async fn list_series_projects_only_the_identifier_and_name() -> Result<(), Box<dyn Error>> {
        // Arrange: the aggregate carries a season, an episode, a genre and
        // dates, so the projection has something to drop.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let season = sample_season(1, vec![sample_episode(1, 11)?]);
        let first = sample_series(1, vec![season])?;
        let second = sample_series(2, Vec::new())?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .withf(|filter| filter.id.is_none() && filter.name.is_none())
            .return_once(move |_| Box::pin(async move { Ok(vec![first, second]) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = ListSeries::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListSeriesCommand::new(1, None, None, None))
            .await?;

        // Assert
        assert_eq!(response.total(), 2, "the total must count every show");
        assert_eq!(
            response.series(),
            [
                ListSeriesItem::new(1, "Series 1".to_owned()),
                ListSeriesItem::new(2, "Series 2".to_owned()),
            ],
            "the listing must carry only the identifier and name of each show"
        );
        assert!(
            harness.committed.load(Ordering::SeqCst),
            "a successful listing must commit"
        );
        Ok(())
    }

    #[tokio::test]
    async fn list_series_forwards_the_filter_and_paginates() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let first = sample_series(1, Vec::new())?;
        let second = sample_series(2, Vec::new())?;
        let third = sample_series(3, Vec::new())?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .withf(|filter| filter.name.as_deref() == Some("Series"))
            .return_once(move |_| Box::pin(async move { Ok(vec![first, second, third]) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = ListSeries::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListSeriesCommand::new(
                1,
                Some("Series".to_owned()),
                Some(1),
                Some(1),
            ))
            .await?;

        // Assert
        assert_eq!(response.total(), 3, "the total must precede pagination");
        assert_eq!(
            response.series().first().map(ListSeriesItem::id),
            Some(2),
            "the page must start at the requested offset"
        );
        Ok(())
    }

    #[tokio::test]
    async fn list_series_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockSeriesRepository::new());
        let use_case: UseCase = ListSeries::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListSeriesCommand::new(999, None, None, None))
            .await;

        // Assert
        assert!(
            matches!(result, Err(ListSeriesError::NoSuchCaller)),
            "a deleted caller must be rejected"
        );
        assert!(
            harness.rolled_back.load(Ordering::SeqCst),
            "a rejected caller must roll back"
        );
        Ok(())
    }

    #[tokio::test]
    async fn list_series_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = ListSeries::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListSeriesCommand::new(1, None, None, None))
            .await;

        // Assert
        assert!(
            matches!(result, Err(ListSeriesError::Unknown(_))),
            "a repository failure must map to an unknown error"
        );
        assert!(
            harness.rolled_back.load(Ordering::SeqCst),
            "a repository failure must roll back"
        );
        Ok(())
    }

    #[tokio::test]
    async fn list_series_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, series);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = ListSeries::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListSeriesCommand::new(1, None, None, None))
            .await;

        // Assert
        assert!(
            matches!(result, Err(ListSeriesError::Unknown(_))),
            "a commit failure must map to an unknown error"
        );
        assert!(
            harness.committed.load(Ordering::SeqCst),
            "the use case must attempt the commit"
        );
        Ok(())
    }
}
