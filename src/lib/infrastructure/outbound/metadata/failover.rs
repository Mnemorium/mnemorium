//! Ordered failover chain over the configured metadata providers.

use crate::application::security_event;
use crate::domain::model::media_candidate::Candidate;
use crate::domain::model::media_candidate::ProviderId;
use crate::domain::model::media_candidate::Target;
use crate::domain::model::media_metadata::MediaMetadata;
use crate::domain::model::media_query::MediaQuery;
use crate::domain::port::error::ExternalServiceError;
use crate::domain::port::video_metadata_provider::VideoMetadataProvider;

/// Failover chain implementing the provider port over its configured links.
///
/// The links are already ordered by the index the configuration declares. A
/// search tries them in that order and returns the first non-empty result; a
/// lookup is routed to the provider named by the reference and is never retried
/// elsewhere. Each link is asked at most once per operation.
pub struct FailoverVideoMetadataProvider<L> {
    /// Configured links, ordered by their index.
    links: Vec<(ProviderId, L)>,
}

impl<L: VideoMetadataProvider> FailoverVideoMetadataProvider<L> {
    /// Create a chain over `links`, already ordered by index.
    #[must_use]
    pub fn new(links: Vec<(ProviderId, L)>) -> Self {
        Self { links }
    }
}

impl<L: VideoMetadataProvider> VideoMetadataProvider for FailoverVideoMetadataProvider<L> {
    async fn lookup(&self, target: &Target) -> Result<Option<MediaMetadata>, ExternalServiceError> {
        let provider = target.reference().provider();
        let Some(configured) = self.links.iter().find(|link| link.0 == provider) else {
            security_event::metadata_provider_unconfigured(provider.as_str());
            return Ok(None);
        };
        configured.1.lookup(target).await
    }

