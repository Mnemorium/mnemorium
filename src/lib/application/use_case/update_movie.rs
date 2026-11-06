use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::field_update::FieldUpdate;
use crate::application::port::update_movie::{
    UpdateMovieCommand, UpdateMovieError, UpdateMovieResponse, UpdateMovieUseCase,
};
use crate::application::security_event;
use crate::domain::model::movie::Movie;
use crate::domain::model::permission::Action;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::movie_repository::{MovieFilter, MovieRepository as _};
use crate::domain::port::permission_repository::{PermissionFilter, PermissionRepository as _};
use crate::domain::port::unit_of_work::{UnitOfWork as _, UnitOfWorkFactory};
use crate::domain::port::user_repository::{UserFilter, UserRepository as _};
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::authorization::{Authorization, Decision};
use crate::domain::service::country_code::CountryCode;
use crate::domain::service::name_policy::{NamePolicy, NamePolicyError};

/// Use case implementation for updating a film partially.
pub(crate) struct UpdateMovie<F> {
    /// Factory opening the unit of work wrapping the update.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> UpdateMovie<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> UpdateMovieUseCase for UpdateMovie<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: UpdateMovieCommand,
    ) -> Pin<Box<dyn Future<Output = Result<UpdateMovieResponse, UpdateMovieError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| UpdateMovieError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| UpdateMovieError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(UpdateMovieError::NoSuchCaller)?;

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| UpdateMovieError::Unknown(error.into()))?;

                let movie = unit_of_work
                    .movies()
                    .search(&MovieFilter {
                        id: Some(command.movie_id()),
                        ..MovieFilter::default()
                    })
                    .await
                    .map_err(|error| UpdateMovieError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(UpdateMovieError::NoSuchMovie)?;

                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::LibraryRead, &movie)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(UpdateMovieError::NoSuchMovie);
                }
                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::LibraryManage, &movie)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(if denial.conceals() {
                        UpdateMovieError::NoSuchMovie
                    } else {
                        UpdateMovieError::Forbidden
                    });
                }

                let name = match command.name() {
                    Some(requested) => {
                        NamePolicy::validate(requested.to_owned()).map_err(|error| match error {
                            NamePolicyError::Empty => {
                                security_event::input_validation_failed("name", "empty");
                                UpdateMovieError::InvalidName
                            }
                            NamePolicyError::TooLong => {
                                security_event::input_validation_failed("name", "too_long");
                                UpdateMovieError::NameTooLong
                            }
                        })?
                    }
                    None => movie.name().to_owned(),
                };

                let country_of_origin = match command.country_of_origin().clone() {
                    FieldUpdate::Absent => movie.country_of_origin().map(str::to_owned),
                    FieldUpdate::Clear => None,
                    FieldUpdate::Set(value) => {
                        Some(CountryCode::validate(&value).map_err(|_| {
                            security_event::input_validation_failed(
                                "country_of_origin",
                                "invalid_value",
                            );
                            UpdateMovieError::InvalidCountry
                        })?)
                    }
                };

                let genres = match command.genres() {
                    Some(requested) => {
                        if requested.iter().any(String::is_empty) {
                            security_event::input_validation_failed("genres", "empty");
                            return Err(UpdateMovieError::InvalidGenre);
                        }
                        requested.to_vec()
                    }
                    None => movie.genre_ids().to_vec(),
                };

                let plot = command
                    .plot()
                    .map_or_else(|| movie.plot().to_owned(), str::to_owned);
                let release_date = command.release_date().resolve(movie.release_date());
                let cover_image_id = command.cover_image_id().resolve(movie.cover_image_id());

                let updated = Movie::try_new(
                    movie.id(),
                    name,
                    plot,
                    country_of_origin,
                    release_date,
                    genres,
                    cover_image_id,
                    movie.video().clone(),
                )
                .map_err(|_| UpdateMovieError::InvalidName)?;

                let stored = unit_of_work
                    .movies()
                    .update(updated)
                    .await
                    .map_err(|error| {
                        if matches!(
                            error,
                            RepositoryError::AlreadyExist | RepositoryError::Conflict
                        ) {
                            UpdateMovieError::Conflict
                        } else {
                            UpdateMovieError::Unknown(error.into())
                        }
                    })?
                    .ok_or(UpdateMovieError::NoSuchMovie)?;

                Ok(UpdateMovieResponse::new(stored))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| UpdateMovieError::Unknown(error.into()))?;
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
    use crate::domain::model::permission::{Permission, Scope};
    use crate::domain::model::user::{Role, User};
    use crate::domain::port::movie_repository::MockMovieRepository;
    use crate::domain::port::permission_repository::MockPermissionRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::*;
    use chrono::NaiveDate;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = UpdateMovie<TestFactory>;

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
        movies: MockMovieRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            movies,
            permissions,
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    /// Expect the film load to find `movie`.
    fn expect_movie(movies: &mut MockMovieRepository, movie: Movie) {
        let movie_id = movie.id();
        movies
            .expect_search()
            .times(1)
            .withf(move |filter| filter.id == Some(movie_id))
            .return_once(move |_| {
                let found = movie.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
    }

    /// A command that names no field.
    fn empty_command() -> UpdateMovieCommand {
        UpdateMovieCommand::new(
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

    #[tokio::test]
    async fn update_movie_replaces_metadata_keeping_the_video() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_movie(7)?;
        let mut movies = MockMovieRepository::new();
        expect_movie(&mut movies, stored);
        movies
            .expect_update()
            .times(1)
            .withf(|movie| {
                movie.id() == 7
                    && movie.name() == "New Name"
                    && movie.plot() == "New plot"
                    && movie.country_of_origin() == Some("GB")
                    && movie.genre_ids() == ["COMEDY".to_owned()]
                    && movie.cover_image_id() == Some(9)
                    && movie.video_id() == 7
            })
            .returning(|movie| Box::pin(async move { Ok(Some(movie)) }));
        let harness = harness_with(users, no_permissions(), movies);
        let use_case: UseCase = UpdateMovie::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdateMovieCommand::new(
                1,
                7,
                Some("New Name".to_owned()),
                Some("New plot".to_owned()),
                FieldUpdate::Set("gb".to_owned()),
                FieldUpdate::Set(NaiveDate::from_ymd_opt(1960, 1, 2).unwrap_or_default()),
                Some(vec!["COMEDY".to_owned()]),
                FieldUpdate::Set(9),
            ))
            .await?;

        // Assert
        assert_eq!(response.movie().name(), "New Name");
        assert_eq!(response.movie().country_of_origin(), Some("GB"));
        assert_eq!(response.movie().video_id(), 7);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_movie_absent_fields_keep_the_stored_values() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_movie(7)?;
        let mut movies = MockMovieRepository::new();
        expect_movie(&mut movies, stored);
        movies
            .expect_update()
            .times(1)
            .withf(|movie| {
                movie.name() == "Movie 7"
                    && movie.plot() == "A plot"
                    && movie.country_of_origin() == Some("US")
                    && movie.genre_ids() == ["DRAMA".to_owned()]
                    && movie.cover_image_id().is_none()
                    && movie.release_date().is_some()
            })
            .returning(|movie| Box::pin(async move { Ok(Some(movie)) }));
        let harness = harness_with(users, no_permissions(), movies);
        let use_case: UseCase = UpdateMovie::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(empty_command()).await?;

        // Assert
        assert_eq!(response.movie().name(), "Movie 7");
        assert_eq!(response.movie().plot(), "A plot");
        Ok(())
    }

    #[tokio::test]
    async fn update_movie_null_clears_optional_values() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_movie(7)?;
        let mut movies = MockMovieRepository::new();
        expect_movie(&mut movies, stored);
        movies
            .expect_update()
            .times(1)
            .withf(|movie| {
                movie.country_of_origin().is_none()
                    && movie.release_date().is_none()
                    && movie.cover_image_id().is_none()
            })
            .returning(|movie| Box::pin(async move { Ok(Some(movie)) }));
        let harness = harness_with(users, no_permissions(), movies);
        let use_case: UseCase = UpdateMovie::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(UpdateMovieCommand::new(
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
        assert!(response.movie().country_of_origin().is_none());
        assert!(response.movie().release_date().is_none());
        assert!(response.movie().cover_image_id().is_none());
        Ok(())
    }

    #[tokio::test]
    async fn update_movie_unknown_movie_returns_no_such_movie() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut movies = MockMovieRepository::new();
        movies
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, no_permissions(), movies);
        let use_case: UseCase = UpdateMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(empty_command()).await;

        // Assert
        assert!(matches!(result, Err(UpdateMovieError::NoSuchMovie)));
        Ok(())
    }

    #[tokio::test]
    async fn update_movie_concealed_movie_returns_no_such_movie() -> Result<(), Box<dyn Error>> {
        // Arrange: the caller is shut out of the library entirely.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let stored = sample_movie(7)?;
        let mut movies = MockMovieRepository::new();
        expect_movie(&mut movies, stored);
        let mut permissions = MockPermissionRepository::new();
        expect_permissions(
            &mut permissions,
            1,
            vec![Permission::try_new(1, Action::LibraryRead, Scope::None)?],
        );
        let harness = harness_with(users, permissions, movies);
        let use_case: UseCase = UpdateMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(empty_command()).await;

        // Assert
        assert!(matches!(result, Err(UpdateMovieError::NoSuchMovie)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_movie_readable_but_not_manageable_is_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange: a standard caller may read the public film but not manage it.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let stored = sample_movie(7)?;
        let mut movies = MockMovieRepository::new();
        expect_movie(&mut movies, stored);
        let harness = harness_with(users, no_permissions(), movies);
        let use_case: UseCase = UpdateMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(empty_command()).await;

        // Assert
        assert!(matches!(result, Err(UpdateMovieError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_movie_invalid_cover_is_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange: the repository rejects the cover through the scope trigger.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_movie(7)?;
        let mut movies = MockMovieRepository::new();
        expect_movie(&mut movies, stored);
        movies
            .expect_update()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::Conflict) }));
        let harness = harness_with(users, no_permissions(), movies);
        let use_case: UseCase = UpdateMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateMovieCommand::new(
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
        assert!(matches!(result, Err(UpdateMovieError::Conflict)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn update_movie_empty_name_is_invalid() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_movie(7)?;
        let mut movies = MockMovieRepository::new();
        expect_movie(&mut movies, stored);
        let harness = harness_with(users, no_permissions(), movies);
        let use_case: UseCase = UpdateMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateMovieCommand::new(
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
        assert!(matches!(result, Err(UpdateMovieError::InvalidName)));
        Ok(())
    }

    #[tokio::test]
    async fn update_movie_invalid_country_is_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_movie(7)?;
        let mut movies = MockMovieRepository::new();
        expect_movie(&mut movies, stored);
        let harness = harness_with(users, no_permissions(), movies);
        let use_case: UseCase = UpdateMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateMovieCommand::new(
                1,
                7,
                None,
                None,
                FieldUpdate::Set("USA".to_owned()),
                FieldUpdate::Absent,
                None,
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateMovieError::InvalidCountry)));
        Ok(())
    }

    #[tokio::test]
    async fn update_movie_empty_genre_is_invalid() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_movie(7)?;
        let mut movies = MockMovieRepository::new();
        expect_movie(&mut movies, stored);
        let harness = harness_with(users, no_permissions(), movies);
        let use_case: UseCase = UpdateMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(UpdateMovieCommand::new(
                1,
                7,
                None,
                None,
                FieldUpdate::Absent,
                FieldUpdate::Absent,
                Some(vec![String::new()]),
                FieldUpdate::Absent,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(UpdateMovieError::InvalidGenre)));
        Ok(())
    }

    #[tokio::test]
    async fn update_movie_absent_at_write_returns_no_such_movie() -> Result<(), Box<dyn Error>> {
        // Arrange: the film disappears between the read and the write.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let stored = sample_movie(7)?;
        let mut movies = MockMovieRepository::new();
        expect_movie(&mut movies, stored);
        movies
            .expect_update()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));
        let harness = harness_with(users, no_permissions(), movies);
        let use_case: UseCase = UpdateMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(empty_command()).await;

        // Assert
        assert!(matches!(result, Err(UpdateMovieError::NoSuchMovie)));
        Ok(())
    }
}
