use crate::domain::model::{Action, ResourceKind, ResourceRef};

use crate::application::authorization::authorize;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{
    ResolveVideoMetadataCommand, ResolveVideoMetadataError, ResolveVideoMetadataResponse,
    ResolveVideoMetadataUseCase,
};
use crate::domain::model::{MediaMetadata, Target};
use crate::domain::port::{
    IdentityUnitOfWork, UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _,
    UserUnitOfWork, VideoMetadataProvider,
};

/// Use case implementation for resolving one video target's metadata.
///
/// The use case owns the availability rule: with no configured provider it
/// reports [`ResolveVideoMetadataError::Unavailable`], which is distinct from a
/// provider that does not know the target. It also rejects a provider response
/// that names another kind, or another episode position, than the target asked
/// for, so a routed lookup can never publish another item's metadata. It
/// resolves once through the configured provider and never persists or logs the
/// target. It is guarded by `ViewMetadata` on the catalogue: the resolution addresses no
/// catalogue resource, so a denial conceals the whole surface.
pub(crate) struct ResolveVideoMetadata<F, P> {
    /// Configured provider chain, absent when video identification is unavailable.
    metadata_provider: Option<Arc<P>>,
    /// Factory opening the unit of work wrapping the caller load and the lookup.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, P: VideoMetadataProvider> ResolveVideoMetadata<F, P> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>, metadata_provider: Option<Arc<P>>) -> Self {
        Self {
            metadata_provider,
            unit_of_work_factory,
        }
    }
}

