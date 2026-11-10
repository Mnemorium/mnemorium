use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{
    CreateMovieCommand, CreateMovieError, CreateMovieResponse, CreateMovieUseCase,
};
use crate::application::security_event;
use crate::domain::model::Movie;
use crate::domain::port::{
    EpisodeFilter, IdentityUnitOfWork, LibraryUnitOfWork, MovieFilter, MovieRepository as _,
    PermissionFilter, PermissionRepository as _, RepositoryError, SeriesRepository as _,
    UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork,
    VideoFilter, VideoRepository as _,
};
use crate::domain::service::{Authorization, CountryCode, Decision, NamePolicy, NamePolicyError};

/// Use case implementation for creating a film from a video.
pub(crate) struct CreateMovie<F> {
    /// Factory opening the unit of work wrapping the creation.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> CreateMovie<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> CreateMovieUseCase for CreateMovie<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: CreateMovieCommand,
    ) -> Pin<Box<dyn Future<Output = Result<CreateMovieResponse, CreateMovieError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| CreateMovieError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| CreateMovieError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(CreateMovieError::NoSuchCaller)?;

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| CreateMovieError::Unknown(error.into()))?;

                if let Decision::Deny(denial) =
                    Authorization::decide_library_manage_surface(&caller, &permissions)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(CreateMovieError::Forbidden);
                }

                let name =
                    NamePolicy::validate(command.name().to_owned()).map_err(
                        |error| match error {
                            NamePolicyError::Empty => {
                                security_event::input_validation_failed("name", "empty");
                                CreateMovieError::InvalidName
                            }
                            NamePolicyError::TooLong => {
                                security_event::input_validation_failed("name", "too_long");
                                CreateMovieError::NameTooLong
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
                            CreateMovieError::InvalidCountry
                        })
                    })
                    .transpose()?;

                if command.genres().iter().any(String::is_empty) {
                    security_event::input_validation_failed("genres", "empty");
                    return Err(CreateMovieError::InvalidGenre);
                }

                let video = unit_of_work
                    .videos()
                    .search(&VideoFilter {
                        id: Some(command.video_id()),
                        ..VideoFilter::default()
                    })
                    .await
                    .map_err(|error| CreateMovieError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(CreateMovieError::NoSuchVideo)?;

                let existing = unit_of_work
                    .movies()
                    .search(&MovieFilter {
                        video_id: Some(command.video_id()),
                        ..MovieFilter::default()
                    })
                    .await
                    .map_err(|error| CreateMovieError::Unknown(error.into()))?
                    .into_iter()
                    .next();
                if let Some(movie) = existing {
                    return Ok(CreateMovieResponse::new(movie, false));
                }

                let attached = unit_of_work
                    .series()
                    .search_episodes(&EpisodeFilter {
                        video_id: Some(command.video_id()),
                        ..EpisodeFilter::default()
                    })
                    .await
                    .map_err(|error| CreateMovieError::Unknown(error.into()))?;
                if !attached.is_empty() {
                    return Err(CreateMovieError::Conflict);
                }

                let movie = Movie::try_new(
                    0,
                    name,
                    command.plot().unwrap_or_default().to_owned(),
                    country_of_origin,
                    command.release_date(),
                    command.genres().to_vec(),
                    command.cover_image_id(),
                    video,
                )
                .map_err(|_| CreateMovieError::InvalidName)?;

                let stored = unit_of_work.movies().create(movie).await.map_err(|error| {
                    if matches!(
                        error,
                        RepositoryError::AlreadyExist
                            | RepositoryError::Conflict
                            | RepositoryError::DataIntegrityViolation
                    ) {
                        CreateMovieError::Conflict
                    } else {
                        CreateMovieError::Unknown(error.into())
                    }
                })?;

                Ok(CreateMovieResponse::new(stored, true))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| CreateMovieError::Unknown(error.into()))?;
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

    use crate::domain::model::{Role, User};

    use crate::domain::port::{
        MockMovieRepository, MockPermissionRepository, MockSeriesRepository, MockUserRepository,
        MockVideoRepository,
    };

    use crate::test_helpers::*;
    use chrono::NaiveDate;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = CreateMovie<TestFactory>;

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
        series: MockSeriesRepository,
        videos: MockVideoRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            movies,
            permissions,
            series,
            users,
            videos,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    /// A command naming the seeded video 5.
    fn command() -> CreateMovieCommand {
        CreateMovieCommand::new(
            1,
            5,
            "Some Like It Hot".to_owned(),
            Some("A plot".to_owned()),
            Some("us".to_owned()),
            Some(NaiveDate::from_ymd_opt(1959, 8, 17).unwrap_or_default()),
            vec!["COMEDY".to_owned()],
            None,
        )
    }

    #[tokio::test]
    async fn create_movie_new_film_reports_created() -> Result<(), Box<dyn Error>> {
        // Arrange: the video is free and no film or episode backs it.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut videos = MockVideoRepository::new();
        let video = sample_video(5, 5)?;
        videos
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(5))
            .return_once(move |_| {
                let found = video.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
        let mut movies = MockMovieRepository::new();
        movies
            .expect_search()
            .times(1)
            .withf(|filter| filter.video_id == Some(5))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let stored = sample_movie(7)?;
        movies
            .expect_create()
            .times(1)
            .withf(|movie| {
                movie.name() == "Some Like It Hot"
                    && movie.plot() == "A plot"
                    && movie.country_of_origin() == Some("US")
                    && movie.genre_ids() == ["COMEDY".to_owned()]
                    && movie.video_id() == 5
            })
            .return_once(move |_| {
                let created = stored.clone();
                Box::pin(async move { Ok(created) })
            });
        let mut series = MockSeriesRepository::new();
        series
            .expect_search_episodes()
            .times(1)
            .withf(|filter| filter.video_id == Some(5))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, no_permissions(), movies, series, videos);
        let use_case: UseCase = CreateMovie::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(command()).await?;

        // Assert
        assert!(response.created(), "a free video must create the film");
        assert_eq!(response.movie().id(), 7);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_movie_existing_film_returns_it_unchanged() -> Result<(), Box<dyn Error>> {
        // Arrange: the video already backs film 7.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut videos = MockVideoRepository::new();
        let video = sample_video(5, 5)?;
        videos.expect_search().times(1).return_once(move |_| {
            let found = video.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
        let existing = sample_movie(7)?;
        let mut movies = MockMovieRepository::new();
        movies.expect_search().times(1).return_once(move |_| {
            let found = existing.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
        movies.expect_create().times(0);
        let mut series = MockSeriesRepository::new();
        series.expect_search_episodes().times(0);
        let harness = harness_with(users, no_permissions(), movies, series, videos);
        let use_case: UseCase = CreateMovie::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(command()).await?;

        // Assert
        assert!(
            !response.created(),
            "an existing film must not be recreated"
        );
        assert_eq!(response.movie().id(), 7);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_movie_standard_user_is_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let harness = harness_with(
            users,
            no_permissions(),
            MockMovieRepository::new(),
            MockSeriesRepository::new(),
            MockVideoRepository::new(),
        );
        let use_case: UseCase = CreateMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(command()).await;

        // Assert
        assert!(matches!(result, Err(CreateMovieError::Forbidden)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_movie_unknown_video_is_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut videos = MockVideoRepository::new();
        videos
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(
            users,
            no_permissions(),
            MockMovieRepository::new(),
            MockSeriesRepository::new(),
            videos,
        );
        let use_case: UseCase = CreateMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(command()).await;

        // Assert
        assert!(matches!(result, Err(CreateMovieError::NoSuchVideo)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_movie_empty_name_is_invalid() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let harness = harness_with(
            users,
            no_permissions(),
            MockMovieRepository::new(),
            MockSeriesRepository::new(),
            MockVideoRepository::new(),
        );
        let use_case: UseCase = CreateMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMovieCommand::new(
                1,
                5,
                "   ".to_owned(),
                None,
                None,
                None,
                Vec::new(),
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateMovieError::InvalidName)));
        Ok(())
    }

    #[tokio::test]
    async fn create_movie_invalid_country_is_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let harness = harness_with(
            users,
            no_permissions(),
            MockMovieRepository::new(),
            MockSeriesRepository::new(),
            MockVideoRepository::new(),
        );
        let use_case: UseCase = CreateMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMovieCommand::new(
                1,
                5,
                "Some Like It Hot".to_owned(),
                None,
                Some("USA".to_owned()),
                None,
                Vec::new(),
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateMovieError::InvalidCountry)));
        Ok(())
    }

    #[tokio::test]
    async fn create_movie_empty_genre_is_invalid() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let harness = harness_with(
            users,
            no_permissions(),
            MockMovieRepository::new(),
            MockSeriesRepository::new(),
            MockVideoRepository::new(),
        );
        let use_case: UseCase = CreateMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMovieCommand::new(
                1,
                5,
                "Some Like It Hot".to_owned(),
                None,
                None,
                None,
                vec![String::new()],
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateMovieError::InvalidGenre)));
        Ok(())
    }

    #[tokio::test]
    async fn create_movie_video_backing_an_episode_is_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange: an episode already backs video 5.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut videos = MockVideoRepository::new();
        let video = sample_video(5, 5)?;
        videos.expect_search().times(1).return_once(move |_| {
            let found = video.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
        let mut movies = MockMovieRepository::new();
        movies
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        movies.expect_create().times(0);
        let attached = sample_episode(1, 5)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search_episodes()
            .times(1)
            .return_once(move |_| {
                let found = attached.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
        let harness = harness_with(users, no_permissions(), movies, series, videos);
        let use_case: UseCase = CreateMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(command()).await;

        // Assert
        assert!(matches!(result, Err(CreateMovieError::Conflict)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_movie_invalid_cover_is_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange: the repository rejects the cover through the scope trigger.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut videos = MockVideoRepository::new();
        let video = sample_video(5, 5)?;
        videos.expect_search().times(1).return_once(move |_| {
            let found = video.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
        let mut movies = MockMovieRepository::new();
        movies
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        movies
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::Conflict) }));
        let mut series = MockSeriesRepository::new();
        series
            .expect_search_episodes()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, no_permissions(), movies, series, videos);
        let use_case: UseCase = CreateMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMovieCommand::new(
                1,
                5,
                "Some Like It Hot".to_owned(),
                None,
                None,
                None,
                Vec::new(),
                Some(9),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateMovieError::Conflict)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_movie_missing_cover_is_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange: the repository rejects the cover through the scope trigger.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut videos = MockVideoRepository::new();
        let video = sample_video(5, 5)?;
        videos.expect_search().times(1).return_once(move |_| {
            let found = video.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
        let mut movies = MockMovieRepository::new();
        movies
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        movies
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::DataIntegrityViolation) }));
        let mut series = MockSeriesRepository::new();
        series
            .expect_search_episodes()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, no_permissions(), movies, series, videos);
        let use_case: UseCase = CreateMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(CreateMovieCommand::new(
                1,
                5,
                "Some Like It Hot".to_owned(),
                None,
                None,
                None,
                Vec::new(),
                Some(9),
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(CreateMovieError::Conflict)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn create_movie_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(
            users,
            no_permissions(),
            MockMovieRepository::new(),
            MockSeriesRepository::new(),
            MockVideoRepository::new(),
        );
        let use_case: UseCase = CreateMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(command()).await;

        // Assert
        assert!(matches!(result, Err(CreateMovieError::NoSuchCaller)));
        Ok(())
    }

    #[tokio::test]
    async fn create_movie_dependency_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange: the video load fails at the port.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut videos = MockVideoRepository::new();
        videos
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::Unavailable) }));
        let harness = harness_with(
            users,
            no_permissions(),
            MockMovieRepository::new(),
            MockSeriesRepository::new(),
            videos,
        );
        let use_case: UseCase = CreateMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(command()).await;

        // Assert
        assert!(matches!(result, Err(CreateMovieError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
