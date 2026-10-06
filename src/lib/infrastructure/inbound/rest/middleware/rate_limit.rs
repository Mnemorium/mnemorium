//! Per-client rate limiting for the login endpoint.
//!
// TODO(system-test): the limiter is covered by unit and handler tests only. A
// system test that exercises both client-IP topologies and asserts the `429`
// plus `Retry-After` is still owed (see #156).

use std::net::IpAddr;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::extract::ConnectInfo;
use axum::http::HeaderMap;
use axum::http::Request;
use axum::response::IntoResponse as _;
use axum::response::Response;
use tower_governor::GovernorError;
use tower_governor::GovernorLayer;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::KeyExtractor;
use tower_governor::key_extractor::PeerIpKeyExtractor;
use tracing::error;
use tracing::warn;

use crate::domain::model::rate_limit::ClientIpHeader;
use crate::domain::model::rate_limit::RateLimit;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::app_state::AppState;

/// A [`KeyExtractor`] that trusts one declared client-address header from the
/// configured proxy addresses.
///
/// The key is the immediate peer address unless the peer is a trusted proxy. In
/// that case the client address comes from the single header the operator
/// declared the proxy overwrites (`client_ip_header`), walking a chain
/// right-to-left and skipping trusted hops. Because only that one header is
/// read, a client cannot influence the key by sending a different header
/// family: if the declared header is absent or unparseable, the peer address is
/// used.
#[derive(Debug, Clone)]
pub struct TrustedProxyKeyExtractor {
    /// Header the trusted proxy overwrites with the client address.
    header: ClientIpHeader,
    /// Addresses whose forwarded headers are trusted.
    trusted: Arc<[IpAddr]>,
}

impl TrustedProxyKeyExtractor {
    /// Return the client address for `request`, or `None` when no peer address
    /// is available.
    fn client_address<T>(&self, request: &Request<T>) -> Option<IpAddr> {
        let peer = request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|connect_info| connect_info.0.ip())?;
        if !self.trusted.contains(&peer) {
            return Some(peer);
        }
        Some(self.forwarded_address(request.headers()).unwrap_or(peer))
    }

    /// Resolve the client address from the declared header, walking the chain
    /// right-to-left and skipping trusted hops.
    fn forwarded_address(&self, headers: &HeaderMap) -> Option<IpAddr> {
        let chain = self.header_chain(headers)?;
        chain
            .into_iter()
            .rev()
            .find(|address| !self.trusted.contains(address))
    }

    /// Return the forwarded chain from the declared header only.
    fn header_chain(&self, headers: &HeaderMap) -> Option<Vec<IpAddr>> {
        let value = headers.get(self.header.as_str())?.to_str().ok()?;
        match self.header {
            ClientIpHeader::XForwardedFor => parse_address_list(value),
            ClientIpHeader::XRealIp => parse_address(value).map(|address| vec![address]),
            ClientIpHeader::Forwarded => parse_forwarded(value),
        }
    }

    /// Create a new extractor trusting `trusted` and reading `header`.
    #[must_use]
    pub fn new(header: ClientIpHeader, trusted: Vec<IpAddr>) -> Self {
        Self {
            header,
            trusted: trusted.into(),
        }
    }
}

impl KeyExtractor for TrustedProxyKeyExtractor {
    type Key = IpAddr;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        self.client_address(req)
            .ok_or(GovernorError::UnableToExtractKey)
    }
}

/// Periodic upkeep for the login rate limiter.
///
/// `governor` keeps one entry per client address and never reclaims it on its
/// own; an attacker rotating addresses would otherwise grow the map without
/// bound. Each prune closure drops the entries whose quota has fully
/// replenished. The closures are opaque so no `governor` type leaks into a
/// signature.
#[derive(Default)]
pub struct RateLimitCleanup {
    /// Per-limiter prune closures.
    prunes: Vec<Box<dyn Fn() + Send + Sync>>,
}

