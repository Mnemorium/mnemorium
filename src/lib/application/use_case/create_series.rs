use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::create_series::{
    CreateSeriesCommand, CreateSeriesError, CreateSeriesResponse, CreateSeriesUseCase,
};
use crate::application::security_event;
use crate::domain::model::series::Series;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::permission_repository::{PermissionFilter, PermissionRepository as _};
use crate::domain::port::series_repository::SeriesRepository as _;
use crate::domain::port::unit_of_work::{UnitOfWork as _, UnitOfWorkFactory};
use crate::domain::port::user_repository::{UserFilter, UserRepository as _};
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::authorization::{Authorization, Decision};
use crate::domain::service::country_code::CountryCode;
use crate::domain::service::name_policy::{NamePolicy, NamePolicyError};

/// Use case implementation for creating a show.
pub(crate) struct CreateSeries<F> {
    /// Factory opening the unit of work wrapping the creation.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> CreateSeries<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> CreateSeriesUseCase for CreateSeries<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: CreateSeriesCommand,
    ) -> Pin<
        Box<dyn Future<Output = Result<CreateSeriesResponse, CreateSeriesError>> + Send + 'future>,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| CreateSeriesError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| CreateSeriesError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(CreateSeriesError::NoSuchCaller)?;

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| CreateSeriesError::Unknown(error.into()))?;

                if let Decision::Deny(denial) =
                    Authorization::decide_library_manage_surface(&caller, &permissions)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(CreateSeriesError::Forbidden);
                }

                let name =
                    NamePolicy::validate(command.name().to_owned()).map_err(
                        |error| match error {
                            NamePolicyError::Empty => {
                                security_event::input_validation_failed("name", "empty");
                                CreateSeriesError::InvalidName
                            }
                            NamePolicyError::TooLong => {
                                security_event::input_validation_failed("name", "too_long");
                                CreateSeriesError::NameTooLong
                            }
                        },
                    )?;

                let country_of_origin = command
                    .country_of_origin()
                    .map(|value| {
                        CountryCode::validate(value).map_err(|_| {
                            security_event::input_validation_failed(
                                "country_of_origin",
                                "invalid_value",
                            );
                            CreateSeriesError::InvalidCountry
                        })
                    })
                    .transpose()?;

                if command.genres().iter().any(String::is_empty) {
                    security_event::input_validation_failed("genres", "empty");
                    return Err(CreateSeriesError::InvalidGenre);
                }

                let series = Series::try_new(
                    0,
                    name,
                    command.plot().unwrap_or_default().to_owned(),
                    country_of_origin,
                    command.first_air_date(),
                    command.genres().to_vec(),
                    command.cover_image_id(),
                    Vec::new(),
                )
                .map_err(|_| CreateSeriesError::InvalidName)?;

                let stored = unit_of_work
                    .series()
                    .create(series)
                    .await
                    .map_err(|error| {
                        if matches!(
                            error,
                            RepositoryError::AlreadyExist | RepositoryError::Conflict
                        ) {
                            CreateSeriesError::Conflict
                        } else {
                            CreateSeriesError::Unknown(error.into())
                        }
                    })?;

                Ok(CreateSeriesResponse::new(stored))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| CreateSeriesError::Unknown(error.into()))?;
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
    use crate::domain::alias::NumericID;
    use crate::domain::model::user::{Role, User};
    use crate::domain::port::permission_repository::MockPermissionRepository;
    use crate::domain::port::series_repository::MockSeriesRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::*;
    use chrono::NaiveDate;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = CreateSeries<TestFactory>;

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

    /// A valid command.
    fn command() -> CreateSeriesCommand {
        CreateSeriesCommand::new(
            1,
            "Twin Peaks".to_owned(),
            Some("A plot".to_owned()),
            Some("us".to_owned()),
            Some(NaiveDate::from_ymd_opt(1990, 4, 8).unwrap_or_default()),
            vec!["MYSTERY".to_owned()],
            None,
        )
    }

    #[tokio::test]
    async fn create_series_stores_the_show_with_no_seasons() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_series(7, Vec::new())?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_create()
            .times(1)
            .withf(|candidate| {
                candidate.name() == "Twin Peaks"
                    && candidate.plot() == "A plot"
                    && candidate.country_of_origin() == Some("US")
                    && candidate.genre_ids() == ["MYSTERY".to_owned()]
                    && candidate.seasons().is_empty()
            })
            .return_once(move |_| {
                let created = stored.clone();
                Box::pin(async move { Ok(created) })
            });
        let harness = harness_with(users, no_permissions(), series);
        let use_case: UseCase = CreateSeries::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(CreateSeriesCommand::new(
                1,
                "  Twin Peaks  ".to_owned(),
                Some("A plot".to_owned()),
                Some("us".to_owned()),
                Some(NaiveDate::from_ymd_opt(1990, 4, 8).unwrap_or_default()),
                vec!["MYSTERY".to_owned()],
                None,
            ))
            .await?;

        // Assert
        assert_eq!(response.series().id(), 7);
        assert!(
            response.series().seasons().is_empty(),
            "a new show must hold no season"
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_series_repeated_name_returns_a_second_show() -> Result<(), Box<dyn Error>> {
        // Arrange: the schema holds no unique show name, so creation never looks
        // one up before inserting.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_series(8, Vec::new())?;
        let mut series = MockSeriesRepository::new();
        series.expect_create().times(1).return_once(move |_| {
            let created = stored.clone();
            Box::pin(async move { Ok(created) })
        });
        let harness = harness_with(users, no_permissions(), series);
        let use_case: UseCase = CreateSeries::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(command()).await?;

        // Assert
        assert_eq!(
            response.series().id(),
            8,
            "a repeated name must create a second show rather than return the first"
        );
        Ok(())
    }

    #[tokio::test]
    async fn create_series_unknown_genre_is_written() -> Result<(), Box<dyn Error>> {
        // Arrange: the genre vocabulary is open on write.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_series(7, Vec::new())?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_create()
            .times(1)
            .withf(|candidate| candidate.genre_ids() == ["SPACE_OPERA".to_owned()])
            .return_once(move |_| {
                let created = stored.clone();
                Box::pin(async move { Ok(created) })
            });
        let harness = harness_with(users, no_permissions(), series);
        let use_case: UseCase = CreateSeries::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(CreateSeriesCommand::new(
                1,
                "Twin Peaks".to_owned(),
                None,
                None,
                None,
                vec!["SPACE_OPERA".to_owned()],
                None,
            ))
            .await?;

        // Assert
        assert_eq!(response.series().id(), 7);
        Ok(())
    }

    #[tokio::test]
    async fn create_series_empty_name_is_invalid() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let harness = harness_with(users, no_permissions(), MockSeriesRepository::new());
        let use_case: UseCase = CreateSeries::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateSeriesCommand::new(
                1,
                "   ".to_owned(),
                None,
                None,
                None,
                Vec::new(),
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateSeriesError::InvalidName)));
        Ok(())
    }

    #[tokio::test]
    async fn create_series_invalid_country_is_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let harness = harness_with(users, no_permissions(), MockSeriesRepository::new());
        let use_case: UseCase = CreateSeries::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateSeriesCommand::new(
                1,
                "Twin Peaks".to_owned(),
                None,
                Some("USA".to_owned()),
                None,
                Vec::new(),
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateSeriesError::InvalidCountry)));
        Ok(())
    }

    #[tokio::test]
    async fn create_series_empty_genre_is_invalid() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let harness = harness_with(users, no_permissions(), MockSeriesRepository::new());
        let use_case: UseCase = CreateSeries::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateSeriesCommand::new(
                1,
                "Twin Peaks".to_owned(),
                None,
                None,
                None,
                vec![String::new()],
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateSeriesError::InvalidGenre)));
        Ok(())
    }

    #[tokio::test]
    async fn create_series_standard_user_is_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let harness = harness_with(users, no_permissions(), MockSeriesRepository::new());
        let use_case: UseCase = CreateSeries::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(command()).await;

        // Assert
        assert!(matches!(result, Err(CreateSeriesError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_series_invalid_cover_is_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange: the repository rejects the cover through the scope trigger.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::Conflict) }));
        let harness = harness_with(users, no_permissions(), series);
        let use_case: UseCase = CreateSeries::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateSeriesCommand::new(
                1,
                "Twin Peaks".to_owned(),
                None,
                None,
                None,
                Vec::new(),
                Some(9),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateSeriesError::Conflict)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
