//! `OMDb` adapter: maps `OMDb`'s search and detail payloads onto the canonical port.
//!
//! `OMDb` reports "not found" as HTTP 200 with `Response:"False"`, and an invalid
//! key the same way with `Error:"Invalid API key!"`; the latter is an
//! authentication failure, the former absence. The key travels in the query
//! string, so a failure is classified and logged by kind, never by the request
//! address.

use std::future::Future;
use std::time::Duration;

use chrono::NaiveDate;
use reqwest::Client;
use reqwest::RequestBuilder;
use reqwest::StatusCode;
use serde::Deserialize;

use crate::application::security_event;
use crate::domain::model::genre::Genre;
use crate::domain::model::media_candidate::Candidate;
use crate::domain::model::media_candidate::ExternalRef;
use crate::domain::model::media_candidate::ProviderId;
use crate::domain::model::media_candidate::Target;
use crate::domain::model::media_metadata::MediaMetadata;
use crate::domain::model::media_query::MediaQuery;
use crate::domain::model::media_query::VideoKind;
use crate::domain::port::error::ExternalServiceError;
use crate::domain::port::video_metadata_provider::VideoMetadataProvider;
use crate::infrastructure::outbound::reqwest::ArtworkClient;
use crate::infrastructure::outbound::reqwest::REQUEST_TIMEOUT;
use crate::infrastructure::outbound::reqwest::classify_status;
use crate::infrastructure::outbound::reqwest::classify_transport_error;
use crate::infrastructure::outbound::reqwest::log_provider_fault;
use crate::infrastructure::outbound::reqwest::read_payload;
use crate::infrastructure::outbound::reqwest::request_url;

/// Base address of the `OMDb` API.
const DEFAULT_BASE_URL: &str = "https://www.omdbapi.com/";

/// Hosts `OMDb` serves poster artwork from.
const OMDB_ARTWORK_HOSTS: &[&str] = &["ia.media-imdb.com", "m.media-amazon.com"];

/// Security-event operation name owned by the `OMDb` adapter.
const OPERATION: &str = "video_metadata";

/// One `OMDb` detail payload, for a film, a show or an episode.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DetailPayload {
    /// Country names the provider reports.
    #[serde(default)]
    country: Option<String>,
    /// Refusal message the provider reports.
    #[serde(default)]
    error: Option<String>,
    /// Genre names the provider reports.
    #[serde(default)]
    genre: Option<String>,
    /// Plot summary.
    #[serde(default)]
    plot: Option<String>,
    /// Address of the poster.
    #[serde(default)]
    poster: Option<String>,
    /// Date the work was released or first aired, in `OMDb`'s format.
    #[serde(default)]
    released: Option<String>,
    /// Whether the provider answered.
    #[serde(default)]
    response: Option<String>,
    /// Title the provider reports.
    #[serde(default)]
    title: Option<String>,
}

/// One `OMDb` search hit.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SearchHit {
    /// Identifier `OMDb` issued for the work.
    #[serde(default, rename = "imdbID")]
    imdb_id: Option<String>,
    /// Title the provider reports.
    #[serde(default)]
    title: Option<String>,
    /// Year the provider reports.
    #[serde(default)]
    year: Option<String>,
}

/// The result of one `OMDb` search.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SearchPayload {
    /// Refusal message the provider reports.
    #[serde(default)]
    error: Option<String>,
    /// Whether the provider answered.
    #[serde(default)]
    response: Option<String>,
    /// Hits the search returned, in `OMDb`'s order.
    #[serde(default)]
    search: Option<Vec<SearchHit>>,
}

/// Video metadata provider backed by the `OMDb` API.
pub struct OmdbVideoMetadataProvider {
    /// API key the request carries.
    api_key: String,
    /// Client downloading the artwork under the adapter's policy.
    artwork: ArtworkClient,
    /// Base address of the API.
    base_url: String,
    /// Client carrying the request timeout.
    client: Client,
}