impl<F, P> ResolveVideoMetadataUseCase for ResolveVideoMetadata<F, P>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
    P: VideoMetadataProvider,
{
    fn execute<'future>(
        &'future self,
        command: ResolveVideoMetadataCommand,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<
                        Option<ResolveVideoMetadataResponse>,
                        ResolveVideoMetadataError,
                    >,
                > + Send
                + 'future,
        >,
    > {
        let metadata_provider = self.metadata_provider.as_ref().map(Arc::clone);
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| ResolveVideoMetadataError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| ResolveVideoMetadataError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(ResolveVideoMetadataError::NoSuchCaller)?;

                // The denial is a security decision the use case owns, so it logs
                // the catalogued `authz_failed` event (`OBS-002`, `OBS-006`); the
                // caller still receives the concealed not-found result.
                authorize(
                    &mut unit_of_work.resources(),
                    &caller,
                    ResourceRef::new(ResourceKind::Catalogue, 0),
                    Action::ViewMetadata,
                    None,
                )
                .await
                .map_err(|_| ResolveVideoMetadataError::NotPermitted)?;

                let Some(provider) = metadata_provider else {
                    return Err(ResolveVideoMetadataError::Unavailable);
                };

                let resolved = provider
                    .lookup(command.target())
                    .await
                    .map_err(|error| ResolveVideoMetadataError::Unknown(error.into()))?;
                let Some(metadata) = resolved else {
                    return Ok(None);
                };
                match (command.target(), &metadata) {
                    (&Target::Movie(_), &MediaMetadata::Movie { .. })
                    | (&Target::Series(_), &MediaMetadata::Series { .. }) => {}
                    (
                        &Target::Episode {
                            episode: requested_episode,
                            season: requested_season,
                            ..
                        },
                        &MediaMetadata::Episode {
                            episode, season, ..
                        },
                    ) if episode == requested_episode && season == requested_season => {}
                    (&Target::Episode { .. }, &MediaMetadata::Episode { .. }) => {
                        return Err(ResolveVideoMetadataError::PositionMismatch);
                    }
                    _ => return Err(ResolveVideoMetadataError::KindMismatch),
                }
                Ok(Some(ResolveVideoMetadataResponse::new(metadata)))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| ResolveVideoMetadataError::Unknown(error.into()))?;
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
    use crate::domain::model::{ExternalRef, ProviderId};
    use allure_cargotest::allure_test;

    use crate::domain::port::{
        ExternalServiceError, MockResourceRepository, MockUserRepository, MockVideoMetadataProvider,
    };
    use crate::test_helpers::*;
    use chrono::NaiveDate;
    use std::error::Error;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// The caller every test resolves for.
    const CALLER_ID: i64 = 1;

    type UseCase = ResolveVideoMetadata<TestFactory, MockVideoMetadataProvider>;

    /// A use case under test together with its transaction-lifecycle flags.
    struct Harness {
        /// Set when the unit of work is committed.
        committed: Arc<AtomicBool>,
        /// Set when the unit of work is rolled back.
        rolled_back: Arc<AtomicBool>,
        /// The use case under test.
        use_case: UseCase,
    }

    /// Build a use case over the given caller and permission repositories.
    fn use_case_with(
        users: MockUserRepository,
        resources: MockResourceRepository,
        provider: Option<MockVideoMetadataProvider>,
    ) -> Harness {
        let unit_of_work = TestUnitOfWork {
            resources,
            users,
            ..TestUnitOfWork::default()
        };
        let committed = Arc::clone(&unit_of_work.committed);
        let rolled_back = Arc::clone(&unit_of_work.rolled_back);
        let factory = Arc::new(TestFactory::new(unit_of_work));

        Harness {
            committed,
            rolled_back,
            use_case: ResolveVideoMetadata::new(factory, provider.map(Arc::new)),
        }
    }

    /// A reference to a film at TMDB.
    fn reference() -> ExternalRef {
        ExternalRef::new(ProviderId::Tmdb, "27205".to_owned())
    }

    /// The film target every film test resolves.
    fn movie_target() -> Target {
        Target::Movie(reference())
    }

    /// Film metadata for `Inception`.
    fn movie_metadata() -> MediaMetadata {
        MediaMetadata::Movie {
            artwork: None,
            country_of_origin: Some("US".to_owned()),
            genres: Vec::new(),
            plot: "A plot".to_owned(),
            release_date: NaiveDate::from_ymd_opt(2010, 7, 16),
            title: "Inception".to_owned(),
        }
    }

    /// Episode metadata for the addressed position.
    fn episode_metadata(season: u32, episode: u32) -> MediaMetadata {
        MediaMetadata::Episode {
            air_date: NaiveDate::from_ymd_opt(1990, 4, 8),
            artwork: None,
            episode,
            plot: "A plot".to_owned(),
            season,
            title: "Pilot".to_owned(),
        }
    }

    /// A provider that resolves to `metadata`.
    fn resolving(metadata: MediaMetadata) -> MockVideoMetadataProvider {
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_lookup()
            .times(1)
            .return_once(move |_| Box::pin(async move { Ok(Some(metadata)) }));
        provider.expect_search().times(0);
        provider
    }

    #[allure_test]
    #[tokio::test]
    async fn resolve_video_metadata_routes_a_film_lookup() -> Result<(), Box<dyn Error>> {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-003");
        allure.feature("REQ-METADATA-ENRICHMENT-008");
        allure.story("SCN-METADATA-ENRICHMENT-003-A");
        allure.story("SCN-METADATA-ENRICHMENT-008-A");
        // Arrange
        let harness = use_case_with(
            any_caller(),
            no_resources(),
            Some(resolving(movie_metadata())),
        );

        // Act
        let response = harness
            .use_case
            .execute(ResolveVideoMetadataCommand::new(CALLER_ID, movie_target()))
            .await?
            .ok_or("the provider must resolve the reference")?;

        // Assert
        assert_eq!(response.metadata(), &movie_metadata());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn resolve_video_metadata_reports_an_unknown_reference() -> Result<(), Box<dyn Error>> {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-017");
        allure.story("SCN-METADATA-ENRICHMENT-017-G");
        // Arrange: the provider does not know the reference.
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_lookup()
            .times(1)
            .return_once(|_| Box::pin(async { Ok(None) }));
        let harness = use_case_with(any_caller(), no_resources(), Some(provider));

        // Act
        let response = harness
            .use_case
            .execute(ResolveVideoMetadataCommand::new(CALLER_ID, movie_target()))
            .await?;

        // Assert
        assert!(
            response.is_none(),
            "an unknown reference must be no response"
        );
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn resolve_video_metadata_reports_unavailable_without_a_provider()
    -> Result<(), Box<dyn Error>> {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-017");
        allure.story("SCN-METADATA-ENRICHMENT-017-F");
        // Arrange: no provider is configured.
        let harness = use_case_with(any_caller(), no_resources(), None);

        // Act
        let result = harness
            .use_case
            .execute(ResolveVideoMetadataCommand::new(CALLER_ID, movie_target()))
            .await;

        // Assert
        assert!(
            matches!(result, Err(ResolveVideoMetadataError::Unavailable)),
            "no configured provider must report unavailable, got: {result:?}"
        );
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn resolve_video_metadata_reports_a_provider_failure() -> Result<(), Box<dyn Error>> {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-008");
        allure.story("SCN-METADATA-ENRICHMENT-008-B");
        // Arrange
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_lookup()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ExternalServiceError::RateLimited) }));
        let harness = use_case_with(any_caller(), no_resources(), Some(provider));

        // Act
        let result = harness
            .use_case
            .execute(ResolveVideoMetadataCommand::new(CALLER_ID, movie_target()))
            .await;

        // Assert
        assert!(
            matches!(result, Err(ResolveVideoMetadataError::Unknown(_))),
            "a provider failure must be reported, got: {result:?}"
        );
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn resolve_video_metadata_rejects_another_episode_position() -> Result<(), Box<dyn Error>>
    {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-017");
        allure.story("SCN-METADATA-ENRICHMENT-017-K");
        // Arrange: the provider reports episode 3 while the target asks for 2.
        let target = Target::Episode {
            episode: 2,
            reference: reference(),
            season: 1,
        };
        let harness = use_case_with(
            any_caller(),
            no_resources(),
            Some(resolving(episode_metadata(1, 3))),
        );

        // Act
        let result = harness
            .use_case
            .execute(ResolveVideoMetadataCommand::new(CALLER_ID, target))
            .await;

        // Assert
        assert!(
            matches!(result, Err(ResolveVideoMetadataError::PositionMismatch)),
            "a provider position that differs must be rejected, got: {result:?}"
        );
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn resolve_video_metadata_accepts_the_addressed_episode() -> Result<(), Box<dyn Error>> {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-003");
        allure.feature("REQ-METADATA-ENRICHMENT-017");
        allure.story("SCN-METADATA-ENRICHMENT-003-C");
        allure.story("SCN-METADATA-ENRICHMENT-017-H");
        // Arrange
        let target = Target::Episode {
            episode: 2,
            reference: reference(),
            season: 1,
        };
        let harness = use_case_with(
            any_caller(),
            no_resources(),
            Some(resolving(episode_metadata(1, 2))),
        );

        // Act
        let response = harness
            .use_case
            .execute(ResolveVideoMetadataCommand::new(CALLER_ID, target))
            .await?;

        // Assert
        assert_eq!(
            response.map(|resolved| resolved.metadata().clone()),
            Some(episode_metadata(1, 2))
        );
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn resolve_video_metadata_rejects_another_kind() -> Result<(), Box<dyn Error>> {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-008");
        allure.story("SCN-METADATA-ENRICHMENT-008-C");
        // Arrange: the film route resolves a show.
        let harness = use_case_with(
            any_caller(),
            no_resources(),
            Some(resolving(MediaMetadata::Series {
                artwork: None,
                country_of_origin: None,
                first_air_date: None,
                genres: Vec::new(),
                plot: String::new(),
                title: "Twin Peaks".to_owned(),
            })),
        );

        // Act
        let result = harness
            .use_case
            .execute(ResolveVideoMetadataCommand::new(CALLER_ID, movie_target()))
            .await;

        // Assert
        assert!(
            matches!(result, Err(ResolveVideoMetadataError::KindMismatch)),
            "a response of another kind must be rejected, got: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn resolve_video_metadata_reports_a_missing_caller() -> Result<(), Box<dyn Error>> {
        // Arrange: the caller load finds no user.
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = use_case_with(users, no_resources(), None);

        // Act
        let result = harness
            .use_case
            .execute(ResolveVideoMetadataCommand::new(CALLER_ID, movie_target()))
            .await;

        // Assert
        assert!(
            matches!(result, Err(ResolveVideoMetadataError::NoSuchCaller)),
            "an unknown caller must be reported, got: {result:?}"
        );
        Ok(())
    }
}
