//! Per-client rate limiting for the login endpoint.
//!
// TODO(system-test): the limiter is covered by unit and handler tests only. A
// system test that exercises both `behind_proxy` topologies and asserts the
// `429` plus `Retry-After` is still owed (see #156).
//
// TODO(rate-limit): `behind_proxy` trusts the forwarded headers unconditionally.
// Harden it to trust them only from configured proxy addresses before this is
// exposed to an untrusted network.

use std::sync::Arc;

use axum::Router;
use axum::response::IntoResponse as _;
use axum::response::Response;
use tower_governor::GovernorError;
use tower_governor::GovernorLayer;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::PeerIpKeyExtractor;
use tower_governor::key_extractor::SmartIpKeyExtractor;
use tracing::error;
use tracing::warn;

use crate::domain::model::rate_limit::RateLimit;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::app_state::AppState;

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

/// Apply the login rate limiter to `router`, returning the cleanup it needs.
///
/// The limiter is built when the router is built, so a change to the
/// rate-limit settings is a startup-only setting (`STY-RUST-082`). The client
/// key is the peer address by default; when the server sits behind a reverse
/// proxy (`behind_proxy`), the key is read from the forwarded headers.
///
/// A validated [`RateLimit`] cannot make the governor configuration invalid;
/// should it ever be, the limiter is skipped so the endpoint stays reachable.
pub fn apply_login_rate_limit(
    router: Router<AppState>,
    rate_limit: &RateLimit,
) -> (Router<AppState>, RateLimitCleanup) {
    if rate_limit.behind_proxy() {
        let mut builder = GovernorConfigBuilder::default().key_extractor(SmartIpKeyExtractor);
        builder
            .per_second(rate_limit.period_seconds())
            .burst_size(rate_limit.burst_size());
        let Some(config) = builder.finish() else {
            error!("the login rate limit configuration is invalid; rate limiting is disabled");
            return (router, RateLimitCleanup::new());
        };
        let limiter = Arc::clone(config.limiter());
        let cleanup = RateLimitCleanup::from_prune(Box::new(move || limiter.retain_recent()));
        let layer = GovernorLayer::new(config).error_handler(rate_limit_error);
        (router.route_layer(layer), cleanup)
    } else {
        let mut builder = GovernorConfigBuilder::default().key_extractor(PeerIpKeyExtractor);
        builder
            .per_second(rate_limit.period_seconds())
            .burst_size(rate_limit.burst_size());
        let Some(config) = builder.finish() else {
            error!("the login rate limit configuration is invalid; rate limiting is disabled");
            return (router, RateLimitCleanup::new());
        };
        let limiter = Arc::clone(config.limiter());
        let cleanup = RateLimitCleanup::from_prune(Box::new(move || limiter.retain_recent()));
        let layer = GovernorLayer::new(config).error_handler(rate_limit_error);
        (router.route_layer(layer), cleanup)
    }
}

/// Map a governor rejection onto the API error envelope.
///
/// A rejected attempt is a reviewable security event (`OBS-005`, `OBS-006`).
/// The client address is personal data and is never logged (`OBS-004`).
fn rate_limit_error(error: GovernorError) -> Response {
    match error {
        GovernorError::TooManyRequests { headers, .. } => {
            warn!(
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
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use axum::http::HeaderMap;
    use axum::http::HeaderValue;
    use axum::http::StatusCode;
    use tower_governor::GovernorError;

    use crate::test_helpers::error_message_of;

    use super::RateLimitCleanup;
    use super::rate_limit_error;

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