impl OmdbVideoMetadataProvider {
    /// Emit the owning layer's event for `error` and return it unchanged.
    fn fault(error: ExternalServiceError) -> ExternalServiceError {
        log_provider_fault(OPERATION, error.classification());
        error
    }

    /// Create a provider talking to the production endpoint.
    #[must_use]
    pub fn new(api_key: String) -> Self {
        Self::with_base_url(api_key, DEFAULT_BASE_URL.to_owned(), REQUEST_TIMEOUT)
    }

    /// Build the request for the base address, carrying the credential.
    fn request(&self, pairs: &[(&str, &str)]) -> Option<RequestBuilder> {
        let mut all: Vec<(&str, &str)> = pairs.to_vec();
        all.push(("apikey", self.api_key.as_str()));
        let url = request_url(&self.base_url, "", &all)?;
        Some(self.client.get(url))
    }

    /// Replace the artwork client, so a test can stand the image CDN in for
    /// `wiremock`.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_artwork_client(mut self, artwork: ArtworkClient) -> Self {
        self.artwork = artwork;
        self
    }

    /// Create a provider talking to `base_url`, with the given timeout.
    #[must_use]
    pub fn with_base_url(api_key: String, base_url: String, timeout: Duration) -> Self {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();
        Self {
            api_key,
            artwork: ArtworkClient::new(OMDB_ARTWORK_HOSTS),
            base_url,
            client,
        }
    }
}

