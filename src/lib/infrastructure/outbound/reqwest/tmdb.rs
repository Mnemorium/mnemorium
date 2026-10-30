//! TMDB adapter: maps TMDB's search and detail payloads onto the canonical port.
//!
//! Authentication prefers a bearer read token (a v4 token is a dotted JWT);
//! a v3 API key travels in the query string and is therefore subject to the
//! sanitizing rule: a failure is classified and logged by kind, never by the
//! request address.

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
use crate::infrastructure::outbound::reqwest::classify_status;
use crate::infrastructure::outbound::reqwest::classify_transport_error;
use crate::infrastructure::outbound::reqwest::log_provider_fault;
use crate::infrastructure::outbound::reqwest::read_payload;
use crate::infrastructure::outbound::reqwest::request_url;

/// Base address of the TMDB API.
const DEFAULT_BASE_URL: &str = "https://api.themoviedb.org/3";

/// Base address of the TMDB image CDN.
const DEFAULT_ARTWORK_BASE_URL: &str = "https://image.tmdb.org/t/p/w500";

/// Time one TMDB request may take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// One country entry of a TMDB detail payload.
#[derive(Debug, Deserialize)]
struct CountryPayload {
    /// ISO 3166-1 alpha-2 code the film was produced in.
    iso_3166_1: String,
}

/// One detail payload, for a film, a show or an episode.
#[derive(Debug, Deserialize)]
struct DetailPayload {
    /// Air date of an episode.
    #[serde(default)]
    air_date: Option<String>,
    /// Date the show first aired.
    #[serde(default)]
    first_air_date: Option<String>,
    /// Genres the work is associated with.
    #[serde(default)]
    genres: Vec<GenrePayload>,
    /// Title of a show or an episode.
    #[serde(default)]
    name: Option<String>,
    /// Countries of origin of a show.
    #[serde(default)]
    origin_country: Vec<String>,
    /// Plot summary.
    #[serde(default)]
    overview: Option<String>,
    /// Path of the poster on the image CDN.
    #[serde(default)]
    poster_path: Option<String>,
    /// Countries of origin of a film.
    #[serde(default)]
    production_countries: Vec<CountryPayload>,
    /// Date the film was released.
    #[serde(default)]
    release_date: Option<String>,
    /// Path of the episode still on the image CDN.
    #[serde(default)]
    still_path: Option<String>,
    /// Title of a film.
    #[serde(default)]
    title: Option<String>,
}

/// One genre entry of a TMDB payload.
#[derive(Debug, Deserialize)]
struct GenrePayload {
    /// Genre name as TMDB spells it.
    name: String,
}

/// One search hit of a TMDB payload.
#[derive(Debug, Deserialize)]
struct SearchHit {
    /// Date the show first aired.
    #[serde(default)]
    first_air_date: Option<String>,
    /// Identifier TMDB issued for the work.
    id: u64,
    /// Title of a show.
    #[serde(default)]
    name: Option<String>,
    /// Date the film was released.
    #[serde(default)]
    release_date: Option<String>,
    /// Title of a film.
    #[serde(default)]
    title: Option<String>,
}

/// The results of one TMDB search.
#[derive(Debug, Deserialize)]
struct SearchPayload {
    /// Hits the search returned, in TMDB's order.
    #[serde(default)]
    results: Vec<SearchHit>,
}

/// Video metadata provider backed by the TMDB API.
pub struct TmdbVideoMetadataProvider {
    /// API key the request carries.
    api_key: String,
    /// Client downloading the artwork under the adapter's policy.
    artwork: ArtworkClient,
    /// Base address the poster paths are resolved against.
    artwork_base_url: String,
    /// Base address of the API.
    base_url: String,
    /// Client carrying the request timeout.
    client: Client,
    /// Whether the key is a v4 read token, sent as a bearer credential.
    use_bearer: bool,
}

impl TmdbVideoMetadataProvider {
    /// Emit the owning layer's event for `error` and return it unchanged.
    fn fault(error: ExternalServiceError) -> ExternalServiceError {
        log_provider_fault(error.classification());
        error
    }

