use crate::domain::model::{Action, ChildKind, ResourceKind, ResourceRef};

use crate::application::authorization::authorize;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{
    PutSeriesSeasonCommand, PutSeriesSeasonError, PutSeriesSeasonResponse, PutSeriesSeasonUseCase,
};
use crate::domain::model::{Season, SeasonIndex};
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, RepositoryError, SeriesFilter, SeriesRepository as _,
    UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork,
};

/// Use case implementation for creating or replacing a show season.
pub(crate) struct PutSeriesSeason<F> {
    /// Factory opening the unit of work wrapping the write.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> PutSeriesSeason<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> PutSeriesSeasonUseCase for PutSeriesSeason<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: PutSeriesSeasonCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PutSeriesSeasonResponse, PutSeriesSeasonError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| PutSeriesSeasonError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| PutSeriesSeasonError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(PutSeriesSeasonError::NoSuchCaller)?;

                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::Series, command.series_id()),
                    Action::CreateChild,
                    Some(ChildKind::Season),
                )
                .await
                .map_err(|error| {
                    if error.conceals() {
                        PutSeriesSeasonError::NoSuchSeries
                    } else {
                        PutSeriesSeasonError::Forbidden
                    }
                })?;

                let series = unit_of_work
                    .series()
                    .search(&SeriesFilter {
                        id: Some(command.series_id()),
                        ..SeriesFilter::default()
                    })
                    .await
                    .map_err(|error| PutSeriesSeasonError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(PutSeriesSeasonError::NoSuchSeries)?;

                let existed = series
                    .seasons()
                    .iter()
                    .any(|season| season.index().value() == command.season_index());

                let season = Season::new(
                    SeasonIndex::new(command.season_index()),
                    command.name().map(str::to_owned),
                    command.cover_image_id(),
                    Vec::new(),
                );

                let stored = unit_of_work
                    .series()
                    .save_season(series.id(), season)
                    .await
                    .map_err(|error| {
                        if matches!(
                            error,
                            RepositoryError::Conflict | RepositoryError::DataIntegrityViolation
                        ) {
                            PutSeriesSeasonError::Conflict
                        } else {
                            PutSeriesSeasonError::Unknown(error.into())
                        }
                    })?
                    .ok_or(PutSeriesSeasonError::NoSuchSeries)?;

                Ok(PutSeriesSeasonResponse::new(stored, !existed))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| PutSeriesSeasonError::Unknown(error.into()))?;
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
    use allure_cargotest::allure_test;

    use crate::domain::model::{Role, Series, User};

    use crate::domain::port::{MockResourceRepository, MockSeriesRepository, MockUserRepository};

    use crate::test_helpers::*;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = PutSeriesSeason<TestFactory>;

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

    /// A command creating season 1 of show 7.
    fn command() -> PutSeriesSeasonCommand {
        PutSeriesSeasonCommand::new(1, 7, 1, Some("Season One".to_owned()), Some(9))
    }

    #[allure_test]
    #[tokio::test]
    async fn put_series_season_new_index_reports_created() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-010");
        allure.story("SCN-SERIES-010-A");
        // Arrange: the show holds no season.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_series(7, Vec::new())?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, stored);
        let written = Season::new(
            SeasonIndex::new(1),
            Some("Season One".to_owned()),
            Some(9),
            Vec::new(),
        );
        series
            .expect_save_season()
            .times(1)
            .withf(|series_id, season| {
                *series_id == 7 && season.index().value() == 1 && season.episodes().is_empty()
            })
            .return_once(move |_, _| {
                let copy = written.clone();
                Box::pin(async move { Ok(Some(copy)) })
            });
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = PutSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(command()).await?;

        // Assert
        assert!(response.created(), "a free index must create the season");
        assert_eq!(response.season().index().value(), 1);
        assert_eq!(response.season().name(), Some("Season One"));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn put_series_season_existing_index_reports_replaced_keeping_episodes()
    -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-010");
        allure.story("SCN-SERIES-010-B");
        allure.story("SCN-SERIES-010-C");
        // Arrange: season 1 already holds an episode.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_series(7, vec![sample_season(1, vec![sample_episode(1, 20)?])])?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, stored);
        let written = sample_season(1, vec![sample_episode(1, 20)?]);
        series
            .expect_save_season()
            .times(1)
            .return_once(move |_, _| {
                let copy = written.clone();
                Box::pin(async move { Ok(Some(copy)) })
            });
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = PutSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(command()).await?;

        // Assert
        assert!(!response.created(), "an existing index must be replaced");
        assert_eq!(
            response.season().episodes().len(),
            1,
            "a replaced season must keep its episodes"
        );
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn put_series_season_unknown_show_returns_no_such_series() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-010");
        allure.story("SCN-SERIES-010-D");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = PutSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(command()).await;

        // Assert
        assert!(matches!(result, Err(PutSeriesSeasonError::NoSuchSeries)));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn put_series_season_absent_at_write_returns_no_such_series() -> Result<(), Box<dyn Error>>
    {
        allure.epic("Series");
        allure.feature("REQ-SERIES-010");
        allure.story("SCN-SERIES-010-D");
        // Arrange: the show disappears between the read and the write.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_series(7, Vec::new())?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, stored);
        series
            .expect_save_season()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(None) }));
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = PutSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(command()).await;

        // Assert
        assert!(matches!(result, Err(PutSeriesSeasonError::NoSuchSeries)));
        Ok(())
    }
    #[allure_test]
    #[tokio::test]
    async fn put_series_season_invalid_cover_is_conflict() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-010");
        allure.story("SCN-SERIES-010-E");
        // Arrange: the repository rejects the cover through the scope trigger.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_series(7, Vec::new())?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, stored);
        series
            .expect_save_season()
            .times(1)
            .returning(|_, _| Box::pin(async { Err(RepositoryError::Conflict) }));
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = PutSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(command()).await;

        // Assert
        assert!(matches!(result, Err(PutSeriesSeasonError::Conflict)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn put_series_season_missing_cover_is_conflict() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-010");
        allure.story("SCN-SERIES-010-E");
        // Arrange: the repository rejects the cover through the scope trigger.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_series(7, Vec::new())?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, stored);
        series
            .expect_save_season()
            .times(1)
            .returning(|_, _| Box::pin(async { Err(RepositoryError::DataIntegrityViolation) }));
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = PutSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(command()).await;

        // Assert
        assert!(matches!(result, Err(PutSeriesSeasonError::Conflict)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn put_series_season_null_name_and_cover_are_stored() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-010");
        allure.story("SCN-SERIES-010-A");
        // Arrange: a put with neither a name nor a cover clears both.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_series(7, vec![sample_season(1, Vec::new())])?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, stored);
        let written = Season::new(SeasonIndex::new(1), None, None, Vec::new());
        series
            .expect_save_season()
            .times(1)
            .withf(|_, season| season.name().is_none() && season.cover_image_id().is_none())
            .return_once(move |_, _| {
                let copy = written.clone();
                Box::pin(async move { Ok(Some(copy)) })
            });
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = PutSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(PutSeriesSeasonCommand::new(1, 7, 1, None, None))
            .await?;

        // Assert
        assert!(response.season().name().is_none());
        assert!(response.season().cover_image_id().is_none());
        Ok(())
    }
}
