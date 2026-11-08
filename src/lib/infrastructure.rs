/// The inbound adapters: the REST surface and the media-ingest runner.
pub(crate) mod inbound;
/// The tracing subscriber and its configuration.
pub(crate) mod logging;
/// The outbound adapters: datastore, file system, providers and generators.
pub(crate) mod outbound;
/// The test-only seed data; its contents are gated on the `test-seed` feature.
pub(crate) mod seed;
/// The runtime use-case factories the inbound adapters resolve through.
pub(crate) mod use_case_factory;
