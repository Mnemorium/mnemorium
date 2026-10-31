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
//! bounded in size and time and accepted only when the bytes sniff as an image.
//! The video adapters never follow a redirect; the music adapter's archive
//! addresses are followed, but only to a host already on its own allow-list. A
//! failed download yields no artwork and one catalogued event; it never fails
//! the lookup.

pub mod musicbrainz;
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

/// Number of redirect hops one archive download may take before it is abandoned.
const ARTWORK_MAX_REDIRECTS: usize = 3;

/// Time one metadata request may take.
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Client downloading provider artwork under the adapter's policy.
pub(crate) struct ArtworkClient {
    /// Hosts the client accepts; each adapter supplies its provider's own list.
    allowed_hosts: Vec<String>,
    /// Client that never follows a redirect and bounds one download's time.
    client: Client,
    /// Whether the address must use https.
    secure_only: bool,
}

impl ArtworkClient {
    /// Return whether `host` is one the client accepts.
    ///
    /// An entry admits the host itself and every subdomain of it, because the
    /// Cover Art Archive serves each image from a per-item CDN subdomain such as
    /// `dn710607.ca.archive.org`.
    pub(crate) fn accepts_host(&self, host: &str) -> bool {
        host_is_allowed(&self.allowed_hosts, host)
    }

    /// Read `response` as artwork under the size and content rules.
    async fn download(&self, provider: ProviderId, mut response: Response) -> Option<Artwork> {
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

    /// Download the artwork at `address` for `provider`.
    ///
    /// Returns `None` and emits one catalogued event when the address is not
    /// accepted, the download fails, or the content is too large or not an
    /// image. A redirect is never followed: the response itself must be the
    /// image.
    pub(crate) async fn fetch(&self, provider: ProviderId, address: &str) -> Option<Artwork> {
        let url = self.permitted_url(provider, address, false)?;
        let Ok(response) = self.client.get(url).send().await else {
            return Self::reject(provider, "transport");
        };
        self.download(provider, response).await
    }

    /// Download the artwork at `address`, following the provider's own hops.
    ///
    /// The Cover Art Archive names its images in plain http on its own host and
    /// redirects every download on to `archive.org`, so an allow-listed http
    /// address is upgraded to https before the request and each hop is re-checked
    /// against the allow-list. The same size, time and content rules apply to the
    /// bytes that finally arrive, and a hop out of the allow-list yields no
    /// artwork and one event.
    pub(crate) async fn fetch_redirected(
        &self,
        provider: ProviderId,
        address: &str,
    ) -> Option<Artwork> {
        let url = self.permitted_url(provider, address, true)?;
        // The hop grant belongs to the archive download alone, so the client that
        // follows it is built here and the shared `client` keeps refusing
        // redirects for every other adapter.
        let allowed = self.allowed_hosts.clone();
        let secure_only = self.secure_only;
        let client = Client::builder()
            .redirect(Policy::custom(move |attempt| {
                let target = attempt.url();
                let permitted = target.scheme() == "https" || !secure_only;
                let known = target
                    .host_str()
                    .is_some_and(|host| host_is_allowed(&allowed, host));
                if permitted && known && attempt.previous().len() < ARTWORK_MAX_REDIRECTS {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .timeout(ARTWORK_TIMEOUT)
            .build()
            .unwrap_or_default();
        let Ok(response) = client.get(url).send().await else {
            return Self::reject(provider, "transport");
        };
        if !self.accepts_host(response.url().host_str().unwrap_or_default()) {
            return Self::reject(provider, "host");
        }
        self.download(provider, response).await
    }

    /// Create a client that accepts one plain-http test host.
    ///
    /// The policy is otherwise unchanged, so a test still proves the host
    /// allow-list and the hop limit.
    #[cfg(test)]
    pub(crate) fn for_test(host: &str) -> Self {
        let client = Client::builder()
            .redirect(Policy::none())
            .timeout(ARTWORK_TIMEOUT)
            .build()
            .unwrap_or_default();
        Self {
            allowed_hosts: vec![host.to_owned()],
            client,
            secure_only: false,
        }
    }

    /// Create a client accepting `hosts` under the production policy.
    pub(crate) fn new(hosts: &[&str]) -> Self {
        let client = Client::builder()
            .redirect(Policy::none())
            .timeout(ARTWORK_TIMEOUT)
            .build()
            // A builder failure means the TLS backend cannot be initialised at
            // all, which the first request then reports as a classified fault.
            .unwrap_or_default();
        Self {
            allowed_hosts: hosts.iter().map(|host| (*host).to_owned()).collect(),
            client,
            secure_only: true,
        }
    }

    /// Validate `address` and return the URL to request.
    ///
    /// `upgrade` allows an allow-listed plain-http address to be re-schemed to
    /// https rather than refused, which is what the archive's index names.
    fn permitted_url(&self, provider: ProviderId, address: &str, upgrade: bool) -> Option<Url> {
        let Ok(mut url) = Url::parse(address) else {
            return Self::reject(provider, "address");
        };
        if upgrade
            && self.secure_only
            && url.scheme() == "http"
            && url.host_str().is_some_and(|host| self.accepts_host(host))
            && url.set_scheme("https").is_err()
        {
            return Self::reject(provider, "scheme");
        }
        if self.secure_only && url.scheme() != "https" {
            return Self::reject(provider, "scheme");
        }
        if !self.accepts_host(url.host_str().unwrap_or_default()) {
            return Self::reject(provider, "host");
        }
        Some(url)
    }

    /// Emit the artwork-failure event for `provider` and yield no artwork.
    fn reject<T>(provider: ProviderId, reason: &str) -> Option<T> {
        security_event::metadata_artwork_failed(provider.as_str(), reason);
        None
    }
}

/// Return whether `host` is one of `hosts` or a subdomain of one.
///
/// The comparison is case-insensitive and an entry must be the whole host or a
/// dot-delimited suffix, so `notarchive.org` is not an `archive.org` subdomain.
fn host_is_allowed(hosts: &[String], host: &str) -> bool {
    hosts.iter().any(|entry| {
        if host.eq_ignore_ascii_case(entry) {
            return true;
        }
        host.len()
            .checked_sub(entry.len())
            .filter(|start| *start > 0)
            .is_some_and(|start| {
                host.get(start..)
                    .is_some_and(|tail| tail.eq_ignore_ascii_case(entry))
                    && host
                        .get(..start)
                        .is_some_and(|prefix| prefix.ends_with('.'))
            })
    })
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
pub(crate) fn log_provider_fault(operation: &str, classification: &str) {
    error!(
        target: "security",
        event = "port_fault",
        kind = %classification,
        operation = %operation,
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
