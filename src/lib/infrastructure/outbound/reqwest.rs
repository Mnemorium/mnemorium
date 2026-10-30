//! Shared pieces of the HTTP metadata adapters.
//!
//! Both adapters classify every failure at this boundary: the raw `reqwest`
//! error, whose `Display` can carry the request address and therefore an `OMDb`
//! query key, is never propagated or logged. A failure is reported as a
//! value-less [`ExternalServiceError`] and logged once as the catalogued
//! `port_fault` with its stable classification (`OBS-002`, `OBS-003`).
//!
//! Artwork is untrusted content fetched from a provider-supplied address, so
//! the download is https-only, restricted to the provider's own image hosts,
//! bounded in size and time, never follows a redirect and is accepted only when
//! the bytes sniff as an image. A failed download yields no artwork and one
//! catalogued event; it never fails the lookup.

pub mod omdb;
pub mod tmdb;

use std::time::Duration;

use reqwest::Client;
use reqwest::Response;
use reqwest::StatusCode;
use reqwest::Url;
use reqwest::redirect::Policy;
use serde::de::DeserializeOwned;
use tracing::error;

use crate::application::security_event;
use crate::domain::model::artwork::Artwork;
use crate::domain::model::media_candidate::ProviderId;
use crate::domain::port::error::ExternalServiceError;

/// Largest artwork the server accepts, in bytes.
pub(crate) const ARTWORK_MAX_BYTES: usize = 5 * 1024 * 1024;

/// Maximum time one artwork download may take.
const ARTWORK_TIMEOUT: Duration = Duration::from_secs(10);

/// Hosts `OMDb` serves poster artwork from.
const OMDB_ARTWORK_HOSTS: &[&str] = &["ia.media-imdb.com", "m.media-amazon.com"];

/// Security-event operation name owned by both metadata adapters.
const OPERATION: &str = "video_metadata";

/// Hosts TMDB serves poster artwork from.
const TMDB_ARTWORK_HOSTS: &[&str] = &["image.tmdb.org"];

/// Client downloading provider artwork under the adapter's policy.
pub(crate) struct ArtworkClient {
    /// Hosts the client accepts, when the default per-provider list is not used.
    allowed_hosts: Option<Vec<String>>,
    /// Client that never follows a redirect and bounds one download's time.
    client: Client,
    /// Whether the address must use https.
    secure_only: bool,
}

impl ArtworkClient {
    /// Download the artwork at `address` for `provider`.
    ///
    /// Returns `None` and emits one catalogued event when the address is not
    /// accepted, the download fails, or the content is too large or not an
    /// image.
    pub(crate) async fn fetch(&self, provider: ProviderId, address: &str) -> Option<Artwork> {
        let Ok(url) = Url::parse(address) else {
            return Self::reject(provider, "address");
        };
        if self.secure_only && url.scheme() != "https" {
            return Self::reject(provider, "scheme");
        }
        let host = url.host_str().unwrap_or_default();
        let permitted = match self.allowed_hosts.as_deref() {
            Some(hosts) => hosts
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(host)),
            None => match provider {
                ProviderId::Tmdb => TMDB_ARTWORK_HOSTS,
                ProviderId::Omdb => OMDB_ARTWORK_HOSTS,
            }
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(host)),
        };
        if !permitted {
            return Self::reject(provider, "host");
        }

        let Ok(mut response) = self.client.get(url).send().await else {
            return Self::reject(provider, "transport");
        };
        if !response.status().is_success() {
            return Self::reject(provider, "status");
        }
        let mut bytes: Vec<u8> = Vec::new();
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    if bytes.len().saturating_add(chunk.len()) > ARTWORK_MAX_BYTES {
                        return Self::reject(provider, "size");
                    }
                    bytes.extend_from_slice(&chunk);
                }
                Ok(None) => break,
                Err(_) => return Self::reject(provider, "transport"),
            }
        }
        let Some(kind) = infer::get(&bytes) else {
            return Self::reject(provider, "content_type");
        };
        let media_type = kind.mime_type().to_owned();
        if !media_type.starts_with("image/") {
            return Self::reject(provider, "content_type");
        }
        Some(Artwork::new(bytes, media_type))
    }

    /// Create a client that accepts one plain-http test host.
    ///
    /// The policy is otherwise unchanged, so a test still proves the host
    /// allow-list and the redirect refusal.
    #[cfg(test)]
    pub(crate) fn for_test(host: &str) -> Self {
        let client = Client::builder()
            .redirect(Policy::none())
            .timeout(ARTWORK_TIMEOUT)
            .build()
            .unwrap_or_default();
        Self {
            allowed_hosts: Some(vec![host.to_owned()]),
            client,
            secure_only: false,
        }
    }

    /// Create a client with the production policy.
    pub(crate) fn new() -> Self {
        let client = Client::builder()
            .redirect(Policy::none())
            .timeout(ARTWORK_TIMEOUT)
            .build()
            // A builder failure means the TLS backend cannot be initialised at
            // all, which the first request then reports as a classified fault.
            .unwrap_or_default();
        Self {
            allowed_hosts: None,
            client,
            secure_only: true,
        }
    }

    /// Emit the artwork-failure event for `provider` and yield no artwork.
    fn reject(provider: ProviderId, reason: &str) -> Option<Artwork> {
        security_event::metadata_artwork_failed(provider.as_str(), reason);
        None
    }
}

