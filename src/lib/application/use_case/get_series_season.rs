use crate::application::authorization::authorize;
use crate::domain::model::{ResourceKind, ResourceRef};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{
    GetSeriesSeasonCommand, GetSeriesSeasonError, GetSeriesSeasonResponse, GetSeriesSeasonUseCase,
};
use crate::domain::model::Action;
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, SeriesFilter, SeriesRepository as _, UnitOfWork as _,
    UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork,
};

/// Use case implementation for fetching one season of a show.
pub(crate) struct GetSeriesSeason<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetSeriesSeason<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetSeriesSeasonUseCase for GetSeriesSeason<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: GetSeriesSeasonCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetSeriesSeasonResponse, GetSeriesSeasonError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetSeriesSeasonError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| GetSeriesSeasonError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetSeriesSeasonError::NoSuchCaller)?;

                let shown = unit_of_work
                    .series()
                    .search(&SeriesFilter {
                        id: Some(command.series_id()),
                        ..SeriesFilter::default()
                    })
                    .await
                    .map_err(|error| GetSeriesSeasonError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetSeriesSeasonError::NoSuchSeries)?;
                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::Series, shown.id()),
                    Action::View,
                    None,
                )
                .await
                .map_err(|_| GetSeriesSeasonError::NoSuchSeries)?;

                let season = shown
                    .seasons()
                    .iter()
                    .find(|season| season.index().value() == command.season_index())
                    .ok_or(GetSeriesSeasonError::NoSuchSeason)?;

                Ok(GetSeriesSeasonResponse::new(
                    shown.id(),
                    season.index().value(),
                    season.name().map(str::to_owned),
                    season.episodes().len(),
                    season.cover_image_id(),
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetSeriesSeasonError::Unknown(error.into()))?;
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
    use crate::domain::model::{Season, SeasonIndex, Series};
    use allure_cargotest::allure_test;

    use crate::domain::port::{MockSeriesRepository, MockUserRepository, RepositoryError};

    use crate::test_helpers::*;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = GetSeriesSeason<TestFactory>;

    /// Build a Library test harness around the given mocks.
    fn harness_with(
        users: MockUserRepository,
        series: MockSeriesRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            resources: no_resources(),
            series,
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    #[allure_test]
    #[tokio::test]
    async fn get_series_season_returns_the_requested_season() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-004");
        allure.story("SCN-SERIES-004-A");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let season = sample_season(1, vec![sample_episode(1, 11)?]);
        let shown = sample_series(3, vec![season])?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(3))
            .return_once(move |_| Box::pin(async move { Ok(vec![shown]) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = GetSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(GetSeriesSeasonCommand::new(1, 3, 1))
            .await?;

        // Assert
        assert_eq!(
            response,
            GetSeriesSeasonResponse::new(3, 1, None, 1, None),
            "the representation must carry the season summary"
        );
        assert!(
            harness.committed.load(Ordering::SeqCst),
            "a successful read must commit"
        );
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn get_series_season_carries_the_season_cover() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-004");
        allure.story("SCN-SERIES-004-A");
        // Arrange: the season carries its own artwork.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let season = Season::new(SeasonIndex::new(1), None, Some(20), Vec::new());
        assert_eq!(
            season.cover_image_id(),
            Some(20),
            "a season built with artwork must report the identifier"
        );
        let shown = Series::try_new(
            3,
            "Series 3".to_owned(),
            "A plot".to_owned(),
            Some("US".to_owned()),
            Some(video_date()),
            Vec::new(),
            None,
            vec![season],
        )?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![shown]) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = GetSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(GetSeriesSeasonCommand::new(1, 3, 1))
            .await?;

        // Assert
        assert_eq!(response.cover_image_id(), Some(20));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn get_series_season_unknown_show_returns_no_such_series() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-004");
        allure.story("SCN-SERIES-004-C");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = GetSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(GetSeriesSeasonCommand::new(1, 404, 1))
            .await;

        // Assert
        assert!(
            matches!(result, Err(GetSeriesSeasonError::NoSuchSeries)),
            "an unknown show must be reported"
        );
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn get_series_season_unknown_season_returns_no_such_season() -> Result<(), Box<dyn Error>>
    {
        allure.epic("Series");
        allure.feature("REQ-SERIES-004");
        allure.story("SCN-SERIES-004-C");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let shown = sample_series(3, vec![sample_season(1, Vec::new())])?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![shown]) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = GetSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetSeriesSeasonCommand::new(1, 3, 9)).await;

        // Assert
        assert!(
            matches!(result, Err(GetSeriesSeasonError::NoSuchSeason)),
            "a season the show does not hold must be reported"
        );
        assert!(
            harness.rolled_back.load(Ordering::SeqCst),
            "a missing season must roll back"
        );
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn get_series_season_scopes_the_index_to_the_series() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-004");
        allure.story("SCN-SERIES-004-B");
        // Arrange: both shows hold a season 1, so the path must pick the one
        // belonging to the requested show.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let second_show = sample_series(2, vec![sample_season(1, vec![sample_episode(1, 21)?])])?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(2))
            .return_once(move |_| Box::pin(async move { Ok(vec![second_show]) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = GetSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(GetSeriesSeasonCommand::new(1, 2, 1))
            .await?;

        // Assert
        assert_eq!(
            response.series_id(),
            2,
            "the season must belong to the requested show"
        );
        assert_eq!(response.index(), 1, "the season index must be preserved");
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn get_series_season_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>>
    {
        allure.epic("Series");
        allure.feature("REQ-SERIES-004");
        allure.story("SCN-SERIES-004-A");
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockSeriesRepository::new());
        let use_case: UseCase = GetSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(GetSeriesSeasonCommand::new(999, 3, 1))
            .await;

        // Assert
        assert!(
            matches!(result, Err(GetSeriesSeasonError::NoSuchCaller)),
            "a deleted caller must be rejected"
        );
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn get_series_season_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-004");
        allure.story("SCN-SERIES-004-A");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = GetSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetSeriesSeasonCommand::new(1, 3, 1)).await;

        // Assert
        assert!(
            matches!(result, Err(GetSeriesSeasonError::Unknown(_))),
            "a repository failure must map to an unknown error"
        );
        Ok(())
    }
}