impl VideoMetadataProvider for OmdbVideoMetadataProvider {
    #[expect(
        clippy::manual_async_fn,
        reason = "the implementation mirrors the port's return-position `impl Future` declaration"
    )]
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "the arms read the target's position out of a non-Copy enum matched by reference"
    )]
    fn lookup(
        &self,
        target: &Target,
    ) -> impl Future<Output = Result<Option<MediaMetadata>, ExternalServiceError>> + Send {
        async move {
            let reference = target.reference();
            if reference.provider() != ProviderId::Omdb {
                return Ok(None);
            }
            let id = reference.id();
            if id.is_empty()
                || !id.chars().all(|character| {
                    character.is_ascii_alphanumeric() || character == '-' || character == '_'
                })
            {
                return Ok(None);
            }
            let season_value;
            let episode_value;
            let mut pairs: Vec<(&str, &str)> = vec![("i", id), ("plot", "full")];
            match target {
                Target::Episode {
                    season, episode, ..
                } => {
                    season_value = season.to_string();
                    episode_value = episode.to_string();
                    pairs.push(("season", &season_value));
                    pairs.push(("episode", &episode_value));
                }
                Target::Movie(_) | Target::Series(_) => {}
            }
            let Some(request) = self.request(&pairs) else {
                return Err(Self::fault(ExternalServiceError::InvalidRequest));
            };
            let response = match request.send().await {
                Ok(response) => response,
                Err(failure) => return Err(Self::fault(classify_transport_error(&failure))),
            };
            if response.status() == StatusCode::NOT_FOUND {
                return Ok(None);
            }
            classify_status(response.status()).map_err(Self::fault)?;
            let payload = match read_payload::<DetailPayload>(response).await {
                Ok(payload) => payload,
                Err(error) => return Err(Self::fault(error)),
            };
            if payload.response.as_deref() == Some("False") {
                if let Some(error) = refusal(payload.error.as_deref()) {
                    return Err(Self::fault(error));
                }
                return Ok(None);
            }
            let Some(title) = payload.title.filter(|title| !title.is_empty()) else {
                return Ok(None);
            };
            let plot = payload.plot.unwrap_or_default();
            let poster = payload
                .poster
                .as_deref()
                .filter(|poster| !poster.is_empty() && *poster != "N/A");
            let artwork = match poster {
                Some(address) => self.artwork.fetch(ProviderId::Omdb, address).await,
                None => None,
            };

            match target {
                Target::Episode {
                    season, episode, ..
                } => Ok(Some(MediaMetadata::Episode {
                    air_date: parse_release_date(payload.released.as_deref()),
                    artwork,
                    episode: *episode,
                    plot,
                    season: *season,
                    title,
                })),
                Target::Movie(_) => Ok(Some(MediaMetadata::Movie {
                    artwork,
                    country_of_origin: mapped_country(payload.country.as_deref()),
                    genres: mapped_genres(payload.genre.as_deref()),
                    plot,
                    release_date: parse_release_date(payload.released.as_deref()),
                    title,
                })),
                Target::Series(_) => Ok(Some(MediaMetadata::Series {
                    artwork,
                    country_of_origin: mapped_country(payload.country.as_deref()),
                    first_air_date: parse_release_date(payload.released.as_deref()),
                    genres: mapped_genres(payload.genre.as_deref()),
                    plot,
                    title,
                })),
            }
        }
    }

    #[expect(
        clippy::manual_async_fn,
        reason = "the implementation mirrors the port's return-position `impl Future` declaration"
    )]
    fn search(
        &self,
        query: &MediaQuery,
    ) -> impl Future<Output = Result<Vec<Candidate>, ExternalServiceError>> + Send {
        async move {
            let kind = query.kind();
            let kind_parameter = match kind {
                VideoKind::Movie => "movie",
                VideoKind::Series => "series",
            };
            let year_value = query.year().map(|year| year.to_string());
            let mut pairs: Vec<(&str, &str)> = vec![("s", query.title()), ("type", kind_parameter)];
            if let Some(year) = year_value.as_deref() {
                pairs.push(("y", year));
            }
            let Some(request) = self.request(&pairs) else {
                return Err(Self::fault(ExternalServiceError::InvalidRequest));
            };
            let response = match request.send().await {
                Ok(response) => response,
                Err(failure) => return Err(Self::fault(classify_transport_error(&failure))),
            };
            if response.status() == StatusCode::NOT_FOUND {
                return Ok(Vec::new());
            }
            classify_status(response.status()).map_err(Self::fault)?;
            let payload = match read_payload::<SearchPayload>(response).await {
                Ok(payload) => payload,
                Err(error) => return Err(Self::fault(error)),
            };
            if payload.response.as_deref() == Some("False") {
                if let Some(error) = refusal(payload.error.as_deref()) {
                    return Err(Self::fault(error));
                }
                return Ok(Vec::new());
            }
            Ok(payload
                .search
                .unwrap_or_default()
                .into_iter()
                .filter_map(|hit| {
                    let title = hit.title.filter(|value| !value.is_empty())?;
                    let id = hit.imdb_id.filter(|value| !value.is_empty())?;
                    let year = hit
                        .year
                        .as_deref()
                        .and_then(|value| value.get(..4))
                        .and_then(|digits| digits.parse::<u32>().ok());
                    Some(Candidate::new(
                        ExternalRef::new(ProviderId::Omdb, id),
                        kind,
                        title,
                        year,
                    ))
                })
                .collect())
        }
    }
}