    async fn search(&self, query: &MediaQuery) -> Result<Vec<Candidate>, ExternalServiceError> {
        let mut first_failure = None;
        for link in &self.links {
            match link.1.search(query).await {
                Ok(candidates) if !candidates.is_empty() => return Ok(candidates),
                Ok(_) => {}
                Err(error) => {
                    if matches!(
                        error,
                        ExternalServiceError::InvalidRequest
                            | ExternalServiceError::SerializationFailure
                    ) {
                        // The request was the system's own defect: asking
                        // another provider would conceal it.
                        return Err(error);
                    }
                    security_event::metadata_provider_fallback(
                        link.0.as_str(),
                        error.classification(),
                    );
                    first_failure = first_failure.or(Some(error));
                }
            }
        }
        first_failure.map_or(Ok(Vec::new()), Err)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use std::time::Duration;

    use rstest::rstest;
    use tracing_test::traced_test;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    use crate::domain::model::media_candidate::Candidate;
    use crate::domain::model::media_candidate::ExternalRef;
    use crate::domain::model::media_candidate::ProviderId;
    use crate::domain::model::media_candidate::Target;
    use crate::domain::model::media_metadata::MediaMetadata;
    use crate::domain::model::media_query::MediaQuery;
    use crate::domain::model::media_query::MediaQueryError;
    use crate::domain::model::media_query::VideoKind;
    use crate::domain::port::error::ExternalServiceError;
    use crate::domain::port::video_metadata_provider::MockVideoMetadataProvider;
    use crate::domain::port::video_metadata_provider::VideoMetadataProvider as _;

    use crate::infrastructure::outbound::reqwest::tmdb::TmdbVideoMetadataProvider;

    use super::FailoverVideoMetadataProvider;

    /// The golden search payload the stub serves.
    const SEARCH_MOVIE: &str = include_str!("../reqwest/fixtures/tmdb/search_movie.json");

    /// A candidate for `Inception`, as a provider would report it.
    fn candidate() -> Candidate {
        Candidate::new(
            ExternalRef::new(ProviderId::Tmdb, "27205".to_owned()),
            VideoKind::Movie,
            "Inception".to_owned(),
            Some(2010),
        )
    }

    /// The film metadata a routed lookup yields.
    fn metadata() -> MediaMetadata {
        MediaMetadata::Movie {
            artwork: None,
            country_of_origin: Some("US".to_owned()),
            genres: Vec::new(),
            plot: String::new(),
            release_date: None,
            title: "Inception".to_owned(),
        }
    }

    /// The query every search test uses.
    ///
    /// # Errors
    ///
    /// Returns the constructor's error when the title is empty, which no test
    /// passes.
    fn query() -> Result<MediaQuery, MediaQueryError> {
        MediaQuery::try_new(VideoKind::Movie, "Inception".to_owned(), Some(2010), None)
    }

    /// A link that hits once and is never asked to resolve metadata.
    fn hitting_link() -> MockVideoMetadataProvider {
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(vec![candidate()]) }));
        provider.expect_lookup().times(0);
        provider
    }

    /// A link that answers once with no candidate.
    fn empty_link() -> MockVideoMetadataProvider {
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        provider.expect_lookup().times(0);
        provider
    }

    /// A link that fails once with the classification `classification`.
    fn failing_link(classification: &'static str) -> MockVideoMetadataProvider {
        let mut provider = MockVideoMetadataProvider::new();
        provider.expect_search().times(1).returning(move |_| {
            let error = match classification {
                "communication_failure" => ExternalServiceError::CommunicationFailure,
                "dependency_failure" => ExternalServiceError::DependencyFailure,
                "deserialization_failure" => ExternalServiceError::DeserializationFailure,
                "forbidden" => ExternalServiceError::Forbidden,
                "invalid_request" => ExternalServiceError::InvalidRequest,
                "rate_limited" => ExternalServiceError::RateLimited,
                "retryable_failure" => ExternalServiceError::RetryableFailure,
                "serialization_failure" => ExternalServiceError::SerializationFailure,
                "timeout" => ExternalServiceError::Timeout,
                "unauthorized" => ExternalServiceError::Unauthorized,
                "unavailable" => ExternalServiceError::Unavailable,
                _ => ExternalServiceError::Unknown(anyhow::anyhow!("the link failed")),
            };
            Box::pin(async move { Err(error) })
        });
        provider.expect_lookup().times(0);
        provider
    }

    #[tokio::test]
    async fn failover_returns_a_real_adapter_hit() -> Result<(), Box<dyn Error>> {
        // Arrange: the chain's link is the production adapter, stubbed by
        // `wiremock`, so the composition itself is exercised end to end.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search/movie"))
            .respond_with(ResponseTemplate::new(200).set_body_string(SEARCH_MOVIE))
            .mount(&server)
            .await;
        let adapter = TmdbVideoMetadataProvider::with_endpoints(
            "sentinel".to_owned(),
            server.uri(),
            server.uri(),
            Duration::from_secs(5),
        );
        let chain = FailoverVideoMetadataProvider::new(vec![(ProviderId::Tmdb, adapter)]);

        // Act
        let candidates = chain.search(&query()?).await?;

        // Assert
        assert_eq!(candidates.first().map(Candidate::title), Some("Inception"));
        Ok(())
    }

    #[tokio::test]
    async fn failover_returns_the_first_hit_without_asking_the_next() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut second = MockVideoMetadataProvider::new();
        second.expect_search().times(0);
        let chain = FailoverVideoMetadataProvider::new(vec![
            (ProviderId::Tmdb, hitting_link()),
            (ProviderId::Omdb, second),
        ]);

        // Act
        let candidates = chain.search(&query()?).await?;

        // Assert
        assert_eq!(candidates, vec![candidate()]);
        Ok(())
    }

    #[tokio::test]
    async fn failover_advances_past_an_empty_result() -> Result<(), Box<dyn Error>> {
        // Arrange
        let chain = FailoverVideoMetadataProvider::new(vec![
            (ProviderId::Tmdb, empty_link()),
            (ProviderId::Omdb, hitting_link()),
        ]);

        // Act
        let candidates = chain.search(&query()?).await?;

        // Assert
        assert_eq!(candidates, vec![candidate()]);
        Ok(())
    }

    #[tokio::test]
    async fn failover_reports_no_candidate_when_every_link_is_empty() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let chain = FailoverVideoMetadataProvider::new(vec![
            (ProviderId::Tmdb, empty_link()),
            (ProviderId::Omdb, empty_link()),
        ]);

        // Act
        let candidates = chain.search(&query()?).await?;

        // Assert
        assert!(candidates.is_empty(), "a miss is not a failure");
        Ok(())
    }

    #[tokio::test]
    async fn failover_reports_the_failure_when_no_result_arrives() -> Result<(), Box<dyn Error>> {
        // Arrange: the first link is empty, the second fails.
        let chain = FailoverVideoMetadataProvider::new(vec![
            (ProviderId::Tmdb, empty_link()),
            (ProviderId::Omdb, failing_link("timeout")),
        ]);

        // Act
        let failure = chain
            .search(&query()?)
            .await
            .err()
            .ok_or("a failure with no result must be reported")?;

        // Assert
        assert_eq!(failure.classification(), "timeout");
        Ok(())
    }

    #[tokio::test]
    async fn failover_returns_the_first_failure_when_every_link_fails() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the primary fails first, so its diagnosis is the answer.
        let chain = FailoverVideoMetadataProvider::new(vec![
            (ProviderId::Tmdb, failing_link("timeout")),
            (ProviderId::Omdb, failing_link("rate_limited")),
        ]);

        // Act
        let failure = chain
            .search(&query()?)
            .await
            .err()
            .ok_or("every link failed, so the search must fail")?;

        // Assert
        assert_eq!(failure.classification(), "timeout");
        Ok(())
    }

    #[rstest]
    #[case::communication("communication_failure")]
    #[case::dependency("dependency_failure")]
    #[case::deserialization("deserialization_failure")]
    #[case::forbidden("forbidden")]
    #[case::rate_limited("rate_limited")]
    #[case::retryable("retryable_failure")]
    #[case::timeout("timeout")]
    #[case::unauthorized("unauthorized")]
    #[case::unavailable("unavailable")]
    #[case::unknown("unknown")]
    #[tokio::test]
    async fn failover_advances_on_a_classified_failure(
        #[case] classification: &'static str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange: the first link fails, the second hits.
        let chain = FailoverVideoMetadataProvider::new(vec![
            (ProviderId::Tmdb, failing_link(classification)),
            (ProviderId::Omdb, hitting_link()),
        ]);

        // Act
        let candidates = chain.search(&query()?).await?;

        // Assert
        assert_eq!(candidates, vec![candidate()]);
        Ok(())
    }

    #[rstest]
    #[case::invalid_request("invalid_request")]
    #[case::serialization("serialization_failure")]
    #[tokio::test]
    async fn failover_does_not_advance_on_a_system_defect(
        #[case] classification: &'static str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange: a defect in the request must not be concealed behind another
        // provider, so the second link is never asked.
        let mut second = MockVideoMetadataProvider::new();
        second.expect_search().times(0);
        let chain = FailoverVideoMetadataProvider::new(vec![
            (ProviderId::Tmdb, failing_link(classification)),
            (ProviderId::Omdb, second),
        ]);

        // Act
        let failure = chain
            .search(&query()?)
            .await
            .err()
            .ok_or("the defect must be reported")?;

        // Assert
        assert_eq!(failure.classification(), classification);
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn failover_records_the_fallback_provider_and_classification()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the primary times out, so the chain falls over once.
        let chain = FailoverVideoMetadataProvider::new(vec![
            (ProviderId::Tmdb, failing_link("timeout")),
            (ProviderId::Omdb, hitting_link()),
        ]);

        // Act
        let _candidates = chain.search(&query()?).await?;

        // Assert
        logs_contain("event=\"metadata_provider_fallback\"");
        logs_contain("provider=\"tmdb\"");
        logs_contain("classification=\"timeout\"");
        Ok(())
    }

    #[tokio::test]
    async fn lookup_routes_to_the_reference_provider() -> Result<(), Box<dyn Error>> {
        // Arrange: only the OMDb link is allowed to resolve the reference.
        let mut tmdb = MockVideoMetadataProvider::new();
        tmdb.expect_lookup().times(0);
        tmdb.expect_search().times(0);
        let mut omdb = MockVideoMetadataProvider::new();
        omdb.expect_lookup()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(metadata())) }));
        let chain = FailoverVideoMetadataProvider::new(vec![
            (ProviderId::Tmdb, tmdb),
            (ProviderId::Omdb, omdb),
        ]);
        let target = Target::Movie(ExternalRef::new(ProviderId::Omdb, "tt1375666".to_owned()));

        // Act
        let resolved = chain.lookup(&target).await?;

        // Assert
        assert_eq!(resolved, Some(metadata()));
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn lookup_reports_an_unconfigured_provider_as_absent() -> Result<(), Box<dyn Error>> {
        // Arrange: the reference names a provider the chain does not hold.
        let mut tmdb = MockVideoMetadataProvider::new();
        tmdb.expect_lookup().times(0);
        let chain = FailoverVideoMetadataProvider::new(vec![(ProviderId::Tmdb, tmdb)]);
        let target = Target::Movie(ExternalRef::new(ProviderId::Omdb, "tt1375666".to_owned()));

        // Act
        let resolved = chain.lookup(&target).await?;

        // Assert
        assert_eq!(
            resolved, None,
            "an unconfigured provider yields no metadata"
        );
        logs_contain("event=\"metadata_provider_unconfigured\"");
        Ok(())
    }

    #[tokio::test]
    async fn lookup_returns_the_routed_failure() -> Result<(), Box<dyn Error>> {
        // Arrange: the routed link fails and no other link may be asked.
        let mut tmdb = MockVideoMetadataProvider::new();
        tmdb.expect_lookup().times(0);
        let mut omdb = MockVideoMetadataProvider::new();
        omdb.expect_lookup()
            .times(1)
            .returning(|_| Box::pin(async { Err(ExternalServiceError::RateLimited) }));
        let chain = FailoverVideoMetadataProvider::new(vec![
            (ProviderId::Tmdb, tmdb),
            (ProviderId::Omdb, omdb),
        ]);
        let target = Target::Movie(ExternalRef::new(ProviderId::Omdb, "tt1375666".to_owned()));

        // Act
        let failure = chain
            .lookup(&target)
            .await
            .err()
            .ok_or("a routed failure must be reported")?;

        // Assert
        assert_eq!(failure.classification(), "rate_limited");
        Ok(())
    }
}