    /// Create a provider talking to the production endpoints.
    #[must_use]
    pub fn new(api_key: String) -> Self {
        Self::with_endpoints(
            api_key,
            DEFAULT_BASE_URL.to_owned(),
            DEFAULT_ARTWORK_BASE_URL.to_owned(),
            REQUEST_TIMEOUT,
        )
    }

    /// Build the request for `path`, carrying the configured credential.
    ///
    /// A v3 key is appended to the query pairs; a v4 read token is sent as a
    /// bearer credential.
    fn request(&self, path: &str, pairs: &[(&str, &str)]) -> Option<RequestBuilder> {
        let mut all: Vec<(&str, &str)> = pairs.to_vec();
        if !self.use_bearer {
            all.push(("api_key", self.api_key.as_str()));
        }
        let url = request_url(&self.base_url, path, &all)?;
        let request = self.client.get(url);
        Some(if self.use_bearer {
            request.bearer_auth(self.api_key.as_str())
        } else {
            request
        })
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
        Self::with_endpoints(
            api_key,
            base_url,
            DEFAULT_ARTWORK_BASE_URL.to_owned(),
            timeout,
        )
    }

    /// Create a provider with every address and the timeout as inputs.
    ///
    /// The artwork base is a separate input so a test can stand the image CDN
    /// in for `wiremock` too.
    #[must_use]
    pub fn with_endpoints(
        api_key: String,
        base_url: String,
        artwork_base_url: String,
        timeout: Duration,
    ) -> Self {
        // A v4 read token is a dotted JWT; a v3 key is a plain identifier and is
        // sent in the query string.
        let use_bearer = api_key.contains('.');
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();
        Self {
            api_key,
            artwork: ArtworkClient::new(),
            artwork_base_url,
            base_url,
            client,
            use_bearer,
        }
    }
}

