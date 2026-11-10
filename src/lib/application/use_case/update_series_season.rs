use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tracing::warn;

use crate::application::port::{
    UpdateSeriesSeasonCommand, UpdateSeriesSeasonError, UpdateSeriesSeasonResponse,
    UpdateSeriesSeasonUseCase,
};
use crate::domain::model::{Action, Season};
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, PermissionFilter, PermissionRepository as _,
    RepositoryError, SeriesFilter, SeriesRepository as _, UnitOfWork as _, UnitOfWorkFactory,
    UserFilter, UserRepository as _, UserUnitOfWork,
};
use crate::domain::service::{Authorization, Decision};

/// Use case implementation for updating a show season partially.
pub(crate) struct UpdateSeriesSeason<F> {
    /// Factory opening the unit of work wrapping the update.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> UpdateSeriesSeason<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> UpdateSeriesSeasonUseCase for UpdateSeriesSeason<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: UpdateSeriesSeasonCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<UpdateSeriesSeasonResponse, UpdateSeriesSeasonError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| UpdateSeriesSeasonError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| UpdateSeriesSeasonError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(UpdateSeriesSeasonError::NoSuchCaller)?;

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| UpdateSeriesSeasonError::Unknown(error.into()))?;

                let series = unit_of_work
                    .series()
                    .search(&SeriesFilter {
                        id: Some(command.series_id()),
                        ..SeriesFilter::default()
                    })
                    .await
                    .map_err(|error| UpdateSeriesSeasonError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(UpdateSeriesSeasonError::NoSuchSeries)?;

                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::LibraryRead, &series)
                {
                    warn!(
                        target: "security",
                        event = "authz_failed",
                        actor = caller.id(),
                        action = %denial.action().verb(),
                        target = %denial.action().target(),
                        "denied an action"
                    );
                    return Err(UpdateSeriesSeasonError::NoSuchSeries);
                }
                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::LibraryManage, &series)
                {
                    warn!(
                        target: "security",
                        event = "authz_failed",
                        actor = caller.id(),
                        action = %denial.action().verb(),
                        target = %denial.action().target(),
                        "denied an action"
                    );
                    return Err(if denial.conceals() {
                        UpdateSeriesSeasonError::NoSuchSeries
                    } else {
                        UpdateSeriesSeasonError::Forbidden
                    });
                }

                let season = series
                    .seasons()
                    .iter()
                    .find(|season| season.index().value() == command.season_index())
                    .ok_or(UpdateSeriesSeasonError::NoSuchSeason)?;

                let name = command.name().resolve(season.name().map(str::to_owned));
                let cover_image_id = command.cover_image_id().resolve(season.cover_image_id());

                let updated = Season::new(
                    season.index(),
                    name,
                    cover_image_id,
                    season.episodes().to_vec(),
                );

                let stored = unit_of_work
                    .series()
                    .save_season(series.id(), updated)
                    .await
                    .map_err(|error| {
                        if matches!(
                            error,
                            RepositoryError::Conflict | RepositoryError::DataIntegrityViolation
                        ) {
                            UpdateSeriesSeasonError::Conflict
                        } else {
                            UpdateSeriesSeasonError::Unknown(error.into())
                        }
                    })?
                    .ok_or(UpdateSeriesSeasonError::NoSuchSeason)?;

                Ok(UpdateSeriesSeasonResponse::new(stored))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| UpdateSeriesSeasonError::Unknown(error.into()))?;
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

    use crate::domain::NumericID;

    use crate::domain::model::{Permission, Role, Scope, SeasonIndex, Series, User};

    use crate::domain::port::{MockPermissionRepository, MockSeriesRepository, MockUserRepository};

    use crate::test_helpers::*;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = UpdateSeriesSeason<TestFactory>;

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
        permissions: MockPermissionRepository,
        series: MockSeriesRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            permissions,
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

    /// A show holding season 1 with one episode and a stored name and cover.
    fn series_with_season() -> Result<Series, Box<dyn Error>> {
        let season = Season::new(
            SeasonIndex::new(1),
            Some("Old Name".to_owned()),
            Some(9),
            vec![sample_episode(1, 20)?],
        );
        sample_series(7, vec![season])
    }

    #[tokio::test]
    async fn update_series_season_replaces_name_and_cover_keeping_episodes()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, series_with_season()?);
        let written = Season::new(
            SeasonIndex::new(1),
            Some("New Name".to_owned()),
            Some(9),
            vec![sample_episode(1, 20)?],
        );
        series
            .expect_save_season()
            .times(1)
            .withf(|series_id, season| {
                *series_id == 7
                    && season.index().value() == 1
                    && season.name() == Some("New Name")
                    && season.cover_image_id() == Some(9)
                    && season.episodes().len() == 1
            })
            .return_once(move |_, _| {
                let copy = written.clone();
                Box::pin(async move { Ok(Some(copy)) })
            });
        let harness = harness_with(users, no_permissions(), series);
        let use_case: UseCase = UpdateSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdateSeriesSeasonCommand::new(
                1,
                7,
                1,
                FieldUpdate::Set("New Name".to_owned()),
                FieldUpdate::Set(9),
            ))
            .await?;

        // Assert
        assert_eq!(response.season().name(), Some("New Name"));
        assert_eq!(response.season().episodes().len(), 1);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_series_season_null_name_and_cover_clear() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, series_with_season()?);
        let written = Season::new(SeasonIndex::new(1), None, None, Vec::new());
        series
            .expect_save_season()
            .times(1)
            .withf(|_, season| season.name().is_none() && season.cover_image_id().is_none())
            .return_once(move |_, _| {
                let copy = written.clone();
                Box::pin(async move { Ok(Some(copy)) })
            });
        let harness = harness_with(users, no_permissions(), series);
        let use_case: UseCase = UpdateSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdateSeriesSeasonCommand::new(
                1,
                7,
                1,
                FieldUpdate::Clear,
                FieldUpdate::Clear,
            ))
            .await?;

        // Assert
        assert!(response.season().name().is_none());
        assert!(response.season().cover_image_id().is_none());
        Ok(())
    }

    #[tokio::test]
    async fn update_series_season_absent_season_returns_no_such_season()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the show holds no season at index 1.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, sample_series(7, Vec::new())?);
        let harness = harness_with(users, no_permissions(), series);
        let use_case: UseCase = UpdateSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateSeriesSeasonCommand::new(
                1,
                7,
                1,
                FieldUpdate::Absent,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateSeriesSeasonError::NoSuchSeason)));
        Ok(())
    }

    #[tokio::test]
    async fn update_series_season_unknown_show_returns_no_such_series() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, no_permissions(), series);
        let use_case: UseCase = UpdateSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateSeriesSeasonCommand::new(
                1,
                7,
                1,
                FieldUpdate::Absent,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateSeriesSeasonError::NoSuchSeries)));
        Ok(())
    }

    #[tokio::test]
    async fn update_series_season_standard_user_is_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange: a standard caller may read the public show but not manage it.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, series_with_season()?);
        let harness = harness_with(users, no_permissions(), series);
        let use_case: UseCase = UpdateSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateSeriesSeasonCommand::new(
                1,
                7,
                1,
                FieldUpdate::Absent,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateSeriesSeasonError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_series_season_concealed_show_returns_no_such_series()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the caller is shut out of the library entirely.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, series_with_season()?);
        let mut permissions = MockPermissionRepository::new();
        expect_permissions(
            &mut permissions,
            1,
            vec![Permission::try_new(1, Action::LibraryRead, Scope::None)?],
        );
        let harness = harness_with(users, permissions, series);
        let use_case: UseCase = UpdateSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateSeriesSeasonCommand::new(
                1,
                7,
                1,
                FieldUpdate::Absent,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateSeriesSeasonError::NoSuchSeries)));
        Ok(())
    }

    #[tokio::test]
    async fn update_series_season_invalid_cover_is_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange: the repository rejects the cover through the scope trigger.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, series_with_season()?);
        series
            .expect_save_season()
            .times(1)
            .returning(|_, _| Box::pin(async { Err(RepositoryError::Conflict) }));
        let harness = harness_with(users, no_permissions(), series);
        let use_case: UseCase = UpdateSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateSeriesSeasonCommand::new(
                1,
                7,
                1,
                FieldUpdate::Absent,
                FieldUpdate::Set(11),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateSeriesSeasonError::Conflict)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_series_season_missing_cover_is_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange: the repository rejects the cover through the scope trigger.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, series_with_season()?);
        series
            .expect_save_season()
            .times(1)
            .returning(|_, _| Box::pin(async { Err(RepositoryError::DataIntegrityViolation) }));
        let harness = harness_with(users, no_permissions(), series);
        let use_case: UseCase = UpdateSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateSeriesSeasonCommand::new(
                1,
                7,
                1,
                FieldUpdate::Absent,
                FieldUpdate::Set(11),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateSeriesSeasonError::Conflict)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_series_season_absent_at_write_returns_no_such_season()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the season disappears between the read and the write.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, series_with_season()?);
        series
            .expect_save_season()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(None) }));
        let harness = harness_with(users, no_permissions(), series);
        let use_case: UseCase = UpdateSeriesSeason::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateSeriesSeasonCommand::new(
                1,
                7,
                1,
                FieldUpdate::Absent,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateSeriesSeasonError::NoSuchSeason)));
        Ok(())
    }
}
