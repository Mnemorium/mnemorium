use crate::domain::model::{Action, ResourceKind, ResourceRef};

use crate::application::authorization::authorize;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::{
    IdentifyMusicCommand, IdentifyMusicError, IdentifyMusicResponse, IdentifyMusicUseCase,
};
use crate::domain::port::{
    IdentityUnitOfWork, MusicMetadataProvider, UnitOfWork as _, UnitOfWorkFactory, UserFilter,
    UserRepository as _, UserUnitOfWork,
};

/// Use case implementation for identifying music from a query.
///
/// The use case owns the availability rule: with no configured provider it
/// reports [`IdentifyMusicError::Unavailable`], which is distinct from a search
/// that ran and found no candidate. It searches once, returns the provider's
/// candidates in the provider's order and never ranks, persists or logs them.
/// It is guarded by `ViewMetadata` on the catalogue: the search addresses no catalogue resource,
/// so a denial conceals the whole surface rather than one album.
pub(crate) struct IdentifyMusic<F, P> {
    /// Configured provider, absent when music identification is unavailable.
    metadata_provider: Option<Arc<P>>,
    /// Factory opening the unit of work wrapping the caller load and the search.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, P: MusicMetadataProvider> IdentifyMusic<F, P> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>, metadata_provider: Option<Arc<P>>) -> Self {
        Self {
            metadata_provider,
            unit_of_work_factory,
        }
    }
}

