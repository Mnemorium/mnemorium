//! One configuration layer, before the defaults are filled in.
//!
//! A [`PartialConfiguration`] mirrors [`Configuration`](super::configuration::Configuration)
//! with every leaf optional: a leaf that is `None` keeps the value of the layer
//! below, and a leaf that is `Some` overrides it. Both configuration loaders
//! produce this one document — the file loader from `config.yaml`, the
//! environment loader from the `MNEMORIUM_*` variables — and the load
//! configuration use case merges it over the layer below, so no adapter owns a
//! merge of its own.
//!
//! The fields are public because this is the document an adapter deserializes
//! into and the use case reads to merge, not a value object with an invariant:
//! validation stays in the domain constructors the merge calls.
//!
//! [`Configuration`]: super::configuration::Configuration

/// The settings one configuration layer names.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct PartialConfiguration {
    /// Asset-related settings named by the layer.
    pub asset: PartialAsset,
    /// Library-related settings named by the layer.
    pub library: PartialLibrary,
    /// Logging-related settings named by the layer.
    pub logging: PartialLogging,
    /// Persistence-related settings named by the layer.
    pub persistence: PartialPersistence,
    /// Security-related settings named by the layer.
    pub security: PartialSecurity,
}

/// Asset-related settings named by the layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct PartialAsset {
    /// Upload-related settings named by the layer.
    pub upload: PartialAssetUpload,
}

/// Upload-related settings named by the layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct PartialAssetUpload {
    /// Size of every chunk except the last, in bytes.
    pub chunk_size_bytes: Option<u64>,
    /// Lifetime of an upload session, in seconds.
    pub expiry_seconds: Option<u64>,
    /// Maximum size of a single uploaded file, in bytes.
    pub max_file_size_bytes: Option<u64>,
}

/// Library-related settings named by the layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct PartialLibrary {
    /// Metadata settings named by the layer.
    pub metadata: PartialLibraryMetadata,
}

/// Metadata settings named by the layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct PartialLibraryMetadata {
    /// Video metadata settings named by the layer.
    pub video: PartialVideoMetadata,
}

/// Video metadata settings named by the layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct PartialVideoMetadata {
    /// Configured providers, when the layer names the list.
    pub providers: Option<Vec<PartialVideoProvider>>,
}

/// One configured video metadata provider named by the layer.
///
/// The name is kept as a string so the domain constructor, not serde, rejects
/// an unknown provider and names the setting.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct PartialVideoProvider {
    /// API key the adapter presents, when the layer names one.
    pub api_key: Option<String>,
    /// Position in the failover order, when the layer names one.
    pub index: Option<u32>,
    /// Provider name, when the layer names one.
    pub name: Option<String>,
}

/// Logging-related settings named by the layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct PartialLogging {
    /// Whether to colour the standard-output sink with ANSI escape codes.
    pub ansi: Option<bool>,
    /// Verbosity filter applied to the `tracing` instrumentation.
    pub level: Option<String>,
    /// Maximum number of rotated log files to keep.
    pub max_files: Option<u32>,
    /// Rotation period of the file sink.
    pub rotation: Option<super::configuration::Rotation>,
}

/// Persistence-related settings named by the layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct PartialPersistence {
    /// `SQLite3` datastore settings named by the layer.
    pub sqlite3: PartialSqlite3,
}

/// `SQLite3` datastore settings named by the layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct PartialSqlite3 {
    /// Maximum number of connections to the database.
    pub max_connections: Option<u32>,
}

/// Security-related settings named by the layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct PartialSecurity {
    /// `JWT` settings named by the layer.
    pub jwt: PartialJwt,
    /// Whether the Root Admin default password is logged on runtime start.
    pub log_root_admin_password: Option<bool>,
    /// Login rate-limiting settings named by the layer.
    pub rate_limit: PartialRateLimit,
}

/// `JWT` settings named by the layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct PartialJwt {
    /// Lifetime of a token, in seconds.
    pub ttl: Option<u64>,
}

/// Login rate-limiting settings named by the layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct PartialRateLimit {
    /// Number of requests a client address may spend before throttling.
    pub burst_size: Option<u32>,
    /// Header the trusted proxy overwrites with the client address.
    pub client_ip_header: Option<String>,
    /// Seconds after which one element of the quota is replenished.
    pub period_seconds: Option<u64>,
    /// Comma-separated addresses of the trusted reverse proxies.
    pub trusted_proxies: Option<String>,
}