impl RateLimitCleanup {
    /// Build a cleanup from a single prune closure.
    #[cfg_attr(
        not(test),
        expect(
            clippy::single_call_fn,
            reason = "the single-limiter constructor keeps the prune list private"
        )
    )]
    fn from_prune(prune: Box<dyn Fn() + Send + Sync>) -> Self {
        Self {
            prunes: vec![prune],
        }
    }

    /// Fold `other`'s prune closures into `self`.
    pub fn merge(&mut self, other: Self) {
        self.prunes.extend(other.prunes);
    }

    /// Create an empty cleanup.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Run every registered prune.
    pub fn prune(&self) {
        for prune in &self.prunes {
            prune();
        }
    }
}

/// A process-lifetime login rate limiter and its periodic cleanup.
///
/// It is the stateful adapter the login route is wrapped with: the composition
/// root constructs it once (`STY-RUST-072`, `STY-RUST-073`) and hands it to the
/// router builder, which applies it to the `login` route. The `governor`
/// concrete types stay private to this module.
pub struct LoginRateLimiter {
    /// The wired governor layer, erased behind `apply`.
    apply: Box<dyn Fn(Router<AppState>) -> Router<AppState> + Send + Sync>,
    /// Periodic upkeep the composition root drives.
    cleanup: RateLimitCleanup,
}

impl LoginRateLimiter {
    /// Apply the limiter to the `login` sub-router.
    pub fn apply(&self, router: Router<AppState>) -> Router<AppState> {
        (self.apply)(router)
    }

    /// Build the login limiter from `rate_limit`.
    ///
    /// A validated [`RateLimit`] cannot make the governor configuration
    /// invalid; should it ever be, the limiter is skipped so the endpoint stays
    /// reachable.
    #[must_use]
    pub fn new(rate_limit: &RateLimit) -> Self {
        if rate_limit.trusted_proxies().is_empty() {
            build(rate_limit, PeerIpKeyExtractor)
        } else {
            build(
                rate_limit,
                TrustedProxyKeyExtractor::new(
                    rate_limit.client_ip_header(),
                    rate_limit.trusted_proxies().to_vec(),
                ),
            )
        }
    }

    /// Drive the periodic prune of the limiter's keyed store.
    pub fn prune(&self) {
        self.cleanup.prune();
    }
}

/// Build the governor layer for `extractor` and its prune closure.
fn build<K>(rate_limit: &RateLimit, extractor: K) -> LoginRateLimiter
where
    K: KeyExtractor + Send + Sync + 'static,
    K::Key: Send + Sync,
{
    let mut builder = GovernorConfigBuilder::default().key_extractor(extractor);
    builder
        .per_second(rate_limit.period_seconds())
        .burst_size(rate_limit.burst_size());
    let Some(config) = builder.finish() else {
        error!("the login rate limit configuration is invalid; rate limiting is disabled");
        return LoginRateLimiter {
            apply: Box::new(|router| router),
            cleanup: RateLimitCleanup::new(),
        };
    };
    let limiter = Arc::clone(config.limiter());
    let layer = GovernorLayer::new(config).error_handler(rate_limit_error);
    LoginRateLimiter {
        apply: Box::new(move |router| router.route_layer(layer.clone())),
        cleanup: RateLimitCleanup::from_prune(Box::new(move || limiter.retain_recent())),
    }
}

/// Parse a single address, trimming surrounding whitespace and brackets.
fn parse_address(value: &str) -> Option<IpAddr> {
    value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse()
        .ok()
}

/// Parse a comma-separated list of addresses, discarding unparseable entries.
#[expect(
    clippy::single_call_fn,
    reason = "the header parsing helpers are named for readability"
)]
fn parse_address_list(value: &str) -> Option<Vec<IpAddr>> {
    let addresses: Vec<IpAddr> = value.split(',').filter_map(parse_address).collect();
    (!addresses.is_empty()).then_some(addresses)
}

/// Parse the `for=` element of an RFC 7239 `Forwarded` header.
#[expect(
    clippy::single_call_fn,
    reason = "the RFC 7239 parser is named for readability"
)]
fn parse_forwarded(value: &str) -> Option<Vec<IpAddr>> {
    let addresses: Vec<IpAddr> = value
        .split(',')
        .flat_map(|element| element.split(';'))
        .filter_map(|parameter| parameter.split_once('='))
        .filter(|&(name, _value)| name.trim().eq_ignore_ascii_case("for"))
        .filter_map(|(_name, address)| parse_address(address))
        .collect();
    (!addresses.is_empty()).then_some(addresses)
}