impl VideoMetadataProvider for TmdbVideoMetadataProvider {
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
            if reference.provider() != ProviderId::Tmdb {
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
            let path = match target {
                Target::Movie(_) => format!("/movie/{id}"),
                Target::Series(_) => format!("/tv/{id}"),
                Target::Episode {
                    season, episode, ..
                } => format!("/tv/{id}/season/{season}/episode/{episode}"),
            };
            let Some(request) = self.request(&path, &[]) else {
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
            let poster = match target {
                Target::Episode { .. } => payload.still_path.as_deref(),
                Target::Movie(_) | Target::Series(_) => payload.poster_path.as_deref(),
            };
            let artwork = match poster {
                Some(poster_path) if !poster_path.is_empty() => {
                    let address = format!("{}{poster_path}", self.artwork_base_url);
                    self.artwork.fetch(ProviderId::Tmdb, &address).await
                }
                _ => None,
            };
            let plot = payload.overview.unwrap_or_default();

            match target {
                Target::Episode {
                    season, episode, ..
                } => {
                    let Some(episode_title) = payload.name.filter(|value| !value.is_empty()) else {
                        return Ok(None);
                    };
                    Ok(Some(MediaMetadata::Episode {
                        air_date: parse_date(payload.air_date.as_deref()),
                        artwork,
                        episode: *episode,
                        plot,
                        season: *season,
                        title: episode_title,
                    }))
                }
                Target::Movie(_) => {
                    let Some(movie_title) = payload.title.filter(|value| !value.is_empty()) else {
                        return Ok(None);
                    };
                    let country_of_origin = first_alpha2(
                        payload
                            .production_countries
                            .iter()
                            .map(|country| country.iso_3166_1.as_str()),
                    );
                    Ok(Some(MediaMetadata::Movie {
                        artwork,
                        country_of_origin,
                        genres: mapped_genres(&payload.genres),
                        plot,
                        release_date: parse_date(payload.release_date.as_deref()),
                        title: movie_title,
                    }))
                }
                Target::Series(_) => {
                    let Some(series_title) = payload.name.filter(|value| !value.is_empty()) else {
                        return Ok(None);
                    };
                    let country_of_origin =
                        first_alpha2(payload.origin_country.iter().map(String::as_str));
                    Ok(Some(MediaMetadata::Series {
                        artwork,
                        country_of_origin,
                        first_air_date: parse_date(payload.first_air_date.as_deref()),
                        genres: mapped_genres(&payload.genres),
                        plot,
                        title: series_title,
                    }))
                }
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
            let (path, year_parameter) = match kind {
                VideoKind::Movie => ("/search/movie", "primary_release_year"),
                VideoKind::Series => ("/search/tv", "first_air_date_year"),
            };
            let year_value = query.year().map(|year| year.to_string());
            let mut pairs: Vec<(&str, &str)> = vec![("query", query.title())];
            if let Some(year) = year_value.as_deref() {
                pairs.push((year_parameter, year));
            }
            let Some(request) = self.request(path, &pairs) else {
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
            Ok(payload
                .results
                .into_iter()
                .filter_map(|hit| {
                    let (hit_title, date) = match kind {
                        VideoKind::Movie => (hit.title, hit.release_date),
                        VideoKind::Series => (hit.name, hit.first_air_date),
                    };
                    let title = hit_title.filter(|value| !value.is_empty())?;
                    let year = date
                        .as_deref()
                        .and_then(|value| value.get(..4))
                        .and_then(|digits| digits.parse::<u32>().ok());
                    Some(Candidate::new(
                        ExternalRef::new(ProviderId::Tmdb, hit.id.to_string()),
                        kind,
                        title,
                        year,
                    ))
                })
                .collect())
        }
    }
}

/// Return the first uppercase ISO alpha-2 code among `codes`.
fn first_alpha2<'code>(codes: impl IntoIterator<Item = &'code str>) -> Option<String> {
    codes.into_iter().find_map(|code| {
        let trimmed = code.trim();
        (trimmed.len() == 2 && trimmed.bytes().all(|byte| byte.is_ascii_alphabetic()))
            .then(|| trimmed.to_ascii_uppercase())
    })
}

/// Map TMDB's genre names onto the shared vocabulary, canonically ordered.
fn mapped_genres(entries: &[GenrePayload]) -> Vec<Genre> {
    let mut genres = Vec::with_capacity(entries.len());
    let mut unmatched: u64 = 0;
    for entry in entries {
        match Genre::from_name(&entry.name) {
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

/// Parse an ISO date, treating an absent or placeholder value as absent.
fn parse_date(value: Option<&str>) -> Option<NaiveDate> {
    let text = value?;
    if text.is_empty() || text == "N/A" {
        return None;
    }
    NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()
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
    use wiremock::matchers::header;
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
    use crate::infrastructure::outbound::reqwest::ARTWORK_MAX_BYTES;
    use crate::infrastructure::outbound::reqwest::ArtworkClient;

    use super::TmdbVideoMetadataProvider;

    /// The film search payload `wiremock` serves.
    const SEARCH_MOVIE: &str = include_str!("fixtures/tmdb/search_movie.json");

    /// The show search payload `wiremock` serves.
    const SEARCH_SERIES: &str = include_str!("fixtures/tmdb/search_series.json");

    /// The empty search payload `wiremock` serves.
    const SEARCH_EMPTY: &str = include_str!("fixtures/tmdb/search_empty.json");

    /// The film detail payload `wiremock` serves.
    const MOVIE: &str = include_str!("fixtures/tmdb/movie.json");

    /// The show detail payload `wiremock` serves.
    const SERIES: &str = include_str!("fixtures/tmdb/series.json");

    /// The episode detail payload `wiremock` serves.
    const EPISODE: &str = include_str!("fixtures/tmdb/episode.json");

    /// The film detail payload carrying a genre outside the vocabulary.
    const MOVIE_UNKNOWN_GENRE: &str = include_str!("fixtures/tmdb/movie_unknown_genre.json");

    /// The film detail payload that reports no release date.
    const MOVIE_ABSENT_DATE: &str = include_str!("fixtures/tmdb/movie_absent_date.json");

    /// The film detail payload whose country is not an ISO alpha-2 code.
    const MOVIE_UNMAPPABLE_COUNTRY: &str =
        include_str!("fixtures/tmdb/movie_unmappable_country.json");

    /// The 1×1 image `wiremock` serves as artwork.
    const ARTWORK: &[u8] = include_bytes!("fixtures/artwork/pixel.png");

    /// Timeout the adapter tests give one request.
    const TEST_TIMEOUT: Duration = Duration::from_secs(5);

    /// The sentinel key every test configures, so a leak is detectable.
    const SENTINEL: &str = "SENTINEL_TMDB_KEY";

    /// A provider pointed at `server`, with the artwork host allowed.
    fn provider(server: &MockServer, timeout: Duration) -> TmdbVideoMetadataProvider {
        TmdbVideoMetadataProvider::with_endpoints(
            SENTINEL.to_owned(),
            server.uri(),
            server.uri(),
            timeout,
        )
        .with_artwork_client(ArtworkClient::for_test("127.0.0.1"))
    }

    /// Mount a JSON answer on `route`.
    async fn mount_json(server: &MockServer, route: &str, body: &str) {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(server)
            .await;
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
    async fn tmdb_search_returns_the_provider_order() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search/movie"))
            .and(query_param("query", "Inception"))
            .and(query_param("primary_release_year", "2010"))
            .and(query_param("api_key", SENTINEL))
            .respond_with(ResponseTemplate::new(200).set_body_string(SEARCH_MOVIE))
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
                .map(ExternalRef::provider),
            Some(ProviderId::Tmdb)
        );
        assert_eq!(
            candidates
                .first()
                .map(Candidate::reference)
                .map(ExternalRef::id),
            Some("27205")
        );
        assert_eq!(
            candidates
                .last()
                .map(Candidate::reference)
                .map(ExternalRef::id),
            Some("1089"),
            "the provider order must be preserved"
        );
        Ok(())
    }

    #[tokio::test]
    async fn tmdb_search_maps_a_series_query() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        mount_json(&server, "/search/tv", SEARCH_SERIES).await;
        let provider = provider(&server, TEST_TIMEOUT);
        let query = query(VideoKind::Series, "Breaking Bad", None)?;

        // Act
        let candidates = provider.search(&query).await?;

        // Assert
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates.first().map(Candidate::title),
            Some("Breaking Bad")
        );
        assert_eq!(candidates.first().and_then(Candidate::year), Some(2008));
        Ok(())
    }

    #[tokio::test]
    async fn tmdb_search_reports_no_candidate_for_an_empty_result() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        mount_json(&server, "/search/movie", SEARCH_EMPTY).await;
        let provider = provider(&server, TEST_TIMEOUT);
        let query = query(VideoKind::Movie, "Nothing", None)?;

        // Act
        let candidates = provider.search(&query).await?;

        // Assert
        assert!(candidates.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn tmdb_lookup_maps_a_film_to_canonical_metadata() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        mount_json(&server, "/movie/27205", MOVIE).await;
        let provider = provider(&server, TEST_TIMEOUT);
        let target = Target::Movie(ExternalRef::new(ProviderId::Tmdb, "27205".to_owned()));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert: the canonical film expectation OMDb is asserted against too.
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
            "an unmocked artwork address yields no artwork"
        );
        Ok(())
    }

