use crate::application::authorization::authorize;
use crate::domain::model::{ResourceKind, ResourceRef};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tracing::warn;

use crate::application::port::{
    UpdateSeriesEpisodeCommand, UpdateSeriesEpisodeError, UpdateSeriesEpisodeResponse,
    UpdateSeriesEpisodeUseCase,
};
use crate::domain::model::{Action, Episode};
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, SeriesFilter, SeriesRepository as _, UnitOfWork as _,
    UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork,
};
use crate::domain::service::{NamePolicy, NamePolicyError};

/// Use case implementation for updating a season episode partially.
pub(crate) struct UpdateSeriesEpisode<F> {
    /// Factory opening the unit of work wrapping the update.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> UpdateSeriesEpisode<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> UpdateSeriesEpisodeUseCase for UpdateSeriesEpisode<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: UpdateSeriesEpisodeCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<UpdateSeriesEpisodeResponse, UpdateSeriesEpisodeError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| UpdateSeriesEpisodeError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| UpdateSeriesEpisodeError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(UpdateSeriesEpisodeError::NoSuchCaller)?;

                let series = unit_of_work
                    .series()
                    .search(&SeriesFilter {
                        id: Some(command.series_id()),
                        ..SeriesFilter::default()
                    })
                    .await
                    .map_err(|error| UpdateSeriesEpisodeError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(UpdateSeriesEpisodeError::NoSuchSeries)?;

                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::Series, series.id()),
                    Action::View,
                    None,
                )
                .await
                .map_err(|_| UpdateSeriesEpisodeError::NoSuchSeries)?;
                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::Series, series.id()),
                    Action::EditMetadata,
                    None,
                )
                .await
                .map_err(|error| {
                    if error.conceals() {
                        UpdateSeriesEpisodeError::NoSuchSeries
                    } else {
                        UpdateSeriesEpisodeError::Forbidden
                    }
                })?;

                let season = series
                    .seasons()
                    .iter()
                    .find(|season| season.index().value() == command.season_index())
                    .ok_or(UpdateSeriesEpisodeError::NoSuchSeason)?;
                let episode = season
                    .episodes()
                    .iter()
                    .find(|episode| episode.index().value() == command.episode_index())
                    .ok_or(UpdateSeriesEpisodeError::NoSuchEpisode)?;

                let name = match command.name() {
                    Some(requested) => {
                        NamePolicy::validate(requested.to_owned()).map_err(|error| match error {
                            NamePolicyError::Empty => {
                                warn!(
                                    target: "security",
                                    event = "input_validation_failed",
                                    field = %"name",
                                    reason = %"empty",
                                    "rejected a client-supplied value"
                                );
                                UpdateSeriesEpisodeError::InvalidName
                            }
                            NamePolicyError::TooLong => {
                                warn!(
                                    target: "security",
                                    event = "input_validation_failed",
                                    field = %"name",
                                    reason = %"too_long",
                                    "rejected a client-supplied value"
                                );
                                UpdateSeriesEpisodeError::NameTooLong
                            }
                        })?
                    }
                    None => episode.name().to_owned(),
                };

                let plot = command
                    .plot()
                    .map_or_else(|| episode.plot().to_owned(), str::to_owned);
                let air_date = command.air_date().resolve(episode.air_date());

                let updated = Episode::try_new(
                    episode.index(),
                    name,
                    plot,
                    air_date,
                    episode.video().clone(),
                )
                .map_err(|_| UpdateSeriesEpisodeError::InvalidName)?;

                let stored = unit_of_work
                    .series()
                    .save_episode(series.id(), command.season_index(), updated)
                    .await
                    .map_err(|error| UpdateSeriesEpisodeError::Unknown(error.into()))?
                    .ok_or(UpdateSeriesEpisodeError::NoSuchEpisode)?;

                Ok(UpdateSeriesEpisodeResponse::new(stored))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| UpdateSeriesEpisodeError::Unknown(error.into()))?;
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
    use crate::application::FieldUpdate;
    use allure_cargotest::allure_test;

    use crate::domain::NumericID;

    use crate::domain::model::{EpisodeIndex, Role, Series, User};

    use crate::domain::port::{
        MockResourceRepository, MockSeriesRepository, MockUserRepository, RepositoryError,
    };

    use crate::test_helpers::*;
    use chrono::NaiveDate;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = UpdateSeriesEpisode<TestFactory>;

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
        resources: MockResourceRepository,
        series: MockSeriesRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            resources,
            series,
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    /// Expect the show load to find `stored`.
    fn expect_series(series: &mut MockSeriesRepository, stored: Series) {
        let series_id = stored.id();
        series
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(series_id))
            .return_once(move |_| {
                let found = stored.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
    }

    /// A show whose season 1 holds `episodes`.
    fn series_with_episodes(episodes: Vec<Episode>) -> Result<Series, Box<dyn Error>> {
        sample_series(7, vec![sample_season(1, episodes)])
    }

    /// A command that names no field.
    fn empty_command() -> UpdateSeriesEpisodeCommand {
        UpdateSeriesEpisodeCommand::new(1, 7, 1, 1, None, None, FieldUpdate::Absent)
    }

    #[allure_test]
    #[tokio::test]
    async fn update_series_episode_replaces_metadata_keeping_the_video()
    -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-013");
        allure.story("SCN-SERIES-013-A");
        allure.story("SCN-SERIES-013-C");
        // Arrange: episode 1 backs video 20.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(
            &mut series,
            series_with_episodes(vec![sample_episode(1, 20)?])?,
        );
        let written = Episode::try_new(
            EpisodeIndex::new(1),
            "New Name".to_owned(),
            "New plot".to_owned(),
            Some(NaiveDate::from_ymd_opt(1992, 2, 3).unwrap_or_default()),
            sample_video(20, 20)?,
        )?;
        series
            .expect_save_episode()
            .times(1)
            .withf(|series_id, season_index, episode| {
                *series_id == 7
                    && *season_index == 1
                    && episode.name() == "New Name"
                    && episode.video().id() == 20
            })
            .return_once(move |_, _, _| {
                let copy = written.clone();
                Box::pin(async move { Ok(Some(copy)) })
            });
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = UpdateSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdateSeriesEpisodeCommand::new(
                1,
                7,
                1,
                1,
                Some("New Name".to_owned()),
                Some("New plot".to_owned()),
                FieldUpdate::Set(NaiveDate::from_ymd_opt(1992, 2, 3).unwrap_or_default()),
            ))
            .await?;

        // Assert
        assert_eq!(response.episode().name(), "New Name");
        assert_eq!(response.episode().video().id(), 20);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn update_series_episode_absent_fields_keep_stored_values() -> Result<(), Box<dyn Error>>
    {
        allure.epic("Series");
        allure.feature("REQ-SERIES-013");
        allure.story("SCN-SERIES-013-A");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(
            &mut series,
            series_with_episodes(vec![sample_episode(1, 20)?])?,
        );
        series
            .expect_save_episode()
            .times(1)
            .withf(|_, _, episode| {
                episode.name() == "Episode 1"
                    && episode.plot() == "A plot"
                    && episode.air_date().is_some()
                    && episode.video().id() == 20
            })
            .returning(|_, _, episode| Box::pin(async move { Ok(Some(episode)) }));
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = UpdateSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(empty_command()).await?;

        // Assert
        assert_eq!(response.episode().name(), "Episode 1");
        assert_eq!(response.episode().plot(), "A plot");
        assert!(response.episode().air_date().is_some());
        assert_eq!(response.episode().video().id(), 20);
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn update_series_episode_null_clears_the_air_date() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-013");
        allure.story("SCN-SERIES-013-B");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(
            &mut series,
            series_with_episodes(vec![sample_episode(1, 20)?])?,
        );
        series
            .expect_save_episode()
            .times(1)
            .withf(|_, _, episode| episode.air_date().is_none())
            .returning(|_, _, episode| Box::pin(async move { Ok(Some(episode)) }));
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = UpdateSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdateSeriesEpisodeCommand::new(
                1,
                7,
                1,
                1,
                None,
                None,
                FieldUpdate::Clear,
            ))
            .await?;

        // Assert
        assert!(response.episode().air_date().is_none());
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn update_series_episode_absent_episode_returns_no_such_episode()
    -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-013");
        allure.story("SCN-SERIES-013-D");
        // Arrange: the season holds no episode at index 1.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, series_with_episodes(Vec::new())?);
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = UpdateSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(empty_command()).await;

        // Assert
        assert!(matches!(
            result,
            Err(UpdateSeriesEpisodeError::NoSuchEpisode)
        ));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn update_series_episode_absent_season_returns_no_such_season()
    -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-013");
        allure.story("SCN-SERIES-013-D");
        // Arrange: the show holds no season.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, sample_series(7, Vec::new())?);
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = UpdateSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(empty_command()).await;

        // Assert
        assert!(matches!(
            result,
            Err(UpdateSeriesEpisodeError::NoSuchSeason)
        ));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn update_series_episode_unknown_show_returns_no_such_series()
    -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-013");
        allure.story("SCN-SERIES-013-D");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = UpdateSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(empty_command()).await;

        // Assert
        assert!(matches!(
            result,
            Err(UpdateSeriesEpisodeError::NoSuchSeries)
        ));
        Ok(())
    }
    #[allure_test]
    #[tokio::test]
    async fn update_series_episode_empty_name_is_invalid() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-013");
        allure.story("SCN-SERIES-013-A");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(
            &mut series,
            series_with_episodes(vec![sample_episode(1, 20)?])?,
        );
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = UpdateSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateSeriesEpisodeCommand::new(
                1,
                7,
                1,
                1,
                Some("   ".to_owned()),
                None,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateSeriesEpisodeError::InvalidName)));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn update_series_episode_absent_at_write_returns_no_such_episode()
    -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-013");
        allure.story("SCN-SERIES-013-D");
        // Arrange: the episode disappears between the read and the write.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(
            &mut series,
            series_with_episodes(vec![sample_episode(1, 20)?])?,
        );
        series
            .expect_save_episode()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(None) }));
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = UpdateSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(empty_command()).await;

        // Assert
        assert!(matches!(
            result,
            Err(UpdateSeriesEpisodeError::NoSuchEpisode)
        ));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn update_series_episode_dependency_failure_returns_unknown() -> Result<(), Box<dyn Error>>
    {
        allure.epic("Series");
        allure.feature("REQ-SERIES-013");
        allure.story("SCN-SERIES-013-A");
        // Arrange: the show load fails at the port.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::Unavailable) }));
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = UpdateSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(empty_command()).await;

        // Assert
        assert!(matches!(result, Err(UpdateSeriesEpisodeError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
