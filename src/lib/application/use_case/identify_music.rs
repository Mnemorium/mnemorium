use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::identify_music::IdentifyMusicCommand;
use crate::application::port::identify_music::IdentifyMusicError;
use crate::application::port::identify_music::IdentifyMusicResponse;
use crate::application::port::identify_music::IdentifyMusicUseCase;
use crate::domain::port::music_metadata_provider::MusicMetadataProvider;

/// Use case implementation for identifying music from a query.///
/// The use case owns the availability rule: with no configured provider it
/// reports [`IdentifyMusicError::Unavailable`], which is distinct from a search
/// that ran and found no candidate. It searches once, returns the provider's
/// candidates in the provider's order and never ranks, persists or logs them.
pub struct IdentifyMusic<P> {
    /// Configured provider, absent when music identification is unavailable.
    metadata_provider: Option<Arc<P>>,
}

impl<P: MusicMetadataProvider> IdentifyMusic<P> {
    /// Create a new use case.
    #[must_use]
    pub fn new(metadata_provider: Option<Arc<P>>) -> Self {
        Self { metadata_provider }
    }
}

impl<P: MusicMetadataProvider> IdentifyMusicUseCase for IdentifyMusic<P> {
    fn execute<'future>(
        &'future self,
        command: IdentifyMusicCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<IdentifyMusicResponse, IdentifyMusicError>> + Send + 'future,
        >,
    > {
        let metadata_provider = self.metadata_provider.as_ref().map(Arc::clone);

        Box::pin(async move {
            let Some(provider) = metadata_provider else {
                return Err(IdentifyMusicError::Unavailable);
            };

            let candidates = provider
                .search(command.query())
                .await
                .map_err(|error| IdentifyMusicError::Unknown(error.into()))?;
            Ok(IdentifyMusicResponse::new(candidates))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use crate::application::port::identify_music::IdentifyMusicCommand;
    use crate::application::port::identify_music::IdentifyMusicError;
    use crate::application::port::identify_music::IdentifyMusicUseCase as _;
    use crate::domain::model::media_candidate::ExternalRef;
    use crate::domain::model::media_candidate::ProviderId;
    use crate::domain::model::music_candidate::MusicCandidate;
    use crate::domain::model::music_query::MusicKind;
    use crate::domain::model::music_query::MusicQuery;
    use crate::domain::model::music_query::MusicQueryError;
    use crate::domain::port::error::ExternalServiceError;
    use crate::domain::port::music_metadata_provider::MockMusicMetadataProvider;

    use super::IdentifyMusic;

    /// An album candidate for `Kind of Blue`, as a provider would report it.
    fn candidate(id: &str) -> MusicCandidate {
        MusicCandidate::Album {
            artist: Some("Miles Davis".to_owned()),
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

    #[tokio::test]
    async fn identify_music_rejects_a_query_without_a_title() {
        // Act: the model refuses the query before the use case ever sees it.
        let result = MusicQuery::try_new(MusicKind::Album, String::new(), None, None);

        // Assert
        assert!(
            matches!(result, Err(MusicQueryError::InvalidTitle)),
            "a query with no title must be rejected"
        );
    }

    #[tokio::test]
    async fn identify_music_reports_unavailable_without_a_provider() -> Result<(), Box<dyn Error>> {
        // Arrange: no music provider is configured.
        let use_case = IdentifyMusic::<MockMusicMetadataProvider>::new(None);

        // Act
        let result = use_case.execute(IdentifyMusicCommand::new(query()?)).await;

        // Assert
        assert!(
            matches!(result, Err(IdentifyMusicError::Unavailable)),
            "no configured provider must report unavailable, got: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn identify_music_reports_no_candidate_when_the_search_is_empty()
    -> Result<(), Box<dyn Error>> {
        // Arrange: a configured provider answers with nothing.
        let mut provider = MockMusicMetadataProvider::new();
        provider
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        provider.expect_lookup().times(0);
        let use_case = IdentifyMusic::new(Some(Arc::new(provider)));

        // Act
        let response = use_case
            .execute(IdentifyMusicCommand::new(query()?))
            .await?;

        // Assert
        assert!(
            response.candidates().is_empty(),
            "a search that found nothing is not unavailable"
        );
        Ok(())
    }

    #[tokio::test]
    async fn identify_music_returns_the_providers_candidates_in_its_order()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the provider reports two candidates in its own order.
        let mut provider = MockMusicMetadataProvider::new();
        provider
            .expect_search()
            .times(1)
            .withf(|query| query.title() == "Kind of Blue" && query.artist() == Some("Miles Davis"))
            .returning(|_| Box::pin(async { Ok(vec![candidate("first"), candidate("second")]) }));
        provider.expect_lookup().times(0);
        let use_case = IdentifyMusic::new(Some(Arc::new(provider)));

        // Act
        let response = use_case
            .execute(IdentifyMusicCommand::new(query()?))
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
        let use_case = IdentifyMusic::new(Some(Arc::new(provider)));

        // Act
        let result = use_case.execute(IdentifyMusicCommand::new(query()?)).await;

        // Assert
        assert!(
            matches!(result, Err(IdentifyMusicError::Unknown(_))),
            "a provider failure must be reported, got: {result:?}"
        );
        Ok(())
    }
}