/// Map `OMDb`'s country names onto an uppercase ISO alpha-2 code.
fn mapped_country(value: Option<&str>) -> Option<String> {
    let names = value?;
    names.split(',').find_map(|name| {
        let trimmed = name.trim();
        if trimmed.len() == 2 && trimmed.bytes().all(|byte| byte.is_ascii_alphabetic()) {
            return Some(trimmed.to_ascii_uppercase());
        }
        let folded = trimmed.to_lowercase();
        let code = match folded.as_str() {
            "argentina" => "AR",
            "australia" => "AU",
            "austria" => "AT",
            "belgium" => "BE",
            "brazil" => "BR",
            "canada" => "CA",
            "china" => "CN",
            "czech republic" | "czechia" => "CZ",
            "denmark" => "DK",
            "finland" => "FI",
            "france" => "FR",
            "germany" => "DE",
            "greece" => "GR",
            "hong kong" => "HK",
            "hungary" => "HU",
            "iceland" => "IS",
            "india" => "IN",
            "ireland" => "IE",
            "israel" => "IL",
            "italy" => "IT",
            "japan" => "JP",
            "mexico" => "MX",
            "netherlands" => "NL",
            "new zealand" => "NZ",
            "norway" => "NO",
            "poland" => "PL",
            "portugal" => "PT",
            "romania" => "RO",
            "russia" => "RU",
            "south africa" => "ZA",
            "south korea" => "KR",
            "spain" => "ES",
            "sweden" => "SE",
            "switzerland" => "CH",
            "taiwan" => "TW",
            "turkey" => "TR",
            "ukraine" => "UA",
            "united kingdom" | "great britain" | "uk" => "GB",
            "united states" | "united states of america" | "usa" => "US",
            _ => return None,
        };
        Some(code.to_owned())
    })
}

/// Map `OMDb`'s genre names onto the shared vocabulary, canonically ordered.
fn mapped_genres(value: Option<&str>) -> Vec<Genre> {
    let mut genres = Vec::new();
    let mut unmatched: u64 = 0;
    for name in value.unwrap_or_default().split(',') {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            continue;
        }
        match Genre::from_name(trimmed) {
            Some(genre) => genres.push(genre),
            None => unmatched = unmatched.saturating_add(1),
        }
    }
    if unmatched > 0 {
        security_event::metadata_genre_unmatched(unmatched);
    }
    genres.sort_unstable_by_key(|genre| genre.as_str());
    genres.dedup();
    genres
}

/// Parse `OMDb`'s `16 Jul 2010` release date, treating `N/A` as absent.
fn parse_release_date(value: Option<&str>) -> Option<NaiveDate> {
    let text = value?;
    if text.is_empty() || text == "N/A" {
        return None;
    }
    NaiveDate::parse_from_str(text, "%d %b %Y").ok()
}

