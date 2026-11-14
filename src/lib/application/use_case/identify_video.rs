use crate::domain::model::{Action, ResourceKind, ResourceRef};

use crate::application::authorization::authorize;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{
    IdentifyVideoCommand, IdentifyVideoError, IdentifyVideoResponse, IdentifyVideoUseCase,
};
use crate::domain::port::{
    IdentityUnitOfWork, UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _,
    UserUnitOfWork, VideoMetadataProvider,
};
use crate::domain::service::MediaNameInterpreter;

/// Use case implementation for identifying a video from its name.
///
/// The use case owns the search policy: it interprets the name once, keeps only
/// the queries of the kind the route named, searches them in order, returns the
/// candidates of the first query that yields any and stops on a failure. It
/// never resolves a candidate's metadata, ranks the candidates, persists
/// anything or logs the name. It is guarded by `ViewMetadata` on the catalogue: the search
/// addresses no catalogue resource, so a denial conceals the whole surface.
pub(crate) struct IdentifyVideo<F, P> {
    /// Configured provider chain, absent when identification is unavailable.
    metadata_provider: Option<Arc<P>>,
    /// Factory opening the unit of work wrapping the caller load and the search.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, P: VideoMetadataProvider> IdentifyVideo<F, P> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>, metadata_provider: Option<Arc<P>>) -> Self {
        Self {
            metadata_provider,
            unit_of_work_factory,
        }
    }
}

