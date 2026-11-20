/// The inbound adapters: the REST surface and the media-ingest runner.
pub(crate) mod inbound;
/// The tracing subscriber and its configuration.
mod logging;
/// The outbound adapters: datastore, file system, providers and generators.
pub(crate) mod outbound;
/// Periodic cleanup shared by the login rate limiters.
mod rate_limit;
/// The test-only seed data; its contents are gated on the `test-seed` feature.
mod seed;
/// The runtime use-case factories the inbound adapters resolve through.
pub(crate) mod use_case_factory;

pub(crate) use self::logging::setup;
pub(crate) use self::rate_limit::LoginAttemptLimit;
#[cfg(feature = "test-seed")]
pub(crate) use self::seed::apply;
