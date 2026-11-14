use crate::application::authorization::authorize;
use crate::domain::model::{ResourceKind, ResourceRef};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{GetMovieCommand, GetMovieError, GetMovieResponse, GetMovieUseCase};
use crate::domain::model::Action;
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, MovieFilter, MovieRepository as _, UnitOfWork as _,
    UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork,
};

/// Use case implementation for fetching one film.
pub(crate) struct GetMovie<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetMovie<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
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
                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::Movie, movie.id()),
                    Action::View,
                    None,
                )
                .await
                .map_err(|_| GetMovieError::NoSuchMovie)?;

                Ok(GetMovieResponse::new(
                    movie.id(),
                    movie.name().to_owned(),
                    movie.plot().to_owned(),
                    movie.country_of_origin().map(str::to_owned),
                    movie.release_date(),
                    movie.genre_ids().to_vec(),
                    movie.cover_image_id(),
                    movie.video().clone(),
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
    use super::*;
    use allure_cargotest::allure_test;

    use crate::domain::model::Movie;

    use crate::domain::port::{MockMovieRepository, MockUserRepository, RepositoryError};

    use crate::test_helpers::*;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = GetMovie<TestFactory>;

    /// Build a Library test harness around the given mocks.
    fn harness_with(
        users: MockUserRepository,
        movies: MockMovieRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            resources: no_resources(),
            movies,
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    #[allure_test]
    #[tokio::test]
    async fn get_movie_returns_the_requested_film() -> Result<(), Box<dyn Error>> {
        allure.epic("Movies");
        allure.feature("REQ-MOVIES-003");
        allure.story("SCN-MOVIES-003-A");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let movie = sample_movie(7)?;
        let expected_video = movie.video().clone();
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
                Some("US".to_owned()),
                Some(video_date()),
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

    #[allure_test]
    #[tokio::test]
    async fn get_movie_returns_a_film_with_its_cover() -> Result<(), Box<dyn Error>> {
        allure.epic("Movies");
        allure.feature("REQ-MOVIES-003");
        allure.story("SCN-MOVIES-003-A");
        // Arrange: a film whose artwork is file 19.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let movie = Movie::try_new(
            7,
            "Movie 7".to_owned(),
            "A plot".to_owned(),
            Some("US".to_owned()),
            Some(video_date()),
            vec!["DRAMA".to_owned()],
            Some(19),
            sample_video(7, 7)?,
        )?;
        assert_eq!(
            movie.cover_image_id(),
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
        assert_eq!(response.cover_image_id(), Some(19));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn get_movie_unknown_film_returns_no_such_movie() -> Result<(), Box<dyn Error>> {
        allure.epic("Movies");
        allure.feature("REQ-MOVIES-003");
        allure.story("SCN-MOVIES-003-C");
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
    #[allure_test]
    #[tokio::test]
    async fn get_movie_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        allure.epic("Movies");
        allure.feature("REQ-MOVIES-003");
        allure.story("SCN-MOVIES-003-A");
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

    #[allure_test]
    #[tokio::test]
    async fn get_movie_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        allure.epic("Movies");
        allure.feature("REQ-MOVIES-003");
        allure.story("SCN-MOVIES-003-A");
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