impl<F, P> IdentifyMusicUseCase for IdentifyMusic<F, P>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
    P: MusicMetadataProvider,
{
    fn execute<'future>(
        &'future self,
        command: IdentifyMusicCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<IdentifyMusicResponse, IdentifyMusicError>> + Send + 'future,
        >,
    > {
        let metadata_provider = self.metadata_provider.as_ref().map(Arc::clone);
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| IdentifyMusicError::Unknown(error.into()))?;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.caller_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| IdentifyMusicError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(IdentifyMusicError::NoSuchCaller)?;

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
                .map_err(|_| IdentifyMusicError::NotPermitted)?;

                let Some(provider) = metadata_provider else {
                    return Err(IdentifyMusicError::Unavailable);
                };

                let candidates = provider
                    .search(command.query())
                    .await
                    .map_err(|error| IdentifyMusicError::Unknown(error.into()))?;
                Ok(IdentifyMusicResponse::new(candidates))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| IdentifyMusicError::Unknown(error.into()))?;
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
    use crate::domain::model::{
        ExternalRef, MusicCandidate, MusicKind, MusicQuery, MusicQueryError, ProviderId,
    };
    use allure_cargotest::allure_test;

    use crate::domain::port::{
        ExternalServiceError, MockMusicMetadataProvider, MockResourceRepository, MockUserRepository,
    };
    use crate::test_helpers::*;
    use std::error::Error;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// The caller every test searches for.
    const CALLER_ID: i64 = 1;

    type UseCase = IdentifyMusic<TestFactory, MockMusicMetadataProvider>;

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
        provider: Option<MockMusicMetadataProvider>,
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
            use_case: IdentifyMusic::new(factory, provider.map(Arc::new)),
        }
    }

    /// An album candidate for `Kind of Blue`, as a provider would report it.
    fn candidate(id: &str) -> MusicCandidate {
        MusicCandidate::Album {
            group: Some("Miles Davis".to_owned()),
            reference: ExternalRef::new(ProviderId::MusicBrainz, id.to_owned()),
            title: "Kind of Blue".to_owned(),
            year: Some(1959),
        }
    }

    /// The query every test searches for.
    ///
    /// # Errors
    ///
    /// Returns the constructor's error when the title is empty, which no test
    /// passes.
    fn query() -> Result<MusicQuery, Box<dyn Error>> {
        Ok(MusicQuery::try_new(
            MusicKind::Album,
            "Kind of Blue".to_owned(),
            Some("Miles Davis".to_owned()),
            None,
        )?)
    }

    #[allure_test]
    #[tokio::test]
    async fn identify_music_rejects_a_query_without_a_title() {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-018");
        allure.story("SCN-METADATA-ENRICHMENT-018-D");
        // Act: the model refuses the query before the use case ever sees it.
        let result = MusicQuery::try_new(MusicKind::Album, String::new(), None, None);

        // Assert
        assert!(
            matches!(result, Err(MusicQueryError::InvalidTitle)),
            "a query with no title must be rejected"
        );
    }

    #[allure_test]
    #[tokio::test]
    async fn identify_music_reports_unavailable_without_a_provider() -> Result<(), Box<dyn Error>> {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-001");
        allure.feature("REQ-METADATA-ENRICHMENT-023");
        allure.story("SCN-METADATA-ENRICHMENT-001-A");
        allure.story("SCN-METADATA-ENRICHMENT-023-D");
        // Arrange: no music provider is configured.
        let harness = use_case_with(any_caller(), no_resources(), None);

        // Act
        let result = harness
            .use_case
            .execute(IdentifyMusicCommand::new(CALLER_ID, query()?))
            .await;

        // Assert
        assert!(
            matches!(result, Err(IdentifyMusicError::Unavailable)),
            "no configured provider must report unavailable, got: {result:?}"
        );
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn identify_music_reports_a_missing_caller() -> Result<(), Box<dyn Error>> {
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
            .execute(IdentifyMusicCommand::new(CALLER_ID, query()?))
            .await;

        // Assert
        assert!(
            matches!(result, Err(IdentifyMusicError::NoSuchCaller)),
            "an unknown caller must be reported, got: {result:?}"
        );
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
    #[allure_test]
    #[tokio::test]
    async fn identify_music_reports_no_candidate_when_the_search_is_empty()
    -> Result<(), Box<dyn Error>> {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-001");
        allure.story("SCN-METADATA-ENRICHMENT-001-B");
        // Arrange: a configured provider answers with nothing.
        let mut provider = MockMusicMetadataProvider::new();
        provider
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        provider.expect_lookup().times(0);
        let harness = use_case_with(any_caller(), no_resources(), Some(provider));

        // Act
        let response = harness
            .use_case
            .execute(IdentifyMusicCommand::new(CALLER_ID, query()?))
            .await?;

        // Assert
        assert!(
            response.candidates().is_empty(),
            "a search that found nothing is not unavailable"
        );
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[allure_test]
    #[tokio::test]
    async fn identify_music_returns_the_providers_candidates_in_its_order()
    -> Result<(), Box<dyn Error>> {
        allure.epic("Metadata enrichment");
        allure.feature("REQ-METADATA-ENRICHMENT-002");
        allure.feature("REQ-METADATA-ENRICHMENT-023");
        allure.story("SCN-METADATA-ENRICHMENT-002-B");
        allure.story("SCN-METADATA-ENRICHMENT-023-A");
        // Arrange: the provider reports two candidates in its own order.
        let mut provider = MockMusicMetadataProvider::new();
        provider
            .expect_search()
            .times(1)
            .withf(|query| query.title() == "Kind of Blue" && query.group() == Some("Miles Davis"))
            .returning(|_| Box::pin(async { Ok(vec![candidate("first"), candidate("second")]) }));
        provider.expect_lookup().times(0);
        let harness = use_case_with(any_caller(), no_resources(), Some(provider));

        // Act
        let response = harness
            .use_case
            .execute(IdentifyMusicCommand::new(CALLER_ID, query()?))
            .await?;

        // Assert
        assert_eq!(
            response.candidates(),
            &[candidate("first"), candidate("second")]
        );
        assert_eq!(
            response.candidates().first().map(MusicCandidate::kind),
            Some(MusicKind::Album)
        );
        assert_eq!(
            response.candidates().first().and_then(MusicCandidate::year),
            Some(1959),
            "the query's year evidence must reach the provider"
        );
        Ok(())
    }

    #[tokio::test]
    async fn identify_music_reports_a_provider_failure() -> Result<(), Box<dyn Error>> {
        // Arrange: the configured provider fails.
        let mut provider = MockMusicMetadataProvider::new();
        provider
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(ExternalServiceError::RateLimited) }));
        provider.expect_lookup().times(0);
        let harness = use_case_with(any_caller(), no_resources(), Some(provider));

        // Act
        let result = harness
            .use_case
            .execute(IdentifyMusicCommand::new(CALLER_ID, query()?))
            .await;

        // Assert
        assert!(
            matches!(result, Err(IdentifyMusicError::Unknown(_))),
            "a provider failure must be reported, got: {result:?}"
        );
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
