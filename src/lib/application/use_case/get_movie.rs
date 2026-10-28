use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::get_movie::GetMovieCommand;
use crate::application::port::get_movie::GetMovieError;
use crate::application::port::get_movie::GetMovieResponse;
use crate::application::port::get_movie::GetMovieUseCase;
use crate::application::port::video_summary::VideoSummary;
use crate::application::security_event;
use crate::domain::model::permission::Action;
use crate::domain::port::identity_unit_of_work::IdentityUnitOfWork;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::movie_repository::MovieFilter;
use crate::domain::port::movie_repository::MovieRepository as _;
use crate::domain::port::permission_repository::PermissionFilter;
use crate::domain::port::permission_repository::PermissionRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::authorization::Authorization;
use crate::domain::service::authorization::Decision;

/// Use case implementation for fetching one film.
pub struct GetMovie<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetMovie<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetMovieUseCase for GetMovie<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: GetMovieCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetMovieResponse, GetMovieError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetMovieError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| GetMovieError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetMovieError::NoSuchCaller)?;

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| GetMovieError::Unknown(error.into()))?;

                let movie = unit_of_work
                    .movies()
                    .search(&MovieFilter {
                        id: Some(command.movie_id()),
                        ..MovieFilter::default()
                    })
                    .await
                    .map_err(|error| GetMovieError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(GetMovieError::NoSuchMovie)?;
                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::CatalogRead, &movie)
                {
                    security_event::authorization_failed(
                        caller.id(),
                        denial.action().verb(),
                        denial.action().target(),
                    );
                    return Err(GetMovieError::NoSuchMovie);
                }

                Ok(GetMovieResponse::new(
                    movie.id(),
                    movie.name().to_owned(),
                    movie.plot().to_owned(),
                    movie.country_of_origin().to_owned(),
                    movie.release_date(),
                    movie.genre_ids().to_vec(),
                    movie.cover_file_id(),
                    VideoSummary::from(movie.video()),
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetMovieError::Unknown(error.into()))?;
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

    use crate::application::port::get_movie::GetMovieCommand;
    use crate::application::port::get_movie::GetMovieError;
    use crate::application::port::get_movie::GetMovieResponse;
    use crate::application::port::get_movie::GetMovieUseCase as _;
    use crate::application::port::video_summary::VideoSummary;
    use crate::domain::model::movie::Movie;
    use crate::domain::model::permission::Action;
    use crate::domain::model::permission::Permission;
    use crate::domain::model::permission::Scope;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::movie_repository::MockMovieRepository;
    use crate::domain::port::permission_repository::MockPermissionRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;
    use crate::test_helpers::expect_caller;
    use crate::test_helpers::expect_permissions;
    use crate::test_helpers::no_permissions;
    use crate::test_helpers::sample_movie;
    use crate::test_helpers::sample_video;
    use crate::test_helpers::video_date;

    use super::GetMovie;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = GetMovie<TestFactory>;

    /// Build a Library test harness around the given mocks.
    fn harness_with(
        users: MockUserRepository,
        movies: MockMovieRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            permissions: no_permissions(),
            movies,
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    #[tokio::test]
    async fn get_movie_returns_the_requested_film() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let movie = sample_movie(7)?;
        let expected_video = VideoSummary::from(movie.video());
        let mut movies = MockMovieRepository::new();
        movies
            .expect_search()
            .times(1)
            .withf(|filter| filter.id == Some(7))
            .return_once(move |_| Box::pin(async move { Ok(vec![movie]) }));
        let harness = harness_with(users, movies);
        let use_case: UseCase = GetMovie::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(GetMovieCommand::new(1, 7)).await?;

        // Assert
        assert_eq!(
            response,
            GetMovieResponse::new(
                7,
                "Movie 7".to_owned(),
                "A plot".to_owned(),
                "US".to_owned(),
                video_date(),
                vec!["DRAMA".to_owned()],
                None,
                expected_video,
            ),
            "the representation must carry the requested film"
        );
        assert!(
            harness.committed.load(Ordering::SeqCst),
            "a successful read must commit"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_returns_a_film_with_its_cover() -> Result<(), Box<dyn Error>> {
        // Arrange: a film whose artwork is file 19.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let movie = Movie::try_new(
            7,
            "Movie 7".to_owned(),
            "A plot".to_owned(),
            "US".to_owned(),
            video_date(),
            vec!["DRAMA".to_owned()],
            Some(19),
            sample_video(7, 7)?,
        )?;
        assert_eq!(
            movie.cover_file_id(),
            Some(19),
            "a film built with artwork must report the identifier"
        );
        let mut movies = MockMovieRepository::new();
        movies
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![movie]) }));
        let harness = harness_with(users, movies);
        let use_case: UseCase = GetMovie::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(GetMovieCommand::new(1, 7)).await?;

        // Assert
        assert_eq!(response.cover_file_id(), Some(19));
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_unknown_film_returns_no_such_movie() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut movies = MockMovieRepository::new();
        movies
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, movies);
        let use_case: UseCase = GetMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetMovieCommand::new(1, 404)).await;

        // Assert
        assert!(
            matches!(result, Err(GetMovieError::NoSuchMovie)),
            "an unknown film must be reported"
        );
        assert!(
            harness.rolled_back.load(Ordering::SeqCst),
            "an unknown film must roll back"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_none_override_returns_no_such_movie() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut movies = MockMovieRepository::new();
        let movie = sample_movie(7)?;
        movies
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![movie]) }));
        let mut permissions = MockPermissionRepository::new();
        expect_permissions(
            &mut permissions,
            1,
            vec![Permission::try_new(1, Action::CatalogRead, Scope::None)?],
        );
        let unit_of_work = TestUnitOfWork {
            movies,
            permissions,
            users,
            ..TestUnitOfWork::default()
        };
        let harness = GalleryFactoryHarness::new(unit_of_work);
        let use_case: UseCase = GetMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetMovieCommand::new(1, 7)).await;

        // Assert
        assert!(matches!(result, Err(GetMovieError::NoSuchMovie)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockMovieRepository::new());
        let use_case: UseCase = GetMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetMovieCommand::new(999, 7)).await;

        // Assert
        assert!(
            matches!(result, Err(GetMovieError::NoSuchCaller)),
            "a deleted caller must be rejected"
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_movie_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut movies = MockMovieRepository::new();
        movies
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, movies);
        let use_case: UseCase = GetMovie::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(GetMovieCommand::new(1, 7)).await;

        // Assert
        assert!(
            matches!(result, Err(GetMovieError::Unknown(_))),
            "a repository failure must map to an unknown error"
        );
        assert!(
            harness.rolled_back.load(Ordering::SeqCst),
            "a repository failure must roll back"
        );
        Ok(())
    }
}
