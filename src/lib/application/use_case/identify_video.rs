use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::identify_video::IdentifyVideoCommand;
use crate::application::port::identify_video::IdentifyVideoError;
use crate::application::port::identify_video::IdentifyVideoResponse;
use crate::application::port::identify_video::IdentifyVideoUseCase;
use crate::domain::port::video_metadata_provider::VideoMetadataProvider;
use crate::domain::service::media_name_interpreter::MediaNameInterpreter;

/// Use case implementation for identifying a video from its name.
///
/// The use case owns the search policy: it interprets the name once, searches
/// the ordered queries, returns the candidates of the first query that yields
/// any and stops on a failure. It never resolves a candidate's metadata, ranks
/// the candidates, persists anything or logs the name.
pub struct IdentifyVideo<P> {
    /// Configured provider chain, absent when identification is unavailable.
    metadata_provider: Option<Arc<P>>,
}

impl<P: VideoMetadataProvider> IdentifyVideo<P> {
    /// Create a new use case.
    #[must_use]
    pub fn new(metadata_provider: Option<Arc<P>>) -> Self {
        Self { metadata_provider }
    }
}

impl<P: VideoMetadataProvider> IdentifyVideoUseCase for IdentifyVideo<P> {
    fn execute<'future>(
        &'future self,
        command: IdentifyVideoCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<IdentifyVideoResponse, IdentifyVideoError>> + Send + 'future,
        >,
    > {
        let metadata_provider = self.metadata_provider.as_ref().map(Arc::clone);

        Box::pin(async move {
            let Some(provider) = metadata_provider else {
                return Err(IdentifyVideoError::Unavailable);
            };

            let queries = MediaNameInterpreter::interpret(command.name());
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
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use tracing_test::traced_test;

    use crate::application::port::identify_video::IdentifyVideoCommand;
    use crate::application::port::identify_video::IdentifyVideoError;
    use crate::application::port::identify_video::IdentifyVideoUseCase as _;
    use crate::domain::model::media_candidate::Candidate;
    use crate::domain::model::media_candidate::ExternalRef;
    use crate::domain::model::media_candidate::ProviderId;
    use crate::domain::model::media_query::VideoKind;
    use crate::domain::port::error::ExternalServiceError;
    use crate::domain::port::video_metadata_provider::MockVideoMetadataProvider;

    use super::IdentifyVideo;

    /// A candidate for `Inception`, as a provider would report it.
    fn candidate() -> Candidate {
        Candidate::new(
            ExternalRef::new(ProviderId::Tmdb, "27205".to_owned()),
            VideoKind::Movie,
            "Inception".to_owned(),
            Some(2010),
        )
    }

    #[tokio::test]
    async fn identify_video_returns_the_first_query_that_hits() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_search()
            .times(1)
            .withf(|query| query.title() == "Inception" && query.year() == Some(2010))
            .returning(|_| Box::pin(async { Ok(vec![candidate()]) }));
        provider.expect_lookup().times(0);
        let use_case = IdentifyVideo::new(Some(Arc::new(provider)));

        // Act
        let response = use_case
            .execute(IdentifyVideoCommand::new(
                "Movies/Inception (2010)/Inception.2010.1080p.BluRay.mkv".to_owned(),
            ))
            .await?;

        // Assert
        assert_eq!(response.candidates(), &[candidate()]);
        Ok(())
    }

    #[tokio::test]
    async fn identify_video_advances_past_a_query_that_yields_nothing() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the first query (2010) is empty, the second (2011) hits.
        let mut provider = MockVideoMetadataProvider::new();
        provider.expect_search().times(2).returning(|query| {
            let found = (query.year() == Some(2011))
                .then_some(vec![candidate()])
                .unwrap_or_default();
            Box::pin(async move { Ok(found) })
        });
        provider.expect_lookup().times(0);
        let use_case = IdentifyVideo::new(Some(Arc::new(provider)));

        // Act
        let response = use_case
            .execute(IdentifyVideoCommand::new(
                "Inception (2010)/Inception.2011.1080p.mkv".to_owned(),
            ))
            .await?;

        // Assert
        assert_eq!(response.candidates(), &[candidate()]);
        Ok(())
    }

    #[tokio::test]
    async fn identify_video_stops_on_a_provider_failure() -> Result<(), Box<dyn Error>> {
        // Arrange: the first query fails, so the remaining queries are not searched.
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(ExternalServiceError::RateLimited) }));
        provider.expect_lookup().times(0);
        let use_case = IdentifyVideo::new(Some(Arc::new(provider)));

        // Act
        let result = use_case
            .execute(IdentifyVideoCommand::new("1917.2019.1080p.mkv".to_owned()))
            .await;

        // Assert
        assert!(
            matches!(result, Err(IdentifyVideoError::Unknown(_))),
            "a provider failure must stop the run, got: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn identify_video_reports_no_candidate_for_a_usable_name() -> Result<(), Box<dyn Error>> {
        // Arrange: every query is empty, so the response carries no candidate.
        let mut provider = MockVideoMetadataProvider::new();
        provider
            .expect_search()
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        provider.expect_lookup().times(0);
        let use_case = IdentifyVideo::new(Some(Arc::new(provider)));

        // Act
        let response = use_case
            .execute(IdentifyVideoCommand::new(
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

    #[tokio::test]
    async fn identify_video_does_not_search_an_unusable_name() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut provider = MockVideoMetadataProvider::new();
        provider.expect_search().times(0);
        provider.expect_lookup().times(0);
        let use_case = IdentifyVideo::new(Some(Arc::new(provider)));

        // Act
        let response = use_case
            .execute(IdentifyVideoCommand::new("sample.mkv".to_owned()))
            .await?;

        // Assert
        assert!(response.candidates().is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn identify_video_reports_unavailable_without_a_provider() -> Result<(), Box<dyn Error>> {
        // Arrange: no provider is configured.
        let use_case = IdentifyVideo::<MockVideoMetadataProvider>::new(None);

        // Act
        let result = use_case
            .execute(IdentifyVideoCommand::new(
                "Inception.2010.1080p.mkv".to_owned(),
            ))
            .await;

        // Assert
        assert!(
            matches!(result, Err(IdentifyVideoError::Unavailable)),
            "no provider must read as unavailable, got: {result:?}"
        );
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
        let use_case = IdentifyVideo::new(Some(Arc::new(provider)));

        // Act
        let _response = use_case
            .execute(IdentifyVideoCommand::new(name.to_owned()))
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