/// Classify `OMDb`'s `Response:"False"` refusal: an invalid key is an
/// authentication failure, every other message is "not found".
fn refusal(error: Option<&str>) -> Option<ExternalServiceError> {
    let message = error?;
    message
        .to_ascii_lowercase()
        .contains("invalid api key")
        .then_some(ExternalServiceError::Unauthorized)
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::time::Duration;

    use chrono::Datelike as _;
    use chrono::NaiveDate;
    use reqwest::StatusCode;
    use rstest::rstest;
    use tracing_test::traced_test;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;
    use wiremock::matchers::query_param;

    use crate::domain::model::genre::Genre;
    use crate::domain::model::media_candidate::Candidate;
    use crate::domain::model::media_candidate::ExternalRef;
    use crate::domain::model::media_candidate::ProviderId;
    use crate::domain::model::media_candidate::Target;
    use crate::domain::model::media_metadata::MediaMetadata;
    use crate::domain::model::media_query::MediaQuery;
    use crate::domain::model::media_query::MediaQueryError;
    use crate::domain::model::media_query::VideoKind;
    use crate::domain::port::video_metadata_provider::VideoMetadataProvider as _;
    use crate::infrastructure::outbound::reqwest::ArtworkClient;

    use super::OmdbVideoMetadataProvider;

    /// The search payload `wiremock` serves.
    const SEARCH: &str = include_str!("fixtures/omdb/search.json");

    /// The refusal `wiremock` serves for a search that matched nothing.
    const NOT_FOUND: &str = include_str!("fixtures/omdb/not_found.json");

    /// The refusal `wiremock` serves for an invalid key.
    const INVALID_KEY: &str = include_str!("fixtures/omdb/invalid_key.json");

    /// The film detail payload `wiremock` serves.
    const MOVIE: &str = include_str!("fixtures/omdb/movie.json");

    /// The show detail payload `wiremock` serves.
    const SERIES: &str = include_str!("fixtures/omdb/series.json");

    /// The episode detail payload `wiremock` serves.
    const EPISODE: &str = include_str!("fixtures/omdb/episode.json");

    /// The film detail payload whose date is the `N/A` placeholder.
    const MOVIE_PLACEHOLDERS: &str = include_str!("fixtures/omdb/movie_placeholders.json");

    /// The film detail payload whose country the mapping does not contain.
    const MOVIE_UNMAPPABLE_COUNTRY: &str =
        include_str!("fixtures/omdb/movie_unmappable_country.json");

    /// The 1×1 image `wiremock` serves as artwork.
    const ARTWORK: &[u8] = include_bytes!("fixtures/artwork/pixel.png");

    /// The poster host the golden payloads name.
    const POSTER_HOST: &str = "https://m.media-amazon.com";

    /// Timeout the adapter tests give one request.
    const TEST_TIMEOUT: Duration = Duration::from_secs(5);

    /// The sentinel key every test configures, so a leak is detectable.
    const SENTINEL: &str = "SENTINEL_OMDB_KEY";

    /// A provider pointed at `server`, with the mock host as the artwork host.
    fn provider(server: &MockServer, timeout: Duration) -> OmdbVideoMetadataProvider {
        OmdbVideoMetadataProvider::with_base_url(SENTINEL.to_owned(), server.uri(), timeout)
            .with_artwork_client(ArtworkClient::for_test("127.0.0.1"))
    }

    /// Mount a JSON answer on the base address.
    async fn mount_json(server: &MockServer, body: &str) {
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(server)
            .await;
    }

    /// Point the golden payload's poster at `server`.
    fn with_local_poster(body: &str, server: &MockServer) -> String {
        body.replace(POSTER_HOST, &server.uri())
    }

    /// The query a test searches for.
    ///
    /// # Errors
    ///
    /// Returns the constructor's error when the title is empty, which no test
    /// passes.
    fn query(
        kind: VideoKind,
        title: &str,
        year: Option<u32>,
    ) -> Result<MediaQuery, MediaQueryError> {
        MediaQuery::try_new(kind, title.to_owned(), year, None)
    }

    #[tokio::test]
    async fn omdb_search_returns_the_provider_order() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("s", "Inception"))
            .and(query_param("type", "movie"))
            .and(query_param("y", "2010"))
            .and(query_param("apikey", SENTINEL))
            .respond_with(ResponseTemplate::new(200).set_body_string(SEARCH))
            .expect(1)
            .mount(&server)
            .await;
        let provider = provider(&server, TEST_TIMEOUT);
        let query = query(VideoKind::Movie, "Inception", Some(2010))?;

        // Act
        let candidates = provider.search(&query).await?;

        // Assert
        assert_eq!(candidates.len(), 2, "both hits must be kept, in order");
        assert_eq!(candidates.first().map(Candidate::title), Some("Inception"));
        assert_eq!(candidates.first().and_then(Candidate::year), Some(2010));
        assert_eq!(
            candidates
                .first()
                .map(Candidate::reference)
                .map(ExternalRef::id),
            Some("tt1375666")
        );
        assert_eq!(
            candidates
                .last()
                .map(Candidate::reference)
                .map(ExternalRef::id),
            Some("tt5295894"),
            "the provider order must be preserved"
        );
        Ok(())
    }

    #[tokio::test]
    async fn omdb_search_reports_not_found_as_no_candidate() -> Result<(), Box<dyn Error>> {
        // Arrange: OMDb reports "not found" as HTTP 200 with `Response:"False"`.
        let server = MockServer::start().await;
        mount_json(&server, NOT_FOUND).await;
        let provider = provider(&server, TEST_TIMEOUT);
        let query = query(VideoKind::Movie, "Nothing", None)?;

        // Act
        let candidates = provider.search(&query).await?;

        // Assert
        assert!(candidates.is_empty(), "a miss is not a failure");
        Ok(())
    }

    #[tokio::test]
    async fn omdb_search_classifies_an_invalid_key() -> Result<(), Box<dyn Error>> {
        // Arrange: the refusal arrives with HTTP 200.
        let server = MockServer::start().await;
        mount_json(&server, INVALID_KEY).await;
        let provider = provider(&server, TEST_TIMEOUT);
        let query = query(VideoKind::Movie, "Inception", None)?;

        // Act
        let failure = provider
            .search(&query)
            .await
            .err()
            .ok_or("an invalid key must be an error")?;

        // Assert
        assert_eq!(failure.classification(), "unauthorized");
        Ok(())
    }

    #[tokio::test]
    async fn omdb_lookup_maps_a_film_to_canonical_metadata() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        mount_json(&server, MOVIE).await;
        let provider = provider(&server, TEST_TIMEOUT);
        let target = Target::Movie(ExternalRef::new(ProviderId::Omdb, "tt1375666".to_owned()));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert: the canonical film expectation TMDB is asserted against too.
        let Some(MediaMetadata::Movie {
            title,
            plot,
            release_date,
            country_of_origin,
            genres,
            artwork,
        }) = metadata
        else {
            return Err("a film lookup must yield film metadata".into());
        };
        assert_eq!(title, "Inception");
        assert!(!plot.is_empty(), "the plot must be mapped");
        assert_eq!(
            release_date,
            NaiveDate::from_ymd_opt(2010, 7, 16),
            "the release date must be mapped"
        );
        assert_eq!(release_date.map(|date| date.year()), Some(2_010i32));
        assert_eq!(country_of_origin.as_deref(), Some("US"));
        assert_eq!(
            genres,
            vec![Genre::Action, Genre::ScienceFiction, Genre::Thriller]
        );
        assert_eq!(
            artwork, None,
            "a poster outside the allow-list yields no artwork"
        );
        Ok(())
    }

    #[tokio::test]
    async fn omdb_lookup_treats_a_placeholder_release_date_as_absent() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the payload reports the `N/A` placeholder instead of a date.
        let server = MockServer::start().await;
        mount_json(&server, MOVIE_PLACEHOLDERS).await;
        let provider = provider(&server, TEST_TIMEOUT);
        let target = Target::Movie(ExternalRef::new(ProviderId::Omdb, "tt1375666".to_owned()));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert: a placeholder is absence, never a value.
        let Some(MediaMetadata::Movie { release_date, .. }) = metadata else {
            return Err("a film lookup must yield film metadata".into());
        };
        assert_eq!(
            release_date, None,
            "a placeholder release date must be absent"
        );
        Ok(())
    }

    #[tokio::test]
    async fn omdb_lookup_drops_an_unmappable_country() -> Result<(), Box<dyn Error>> {
        // Arrange: the payload's country has no code in the mapping.
        let server = MockServer::start().await;
        mount_json(&server, MOVIE_UNMAPPABLE_COUNTRY).await;
        let provider = provider(&server, TEST_TIMEOUT);
        let target = Target::Movie(ExternalRef::new(ProviderId::Omdb, "tt1375666".to_owned()));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert: a country the mapping cannot resolve is absent, never guessed.
        let Some(MediaMetadata::Movie {
            country_of_origin, ..
        }) = metadata
        else {
            return Err("a film lookup must yield film metadata".into());
        };
        assert_eq!(
            country_of_origin, None,
            "an unmappable country must be absent"
        );
        Ok(())
    }

    #[tokio::test]
    async fn omdb_lookup_maps_a_series_to_canonical_metadata() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        mount_json(&server, SERIES).await;
        let provider = provider(&server, TEST_TIMEOUT);
        let target = Target::Series(ExternalRef::new(ProviderId::Omdb, "tt0903747".to_owned()));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert
        let Some(MediaMetadata::Series {
            title,
            first_air_date,
            country_of_origin,
            genres,
            ..
        }) = metadata
        else {
            return Err("a show lookup must yield show metadata".into());
        };
        assert_eq!(title, "Breaking Bad");
        assert_eq!(first_air_date, NaiveDate::from_ymd_opt(2008, 1, 20));
        assert_eq!(country_of_origin.as_deref(), Some("US"));
        assert_eq!(genres, vec![Genre::Crime, Genre::Drama, Genre::Thriller]);
        Ok(())
    }

    #[tokio::test]
    async fn omdb_lookup_maps_an_episode_to_canonical_metadata() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        mount_json(&server, EPISODE).await;
        let provider = provider(&server, TEST_TIMEOUT);
        let target = Target::Episode {
            episode: 1,
            reference: ExternalRef::new(ProviderId::Omdb, "tt0903747".to_owned()),
            season: 1,
        };

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert
        let Some(MediaMetadata::Episode {
            title,
            air_date,
            season,
            episode,
            ..
        }) = metadata
        else {
            return Err("an episode lookup must yield episode metadata".into());
        };
        assert_eq!(title, "Pilot");
        assert_eq!(air_date, NaiveDate::from_ymd_opt(2008, 1, 20));
        assert_eq!((season, episode), (1, 1));
        Ok(())
    }

    #[tokio::test]
    async fn omdb_lookup_reports_a_missing_item_as_absent() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        mount_json(&server, NOT_FOUND).await;
        let provider = provider(&server, TEST_TIMEOUT);
        let target = Target::Movie(ExternalRef::new(ProviderId::Omdb, "tt0000000".to_owned()));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert
        assert_eq!(metadata, None, "a missing item is not a port error");
        Ok(())
    }

    #[rstest]
    #[case::unauthorized(StatusCode::UNAUTHORIZED, "unauthorized")]
    #[case::rate_limited(StatusCode::TOO_MANY_REQUESTS, "rate_limited")]
    #[case::server_error(StatusCode::INTERNAL_SERVER_ERROR, "dependency_failure")]
    #[tokio::test]
    async fn omdb_search_classifies_a_failed_status(
        #[case] status: StatusCode,
        #[case] classification: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(status.as_u16()))
            .mount(&server)
            .await;
        let provider = provider(&server, TEST_TIMEOUT);
        let query = query(VideoKind::Movie, "Inception", None)?;

        // Act
        let failure = provider
            .search(&query)
            .await
            .err()
            .ok_or("a failed status must be an error")?;

        // Assert
        assert_eq!(failure.classification(), classification);
        Ok(())
    }

    #[tokio::test]
    async fn omdb_search_classifies_a_malformed_payload() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        mount_json(&server, "{").await;
        let provider = provider(&server, TEST_TIMEOUT);
        let query = query(VideoKind::Movie, "Inception", None)?;

        // Act
        let failure = provider
            .search(&query)
            .await
            .err()
            .ok_or("a malformed payload must be an error")?;

        // Assert
        assert_eq!(failure.classification(), "deserialization_failure");
        Ok(())
    }

    #[tokio::test]
    async fn omdb_search_classifies_a_timeout() -> Result<(), Box<dyn Error>> {
        // Arrange: the answer arrives after the adapter's bound.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(SEARCH)
                    .set_delay(Duration::from_millis(300)),
            )
            .mount(&server)
            .await;
        let provider = provider(&server, Duration::from_millis(50));
        let query = query(VideoKind::Movie, "Inception", None)?;

        // Act
        let failure = provider
            .search(&query)
            .await
            .err()
            .ok_or("a late answer must time out")?;

        // Assert
        assert_eq!(failure.classification(), "timeout");
        Ok(())
    }

    #[tokio::test]
    async fn omdb_search_classifies_a_connection_refusal() -> Result<(), Box<dyn Error>> {
        // Arrange: nothing listens on port 1.
        let provider = OmdbVideoMetadataProvider::with_base_url(
            SENTINEL.to_owned(),
            "http://127.0.0.1:1".to_owned(),
            Duration::from_millis(500),
        );
        let query = query(VideoKind::Movie, "Inception", None)?;

        // Act
        let failure = provider
            .search(&query)
            .await
            .err()
            .ok_or("a refused connection must be an error")?;

        // Assert
        assert_eq!(failure.classification(), "unavailable");
        Ok(())
    }

    #[tokio::test]
    async fn omdb_lookup_downloads_the_artwork_as_content() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        // The artwork mock is mounted first, so the payload's catch-all does not
        // answer the poster request.
        Mock::given(method("GET"))
            .and(path("/images/poster.png"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(ARTWORK.to_vec()))
            .mount(&server)
            .await;
        mount_json(&server, &with_local_poster(MOVIE, &server)).await;
        let provider = provider(&server, TEST_TIMEOUT);
        let target = Target::Movie(ExternalRef::new(ProviderId::Omdb, "tt1375666".to_owned()));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert
        let Some(MediaMetadata::Movie { artwork, .. }) = metadata else {
            return Err("a film lookup must yield film metadata".into());
        };
        let downloaded = artwork.ok_or("the poster must be downloaded")?;
        assert_eq!(downloaded.bytes(), ARTWORK);
        assert_eq!(downloaded.media_type(), "image/png");
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn omdb_lookup_reports_no_artwork_when_the_download_fails() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/images/poster.png"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        mount_json(&server, &with_local_poster(MOVIE, &server)).await;
        let provider = provider(&server, TEST_TIMEOUT);
        let target = Target::Movie(ExternalRef::new(ProviderId::Omdb, "tt1375666".to_owned()));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert
        let Some(MediaMetadata::Movie { artwork, .. }) = metadata else {
            return Err("a film lookup must yield film metadata".into());
        };
        assert_eq!(artwork, None, "a failed download is not a lookup failure");
        logs_contain("event=\"metadata_artwork_failed\"");
        Ok(())
    }

    #[tokio::test]
    async fn omdb_lookup_does_not_follow_a_cross_host_redirect() -> Result<(), Box<dyn Error>> {
        // Arrange: the poster address redirects to another server.
        let origin = MockServer::start().await;
        let target_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/images/poster.png"))
            .respond_with(ResponseTemplate::new(302).insert_header(
                "Location",
                format!("{}/target", target_server.uri()).as_str(),
            ))
            .mount(&origin)
            .await;
        mount_json(&origin, &with_local_poster(MOVIE, &origin)).await;
        Mock::given(method("GET"))
            .and(path("/target"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(ARTWORK.to_vec()))
            .expect(0)
            .mount(&target_server)
            .await;
        let provider = provider(&origin, TEST_TIMEOUT);
        let target = Target::Movie(ExternalRef::new(ProviderId::Omdb, "tt1375666".to_owned()));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert
        let Some(MediaMetadata::Movie { artwork, .. }) = metadata else {
            return Err("a film lookup must yield film metadata".into());
        };
        assert_eq!(artwork, None, "a redirect must not be followed");
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn omdb_never_leaks_the_api_key() -> Result<(), Box<dyn Error>> {
        // Arrange: the key travels in the query string and the provider fails.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("apikey", SENTINEL))
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&server)
            .await;
        let provider = provider(&server, TEST_TIMEOUT);
        let query = query(VideoKind::Movie, "Inception", None)?;

        // Act
        let failure = provider
            .search(&query)
            .await
            .err()
            .ok_or("a failed request must be an error")?;

        // Assert
        assert!(!failure.to_string().contains(SENTINEL));
        assert!(!format!("{failure:?}").contains(SENTINEL));
        logs_assert(|lines: &[&str]| {
            let leaked = lines.iter().any(|line| line.contains(SENTINEL));
            (!leaked)
                .then_some(())
                .ok_or_else(|| "the API key must never reach a log sink".to_owned())
        });
        Ok(())
    }
}
