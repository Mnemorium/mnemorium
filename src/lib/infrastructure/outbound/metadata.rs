//! Composition of the video metadata providers.
//!
//! The chain lives under `outbound/metadata/` because it composes port
//! implementations and has no transport of its own: it tries the configured
//! providers in ascending index order, records each fallback as a catalogued
//! event, and routes a lookup to the provider that issued the reference.

pub mod failover;

use crate::domain::model::configuration::MusicProviders;
use crate::domain::model::configuration::VideoProviders;
use crate::domain::model::media_candidate::Candidate;
use crate::domain::model::media_candidate::ProviderId;
use crate::domain::model::media_candidate::Target;
use crate::domain::model::media_metadata::MediaMetadata;
use crate::domain::model::media_query::MediaQuery;
use crate::domain::port::error::ExternalServiceError;
use crate::domain::port::video_metadata_provider::VideoMetadataProvider;
use crate::infrastructure::outbound::reqwest::musicbrainz::MusicBrainzMetadataProvider;
use crate::infrastructure::outbound::reqwest::omdb::OmdbVideoMetadataProvider;
use crate::infrastructure::outbound::reqwest::tmdb::TmdbVideoMetadataProvider;

/// One configured provider link, selected at startup by configuration.
#[non_exhaustive]
pub enum ConfiguredProvider {
    /// The `OMDb` provider.
    Omdb(OmdbVideoMetadataProvider),
    /// The TMDB provider.
    Tmdb(TmdbVideoMetadataProvider),
}

impl VideoMetadataProvider for ConfiguredProvider {
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "the arms borrow the adapter out of a non-Copy enum matched by reference"
    )]
    async fn lookup(&self, target: &Target) -> Result<Option<MediaMetadata>, ExternalServiceError> {
        match self {
            Self::Omdb(provider) => provider.lookup(target).await,
            Self::Tmdb(provider) => provider.lookup(target).await,
        }
    }

    #[expect(
        clippy::pattern_type_mismatch,
        reason = "the arms borrow the adapter out of a non-Copy enum matched by reference"
    )]
    async fn search(&self, query: &MediaQuery) -> Result<Vec<Candidate>, ExternalServiceError> {
        match self {
            Self::Omdb(provider) => provider.search(query).await,
            Self::Tmdb(provider) => provider.search(query).await,
        }
    }
}

/// Build the configured links, ordered by the index the configuration declares.
///
/// The API key is read here, where the adapter that presents it is built; the
/// configuration itself stays free of transport.
#[must_use]
pub fn configured_links(video: &VideoProviders) -> Vec<(ProviderId, ConfiguredProvider)> {
    video
        .providers()
        .iter()
        .filter_map(|entry| {
            let name = entry.name();
            let api_key = entry.api_key().expose().to_owned();
            let provider = match name {
                ProviderId::Omdb => {
                    ConfiguredProvider::Omdb(OmdbVideoMetadataProvider::new(api_key))
                }
                ProviderId::Tmdb => {
                    ConfiguredProvider::Tmdb(TmdbVideoMetadataProvider::new(api_key))
                }
                // The video settings admit no music provider, so this arm is
                // unreachable through configuration.
                ProviderId::MusicBrainz => return None,
            };
            Some((name, provider))
        })
        .collect()
}

/// Build the configured music provider.
///
/// Returns `None` when the configuration lists no music provider, which is how
/// the caller reports identification as unavailable rather than as a search that
/// ran and found nothing. A chain over one link is a pass-through, so the
/// provider is returned on its own rather than behind a failover chain.
///
/// `MusicBrainz` is the only provider the music section admits, so an entry
/// naming it is the one that builds an adapter; a section the constructor
/// validated can hold no other music provider.
#[must_use]
pub fn configured_music_provider(music: &MusicProviders) -> Option<MusicBrainzMetadataProvider> {
    music
        .providers()
        .iter()
        .any(|entry| entry.name() == ProviderId::MusicBrainz)
        .then(MusicBrainzMetadataProvider::new)
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::time::Duration;

    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    use crate::domain::model::configuration::MusicProvider;
    use crate::domain::model::configuration::MusicProviders;
    use crate::domain::model::configuration::VideoProvider;
    use crate::domain::model::configuration::VideoProviders;
    use crate::domain::model::media_candidate::Candidate;
    use crate::domain::model::media_candidate::ProviderId;
    use crate::domain::model::media_query::MediaQuery;
    use crate::domain::model::media_query::MediaQueryError;
    use crate::domain::model::media_query::VideoKind;
    use crate::domain::port::video_metadata_provider::VideoMetadataProvider as _;
    use crate::infrastructure::outbound::reqwest::tmdb::TmdbVideoMetadataProvider;

    use super::ConfiguredProvider;
    use super::configured_links;
    use super::configured_music_provider;

    /// The golden search payload the stub serves.
    const SEARCH_MOVIE: &str = include_str!("reqwest/fixtures/tmdb/search_movie.json");

    #[tokio::test]
    async fn configured_provider_searches_through_its_adapter() -> Result<(), Box<dyn Error>> {
        // Arrange: a TMDB adapter pointed at a stub, wrapped by the enum.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search/movie"))
            .respond_with(ResponseTemplate::new(200).set_body_string(SEARCH_MOVIE))
            .mount(&server)
            .await;
        let provider = ConfiguredProvider::Tmdb(TmdbVideoMetadataProvider::with_endpoints(
            "sentinel".to_owned(),
            server.uri(),
            server.uri(),
            Duration::from_secs(5),
        ));
        let query = query()?;

        // Act
        let candidates = provider.search(&query).await?;

        // Assert
        assert_eq!(candidates.first().map(Candidate::title), Some("Inception"));
        Ok(())
    }

    #[test]
    fn configured_links_follows_the_configured_index_order() -> Result<(), Box<dyn Error>> {
        // Arrange: the entries are declared in the reverse of their index order.
        let video = VideoProviders::try_new(vec![
            VideoProvider::try_new("omdb", 2, "sentinel-omdb".to_owned())?,
            VideoProvider::try_new("tmdb", 1, "sentinel-tmdb".to_owned())?,
        ])?;

        // Act
        let links = configured_links(&video);

        // Assert
        assert_eq!(links.len(), 2);
        assert_eq!(links.first().map(|link| link.0), Some(ProviderId::Tmdb));
        assert_eq!(links.last().map(|link| link.0), Some(ProviderId::Omdb));
        Ok(())
    }

    /// The query the search test uses.
    ///
    /// # Errors
    ///
    /// Returns the constructor's error when the title is empty.
    fn query() -> Result<MediaQuery, MediaQueryError> {
        MediaQuery::try_new(VideoKind::Movie, "Inception".to_owned(), None, None)
    }

    #[test]
    fn configured_music_provider_is_absent_when_the_list_is_empty() {
        // Arrange: the built-in default lists no music provider.
        let music = MusicProviders::default();

        // Act
        let provider = configured_music_provider(&music);

        // Assert
        assert!(
            provider.is_none(),
            "no configured music provider means identification is unavailable"
        );
    }

    #[test]
    fn configured_music_provider_is_built_from_the_configured_entry() -> Result<(), Box<dyn Error>>
    {
        // Arrange: one music provider, which needs no credential.
        let music = MusicProviders::try_new(vec![MusicProvider::try_new("musicbrainz", 1)?])?;

        // Act
        let provider = configured_music_provider(&music);

        // Assert
        assert!(
            provider.is_some(),
            "a configured music provider must be composed"
        );
        Ok(())
    }
}
