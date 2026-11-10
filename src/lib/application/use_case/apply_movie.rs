use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tracing::warn;

use crate::application::port::{
    ApplyMovieCommand, ApplyMovieError, ApplyMovieResponse, ApplyMovieUseCase,
};
use crate::domain::model::{Action, MediaMetadata, Movie, Target};
use crate::domain::port::{
    IdentityUnitOfWork, LibraryUnitOfWork, MovieFilter, MovieRepository as _, PermissionFilter,
    PermissionRepository as _, RepositoryError, UnitOfWork as _, UnitOfWorkFactory, UserFilter,
    UserRepository as _, UserUnitOfWork, VideoMetadataProvider,
};
use crate::domain::service::{Authorization, Decision};

/// Use case implementation for applying external metadata to a film.
///
/// The write is authoritative: every field the provider reports replaces the
/// stored value, and a date, country or genre list it does not report is
/// cleared. The cover and the backing video are never part of the apply.
pub(crate) struct ApplyMovie<F, P> {
    /// Configured provider chain, absent when video identification is unavailable.
    metadata_provider: Option<Arc<P>>,
    /// Factory opening the unit of work wrapping the write.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, P: VideoMetadataProvider> ApplyMovie<F, P> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>, metadata_provider: Option<Arc<P>>) -> Self {
        Self {
            metadata_provider,
            unit_of_work_factory,
        }
    }
}

