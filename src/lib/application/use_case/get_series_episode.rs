use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::get_series_episode::GetSeriesEpisodeCommand;
use crate::application::port::get_series_episode::GetSeriesEpisodeError;
use crate::application::port::get_series_episode::GetSeriesEpisodeResponse;
use crate::application::port::get_series_episode::GetSeriesEpisodeUseCase;
use crate::application::port::video_summary::VideoSummary;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::series_repository::SeriesFilter;
use crate::domain::port::series_repository::SeriesRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for fetching one episode of a season.
pub struct GetSeriesEpisode<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetSeriesEpisode<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetSeriesEpisodeUseCase for GetSeriesEpisode<F>
where
    F: UnitOfWorkFactory,
    F::Uow: LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: GetSeriesEpisodeCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetSeriesEpisodeResponse, GetSeriesEpisodeError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetSeriesEpisodeError::Unknown(error.into()))?;

            let result = async {
                unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| GetSeriesEpisodeError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetSeriesEpisodeError::NoSuchCaller)?;

                let shown = unit_of_work
                    .series()
                    .search(&SeriesFilter {
                        id: Some(command.series_id()),
                        ..SeriesFilter::default()
                    })
                    .await
                    .map_err(|error| GetSeriesEpisodeError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetSeriesEpisodeError::NoSuchSeries)?;

                let season = shown
                    .seasons()
                    .iter()
                    .find(|season| season.index().value() == command.season_index())
                    .ok_or(GetSeriesEpisodeError::NoSuchSeason)?;

                let episode = season
                    .episodes()
                    .iter()
                    .find(|episode| episode.index().value() == command.episode_index())
                    .ok_or(GetSeriesEpisodeError::NoSuchEpisode)?;

                Ok(GetSeriesEpisodeResponse::new(
                    episode.index().value(),
                    shown.id(),
                    season.index().value(),
                    episode.name().to_owned(),
                    episode.plot().to_owned(),
                    episode.air_date(),
                    VideoSummary::from(episode.video()),
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetSeriesEpisodeError::Unknown(error.into()))?;
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

    use crate::application::port::get_series_episode::GetSeriesEpisodeCommand;
    use crate::application::port::get_series_episode::GetSeriesEpisodeError;
    use crate::application::port::get_series_episode::GetSeriesEpisodeResponse;
    use crate::application::port::get_series_episode::GetSeriesEpisodeUseCase as _;
    use crate::application::port::video_summary::VideoSummary;
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

    use super::GetSeriesEpisode;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = GetSeriesEpisode<TestFactory>;

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
    async fn get_series_episode_returns_the_requested_episode() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let episode = sample_episode(2, 12)?;
        let expected_video = VideoSummary::from(episode.video());
        let shown = sample_series(
            3,
            vec![sample_season(1, vec![sample_episode(1, 11)?, episode])],
        )?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(3))
            .return_once(move |_| Box::pin(async move { Ok(vec![shown]) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = GetSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(GetSeriesEpisodeCommand::new(1, 3, 1, 2))
            .await?;

        // Assert
        assert_eq!(
            response,
            GetSeriesEpisodeResponse::new(
                2,
                3,
                1,
                "Episode 2".to_owned(),
                "A plot".to_owned(),
                video_date(),
                expected_video,
            ),
            "the representation must carry the requested episode"
        );
        assert!(
            harness.committed.load(Ordering::SeqCst),
            "a successful read must commit"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_episode_unknown_show_returns_no_such_series() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = GetSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(GetSeriesEpisodeCommand::new(1, 404, 1, 1))
            .await;

        // Assert
        assert!(
            matches!(result, Err(GetSeriesEpisodeError::NoSuchSeries)),
            "an unknown show must be reported"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_episode_unknown_season_returns_no_such_season() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let shown = sample_series(3, vec![sample_season(1, vec![sample_episode(1, 11)?])])?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![shown]) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = GetSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(GetSeriesEpisodeCommand::new(1, 3, 9, 1))
            .await;

        // Assert
        assert!(
            matches!(result, Err(GetSeriesEpisodeError::NoSuchSeason)),
            "a season the show does not hold must be reported"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_episode_outside_the_season_returns_no_such_episode()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let shown = sample_series(3, vec![sample_season(1, vec![sample_episode(1, 11)?])])?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![shown]) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = GetSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(GetSeriesEpisodeCommand::new(1, 3, 1, 7))
            .await;

        // Assert
        assert!(
            matches!(result, Err(GetSeriesEpisodeError::NoSuchEpisode)),
            "an episode the season does not hold must be reported"
        );
        assert!(
            harness.rolled_back.load(Ordering::SeqCst),
            "a missing episode must roll back"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_episode_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockSeriesRepository::new());
        let use_case: UseCase = GetSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(GetSeriesEpisodeCommand::new(999, 3, 1, 1))
            .await;

        // Assert
        assert!(
            matches!(result, Err(GetSeriesEpisodeError::NoSuchCaller)),
            "a deleted caller must be rejected"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_series_episode_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = GetSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(GetSeriesEpisodeCommand::new(1, 3, 1, 1))
            .await;

        // Assert
        assert!(
            matches!(result, Err(GetSeriesEpisodeError::Unknown(_))),
            "a repository failure must map to an unknown error"
        );
        Ok(())
    }
}
