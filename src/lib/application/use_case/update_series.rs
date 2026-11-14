use crate::application::authorization::authorize;
use crate::domain::model::{ResourceKind, ResourceRef};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tracing::warn;

use crate::application::FieldUpdate;
use crate::application::port::{
    UpdateSeriesCommand, UpdateSeriesError, UpdateSeriesResponse, UpdateSeriesUseCase,
};
use crate::domain::model::{Action, Series};
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, RepositoryError, SeriesFilter, SeriesRepository as _,
    UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork,
};
use crate::domain::service::{CountryCode, NamePolicy, NamePolicyError};

/// Use case implementation for updating a show partially.
pub(crate) struct UpdateSeries<F> {
    /// Factory opening the unit of work wrapping the update.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> UpdateSeries<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> UpdateSeriesUseCase for UpdateSeries<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: UpdateSeriesCommand,
    ) -> Pin<
        Box<dyn Future<Output = Result<UpdateSeriesResponse, UpdateSeriesError>> + Send + 'future>,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| UpdateSeriesError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| UpdateSeriesError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(UpdateSeriesError::NoSuchCaller)?;

                let series = unit_of_work
                    .series()
                    .search(&SeriesFilter {
                        id: Some(command.series_id()),
                        ..SeriesFilter::default()
                    })
                    .await
                    .map_err(|error| UpdateSeriesError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(UpdateSeriesError::NoSuchSeries)?;

                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::Series, series.id()),
                    Action::View,
                    None,
                )
                .await
                .map_err(|_| UpdateSeriesError::NoSuchSeries)?;
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
                        UpdateSeriesError::NoSuchSeries
                    } else {
                        UpdateSeriesError::Forbidden
                    }
                })?;

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
                                UpdateSeriesError::InvalidName
                            }
                            NamePolicyError::TooLong => {
                                warn!(
                                    target: "security",
                                    event = "input_validation_failed",
                                    field = %"name",
                                    reason = %"too_long",
                                    "rejected a client-supplied value"
                                );
                                UpdateSeriesError::NameTooLong
                            }
                        })?
                    }
                    None => series.name().to_owned(),
                };

                let country_of_origin = match command.country_of_origin().clone() {
                    FieldUpdate::Absent => series.country_of_origin().map(str::to_owned),
                    FieldUpdate::Clear => None,
                    FieldUpdate::Set(value) => {
                        Some(CountryCode::validate(&value).map_err(|_| {
                            warn!(
                                target: "security",
                                event = "input_validation_failed",
                                field = %"country_of_origin",
                                reason = %"invalid_value",
                                "rejected a client-supplied value"
                            );
                            UpdateSeriesError::InvalidCountry
                        })?)
                    }
                };

                let genres = match command.genres() {
                    Some(requested) => {
                        if requested.iter().any(String::is_empty) {
                            warn!(
                                target: "security",
                                event = "input_validation_failed",
                                field = %"genres",
                                reason = %"empty",
                                "rejected a client-supplied value"
                            );
                            return Err(UpdateSeriesError::InvalidGenre);
                        }
                        requested.to_vec()
                    }
                    None => series.genre_ids().to_vec(),
                };

                let plot = command
                    .plot()
                    .map_or_else(|| series.plot().to_owned(), str::to_owned);
                let first_air_date = command.first_air_date().resolve(series.first_air_date());
                let cover_image_id = command.cover_image_id().resolve(series.cover_image_id());

                let updated = Series::try_new(
                    series.id(),
                    name,
                    plot,
                    country_of_origin,
                    first_air_date,
                    genres,
                    cover_image_id,
                    series.seasons().to_vec(),
                )
                .map_err(|_| UpdateSeriesError::InvalidName)?;

                let stored = unit_of_work.series().save(updated).await.map_err(|error| {
                    if matches!(
                        error,
                        RepositoryError::AlreadyExist
                            | RepositoryError::Conflict
                            | RepositoryError::DataIntegrityViolation
                    ) {
                        UpdateSeriesError::Conflict
                    } else {
                        UpdateSeriesError::Unknown(error.into())
                    }
                })?;

                Ok(UpdateSeriesResponse::new(stored))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| UpdateSeriesError::Unknown(error.into()))?;
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

    use crate::domain::model::{Role, User};

    use crate::domain::port::{MockResourceRepository, MockSeriesRepository, MockUserRepository};

    use crate::test_helpers::*;
    use chrono::NaiveDate;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = UpdateSeries<TestFactory>;

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

    /// Expect the show load to find `series`.
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

    /// A command that names no field.
    fn empty_command() -> UpdateSeriesCommand {
        UpdateSeriesCommand::new(
            1,
            7,
            None,
            None,
            FieldUpdate::Absent,
            FieldUpdate::Absent,
            None,
            FieldUpdate::Absent,
        )
    }

    #[allure_test]
    #[tokio::test]
    async fn update_series_replaces_metadata_keeping_seasons_and_episodes()
    -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-009");
        allure.story("SCN-SERIES-009-A");
        allure.story("SCN-SERIES-009-C");
        // Arrange: the stored show holds a season with one episode.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_series(7, vec![sample_season(1, vec![sample_episode(1, 20)?])])?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, stored);
        series
            .expect_save()
            .times(1)
            .withf(|candidate| {
                candidate.id() == 7
                    && candidate.name() == "New Name"
                    && candidate.plot() == "New plot"
                    && candidate.country_of_origin() == Some("GB")
                    && candidate.genre_ids() == ["COMEDY".to_owned()]
                    && candidate.seasons().len() == 1
                    && candidate
                        .seasons()
                        .first()
                        .is_some_and(|season| season.episodes().len() == 1)
            })
            .returning(|candidate| Box::pin(async move { Ok(candidate) }));
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = UpdateSeries::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdateSeriesCommand::new(
                1,
                7,
                Some("New Name".to_owned()),
                Some("New plot".to_owned()),
                FieldUpdate::Set("gb".to_owned()),
                FieldUpdate::Set(NaiveDate::from_ymd_opt(1991, 1, 2).unwrap_or_default()),
                Some(vec!["COMEDY".to_owned()]),
                FieldUpdate::Absent,
            ))
            .await?;

        // Assert
        assert_eq!(response.series().name(), "New Name");
        assert_eq!(response.series().country_of_origin(), Some("GB"));
        assert_eq!(response.series().seasons().len(), 1);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn update_series_null_clears_optional_values() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-009");
        allure.story("SCN-SERIES-009-B");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_series(7, Vec::new())?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, stored);
        series
            .expect_save()
            .times(1)
            .withf(|candidate| {
                candidate.country_of_origin().is_none()
                    && candidate.first_air_date().is_none()
                    && candidate.cover_image_id().is_none()
            })
            .returning(|candidate| Box::pin(async move { Ok(candidate) }));
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = UpdateSeries::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdateSeriesCommand::new(
                1,
                7,
                None,
                None,
                FieldUpdate::Clear,
                FieldUpdate::Clear,
                None,
                FieldUpdate::Clear,
            ))
            .await?;

        // Assert
        assert!(response.series().country_of_origin().is_none());
        assert!(response.series().first_air_date().is_none());
        assert!(response.series().cover_image_id().is_none());
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn update_series_unknown_show_returns_no_such_series() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-009");
        allure.story("SCN-SERIES-009-E");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = UpdateSeries::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(empty_command()).await;

        // Assert
        assert!(matches!(result, Err(UpdateSeriesError::NoSuchSeries)));
        Ok(())
    }
    #[allure_test]
    #[tokio::test]
    async fn update_series_invalid_cover_is_conflict() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-009");
        allure.story("SCN-SERIES-009-A");
        // Arrange: the repository rejects the cover through the scope trigger.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_series(7, Vec::new())?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, stored);
        series
            .expect_save()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::Conflict) }));
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = UpdateSeries::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateSeriesCommand::new(
                1,
                7,
                None,
                None,
                FieldUpdate::Absent,
                FieldUpdate::Absent,
                None,
                FieldUpdate::Set(9),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateSeriesError::Conflict)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn update_series_missing_cover_is_conflict() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-009");
        allure.story("SCN-SERIES-009-A");
        // Arrange: the repository rejects the cover through the scope trigger.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_series(7, Vec::new())?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, stored);
        series
            .expect_save()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::DataIntegrityViolation) }));
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = UpdateSeries::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateSeriesCommand::new(
                1,
                7,
                None,
                None,
                FieldUpdate::Absent,
                FieldUpdate::Absent,
                None,
                FieldUpdate::Set(9),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateSeriesError::Conflict)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn update_series_empty_name_is_invalid() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-009");
        allure.story("SCN-SERIES-009-A");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_series(7, Vec::new())?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, stored);
        let harness = harness_with(users, no_resources(), series);
        let use_case: UseCase = UpdateSeries::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateSeriesCommand::new(
                1,
                7,
                Some("   ".to_owned()),
                None,
                FieldUpdate::Absent,
                FieldUpdate::Absent,
                None,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateSeriesError::InvalidName)));
        Ok(())
    }
}
