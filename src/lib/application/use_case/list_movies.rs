use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::list_movies::ListMoviesCommand;
use crate::application::port::list_movies::ListMoviesError;
use crate::application::port::list_movies::ListMoviesItem;
use crate::application::port::list_movies::ListMoviesResponse;
use crate::application::port::list_movies::ListMoviesUseCase;
use crate::application::port::video_summary::VideoSummary;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::movie_repository::MovieFilter;
use crate::domain::port::movie_repository::MovieRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;

/// Use case implementation for listing the films the caller may see.
pub struct ListMovies<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> ListMovies<F> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> ListMoviesUseCase for ListMovies<F>
where
    F: UnitOfWorkFactory,
    F::Uow: LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: ListMoviesCommand,
    ) -> Pin<Box<dyn Future<Output = Result<ListMoviesResponse, ListMoviesError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| ListMoviesError::Unknown(error.into()))?;

            let result = async {
                unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| ListMoviesError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ListMoviesError::NoSuchCaller)?;

                let movies = unit_of_work
                    .movies()
                    .search(&MovieFilter {
                        name: command.name().map(str::to_owned),
                        ..MovieFilter::default()
                    })
                    .await
                    .map_err(|error| ListMoviesError::Unknown(error.into()))?;

                let total = movies.len();
                let offset = command.offset().unwrap_or(0);
                let page = movies
                    .into_iter()
                    .skip(offset)
                    .take(command.limit().unwrap_or(usize::MAX))
                    .map(|movie| {
                        ListMoviesItem::new(
                            movie.id(),
                            movie.name().to_owned(),
                            movie.plot().to_owned(),
                            movie.country_of_origin().to_owned(),
                            movie.release_date(),
                            movie.genre_ids().to_vec(),
                            movie.cover_file_id(),
                            VideoSummary::from(movie.video()),
                        )
                    })
                    .collect();

                Ok(ListMoviesResponse::new(page, total))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| ListMoviesError::Unknown(error.into()))?;
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

    use crate::application::port::list_movies::ListMoviesCommand;
    use crate::application::port::list_movies::ListMoviesError;
    use crate::application::port::list_movies::ListMoviesItem;
    use crate::application::port::list_movies::ListMoviesUseCase as _;
    use crate::application::port::video_summary::VideoSummary;
    use crate::domain::model::movie::Movie;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::movie_repository::MockMovieRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::GalleryFactoryHarness;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWork;
    use crate::test_helpers::expect_caller;
    use crate::test_helpers::sample_movie;
    use crate::test_helpers::sample_video;
    use crate::test_helpers::video_date;

    use super::ListMovies;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = ListMovies<TestFactory>;

    /// Build a Library test harness around the given mocks.
    fn harness_with(
        users: MockUserRepository,
        movies: MockMovieRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            movies,
            users,
            ..TestUnitOfWork::default()
        };
        GalleryFactoryHarness::new(unit_of_work)
    }

    #[tokio::test]
    async fn list_movies_returns_the_page_with_its_video() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let first = sample_movie(1)?;
        let expected_video = VideoSummary::from(first.video());
        let second = sample_movie(2)?;
        let mut movies = MockMovieRepository::new();
        movies
            .expect_search()
            .times(1)
            .withf(|filter| filter.id.is_none() && filter.name.is_none())
            .return_once(move |_| Box::pin(async move { Ok(vec![first, second]) }));
        let harness = harness_with(users, movies);
        let use_case: UseCase = ListMovies::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListMoviesCommand::new(1, None, None, None))
            .await?;

        // Assert
        assert_eq!(response.total(), 2, "the total must count every film");
        assert_eq!(
            response.movies().first(),
            Some(&ListMoviesItem::new(
                1,
                "Movie 1".to_owned(),
                "A plot".to_owned(),
                "US".to_owned(),
                video_date(),
                vec!["DRAMA".to_owned()],
                None,
                expected_video,
            ))
        );
        assert!(
            harness.committed.load(Ordering::SeqCst),
            "a successful listing must commit"
        );
        Ok(())
    }

    #[tokio::test]
    async fn list_movies_carries_the_cover_of_a_film_with_artwork() -> Result<(), Box<dyn Error>> {
        // Arrange: one covered film beside one without artwork.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let covered = Movie::try_new(
            1,
            "Movie 1".to_owned(),
            "A plot".to_owned(),
            "US".to_owned(),
            video_date(),
            Vec::new(),
            Some(19),
            sample_video(1, 1)?,
        )?;
        assert_eq!(
            covered.cover_file_id(),
            Some(19),
            "a film built with artwork must report the identifier"
        );
        let uncovered = sample_movie(2)?;
        assert_eq!(
            uncovered.cover_file_id(),
            None,
            "a film built without artwork must report none"
        );
        let mut movies = MockMovieRepository::new();
        movies
            .expect_search()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(vec![covered, uncovered]) }));
        let harness = harness_with(users, movies);
        let use_case: UseCase = ListMovies::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListMoviesCommand::new(1, None, None, None))
            .await?;

        // Assert
        assert_eq!(
            response
                .movies()
                .first()
                .and_then(ListMoviesItem::cover_file_id),
            Some(19)
        );
        assert_eq!(
            response
                .movies()
                .get(1)
                .and_then(ListMoviesItem::cover_file_id),
            None
        );
        Ok(())
    }

    #[tokio::test]
    async fn list_movies_forwards_the_filter_and_paginates() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let first = sample_movie(1)?;
        let second = sample_movie(2)?;
        let third = sample_movie(3)?;
        let mut movies = MockMovieRepository::new();
        movies
            .expect_search()
            .times(1)
            .withf(|filter| filter.name.as_deref() == Some("Movie"))
            .return_once(move |_| Box::pin(async move { Ok(vec![first, second, third]) }));
        let harness = harness_with(users, movies);
        let use_case: UseCase = ListMovies::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case
            .execute(ListMoviesCommand::new(
                1,
                Some("Movie".to_owned()),
                Some(1),
                Some(1),
            ))
            .await?;

        // Assert
        assert_eq!(response.total(), 3, "the total must precede pagination");
        assert_eq!(
            response.movies().first().map(ListMoviesItem::id),
            Some(2),
            "the page must start at the requested offset"
        );
        Ok(())
    }

    #[tokio::test]
    async fn list_movies_unknown_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, MockMovieRepository::new());
        let use_case: UseCase = ListMovies::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListMoviesCommand::new(999, None, None, None))
            .await;

        // Assert
        assert!(
            matches!(result, Err(ListMoviesError::NoSuchCaller)),
            "a deleted caller must be rejected"
        );
        assert!(
            harness.rolled_back.load(Ordering::SeqCst),
            "a rejected caller must roll back"
        );
        Ok(())
    }

    #[tokio::test]
    async fn list_movies_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut movies = MockMovieRepository::new();
        movies
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = harness_with(users, movies);
        let use_case: UseCase = ListMovies::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListMoviesCommand::new(1, None, None, None))
            .await;

        // Assert
        assert!(
            matches!(result, Err(ListMoviesError::Unknown(_))),
            "a repository failure must map to an unknown error"
        );
        assert!(
            harness.rolled_back.load(Ordering::SeqCst),
            "a repository failure must roll back"
        );
        Ok(())
    }

    #[tokio::test]
    async fn list_movies_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut movies = MockMovieRepository::new();
        movies
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, movies);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case: UseCase = ListMovies::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(ListMoviesCommand::new(1, None, None, None))
            .await;

        // Assert
        assert!(
            matches!(result, Err(ListMoviesError::Unknown(_))),
            "a commit failure must map to an unknown error"
        );
        assert!(
            harness.committed.load(Ordering::SeqCst),
            "the use case must attempt the commit"
        );
        Ok(())
    }
}