/// Map a governor rejection onto the API error envelope.
///
/// A rejected attempt is a reviewable security event (`OBS-005`, `OBS-006`).
/// The client address is personal data and is never logged (`OBS-004`).
#[cfg_attr(
    not(test),
    expect(
        clippy::single_call_fn,
        reason = "the error mapping is named for readability"
    )
)]
fn rate_limit_error(error: GovernorError) -> Response {
    match error {
        GovernorError::TooManyRequests { headers, .. } => {
            warn!(
                target: "security",
                event = "rate_limit_exceeded",
                route = "identity.login",
                reason = "quota_exceeded",
                "rejected a rate-limited login attempt"
            );
            let mut response =
                ApiError::TooManyRequests("too many requests".to_owned()).into_response();
            if let Some(extra) = headers {
                response.headers_mut().extend(extra);
            }
            response
        }
        GovernorError::UnableToExtractKey => {
            error!("the login rate limiter could not extract a client address");
            ApiError::InternalServerError.into_response()
        }
        GovernorError::Other { .. } => ApiError::InternalServerError.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::net::IpAddr;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use axum::extract::ConnectInfo;
    use axum::http::HeaderMap;
    use axum::http::HeaderValue;
    use axum::http::Request;
    use axum::http::StatusCode;
    use tower_governor::GovernorError;
    use tower_governor::key_extractor::KeyExtractor as _;

    use crate::domain::model::rate_limit::ClientIpHeader;
    use crate::test_helpers::error_message_of;

    use super::RateLimitCleanup;
    use super::TrustedProxyKeyExtractor;
    use super::rate_limit_error;

    /// Build a request whose peer is `peer` and that carries `headers`.
    fn request(peer: &str, headers: &[(&str, &str)]) -> Result<Request<()>, Box<dyn Error>> {
        let mut builder = Request::builder().extension(ConnectInfo(peer.parse::<SocketAddr>()?));
        for &(name, value) in headers {
            builder = builder.header(name, value);
        }
        Ok(builder.body(())?)
    }

    #[test]
    fn untrusted_peer_ignores_the_forwarded_header() -> Result<(), Box<dyn Error>> {
        // Arrange
        let extractor =
            TrustedProxyKeyExtractor::new(ClientIpHeader::XForwardedFor, vec!["10.0.0.1".parse()?]);
        let request = request(
            "203.0.113.9:1234",
            &[("x-forwarded-for", "1.2.3.4, 5.6.7.8")],
        )?;

        // Act
        let key = extractor.extract(&request)?;

        // Assert: the peer wins, so a spoofed header cannot mint a bucket.
        assert_eq!(key, "203.0.113.9".parse::<IpAddr>()?);
        Ok(())
    }

    #[test]
    fn trusted_peer_uses_the_rightmost_untrusted_address() -> Result<(), Box<dyn Error>> {
        // Arrange
        let extractor =
            TrustedProxyKeyExtractor::new(ClientIpHeader::XForwardedFor, vec!["10.0.0.1".parse()?]);
        let request = request("10.0.0.1:1234", &[("x-forwarded-for", "1.2.3.4, 10.0.0.2")])?;

        // Act
        let key = extractor.extract(&request)?;

        // Assert: the rightmost non-trusted hop is the client.
        assert_eq!(key, "10.0.0.2".parse::<IpAddr>()?);
        Ok(())
    }

    #[test]
    fn trusted_peer_skips_a_chain_of_trusted_hops() -> Result<(), Box<dyn Error>> {
        // Arrange
        let extractor = TrustedProxyKeyExtractor::new(
            ClientIpHeader::XForwardedFor,
            vec!["10.0.0.1".parse()?, "10.0.0.2".parse()?],
        );
        let request = request(
            "10.0.0.1:1234",
            &[("x-forwarded-for", "203.0.113.7, 10.0.0.2")],
        )?;

        // Act
        let key = extractor.extract(&request)?;

        // Assert
        assert_eq!(key, "203.0.113.7".parse::<IpAddr>()?);
        Ok(())
    }

    #[test]
    fn declared_x_real_ip_wins_over_a_client_supplied_x_forwarded_for() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the proxy sets X-Real-IP; the client tries to smuggle an XFF.
        let extractor =
            TrustedProxyKeyExtractor::new(ClientIpHeader::XRealIp, vec!["10.0.0.1".parse()?]);
        let request = request(
            "10.0.0.1:1234",
            &[("x-real-ip", "203.0.113.5"), ("x-forwarded-for", "1.1.1.1")],
        )?;

        // Act
        let key = extractor.extract(&request)?;

        // Assert: only the declared header is read.
        assert_eq!(key, "203.0.113.5".parse::<IpAddr>()?);
        Ok(())
    }

    #[test]
    fn declared_x_forwarded_for_ignores_a_client_supplied_x_real_ip() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the proxy sets X-Forwarded-For; the client smuggles X-Real-IP.
        let extractor =
            TrustedProxyKeyExtractor::new(ClientIpHeader::XForwardedFor, vec!["10.0.0.1".parse()?]);
        let request = request(
            "10.0.0.1:1234",
            &[("x-forwarded-for", "203.0.113.6"), ("x-real-ip", "1.1.1.1")],
        )?;

        // Act
        let key = extractor.extract(&request)?;

        // Assert
        assert_eq!(key, "203.0.113.6".parse::<IpAddr>()?);
        Ok(())
    }

    #[test]
    fn declared_forwarded_ignores_the_other_headers() -> Result<(), Box<dyn Error>> {
        // Arrange
        let extractor =
            TrustedProxyKeyExtractor::new(ClientIpHeader::Forwarded, vec!["10.0.0.1".parse()?]);
        let request = request(
            "10.0.0.1:1234",
            &[
                ("forwarded", "for=203.0.113.8;proto=https"),
                ("x-forwarded-for", "1.1.1.1"),
                ("x-real-ip", "2.2.2.2"),
            ],
        )?;

        // Act
        let key = extractor.extract(&request)?;

        // Assert
        assert_eq!(key, "203.0.113.8".parse::<IpAddr>()?);
        Ok(())
    }

    #[test]
    fn trusted_peer_without_the_declared_header_falls_back_to_the_peer()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the declared header is absent, but another family is present.
        let extractor =
            TrustedProxyKeyExtractor::new(ClientIpHeader::XRealIp, vec!["10.0.0.1".parse()?]);
        let request = request("10.0.0.1:1234", &[("x-forwarded-for", "1.1.1.1")])?;

        // Act
        let key = extractor.extract(&request)?;

        // Assert: the undeclared header is never consulted.
        assert_eq!(key, "10.0.0.1".parse::<IpAddr>()?);
        Ok(())
    }

    #[test]
    fn missing_peer_address_is_unable_to_extract() -> Result<(), Box<dyn Error>> {
        // Arrange
        let extractor =
            TrustedProxyKeyExtractor::new(ClientIpHeader::XForwardedFor, vec!["10.0.0.1".parse()?]);
        let request = Request::builder().body(())?;

        // Act
        let result = extractor.extract(&request);

        // Assert
        assert!(matches!(result, Err(GovernorError::UnableToExtractKey)));
        Ok(())
    }

    #[test]
    fn prune_runs_every_registered_closure() {
        // Arrange
        let calls = Arc::new(AtomicUsize::new(0));
        let first = Arc::clone(&calls);
        let second = Arc::clone(&calls);
        let mut cleanup = RateLimitCleanup::from_prune(Box::new(move || {
            first.fetch_add(1, Ordering::SeqCst);
        }));
        let other = RateLimitCleanup::from_prune(Box::new(move || {
            second.fetch_add(1, Ordering::SeqCst);
        }));
        cleanup.merge(other);

        // Act
        cleanup.prune();

        // Assert
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn too_many_requests_maps_to_429_and_keeps_retry_after() {
        // Arrange
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", HeaderValue::from_static("7"));
        let error = GovernorError::TooManyRequests {
            wait_time: 7,
            headers: Some(headers),
        };

        // Act
        let response = rate_limit_error(error);

        // Assert
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response
                .headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok()),
            Some("7")
        );
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("too many requests")
        );
    }

    #[test]
    fn unable_to_extract_key_maps_to_500() {
        // Act
        let response = rate_limit_error(GovernorError::UnableToExtractKey);

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