/// Classify a non-success response status onto the port's error family.
///
/// # Errors
///
/// Returns the [`ExternalServiceError`] the status means when it is not a
/// success.
pub(crate) fn classify_status(status: StatusCode) -> Result<(), ExternalServiceError> {
    if status.is_success() {
        return Ok(());
    }
    let error = match status.as_u16() {
        400 | 422 => ExternalServiceError::InvalidRequest,
        401 => ExternalServiceError::Unauthorized,
        403 => ExternalServiceError::Forbidden,
        408 => ExternalServiceError::Timeout,
        429 => ExternalServiceError::RateLimited,
        500..=599 => ExternalServiceError::DependencyFailure,
        _ => ExternalServiceError::Unknown(anyhow::anyhow!("the provider answered unexpectedly")),
    };
    Err(error)
}

/// Classify a `reqwest` failure without carrying any of its detail.
pub(crate) fn classify_transport_error(failure: &reqwest::Error) -> ExternalServiceError {
    if failure.is_timeout() {
        ExternalServiceError::Timeout
    } else if failure.is_connect() {
        ExternalServiceError::Unavailable
    } else if failure.is_request() {
        ExternalServiceError::CommunicationFailure
    } else if failure.is_decode() {
        ExternalServiceError::DeserializationFailure
    } else {
        ExternalServiceError::Unknown(anyhow::anyhow!("the provider request failed"))
    }
}

/// Read and decode one provider payload.
///
/// The body is read before it is decoded, so a transport failure is classified
/// like any other and a malformed body is a deserialization failure.
///
/// # Errors
///
/// Returns the classified [`ExternalServiceError`] when the body cannot be read
/// or does not decode into `T`.
pub(crate) async fn read_payload<T: DeserializeOwned>(
    response: Response,
) -> Result<T, ExternalServiceError> {
    let bytes = response
        .bytes()
        .await
        .map_err(|failure| classify_transport_error(&failure))?;
    serde_json::from_slice(&bytes).map_err(|_| ExternalServiceError::DeserializationFailure)
}

/// Build a provider address from a base, a path and the query pairs.
///
/// The pairs carry the search terms and, for a provider that authenticates by
/// query string, the API key; the built address is never logged or returned in
/// an error.
pub(crate) fn request_url(base: &str, path: &str, pairs: &[(&str, &str)]) -> Option<Url> {
    let mut url = Url::parse(&format!("{base}{path}")).ok()?;
    {
        let mut serializer = url.query_pairs_mut();
        for (name, value) in pairs.iter().copied() {
            serializer.append_pair(name, value);
        }
    }
    Some(url)
}

/// Emit the catalogued `port_fault` (`error`) for one classified provider
/// failure (`OBS-002`, `OBS-005`, `OBS-006`).
pub(crate) fn log_provider_fault(classification: &str) {
    error!(
        target: "security",
        event = "port_fault",
        kind = %classification,
        operation = OPERATION,
        "an outbound dependency failed"
    );
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use reqwest::StatusCode;
    use rstest::rstest;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;

    use super::classify_status;
    use super::read_payload;
    use super::request_url;

    /// The golden payload the stub serves.
    const MOVIE: &str = include_str!("reqwest/fixtures/omdb/movie.json");

    /// The malformed body the stub serves.
    const MALFORMED: &[u8] = include_bytes!("reqwest/fixtures/malformed.json");

    #[tokio::test]
    async fn read_payload_decodes_a_stubbed_response() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(MOVIE))
            .mount(&server)
            .await;
        let response = reqwest::Client::new().get(server.uri()).send().await?;

        // Act
        let payload = read_payload::<serde_json::Value>(response).await?;

        // Assert
        assert_eq!(
            payload.get("Title").and_then(serde_json::Value::as_str),
            Some("Inception")
        );
        Ok(())
    }

    #[tokio::test]
    async fn read_payload_classifies_a_malformed_body() -> Result<(), Box<dyn Error>> {
        // Arrange: the stub serves a body that is not JSON.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(MALFORMED.to_vec()))
            .mount(&server)
            .await;
        let response = reqwest::Client::new().get(server.uri()).send().await?;

        // Act
        let failure = read_payload::<serde_json::Value>(response)
            .await
            .err()
            .ok_or("a malformed body must be an error")?;

        // Assert
        assert_eq!(failure.classification(), "deserialization_failure");
        Ok(())
    }

    #[rstest]
    #[case::success(StatusCode::OK, None)]
    #[case::unauthorized(StatusCode::UNAUTHORIZED, Some("unauthorized"))]
    #[case::forbidden(StatusCode::FORBIDDEN, Some("forbidden"))]
    #[case::invalid(StatusCode::BAD_REQUEST, Some("invalid_request"))]
    #[case::timeout(StatusCode::REQUEST_TIMEOUT, Some("timeout"))]
    #[case::rate_limited(StatusCode::TOO_MANY_REQUESTS, Some("rate_limited"))]
    #[case::dependency(StatusCode::SERVICE_UNAVAILABLE, Some("dependency_failure"))]
    fn classify_status_maps_each_status(
        #[case] status: StatusCode,
        #[case] expected: Option<&str>,
    ) {
        // Act
        let classified = classify_status(status);

        // Assert
        assert_eq!(
            classified.err().map(|error| error.classification()),
            expected
        );
    }

    #[test]
    fn request_url_appends_the_query_pairs() -> Result<(), Box<dyn Error>> {
        // Act
        let url = request_url(
            "https://example.test/3",
            "/search/movie",
            &[("query", "Inception"), ("api_key", "sentinel")],
        )
        .ok_or("the address must parse")?;

        // Assert
        assert_eq!(url.path(), "/3/search/movie");
        let pairs: Vec<(String, String)> = url
            .query_pairs()
            .map(|(name, value)| (name.into_owned(), value.into_owned()))
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("query".to_owned(), "Inception".to_owned()),
                ("api_key".to_owned(), "sentinel".to_owned()),
            ]
        );
        Ok(())
    }
}
