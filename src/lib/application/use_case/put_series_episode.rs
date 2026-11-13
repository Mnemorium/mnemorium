use crate::application::authorization::authorize;
use crate::domain::model::ChildKind;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tracing::warn;

use crate::application::port::{
    PutSeriesEpisodeCommand, PutSeriesEpisodeError, PutSeriesEpisodeResponse,
    PutSeriesEpisodeUseCase,
};
use crate::domain::model::{Action, Episode, EpisodeIndex, ResourceKind, ResourceRef};
use crate::domain::port::{
    EpisodeFilter, IdentityUnitOfWork, LibraryUnitOfWork, MovieFilter, MovieRepository as _,
    RepositoryError, SeriesFilter, SeriesRepository as _, UnitOfWork as _, UnitOfWorkFactory,
    UserFilter, UserRepository as _, UserUnitOfWork, VideoFilter, VideoRepository as _,
};
use crate::domain::service::{NamePolicy, NamePolicyError};

/// Use case implementation for creating or replacing a season episode.
pub(crate) struct PutSeriesEpisode<F> {
    /// Factory opening the unit of work wrapping the write.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> PutSeriesEpisode<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> PutSeriesEpisodeUseCase for PutSeriesEpisode<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: PutSeriesEpisodeCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PutSeriesEpisodeResponse, PutSeriesEpisodeError>>
                + Send
                + 'future,
        >,
    > {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| PutSeriesEpisodeError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| PutSeriesEpisodeError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(PutSeriesEpisodeError::NoSuchCaller)?;

                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::Series, command.series_id()),
                    Action::CreateChild,
                    Some(ChildKind::Episode),
                )
                .await
                .map_err(|error| {
                    if error.conceals() {
                        PutSeriesEpisodeError::NoSuchSeries
                    } else {
                        PutSeriesEpisodeError::Forbidden
                    }
                })?;

                let series = unit_of_work
                    .series()
                    .search(&SeriesFilter {
                        id: Some(command.series_id()),
                        ..SeriesFilter::default()
                    })
                    .await
                    .map_err(|error| PutSeriesEpisodeError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(PutSeriesEpisodeError::NoSuchSeries)?;

                let season = series
                    .seasons()
                    .iter()
                    .find(|season| season.index().value() == command.season_index())
                    .ok_or(PutSeriesEpisodeError::NoSuchSeason)?;

                let name =
                    NamePolicy::validate(command.name().to_owned()).map_err(
                        |error| match error {
                            NamePolicyError::Empty => {
                                warn!(
                                    target: "security",
                                    event = "input_validation_failed",
                                    field = %"name",
                                    reason = %"empty",
                                    "rejected a client-supplied value"
                                );
                                PutSeriesEpisodeError::InvalidName
                            }
                            NamePolicyError::TooLong => {
                                warn!(
                                    target: "security",
                                    event = "input_validation_failed",
                                    field = %"name",
                                    reason = %"too_long",
                                    "rejected a client-supplied value"
                                );
                                PutSeriesEpisodeError::NameTooLong
                            }
                        },
                    )?;

                let video = unit_of_work
                    .videos()
                    .search(&VideoFilter {
                        id: Some(command.video_id()),
                        ..VideoFilter::default()
                    })
                    .await
                    .map_err(|error| PutSeriesEpisodeError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(PutSeriesEpisodeError::NoSuchVideo)?;

                let film = unit_of_work
                    .movies()
                    .search(&MovieFilter {
                        video_id: Some(command.video_id()),
                        ..MovieFilter::default()
                    })
                    .await
                    .map_err(|error| PutSeriesEpisodeError::Unknown(error.into()))?
                    .into_iter()
                    .next();
                if film.is_some() {
                    return Err(PutSeriesEpisodeError::Conflict);
                }

                let existing = season
                    .episodes()
                    .iter()
                    .find(|episode| episode.index().value() == command.episode_index());
                let already_attached =
                    existing.is_some_and(|episode| episode.video().id() == command.video_id());
                if !already_attached {
                    let attached = unit_of_work
                        .series()
                        .search_episodes(&EpisodeFilter {
                            video_id: Some(command.video_id()),
                            ..EpisodeFilter::default()
                        })
                        .await
                        .map_err(|error| PutSeriesEpisodeError::Unknown(error.into()))?;
                    if !attached.is_empty() {
                        return Err(PutSeriesEpisodeError::Conflict);
                    }
                }

                let episode = Episode::try_new(
                    EpisodeIndex::new(command.episode_index()),
                    name,
                    command.plot().unwrap_or_default().to_owned(),
                    command.air_date(),
                    video,
                )
                .map_err(|_| PutSeriesEpisodeError::InvalidName)?;

                let stored = unit_of_work
                    .series()
                    .save_episode(series.id(), command.season_index(), episode)
                    .await
                    .map_err(|error| {
                        if matches!(
                            error,
                            RepositoryError::AlreadyExist | RepositoryError::Conflict
                        ) {
                            PutSeriesEpisodeError::Conflict
                        } else {
                            PutSeriesEpisodeError::Unknown(error.into())
                        }
                    })?
                    .ok_or(PutSeriesEpisodeError::NoSuchSeason)?;

                Ok(PutSeriesEpisodeResponse::new(stored, existing.is_none()))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| PutSeriesEpisodeError::Unknown(error.into()))?;
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

    use crate::domain::port::{
        MockMovieRepository, MockResourceRepository, MockSeriesRepository, MockUserRepository,
        MockVideoRepository,
    };

    use crate::test_helpers::*;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    /// The use case under test, bound to the mocked unit-of-work factory.
    type UseCase = PutSeriesEpisode<TestFactory>;

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
        movies: MockMovieRepository,
        videos: MockVideoRepository,
    ) -> GalleryFactoryHarness {
        let unit_of_work = TestUnitOfWork {
            movies,
            resources,
            series,
            users,
            videos,
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

    /// A show holding season 1 with `episodes`.
    fn series_with_episodes(episodes: Vec<Episode>) -> Result<Series, Box<dyn Error>> {
        sample_series(7, vec![sample_season(1, episodes)])
    }

    /// A command writing episode 1 of season 1 backed by `video_id`.
    fn command(video_id: NumericID) -> PutSeriesEpisodeCommand {
        PutSeriesEpisodeCommand::new(
            1,
            7,
            1,
            1,
            video_id,
            "Pilot".to_owned(),
            Some("A plot".to_owned()),
            None,
        )
    }

    #[allure_test]
    #[tokio::test]
    async fn put_series_episode_new_index_reports_created() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-012");
        allure.story("SCN-SERIES-012-A");
        // Arrange: the season holds no episode and video 5 is free.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, series_with_episodes(Vec::new())?);
        let written = sample_episode(1, 5)?;
        series
            .expect_save_episode()
            .times(1)
            .withf(|series_id, season_index, episode| {
                *series_id == 7
                    && *season_index == 1
                    && episode.index().value() == 1
                    && episode.video().id() == 5
            })
            .return_once(move |_, _, _| {
                let copy = written.clone();
                Box::pin(async move { Ok(Some(copy)) })
            });
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
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        series
            .expect_search_episodes()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, no_resources(), series, movies, videos);
        let use_case: UseCase = PutSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(command(5)).await?;

        // Assert
        assert!(response.created(), "a free index must create the episode");
        assert_eq!(response.episode().index().value(), 1);
        assert_eq!(response.episode().video().id(), 5);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn put_series_episode_existing_index_replaces_the_video() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-012");
        allure.story("SCN-SERIES-012-B");
        allure.story("SCN-SERIES-012-C");
        // Arrange: episode 1 already backs video 20; the request names video 5.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(
            &mut series,
            series_with_episodes(vec![sample_episode(1, 20)?])?,
        );
        let written = sample_episode(1, 5)?;
        series
            .expect_save_episode()
            .times(1)
            .return_once(move |_, _, _| {
                let copy = written.clone();
                Box::pin(async move { Ok(Some(copy)) })
            });
        series
            .expect_search_episodes()
            .times(1)
            .withf(|filter| filter.video_id == Some(5))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
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
        let harness = harness_with(users, no_resources(), series, movies, videos);
        let use_case: UseCase = PutSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(command(5)).await?;

        // Assert
        assert!(!response.created(), "an existing index must be replaced");
        assert_eq!(response.episode().video().id(), 5);
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn put_series_episode_same_video_is_not_a_conflict() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-012");
        allure.story("SCN-SERIES-012-B");
        // Arrange: episode 1 already backs video 5, the same video named again.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(
            &mut series,
            series_with_episodes(vec![sample_episode(1, 5)?])?,
        );
        series.expect_search_episodes().times(0);
        let written = sample_episode(1, 5)?;
        series
            .expect_save_episode()
            .times(1)
            .return_once(move |_, _, _| {
                let copy = written.clone();
                Box::pin(async move { Ok(Some(copy)) })
            });
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
        let harness = harness_with(users, no_resources(), series, movies, videos);
        let use_case: UseCase = PutSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let response = use_case.execute(command(5)).await?;

        // Assert
        assert!(!response.created());
        assert_eq!(response.episode().video().id(), 5);
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn put_series_episode_video_backing_a_film_is_conflict() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-012");
        allure.story("SCN-SERIES-012-D");
        // Arrange: film 9 already backs video 5.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, series_with_episodes(Vec::new())?);
        series.expect_save_episode().times(0);
        let mut videos = MockVideoRepository::new();
        let video = sample_video(5, 5)?;
        videos.expect_search().times(1).return_once(move |_| {
            let found = video.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
        let film = sample_movie(9)?;
        let mut movies = MockMovieRepository::new();
        movies.expect_search().times(1).return_once(move |_| {
            let found = film.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
        let harness = harness_with(users, no_resources(), series, movies, videos);
        let use_case: UseCase = PutSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(command(5)).await;

        // Assert
        assert!(matches!(result, Err(PutSeriesEpisodeError::Conflict)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn put_series_episode_video_backing_another_episode_is_conflict()
    -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-012");
        allure.story("SCN-SERIES-012-D");
        // Arrange: video 5 already backs another episode.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, series_with_episodes(Vec::new())?);
        series.expect_save_episode().times(0);
        let attached = sample_episode(2, 5)?;
        series
            .expect_search_episodes()
            .times(1)
            .return_once(move |_| {
                let found = attached.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
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
        let harness = harness_with(users, no_resources(), series, movies, videos);
        let use_case: UseCase = PutSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(command(5)).await;

        // Assert
        assert!(matches!(result, Err(PutSeriesEpisodeError::Conflict)));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn put_series_episode_unknown_video_is_unprocessable() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-012");
        allure.story("SCN-SERIES-012-E");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, series_with_episodes(Vec::new())?);
        let mut videos = MockVideoRepository::new();
        videos
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(
            users,
            no_resources(),
            series,
            MockMovieRepository::new(),
            videos,
        );
        let use_case: UseCase = PutSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(command(5)).await;

        // Assert
        assert!(matches!(result, Err(PutSeriesEpisodeError::NoSuchVideo)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn put_series_episode_unknown_show_returns_no_such_series() -> Result<(), Box<dyn Error>>
    {
        allure.epic("Series");
        allure.feature("REQ-SERIES-012");
        allure.story("SCN-SERIES-012-F");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(
            users,
            no_resources(),
            series,
            MockMovieRepository::new(),
            MockVideoRepository::new(),
        );
        let use_case: UseCase = PutSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(command(5)).await;

        // Assert
        assert!(matches!(result, Err(PutSeriesEpisodeError::NoSuchSeries)));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn put_series_episode_unknown_season_returns_no_such_season() -> Result<(), Box<dyn Error>>
    {
        allure.epic("Series");
        allure.feature("REQ-SERIES-012");
        allure.story("SCN-SERIES-012-F");
        // Arrange: the show holds no season at index 1.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, sample_series(7, Vec::new())?);
        let harness = harness_with(
            users,
            no_resources(),
            series,
            MockMovieRepository::new(),
            MockVideoRepository::new(),
        );
        let use_case: UseCase = PutSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(command(5)).await;

        // Assert
        assert!(matches!(result, Err(PutSeriesEpisodeError::NoSuchSeason)));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn put_series_episode_empty_name_is_invalid() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-012");
        allure.story("SCN-SERIES-012-E");
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, series_with_episodes(Vec::new())?);
        let harness = harness_with(
            users,
            no_resources(),
            series,
            MockMovieRepository::new(),
            MockVideoRepository::new(),
        );
        let use_case: UseCase = PutSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case
            .execute(PutSeriesEpisodeCommand::new(
                1,
                7,
                1,
                1,
                5,
                "   ".to_owned(),
                None,
                None,
            ))
            .await;

        // Assert
        assert!(matches!(result, Err(PutSeriesEpisodeError::InvalidName)));
        Ok(())
    }
    #[allure_test]
    #[tokio::test]
    async fn put_series_episode_absent_at_write_returns_no_such_season()
    -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-012");
        allure.story("SCN-SERIES-012-F");
        // Arrange: the season disappears between the read and the write.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        expect_series(&mut series, series_with_episodes(Vec::new())?);
        series
            .expect_save_episode()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(None) }));
        series
            .expect_search_episodes()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
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
        let harness = harness_with(users, no_resources(), series, movies, videos);
        let use_case: UseCase = PutSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(command(5)).await;

        // Assert
        assert!(matches!(result, Err(PutSeriesEpisodeError::NoSuchSeason)));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn put_series_episode_dependency_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        allure.epic("Series");
        allure.feature("REQ-SERIES-012");
        allure.story("SCN-SERIES-012-A");
        // Arrange: the show load fails at the port.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut series = MockSeriesRepository::new();
        series
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::Unavailable) }));
        let harness = harness_with(
            users,
            no_resources(),
            series,
            MockMovieRepository::new(),
            MockVideoRepository::new(),
        );
        let use_case: UseCase = PutSeriesEpisode::new(Arc::clone(&harness.factory));

        // Act
        let result = use_case.execute(command(5)).await;

        // Assert
        assert!(matches!(result, Err(PutSeriesEpisodeError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
