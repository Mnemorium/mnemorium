use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::get_series_season::GetSeriesSeasonCommand;
use crate::application::port::get_series_season::GetSeriesSeasonError;
use crate::application::port::get_series_season::GetSeriesSeasonResponse;
use crate::application::port::get_series_season::GetSeriesSeasonUseCase;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::series_repository::SeriesFilter;
use crate::domain::port::series_repository::SeriesRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for fetching one season of a show.
pub struct GetSeriesSeason<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetSeriesSeason<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetSeriesSeasonUseCase for GetSeriesSeason<F>
where
    F: UnitOfWorkFactory,
    F::Uow: LibraryUnitOfWork + UserUnitOfWork,
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
                unit_of_work
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
                    season.cover_file_id(),
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
    use std::error::Error;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use crate::application::port::get_series_season::GetSeriesSeasonCommand;
    use crate::application::port::get_series_season::GetSeriesSeasonError;
    use crate::application::port::get_series_season::GetSeriesSeasonResponse;
    use crate::application::port::get_series_season::GetSeriesSeasonUseCase as _;
    use crate::domain::model::season::Season;
    use crate::domain::model::season::SeasonIndex;
    use crate::domain::model::series::Series;
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
    use crate::test_helpers::video_date;

    use super::GetSeriesSeason;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = GetSeriesSeason<TestFactory>;

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
    async fn get_series_season_returns_the_requested_season() -> Result<(), Box<dyn Error>> {
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

    #[tokio::test]
    async fn get_series_season_carries_the_season_cover() -> Result<(), Box<dyn Error>> {
        // Arrange: the season carries its own artwork.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let season = Season::new(SeasonIndex::new(1), None, Some(20), Vec::new());
        assert_eq!(
            season.cover_file_id(),
            Some(20),
            "a season built with artwork must report the identifier"
        );
        let shown = Series::try_new(
            3,
            "Series 3".to_owned(),
            "A plot".to_owned(),
            "US".to_owned(),
            video_date(),
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
        assert_eq!(response.cover_file_id(), Some(20));
        Ok(())
    }

    #[tokio::test]
    async fn get_series_season_unknown_show_returns_no_such_series() -> Result<(), Box<dyn Error>> {
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

    #[tokio::test]
    async fn get_series_season_unknown_season_returns_no_such_season() -> Result<(), Box<dyn Error>>
    {
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

    #[tokio::test]
    async fn get_series_season_scopes_the_index_to_the_series() -> Result<(), Box<dyn Error>> {
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

    #[tokio::test]
    async fn get_series_season_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>>
    {
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

    #[tokio::test]
    async fn get_series_season_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
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