impl<F, P> IdentifyVideoUseCase for IdentifyVideo<F, P>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
    P: VideoMetadataProvider,
{
    fn execute<'future>(
        &'future self,
        command: IdentifyVideoCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<IdentifyVideoResponse, IdentifyVideoError>> + Send + 'future,
        >,
    > {
        let metadata_provider = self.metadata_provider.as_ref().map(Arc::clone);
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| IdentifyVideoError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| IdentifyVideoError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(IdentifyVideoError::NoSuchCaller)?;

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
                .map_err(|_| IdentifyVideoError::NotPermitted)?;

                let Some(provider) = metadata_provider else {
                    return Err(IdentifyVideoError::Unavailable);
                };

                let queries: Vec<_> = MediaNameInterpreter::interpret(command.name())
                    .into_iter()
                    .filter(|query| query.kind() == command.kind())
                    .collect();
                for query in queries {
                    let candidates = provider
                        .search(&query)
                        .await
                        .map_err(|error| IdentifyVideoError::Unknown(error.into()))?;
                    if !candidates.is_empty() {
                        return Ok(IdentifyVideoResponse::new(candidates));
                    }
                }
                Ok(IdentifyVideoResponse::new(Vec::new()))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| IdentifyVideoError::Unknown(error.into()))?;
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
    use crate::domain::model::{Candidate, ExternalRef, ProviderId, VideoKind};
    use allure_cargotest::allure_test;

    use crate::domain::port::{
        ExternalServiceError, MockResourceRepository, MockUserRepository, MockVideoMetadataProvider,
    };
    use crate::test_helpers::*;
    use std::error::Error;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tracing_test::traced_test;

    /// The caller every test searches for.
    const CALLER_ID: i64 = 1;

    type UseCase = IdentifyVideo<TestFactory, MockVideoMetadataProvider>;

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
            use_case: IdentifyVideo::new(factory, provider.map(Arc::new)),
        }
    }

    /// A candidate for `Inception`, as a provider would report it.
    fn candidate() -> Candidate {
        Candidate::new(
            ExternalRef::new(ProviderId::Tmdb, "27205".to_owned()),
            VideoKind::Movie,
            "Inception".to_owned(),
            Some(2010),
        )
    }

    #[allure_test]
    #[tokio::test]
    async fn identify_video_returns_the_first_query_that_hits() -> Result<(), Box<dyn Error>> {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-012");
        allure.story("SCN-METADATA-ENRICHMENT-012-A");
        // Arrange
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_search()
            .times(1)
            .withf(|query| query.title() == "Inception" && query.year() == Some(2010))
            .returning(|_| Box::pin(async { Ok(vec![candidate()]) }));
        provider.expect_lookup().times(0);
        let harness = use_case_with(any_caller(), no_resources(), Some(provider));

        // Act
        let response = harness
            .use_case
            .execute(IdentifyVideoCommand::new(
                CALLER_ID,
                VideoKind::Movie,
                "Movies/Inception (2010)/Inception.2010.1080p.BluRay.mkv".to_owned(),
            ))
            .await?;

        // Assert
        assert_eq!(response.candidates(), &[candidate()]);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn identify_video_advances_past_a_query_that_yields_nothing() -> Result<(), Box<dyn Error>>
    {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-012");
        allure.story("SCN-METADATA-ENRICHMENT-012-B");
        // Arrange: the first query (2010) is empty, the second (2011) hits.
        let mut provider = MockVideoMetadataProvider::new();
        provider.expect_search().times(2).returning(|query| {
            let found = (query.year() == Some(2011))
                .then_some(vec![candidate()])
                .unwrap_or_default();
            Box::pin(async move { Ok(found) })
        });
        provider.expect_lookup().times(0);
        let harness = use_case_with(any_caller(), no_resources(), Some(provider));

        // Act
        let response = harness
            .use_case
            .execute(IdentifyVideoCommand::new(
                CALLER_ID,
                VideoKind::Movie,
                "Inception (2010)/Inception.2011.1080p.mkv".to_owned(),
            ))
            .await?;

        // Assert
        assert_eq!(response.candidates(), &[candidate()]);
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn identify_video_skips_show_hypotheses_on_the_movie_route() -> Result<(), Box<dyn Error>>
    {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-017");
        allure.story("SCN-METADATA-ENRICHMENT-017-C");
        // Arrange: an episode-shaped name yields show queries only, so the movie
        // route must not search at all.
        let mut provider = MockVideoMetadataProvider::new();
        provider.expect_search().times(0);
        provider.expect_lookup().times(0);
        let harness = use_case_with(any_caller(), no_resources(), Some(provider));

        // Act
        let response = harness
            .use_case
            .execute(IdentifyVideoCommand::new(
                CALLER_ID,
                VideoKind::Movie,
                "Twin Peaks/S01E01 - Pilot.mkv".to_owned(),
            ))
            .await?;

        // Assert
        assert!(
            response.candidates().is_empty(),
            "the movie route must not return a show hypothesis"
        );
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn identify_video_skips_film_hypotheses_on_the_series_route() -> Result<(), Box<dyn Error>>
    {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-017");
        allure.story("SCN-METADATA-ENRICHMENT-017-C");
        // Arrange: a film-shaped name yields film queries only, so the series
        // route must not search at all.
        let mut provider = MockVideoMetadataProvider::new();
        provider.expect_search().times(0);
        provider.expect_lookup().times(0);
        let harness = use_case_with(any_caller(), no_resources(), Some(provider));

        // Act
        let response = harness
            .use_case
            .execute(IdentifyVideoCommand::new(
                CALLER_ID,
                VideoKind::Series,
                "Inception.2010.1080p.BluRay.mkv".to_owned(),
            ))
            .await?;

        // Assert
        assert!(
            response.candidates().is_empty(),
            "the series route must not return a film hypothesis"
        );
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn identify_video_stops_on_a_provider_failure() -> Result<(), Box<dyn Error>> {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-012");
        allure.story("SCN-METADATA-ENRICHMENT-012-C");
        // Arrange: the first query fails, so the remaining queries are not searched.
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(ExternalServiceError::RateLimited) }));
        provider.expect_lookup().times(0);
        let harness = use_case_with(any_caller(), no_resources(), Some(provider));

        // Act
        let result = harness
            .use_case
            .execute(IdentifyVideoCommand::new(
                CALLER_ID,
                VideoKind::Movie,
                "1917.2019.1080p.mkv".to_owned(),
            ))
            .await;

        // Assert
        assert!(
            matches!(result, Err(IdentifyVideoError::Unknown(_))),
            "a provider failure must stop the run, got: {result:?}"
        );
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn identify_video_reports_no_candidate_for_a_usable_name() -> Result<(), Box<dyn Error>> {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-001");
        allure.feature("REQ-METADATA-ENRICHMENT-017");
        allure.story("SCN-METADATA-ENRICHMENT-001-B");
        allure.story("SCN-METADATA-ENRICHMENT-017-E");
        // Arrange: every query is empty, so the response carries no candidate.
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_search()
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        provider.expect_lookup().times(0);
        let harness = use_case_with(any_caller(), no_resources(), Some(provider));

        // Act
        let response = harness
            .use_case
            .execute(IdentifyVideoCommand::new(
                CALLER_ID,
                VideoKind::Movie,
                "The.Matrix.1999.1080p.mkv".to_owned(),
            ))
            .await?;

        // Assert
        assert!(
            response.candidates().is_empty(),
            "a search that found nothing is not unavailable"
        );
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn identify_video_does_not_search_an_unusable_name() -> Result<(), Box<dyn Error>> {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-009");
        allure.story("SCN-METADATA-ENRICHMENT-009-D");
        // Arrange
        let mut provider = MockVideoMetadataProvider::new();
        provider.expect_search().times(0);
        provider.expect_lookup().times(0);
        let harness = use_case_with(any_caller(), no_resources(), Some(provider));

        // Act
        let response = harness
            .use_case
            .execute(IdentifyVideoCommand::new(
                CALLER_ID,
                VideoKind::Movie,
                "sample.mkv".to_owned(),
            ))
            .await?;

        // Assert
        assert!(response.candidates().is_empty());
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn identify_video_reports_unavailable_without_a_provider() -> Result<(), Box<dyn Error>> {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-001");
        allure.feature("REQ-METADATA-ENRICHMENT-017");
        allure.story("SCN-METADATA-ENRICHMENT-001-A");
        allure.story("SCN-METADATA-ENRICHMENT-017-F");
        // Arrange: no provider is configured.
        let harness = use_case_with(any_caller(), no_resources(), None);

        // Act
        let result = harness
            .use_case
            .execute(IdentifyVideoCommand::new(
                CALLER_ID,
                VideoKind::Movie,
                "Inception.2010.1080p.mkv".to_owned(),
            ))
            .await;

        // Assert
        assert!(
            matches!(result, Err(IdentifyVideoError::Unavailable)),
            "no provider must read as unavailable, got: {result:?}"
        );
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn identify_video_reports_a_missing_caller() -> Result<(), Box<dyn Error>> {
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
            .execute(IdentifyVideoCommand::new(
                CALLER_ID,
                VideoKind::Movie,
                "Inception.2010.1080p.mkv".to_owned(),
            ))
            .await;

        // Assert
        assert!(
            matches!(result, Err(IdentifyVideoError::NoSuchCaller)),
            "an unknown caller must be reported, got: {result:?}"
        );
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
    #[tokio::test]
    #[traced_test]
    async fn identify_video_never_logs_the_media_name() -> Result<(), Box<dyn Error>> {
        // Arrange
        let name = "Inception.2010.1080p.mkv";
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_search()
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = use_case_with(any_caller(), no_resources(), Some(provider));

        // Act
        let _response = harness
            .use_case
            .execute(IdentifyVideoCommand::new(
                CALLER_ID,
                VideoKind::Movie,
                name.to_owned(),
            ))
            .await?;

        // Assert
        logs_assert(|lines: &[&str]| {
            let leaked = lines.iter().any(|line| line.contains("Inception"));
            (!leaked)
                .then_some(())
                .ok_or_else(|| "the media name must never reach a log sink".to_owned())
        });
        Ok(())
    }
}