    #[tokio::test]
    async fn tmdb_lookup_reports_an_unreported_release_date_as_absent() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the payload names no release date at all.
        let server = MockServer::start().await;
        mount_json(&server, "/movie/999002", MOVIE_ABSENT_DATE).await;
        let provider = provider(&server, TEST_TIMEOUT);
        let target = Target::Movie(ExternalRef::new(ProviderId::Tmdb, "999002".to_owned()));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert: absence is a value, never today's date or a placeholder.
        let Some(MediaMetadata::Movie { release_date, .. }) = metadata else {
            return Err("a film lookup must yield film metadata".into());
        };
        assert_eq!(
            release_date, None,
            "an unreported release date must be absent"
        );
        Ok(())
    }

    #[tokio::test]
    async fn tmdb_lookup_drops_an_unmappable_country() -> Result<(), Box<dyn Error>> {
        // Arrange: the payload's country is not an ISO alpha-2 code.
        let server = MockServer::start().await;
        mount_json(&server, "/movie/999003", MOVIE_UNMAPPABLE_COUNTRY).await;
        let provider = provider(&server, TEST_TIMEOUT);
        let target = Target::Movie(ExternalRef::new(ProviderId::Tmdb, "999003".to_owned()));

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
    async fn tmdb_lookup_maps_a_series_to_canonical_metadata() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        mount_json(&server, "/tv/1396", SERIES).await;
        let provider = provider(&server, TEST_TIMEOUT);
        let target = Target::Series(ExternalRef::new(ProviderId::Tmdb, "1396".to_owned()));

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
        assert_eq!(genres, vec![Genre::Crime, Genre::Drama]);
        Ok(())
    }

    #[tokio::test]
    async fn tmdb_lookup_maps_an_episode_to_canonical_metadata() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        mount_json(&server, "/tv/1396/season/1/episode/1", EPISODE).await;
        let provider = provider(&server, TEST_TIMEOUT);
        let target = Target::Episode {
            episode: 1,
            reference: ExternalRef::new(ProviderId::Tmdb, "1396".to_owned()),
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
    async fn tmdb_lookup_reports_a_missing_item_as_absent() -> Result<(), Box<dyn Error>> {
        // Arrange: the provider answers 404 for the item.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let provider = provider(&server, TEST_TIMEOUT);
        let target = Target::Movie(ExternalRef::new(ProviderId::Tmdb, "999999".to_owned()));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert
        assert_eq!(metadata, None, "a missing item is not a port error");
        Ok(())
    }

    #[rstest]
    #[case::unauthorized(StatusCode::UNAUTHORIZED, "unauthorized")]
    #[case::forbidden(StatusCode::FORBIDDEN, "forbidden")]
    #[case::rate_limited(StatusCode::TOO_MANY_REQUESTS, "rate_limited")]
    #[case::server_error(StatusCode::INTERNAL_SERVER_ERROR, "dependency_failure")]
    #[tokio::test]
    async fn tmdb_search_classifies_a_failed_status(
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
    async fn tmdb_search_classifies_a_malformed_payload() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        mount_json(&server, "/search/movie", "{").await;
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
    async fn tmdb_search_classifies_a_timeout() -> Result<(), Box<dyn Error>> {
        // Arrange: the answer arrives after the adapter's bound.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(SEARCH_EMPTY)
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
    async fn tmdb_search_classifies_a_connection_refusal() -> Result<(), Box<dyn Error>> {
        // Arrange: nothing listens on port 1.
        let provider = TmdbVideoMetadataProvider::with_endpoints(
            SENTINEL.to_owned(),
            "http://127.0.0.1:1".to_owned(),
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
    async fn tmdb_lookup_downloads_the_artwork_as_content() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        mount_json(&server, "/movie/27205", MOVIE).await;
        Mock::given(method("GET"))
            .and(path("/poster.png"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(ARTWORK.to_vec()))
            .mount(&server)
            .await;
        let provider = provider(&server, TEST_TIMEOUT);
        let target = Target::Movie(ExternalRef::new(ProviderId::Tmdb, "27205".to_owned()));

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
    async fn tmdb_lookup_reports_no_artwork_when_the_download_fails() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the poster address answers an error.
        let server = MockServer::start().await;
        mount_json(&server, "/movie/27205", MOVIE).await;
        Mock::given(method("GET"))
            .and(path("/poster.png"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let provider = provider(&server, TEST_TIMEOUT);
        let target = Target::Movie(ExternalRef::new(ProviderId::Tmdb, "27205".to_owned()));

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
    async fn tmdb_lookup_rejects_oversized_artwork() -> Result<(), Box<dyn Error>> {
        // Arrange: the poster exceeds the size bound.
        let server = MockServer::start().await;
        mount_json(&server, "/movie/27205", MOVIE).await;
        Mock::given(method("GET"))
            .and(path("/poster.png"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(vec![0u8; ARTWORK_MAX_BYTES.saturating_add(1)]),
            )
            .mount(&server)
            .await;
        let provider = provider(&server, TEST_TIMEOUT);
        let target = Target::Movie(ExternalRef::new(ProviderId::Tmdb, "27205".to_owned()));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert
        let Some(MediaMetadata::Movie { artwork, .. }) = metadata else {
            return Err("a film lookup must yield film metadata".into());
        };
        assert_eq!(artwork, None, "oversized artwork must be rejected");
        Ok(())
    }

    #[tokio::test]
    async fn tmdb_lookup_does_not_follow_a_redirect() -> Result<(), Box<dyn Error>> {
        // Arrange: the poster address redirects to another server.
        let origin = MockServer::start().await;
        let target_server = MockServer::start().await;
        mount_json(&origin, "/movie/27205", MOVIE).await;
        Mock::given(method("GET"))
            .and(path("/poster.png"))
            .respond_with(ResponseTemplate::new(302).insert_header(
                "Location",
                format!("{}/target", target_server.uri()).as_str(),
            ))
            .mount(&origin)
            .await;
        Mock::given(method("GET"))
            .and(path("/target"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(ARTWORK.to_vec()))
            .expect(0)
            .mount(&target_server)
            .await;
        let provider = provider(&origin, TEST_TIMEOUT);
        let target = Target::Movie(ExternalRef::new(ProviderId::Tmdb, "27205".to_owned()));

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
    async fn tmdb_lookup_reports_an_unmatched_genre() -> Result<(), Box<dyn Error>> {
        // Arrange: the payload names a genre outside the vocabulary.
        let server = MockServer::start().await;
        mount_json(&server, "/movie/999001", MOVIE_UNKNOWN_GENRE).await;
        let provider = provider(&server, TEST_TIMEOUT);
        let target = Target::Movie(ExternalRef::new(ProviderId::Tmdb, "999001".to_owned()));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert: the term is dropped and only its count is recorded.
        let Some(MediaMetadata::Movie { genres, .. }) = metadata else {
            return Err("a film lookup must yield film metadata".into());
        };
        assert_eq!(genres, vec![Genre::Action]);
        logs_contain("event=\"metadata_genre_unmatched\"");
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn tmdb_never_leaks_the_api_key() -> Result<(), Box<dyn Error>> {
        // Arrange: the key travels in the query string and the provider fails.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("api_key", SENTINEL))
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

    #[tokio::test]
    async fn tmdb_sends_a_v4_token_as_a_bearer_credential() -> Result<(), Box<dyn Error>> {
        // Arrange: a dotted key is a read token.
        let server = MockServer::start().await;
        let token = "header.payload.signature";
        Mock::given(method("GET"))
            .and(path("/search/movie"))
            .and(header("Authorization", format!("Bearer {token}").as_str()))
            .and(query_param("query", "Inception"))
            .respond_with(ResponseTemplate::new(200).set_body_string(SEARCH_EMPTY))
            .expect(1)
            .mount(&server)
            .await;
        let provider = TmdbVideoMetadataProvider::with_endpoints(
            token.to_owned(),
            server.uri(),
            server.uri(),
            TEST_TIMEOUT,
        )
        .with_artwork_client(ArtworkClient::for_test("127.0.0.1"));
        let query = query(VideoKind::Movie, "Inception", None)?;

        // Act
        let candidates = provider.search(&query).await?;

        // Assert
        assert!(candidates.is_empty());
        Ok(())
    }
}
