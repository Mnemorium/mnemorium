use crate::application::authorization::authorize;
use crate::domain::model::{ResourceKind, ResourceRef};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{
    ListSeriesEpisodesCommand, ListSeriesEpisodesError, ListSeriesEpisodesItem,
    ListSeriesEpisodesResponse, ListSeriesEpisodesUseCase,
};
use crate::domain::model::Action;
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, SeriesFilter, SeriesRepository as _, UnitOfWork as _,
    UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork,
};

/// Use case implementation for listing the episodes of one season.
pub(crate) struct ListSeriesEpisodes<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> ListSeriesEpisodes<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> ListSeriesEpisodesUseCase for ListSeriesEpisodes<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: ListSeriesEpisodesCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ListSeriesEpisodesResponse, ListSeriesEpisodesError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| ListSeriesEpisodesError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| ListSeriesEpisodesError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ListSeriesEpisodesError::NoSuchCaller)?;

                let shown = unit_of_work
                    .series()
                    .search(&SeriesFilter {
                        id: Some(command.series_id()),
                        ..SeriesFilter::default()
                    })
                    .await
                    .map_err(|error| ListSeriesEpisodesError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ListSeriesEpisodesError::NoSuchSeries)?;
                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::Series, shown.id()),
                    Action::View,
                    None,
                )
                .await
                .map_err(|_| ListSeriesEpisodesError::NoSuchSeries)?;

                let season = shown
                    .seasons()
                    .iter()
                    .find(|season| season.index().value() == command.season_index())
                    .ok_or(ListSeriesEpisodesError::NoSuchSeason)?;

                let episodes = season
                    .episodes()
                    .iter()
                    .map(|episode| {
                        ListSeriesEpisodesItem::new(
                            episode.index().value(),
                            shown.id(),
                            season.index().value(),
                            episode.name().to_owned(),
                            episode.plot().to_owned(),
                            episode.air_date(),
                            episode.video().clone(),
                        )
                    })
                    .collect();

                Ok(ListSeriesEpisodesResponse::new(episodes))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| ListSeriesEpisodesError::Unknown(error.into()))?;
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
    use crate::domain::port::{MockSeriesRepository, MockUserRepository, RepositoryError};
    use allure_cargotest::allure_test;

    use crate::test_helpers::*;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = ListSeriesEpisodes<TestFactory>;

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
    async fn list_series_episodes_returns_the_season_in_index_order() -> Result<(), Box<dyn Error>>
    {
        allure.epic("Series");
        allure.feature("REQ-SERIES-005");
        allure.story("SCN-SERIES-005-A");
        allure.story("SCN-SERIES-007-A");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let first = sample_episode(1, 11)?;
        let second = sample_episode(2, 12)?;
        let first_video = first.video().clone();
        let second_video = second.video().clone();
        let shown = sample_series(3, vec![sample_season(1, vec![first, second])])?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(3))
            .return_once(move |_| Box::pin(async move { Ok(vec![shown]) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = ListSeriesEpisodes::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListSeriesEpisodesCommand::new(1, 3, 1))
            .await?;

        // Assert
        assert_eq!(
            response.episodes(),
            [
                ListSeriesEpisodesItem::new(
                    1,
                    3,
                    1,
                    "Episode 1".to_owned(),
                    "A plot".to_owned(),
                    Some(video_date()),
                    first_video,
                ),
                ListSeriesEpisodesItem::new(
                    2,
                    3,
                    1,
                    "Episode 2".to_owned(),
                    "A plot".to_owned(),
                    Some(video_date()),
                    second_video,
                ),
            ],
            "the episodes must be in ascending index order"
        );
        assert!(
            harness.committed.load(Ordering::SeqCst),
            "a successful listing must commit"
        );
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn list_series_episodes_of_an_empty_season_returns_none() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-005");
        allure.story("SCN-SERIES-005-A");
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
        let use_case: UseCase = ListSeriesEpisodes::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListSeriesEpisodesCommand::new(1, 3, 1))
            .await?;

        // Assert
        assert!(
            response.episodes().is_empty(),
            "a season without an episode must report none"
        );
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn list_series_episodes_unknown_show_returns_no_such_series() -> Result<(), Box<dyn Error>>
    {
        allure.epic("Series");
        allure.feature("REQ-SERIES-005");
        allure.story("SCN-SERIES-005-C");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = ListSeriesEpisodes::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListSeriesEpisodesCommand::new(1, 404, 1))
            .await;

        // Assert
        assert!(
            matches!(result, Err(ListSeriesEpisodesError::NoSuchSeries)),
            "an unknown show must be reported"
        );
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn list_series_episodes_unknown_season_returns_no_such_season()
    -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-005");
        allure.story("SCN-SERIES-005-C");
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
        let use_case: UseCase = ListSeriesEpisodes::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListSeriesEpisodesCommand::new(1, 3, 9))
            .await;

        // Assert
        assert!(
            matches!(result, Err(ListSeriesEpisodesError::NoSuchSeason)),
            "a season the show does not hold must be reported"
        );
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn list_series_episodes_unknown_caller_returns_no_such_caller()
    -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-005");
        allure.story("SCN-SERIES-005-A");
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockSeriesRepository::new());
        let use_case: UseCase = ListSeriesEpisodes::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListSeriesEpisodesCommand::new(999, 3, 1))
            .await;

        // Assert
        assert!(
            matches!(result, Err(ListSeriesEpisodesError::NoSuchCaller)),
            "a deleted caller must be rejected"
        );
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn list_series_episodes_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>>
    {
        allure.epic("Series");
        allure.feature("REQ-SERIES-005");
        allure.story("SCN-SERIES-005-A");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, series);
        let use_case: UseCase = ListSeriesEpisodes::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListSeriesEpisodesCommand::new(1, 3, 1))
            .await;

        // Assert
        assert!(
            matches!(result, Err(ListSeriesEpisodesError::Unknown(_))),
            "a repository failure must map to an unknown error"
        );
        Ok(())
    }
}