impl<F, P> ApplyMovieUseCase for ApplyMovie<F, P>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
    P: VideoMetadataProvider,
{
    fn execute<'future>(
        &'future self,
        command: ApplyMovieCommand,
    ) -> Pin<Box<dyn Future<Output = Result<ApplyMovieResponse, ApplyMovieError>> + Send + 'future>>
    {
        let metadata_provider = self.metadata_provider.as_ref().map(Arc::clone);
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| ApplyMovieError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| ApplyMovieError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ApplyMovieError::NoSuchCaller)?;

                let permissions = unit_of_work
                    .permissions()
                    .search(&PermissionFilter {
                        user_id: Some(caller.id()),
                        ..PermissionFilter::default()
                    })
                    .await
                    .map_err(|error| ApplyMovieError::Unknown(error.into()))?;

                let movie = unit_of_work
                    .movies()
                    .search(&MovieFilter {
                        id: Some(command.movie_id()),
                        ..MovieFilter::default()
                    })
                    .await
                    .map_err(|error| ApplyMovieError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ApplyMovieError::NoSuchMovie)?;

                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::LibraryRead, &movie)
                {
                    warn!(
                        target: "security",
                        event = "authz_failed",
                        actor = caller.id(),
                        action = %denial.action().verb(),
                        target = %denial.action().target(),
                        "denied an action"
                    );
                    return Err(ApplyMovieError::NoSuchMovie);
                }
                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &permissions, Action::LibraryManage, &movie)
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
                        ApplyMovieError::NoSuchMovie
                    } else {
                        ApplyMovieError::Forbidden
                    });
                }

                let Some(provider) = metadata_provider else {
                    return Err(ApplyMovieError::ProviderUnavailable);
                };

                let target = Target::Movie(command.reference().clone());
                let metadata = provider
                    .lookup(&target)
                    .await
                    .map_err(|_| ApplyMovieError::ProviderFailure)?
                    .ok_or(ApplyMovieError::UnknownReference)?;

                let MediaMetadata::Movie {
                    country_of_origin,
                    genres,
                    plot,
                    release_date,
                    title,
                    ..
                } = metadata
                else {
                    return Err(ApplyMovieError::InvalidReference);
                };

                let updated = Movie::try_new(
                    movie.id(),
                    title,
                    plot,
                    country_of_origin,
                    release_date,
                    genres
                        .iter()
                        .map(|genre| genre.as_str().to_owned())
                        .collect(),
                    movie.cover_image_id(),
                    movie.video().clone(),
                )
                .map_err(|_| ApplyMovieError::InvalidReference)?;

                let stored = unit_of_work.movies().save(updated).await.map_err(|error| {
                    if matches!(
                        error,
                        RepositoryError::AlreadyExist | RepositoryError::Conflict
                    ) {
                        ApplyMovieError::Conflict
                    } else {
                        ApplyMovieError::Unknown(error.into())
                    }
                })?;

                Ok(ApplyMovieResponse::new(stored))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| ApplyMovieError::Unknown(error.into()))?;
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

    use crate::domain::model::{ExternalRef, Genre, ProviderId, Role, User};

    use crate::domain::port::{
        ExternalServiceError, MockMovieRepository, MockPermissionRepository, MockUserRepository,
        MockVideoMetadataProvider,
    };

    use crate::test_helpers::*;
    use chrono::NaiveDate;
    use std::error::Error;
    use std::sync::atomic::Ordering;

    type UseCase = ApplyMovie<TestFactory, MockVideoMetadataProvider>;

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

    fn use_case(
        harness: &GalleryFactoryHarness,
        provider: Option<MockVideoMetadataProvider>,
    ) -> UseCase {
        ApplyMovie::new(Arc::clone(&harness.factory), provider.map(Arc::new))
    }

    fn command() -> ApplyMovieCommand {
        ApplyMovieCommand::new(1, 7, ExternalRef::new(ProviderId::Tmdb, "27205".to_owned()))
    }

    fn movie_metadata() -> MediaMetadata {
        MediaMetadata::Movie {
            artwork: None,
            country_of_origin: Some("GB".to_owned()),
            genres: vec![Genre::Comedy],
            plot: "A new plot".to_owned(),
            release_date: NaiveDate::from_ymd_opt(1960, 1, 2),
            title: "A new name".to_owned(),
        }
    }

    fn loaded(movies: &mut MockMovieRepository, stored: Movie) {
        movies.expect_search().times(1).return_once(move |_| {
            let found = stored.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
    }

    #[tokio::test]
    async fn apply_movie_writes_the_reported_metadata() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut movies = MockMovieRepository::new();
        loaded(&mut movies, sample_movie(7)?);
        movies
            .expect_save()
            .times(1)
            .returning(|movie| Box::pin(async move { Ok(movie) }));
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_lookup()
            .times(1)
            .withf(|target| matches!(target, Target::Movie(_)))
            .return_once(|_| Box::pin(async { Ok(Some(movie_metadata())) }));
        let harness = harness_with(users, no_permissions(), movies);

        // Act
        let response = use_case(&harness, Some(provider))
            .execute(command())
            .await?;

        // Assert
        let movie = response.movie();
        assert_eq!(movie.name(), "A new name");
        assert_eq!(movie.plot(), "A new plot");
        assert_eq!(movie.country_of_origin(), Some("GB"));
        assert_eq!(movie.release_date(), NaiveDate::from_ymd_opt(1960, 1, 2));
        assert_eq!(movie.genre_ids(), ["COMEDY"]);
        assert_eq!(movie.video_id(), 7, "the backing video must be untouched");
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn apply_movie_clears_what_the_provider_does_not_report() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut movies = MockMovieRepository::new();
        loaded(&mut movies, sample_movie(7)?);
        movies
            .expect_save()
            .times(1)
            .returning(|movie| Box::pin(async move { Ok(movie) }));
        let mut provider = MockVideoMetadataProvider::new();
        provider.expect_lookup().times(1).return_once(|_| {
            Box::pin(async {
                Ok(Some(MediaMetadata::Movie {
                    artwork: None,
                    country_of_origin: None,
                    genres: Vec::new(),
                    plot: String::new(),
                    release_date: None,
                    title: "A new name".to_owned(),
                }))
            })
        });
        let harness = harness_with(users, no_permissions(), movies);

        // Act
        let response = use_case(&harness, Some(provider))
            .execute(command())
            .await?;

        // Assert
        assert_eq!(response.movie().country_of_origin(), None);
        assert_eq!(response.movie().release_date(), None);
        assert!(response.movie().genre_ids().is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn apply_movie_reports_a_provider_failure() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut movies = MockMovieRepository::new();
        loaded(&mut movies, sample_movie(7)?);
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_lookup()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ExternalServiceError::RateLimited) }));
        let harness = harness_with(users, no_permissions(), movies);

        // Act
        let result = use_case(&harness, Some(provider)).execute(command()).await;

        // Assert
        assert!(matches!(result, Err(ApplyMovieError::ProviderFailure)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn apply_movie_reports_an_unknown_reference() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut movies = MockMovieRepository::new();
        loaded(&mut movies, sample_movie(7)?);
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_lookup()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(None) }));
        let harness = harness_with(users, no_permissions(), movies);

        // Act
        let result = use_case(&harness, Some(provider)).execute(command()).await;

        // Assert
        assert!(matches!(result, Err(ApplyMovieError::UnknownReference)));
        Ok(())
    }

    #[tokio::test]
    async fn apply_movie_rejects_another_kind() -> Result<(), Box<dyn Error>> {
        // Arrange: the provider resolves a show under the film route.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut movies = MockMovieRepository::new();
        loaded(&mut movies, sample_movie(7)?);
        let mut provider = MockVideoMetadataProvider::new();
        provider.expect_lookup().times(1).return_once(|_| {
            Box::pin(async {
                Ok(Some(MediaMetadata::Series {
                    artwork: None,
                    country_of_origin: None,
                    first_air_date: None,
                    genres: Vec::new(),
                    plot: String::new(),
                    title: "Twin Peaks".to_owned(),
                }))
            })
        });
        let harness = harness_with(users, no_permissions(), movies);

        // Act
        let result = use_case(&harness, Some(provider)).execute(command()).await;

        // Assert
        assert!(matches!(result, Err(ApplyMovieError::InvalidReference)));
        Ok(())
    }

    #[tokio::test]
    async fn apply_movie_reports_an_unconfigured_provider() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut movies = MockMovieRepository::new();
        loaded(&mut movies, sample_movie(7)?);
        let harness = harness_with(users, no_permissions(), movies);

        // Act
        let result = use_case(&harness, None).execute(command()).await;

        // Assert
        assert!(matches!(result, Err(ApplyMovieError::ProviderUnavailable)));
        Ok(())
    }

    #[tokio::test]
    async fn apply_movie_reports_a_missing_film() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut movies = MockMovieRepository::new();
        movies
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = harness_with(users, no_permissions(), movies);

        // Act
        let result = use_case(&harness, None).execute(command()).await;

        // Assert
        assert!(matches!(result, Err(ApplyMovieError::NoSuchMovie)));
        Ok(())
    }

    #[tokio::test]
    async fn apply_movie_forbids_a_caller_without_manage() -> Result<(), Box<dyn Error>> {
        // Arrange: a standard caller may read the film but not manage it.
        let mut users = MockUserRepository::new();
        expect_caller(&mut users, 1)?;
        let mut movies = MockMovieRepository::new();
        loaded(&mut movies, sample_movie(7)?);
        let harness = harness_with(users, no_permissions(), movies);

        // Act
        let result = use_case(&harness, None).execute(command()).await;

        // Assert
        assert!(matches!(result, Err(ApplyMovieError::Forbidden)));
        Ok(())
    }

    #[tokio::test]
    async fn apply_movie_maps_a_write_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut movies = MockMovieRepository::new();
        loaded(&mut movies, sample_movie(7)?);
        movies
            .expect_save()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::Conflict) }));
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_lookup()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(Some(movie_metadata())) }));
        let harness = harness_with(users, no_permissions(), movies);

        // Act
        let result = use_case(&harness, Some(provider)).execute(command()).await;

        // Assert
        assert!(matches!(result, Err(ApplyMovieError::Conflict)));
        Ok(())
    }

    #[tokio::test]
    async fn apply_movie_dependency_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange: the film load fails at the port.
        let mut users = MockUserRepository::new();
        expect_admin(&mut users, 1)?;
        let mut movies = MockMovieRepository::new();
        movies
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::Unavailable) }));
        let harness = harness_with(users, no_permissions(), movies);

        // Act
        let result = use_case(&harness, None).execute(command()).await;

        // Assert
        assert!(matches!(result, Err(ApplyMovieError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
