use std::net::IpAddr;

use std::str::FromStr as _;

/// Default root directory holding the staged and final files.
pub const DEFAULT_STORAGE_ROOT: &str = "data";

/// Default size of every chunk except the last, in bytes.
pub const DEFAULT_CHUNK_SIZE_BYTES: u64 = 5_242_880;

/// Default lifetime of an upload session, in seconds.
pub const DEFAULT_EXPIRY_SECONDS: u64 = 86_400;

/// Default maximum size of a single uploaded file, in bytes (100 GiB).
pub const DEFAULT_MAX_FILE_SIZE_BYTES: u64 = 107_374_182_400;

/// Reserved target carrying security events; a verbosity directive must never
/// target it, or it could suppress a catalog event (`OBS-007`).
const SECURITY_TARGET: &str = "security";

/// Default verbosity filter applied to the `tracing` instrumentation.
pub const DEFAULT_LOG_LEVEL: &str = "debug,sqlx=warn";

/// Default value for [`Logging::ansi`].
pub const DEFAULT_LOG_ANSI: bool = false;

/// Default value for [`Logging::max_files`].
pub const DEFAULT_LOG_MAX_FILES: u32 = 7;

/// Default value for [`Logging::rotation`].
pub const DEFAULT_LOG_ROTATION: Rotation = Rotation::Daily;

/// Default value for [`Jwt::ttl`], in seconds.
pub const DEFAULT_JWT_TTL: u64 = 3600;

/// Default value for [`Sqlite3::max_connections`].
///
/// Fixed rather than derived from the host: the value must be deterministic so
/// the domain performs no OS probe, and `SQLite` serializes writers regardless.
pub const DEFAULT_SQLITE3_MAX_CONNECTIONS: u32 = 8;

/// Default value for [`RateLimit::burst_size`].
const DEFAULT_BURST_SIZE: u32 = 5;

/// Default header carrying the client address, used when no proxy is trusted.
const DEFAULT_CLIENT_IP_HEADER: ClientIpHeader = ClientIpHeader::XForwardedFor;

/// Default value for [`RateLimit::period_seconds`].
const DEFAULT_PERIOD_SECONDS: u64 = 12;

/// Separator between addresses in the [`RateLimit::trusted_proxies`] list.
const TRUSTED_PROXIES_SEPARATOR: char = ',';

/// Error returned when initialising or updating an asset value object.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AssetError {
    /// The chunk size is zero.
    #[error("asset upload chunk_size_bytes must be greater than zero")]
    ChunkSizeZero,
    /// The upload expiry is zero.
    #[error("asset upload expiry_seconds must be greater than zero")]
    ExpiryZero,
    /// The maximum file size does not fit in a signed 64-bit integer.
    #[error("asset upload max_file_size_bytes must fit in a signed 64-bit integer")]
    MaxFileSizeTooLarge,
    /// The maximum file size is zero.
    #[error("asset upload max_file_size_bytes must be greater than zero")]
    MaxFileSizeZero,
    /// The storage root is empty.
    #[error("asset storage root must not be empty")]
    RootEmpty,
}

/// Storage-related settings of the Asset bounded context.
///
/// Deserialization is routed through `AssetStorageConfig` and [`TryFrom`] so
/// that the layered configuration cannot bypass the validation performed by
/// [`AssetStorage::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "AssetStorageConfig")]
#[non_exhaustive]
pub struct AssetStorage {
    /// Root directory holding the staged and final files.
    root: String,
}

/// Unchecked deserialization mirror of [`AssetStorage`].
///
/// Serde builds this snapshot and hands it to the [`TryFrom`] implementation,
/// which validates it through [`AssetStorage::try_new`].
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetStorageConfig {
    /// Root directory holding the staged and final files.
    #[serde(default = "default_storage_root")]
    root: String,
}

impl TryFrom<AssetStorageConfig> for AssetStorage {
    type Error = AssetError;

    fn try_from(config: AssetStorageConfig) -> Result<Self, Self::Error> {
        Self::try_new(config.root)
    }
}

impl AssetStorage {
    /// Return the root directory holding the staged and final files.
    #[must_use]
    pub fn root(&self) -> &str {
        &self.root
    }

    /// Update the root directory holding the staged and final files.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::RootEmpty`] when `root` is empty.
    pub fn set_root(&mut self, root: String) -> Result<(), AssetError> {
        self.root = Self::validate_root(root)?;
        Ok(())
    }

    /// Initialise a new `AssetStorage`, validating `root` is non-empty.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::RootEmpty`] when `root` is empty.
    pub fn try_new(root: String) -> Result<Self, AssetError> {
        let validated_root = Self::validate_root(root)?;
        Ok(Self {
            root: validated_root,
        })
    }

    /// Validate `root`.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::RootEmpty`] when `root` is empty.
    fn validate_root(root: String) -> Result<String, AssetError> {
        if root.is_empty() {
            return Err(AssetError::RootEmpty);
        }
        Ok(root)
    }
}

impl Default for AssetStorage {
    fn default() -> Self {
        Self {
            root: DEFAULT_STORAGE_ROOT.to_owned(),
        }
    }
}

/// Upload-related settings of the Asset bounded context.
///
/// Deserialization is routed through `AssetUploadConfig` and [`TryFrom`] so
/// that the layered configuration cannot bypass the validation performed by
/// [`AssetUpload::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "AssetUploadConfig")]
#[non_exhaustive]
pub struct AssetUpload {
    /// Size of every chunk except the last, in bytes.
    chunk_size_bytes: u64,
    /// Lifetime of an upload session, in seconds.
    expiry_seconds: u64,
    /// Maximum size of a single uploaded file, in bytes.
    max_file_size_bytes: u64,
}

/// Unchecked deserialization mirror of [`AssetUpload`].
///
/// Serde builds this snapshot and hands it to the [`TryFrom`] implementation,
/// which validates it through [`AssetUpload::try_new`].
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetUploadConfig {
    /// Size of every chunk except the last, in bytes.
    #[serde(default = "default_chunk_size_bytes")]
    chunk_size_bytes: u64,
    /// Lifetime of an upload session, in seconds.
    #[serde(default = "default_expiry_seconds")]
    expiry_seconds: u64,
    /// Maximum size of a single uploaded file, in bytes.
    #[serde(default = "default_max_file_size_bytes")]
    max_file_size_bytes: u64,
}

impl TryFrom<AssetUploadConfig> for AssetUpload {
    type Error = AssetError;

    fn try_from(config: AssetUploadConfig) -> Result<Self, Self::Error> {
        Self::try_new(
            config.chunk_size_bytes,
            config.expiry_seconds,
            config.max_file_size_bytes,
        )
    }
}

impl AssetUpload {
    /// Return the size of every chunk except the last, in bytes.
    #[must_use]
    pub fn chunk_size_bytes(&self) -> u64 {
        self.chunk_size_bytes
    }

    /// Return the lifetime of an upload session, in seconds.
    #[must_use]
    pub fn expiry_seconds(&self) -> u64 {
        self.expiry_seconds
    }

    /// Return the maximum size of a single uploaded file, in bytes.
    #[must_use]
    pub fn max_file_size_bytes(&self) -> u64 {
        self.max_file_size_bytes
    }

    /// Update the size of every chunk except the last, in bytes.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::ChunkSizeZero`] when `chunk_size_bytes` is zero.
    pub fn set_chunk_size_bytes(&mut self, chunk_size_bytes: u64) -> Result<(), AssetError> {
        self.chunk_size_bytes = Self::validate_chunk_size_bytes(chunk_size_bytes)?;
        Ok(())
    }

    /// Update the lifetime of an upload session, in seconds.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::ExpiryZero`] when `expiry_seconds` is zero.
    pub fn set_expiry_seconds(&mut self, expiry_seconds: u64) -> Result<(), AssetError> {
        self.expiry_seconds = Self::validate_expiry_seconds(expiry_seconds)?;
        Ok(())
    }

    /// Update the maximum size of a single uploaded file, in bytes.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::MaxFileSizeZero`] when `max_file_size_bytes` is
    /// zero, and [`AssetError::MaxFileSizeTooLarge`] when it exceeds
    /// [`i64::MAX`].
    pub fn set_max_file_size_bytes(&mut self, max_file_size_bytes: u64) -> Result<(), AssetError> {
        self.max_file_size_bytes = Self::validate_max_file_size_bytes(max_file_size_bytes)?;
        Ok(())
    }

    /// Initialise a new `AssetUpload`, validating every setting.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::ChunkSizeZero`] when `chunk_size_bytes` is zero,
    /// [`AssetError::ExpiryZero`] when `expiry_seconds` is zero,
    /// [`AssetError::MaxFileSizeZero`] when `max_file_size_bytes` is zero, and
    /// [`AssetError::MaxFileSizeTooLarge`] when it exceeds [`i64::MAX`].
    pub fn try_new(
        chunk_size_bytes: u64,
        expiry_seconds: u64,
        max_file_size_bytes: u64,
    ) -> Result<Self, AssetError> {
        let validated_chunk_size = Self::validate_chunk_size_bytes(chunk_size_bytes)?;
        let validated_expiry = Self::validate_expiry_seconds(expiry_seconds)?;
        let validated_max_file_size = Self::validate_max_file_size_bytes(max_file_size_bytes)?;
        Ok(Self {
            chunk_size_bytes: validated_chunk_size,
            expiry_seconds: validated_expiry,
            max_file_size_bytes: validated_max_file_size,
        })
    }

    /// Validate `chunk_size_bytes`.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::ChunkSizeZero`] when `chunk_size_bytes` is zero.
    fn validate_chunk_size_bytes(chunk_size_bytes: u64) -> Result<u64, AssetError> {
        if chunk_size_bytes == 0 {
            return Err(AssetError::ChunkSizeZero);
        }
        Ok(chunk_size_bytes)
    }

    /// Validate `expiry_seconds`.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::ExpiryZero`] when `expiry_seconds` is zero.
    fn validate_expiry_seconds(expiry_seconds: u64) -> Result<u64, AssetError> {
        if expiry_seconds == 0 {
            return Err(AssetError::ExpiryZero);
        }
        Ok(expiry_seconds)
    }

    /// Validate `max_file_size_bytes`.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::MaxFileSizeZero`] when `max_file_size_bytes` is
    /// zero, and [`AssetError::MaxFileSizeTooLarge`] when it exceeds
    /// [`i64::MAX`].
    fn validate_max_file_size_bytes(max_file_size_bytes: u64) -> Result<u64, AssetError> {
        if max_file_size_bytes == 0 {
            return Err(AssetError::MaxFileSizeZero);
        }
        if max_file_size_bytes > i64::MAX as u64 {
            return Err(AssetError::MaxFileSizeTooLarge);
        }
        Ok(max_file_size_bytes)
    }
}

impl Default for AssetUpload {
    fn default() -> Self {
        Self {
            chunk_size_bytes: DEFAULT_CHUNK_SIZE_BYTES,
            expiry_seconds: DEFAULT_EXPIRY_SECONDS,
            max_file_size_bytes: DEFAULT_MAX_FILE_SIZE_BYTES,
        }
    }
}

/// Asset-related settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct Asset {
    /// Storage settings.
    storage: AssetStorage,
    /// Upload settings.
    upload: AssetUpload,
}

impl Asset {
    /// Initialise a new `Asset`.
    #[must_use]
    pub fn new(storage: AssetStorage, upload: AssetUpload) -> Self {
        Self { storage, upload }
    }

    /// Return the storage settings.
    #[must_use]
    pub fn storage(&self) -> &AssetStorage {
        &self.storage
    }

    /// Return the upload settings.
    #[must_use]
    pub fn upload(&self) -> &AssetUpload {
        &self.upload
    }
}

/// Error returned when initialising or updating a `Logging` value object.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LoggingError {
    /// The verbosity filter disables logging as a whole (`off`/`none`).
    #[error("logging level must not disable logging")]
    LevelDisablesLogging,
    /// The verbosity filter directives are empty.
    #[error("logging level must not be empty")]
    LevelEmpty,
    /// The verbosity filter targets the reserved `security` target.
    #[error("logging level must not target the reserved security target")]
    LevelTargetsSecurity,
}

/// Rotation period of the log files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "UPPERCASE")]
#[non_exhaustive]
pub enum Rotation {
    /// Rotate the log file once a day.
    Daily,
    /// Rotate the log file once an hour.
    Hourly,
    /// Rotate the log file once a minute.
    Minutely,
    /// Never rotate the log file.
    Never,
}

/// Logging settings.
///
/// Deserialization is routed through `LoggingConfig` and [`TryFrom`] so that the
/// layered configuration cannot bypass the validation performed by
/// [`Logging::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "LoggingConfig")]
#[non_exhaustive]
pub struct Logging {
    /// Whether to colour the standard-output sink with ANSI escape codes.
    ansi: bool,
    /// Verbosity filter applied to the `tracing` instrumentation.
    level: String,
    /// Maximum number of rotated log files to keep; `0` keeps every file.
    max_files: u32,
    /// Rotation period of the file sink.
    rotation: Rotation,
}

/// Unchecked deserialization mirror of [`Logging`].
///
/// Serde builds this snapshot and hands it to the [`TryFrom`] implementation,
/// which validates it through [`Logging::try_new`].
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LoggingConfig {
    /// Whether to colour the standard-output sink with ANSI escape codes.
    #[serde(default = "default_log_ansi")]
    ansi: bool,
    /// Verbosity filter applied to the `tracing` instrumentation.
    #[serde(default = "default_log_level")]
    level: String,
    /// Maximum number of rotated log files to keep; `0` keeps every file.
    #[serde(default = "default_log_max_files")]
    max_files: u32,
    /// Rotation period of the file sink.
    #[serde(default = "default_log_rotation")]
    rotation: Rotation,
}

impl TryFrom<LoggingConfig> for Logging {
    type Error = LoggingError;

    fn try_from(config: LoggingConfig) -> Result<Self, Self::Error> {
        Self::try_new(config.ansi, config.level, config.max_files, config.rotation)
    }
}

impl Default for Logging {
    fn default() -> Self {
        Self {
            ansi: DEFAULT_LOG_ANSI,
            level: DEFAULT_LOG_LEVEL.to_owned(),
            max_files: DEFAULT_LOG_MAX_FILES,
            rotation: DEFAULT_LOG_ROTATION,
        }
    }
}

impl Logging {
    /// Return whether to colour the standard-output sink with ANSI escape
    /// codes.
    #[must_use]
    pub fn ansi(&self) -> bool {
        self.ansi
    }

    /// Return the verbosity filter applied to the `tracing` instrumentation.
    #[must_use]
    pub fn level(&self) -> &str {
        &self.level
    }

    /// Return the maximum number of rotated log files to keep; `0` keeps every
    /// file.
    #[must_use]
    pub fn max_files(&self) -> u32 {
        self.max_files
    }

    /// Return the rotation period of the file sink.
    #[must_use]
    pub fn rotation(&self) -> Rotation {
        self.rotation
    }

    /// Initialise a new `Logging`, validating `level`.
    ///
    /// The level is rejected when it is empty, when it disables logging as a
    /// whole (`off`/`none`), or when it carries a directive for the reserved
    /// `security` target; either would let `logging.level` suppress a catalog
    /// event (`OBS-007`).
    ///
    /// # Errors
    ///
    /// Returns [`LoggingError::LevelEmpty`] when `level` is empty,
    /// [`LoggingError::LevelDisablesLogging`] when `level` is a bare `off` or
    /// `none`, and [`LoggingError::LevelTargetsSecurity`] when a directive
    /// targets the reserved `security` target.
    pub fn try_new(
        ansi: bool,
        level: String,
        max_files: u32,
        rotation: Rotation,
    ) -> Result<Self, LoggingError> {
        if level.is_empty() {
            return Err(LoggingError::LevelEmpty);
        }
        let directives = level.trim();
        if matches!(directives.to_ascii_lowercase().as_str(), "off" | "none") {
            return Err(LoggingError::LevelDisablesLogging);
        }
        if directives
            .split(',')
            .any(|directive| directive.trim().starts_with(SECURITY_TARGET))
        {
            return Err(LoggingError::LevelTargetsSecurity);
        }
        Ok(Self {
            ansi,
            level,
            max_files,
            rotation,
        })
    }
}

/// Error returned when initialising or updating a `Sqlite3` value object.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Sqlite3Error {
    /// The maximum number of connections is zero.
    #[error("sqlite3 max_connections must be greater than zero")]
    InvalidMaxConnections,
}

/// `SQLite3` datastore settings.
///
/// Deserialization is routed through `Sqlite3Config` and [`TryFrom`] so that the
/// layered configuration cannot bypass the validation performed by
/// [`Sqlite3::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "Sqlite3Config")]
#[non_exhaustive]
pub struct Sqlite3 {
    /// Maximum number of connections to the database.
    max_connections: u32,
}

/// Unchecked deserialization mirror of [`Sqlite3`].
///
/// Serde builds this snapshot and hands it to the [`TryFrom`] implementation,
/// which validates it through [`Sqlite3::try_new`].
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Sqlite3Config {
    /// Maximum number of connections to the database.
    #[serde(default = "default_sqlite3_max_connections")]
    max_connections: u32,
}

impl TryFrom<Sqlite3Config> for Sqlite3 {
    type Error = Sqlite3Error;

    fn try_from(config: Sqlite3Config) -> Result<Self, Self::Error> {
        Self::try_new(config.max_connections)
    }
}

impl Sqlite3 {
    /// Return the maximum number of connections to the database.
    #[must_use]
    pub fn max_connections(&self) -> u32 {
        self.max_connections
    }

    /// Update the maximum number of connections to the database.
    ///
    /// # Errors
    ///
    /// Returns [`Sqlite3Error::InvalidMaxConnections`] when `max_connections`
    /// is zero.
    pub fn set_max_connections(&mut self, max_connections: u32) -> Result<(), Sqlite3Error> {
        self.max_connections = Self::validate_max_connections(max_connections)?;
        Ok(())
    }

    /// Initialise a new `Sqlite3`, validating `max_connections`.
    ///
    /// # Errors
    ///
    /// Returns [`Sqlite3Error::InvalidMaxConnections`] when `max_connections`
    /// is zero.
    pub fn try_new(max_connections: u32) -> Result<Self, Sqlite3Error> {
        let validated_max_connections = Self::validate_max_connections(max_connections)?;
        Ok(Self {
            max_connections: validated_max_connections,
        })
    }

    /// Validate `max_connections`.
    ///
    /// # Errors
    ///
    /// Returns [`Sqlite3Error::InvalidMaxConnections`] when `max_connections`
    /// is zero.
    fn validate_max_connections(max_connections: u32) -> Result<u32, Sqlite3Error> {
        if max_connections == 0 {
            return Err(Sqlite3Error::InvalidMaxConnections);
        }
        Ok(max_connections)
    }
}

impl Default for Sqlite3 {
    fn default() -> Self {
        Self {
            max_connections: DEFAULT_SQLITE3_MAX_CONNECTIONS,
        }
    }
}

/// Persistence-related settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct Persistence {
    /// `SQLite3` datastore settings.
    sqlite3: Sqlite3,
}

impl Persistence {
    /// Initialise a new `Persistence`.
    #[must_use]
    pub fn new(sqlite3: Sqlite3) -> Self {
        Self { sqlite3 }
    }

    /// Return the `SQLite3` datastore settings.
    #[must_use]
    pub fn sqlite3(&self) -> &Sqlite3 {
        &self.sqlite3
    }

    /// Return a mutable reference to the `SQLite3` datastore settings.
    #[must_use]
    pub fn sqlite3_mut(&mut self) -> &mut Sqlite3 {
        &mut self.sqlite3
    }
}

/// Error returned when initialising or updating a `Jwt` value object.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum JwtError {
    /// The token lifetime is zero.
    #[error("jwt ttl must be greater than zero")]
    InvalidTtl,
}

/// `JWT` token settings.
///
/// Deserialization is routed through `JwtConfig` and [`TryFrom`] so that the
/// layered configuration cannot bypass the validation performed by
/// [`Jwt::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "JwtConfig")]
#[non_exhaustive]
pub struct Jwt {
    /// Lifetime of a token, in seconds.
    ttl: u64,
}

/// Unchecked deserialization mirror of [`Jwt`].
///
/// Serde builds this snapshot and hands it to the [`TryFrom`] implementation,
/// which validates it through [`Jwt::try_new`].
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct JwtConfig {
    /// Lifetime of a token, in seconds.
    #[serde(default = "default_jwt_ttl")]
    ttl: u64,
}

impl TryFrom<JwtConfig> for Jwt {
    type Error = JwtError;

    fn try_from(config: JwtConfig) -> Result<Self, Self::Error> {
        Self::try_new(config.ttl)
    }
}

impl Default for Jwt {
    fn default() -> Self {
        Self {
            ttl: DEFAULT_JWT_TTL,
        }
    }
}

impl Jwt {
    /// Update the lifetime of a token, in seconds.
    ///
    /// # Errors
    ///
    /// Returns [`JwtError::InvalidTtl`] when `ttl` is zero.
    pub fn set_ttl(&mut self, ttl: u64) -> Result<(), JwtError> {
        self.ttl = Self::validate_ttl(ttl)?;
        Ok(())
    }

    /// Initialise a new `Jwt`, validating `ttl`.
    ///
    /// # Errors
    ///
    /// Returns [`JwtError::InvalidTtl`] when `ttl` is zero.
    pub fn try_new(ttl: u64) -> Result<Self, JwtError> {
        let validated_ttl = Self::validate_ttl(ttl)?;
        Ok(Self { ttl: validated_ttl })
    }

    /// Return the lifetime of a token, in seconds.
    #[must_use]
    pub fn ttl(&self) -> u64 {
        self.ttl
    }

    /// Validate `ttl`.
    ///
    /// # Errors
    ///
    /// Returns [`JwtError::InvalidTtl`] when `ttl` is zero.
    fn validate_ttl(ttl: u64) -> Result<u64, JwtError> {
        if ttl == 0 {
            return Err(JwtError::InvalidTtl);
        }
        Ok(ttl)
    }
}

/// Error returned when initialising or updating a `RateLimit` value object.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RateLimitError {
    /// The burst size is zero.
    #[error("rate limit burst size must be greater than zero")]
    InvalidBurstSize,
    /// The header named as the client-address source is not supported.
    #[error("rate limit client ip header must be one of x-forwarded-for, x-real-ip, forwarded")]
    InvalidClientIpHeader,
    /// The replenishment period is zero.
    #[error("rate limit period must be greater than zero")]
    InvalidPeriodSeconds,
    /// An entry of the trusted-proxy list is not a valid IP address.
    #[error("rate limit trusted proxy must be a valid IP address")]
    InvalidTrustedProxy,
}

/// Header the trusted reverse proxy overwrites with the client address.
///
/// The operator declares exactly one; the limiter reads only that header once
/// the peer is a trusted proxy, so a client cannot influence the key by sending
/// a different header family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ClientIpHeader {
    /// The RFC 7239 `Forwarded` header (`for=` element).
    Forwarded,
    /// The `X-Forwarded-For` header.
    XForwardedFor,
    /// The `X-Real-IP` header.
    XRealIp,
}

impl ClientIpHeader {
    /// Return the header name as it appears on the wire.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match *self {
            Self::Forwarded => "forwarded",
            Self::XForwardedFor => "x-forwarded-for",
            Self::XRealIp => "x-real-ip",
        }
    }

    /// Parse the persisted header name.
    ///
    /// # Errors
    ///
    /// Returns [`RateLimitError::InvalidClientIpHeader`] when `value` is not one
    /// of the supported header names.
    pub fn parse(value: &str) -> Result<Self, RateLimitError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "forwarded" => Ok(Self::Forwarded),
            "x-forwarded-for" => Ok(Self::XForwardedFor),
            "x-real-ip" => Ok(Self::XRealIp),
            _ => Err(RateLimitError::InvalidClientIpHeader),
        }
    }
}

/// Login rate-limiting settings.
///
/// Deserialization is routed through `RateLimitConfig` and [`TryFrom`] so that
/// the layered configuration cannot bypass the validation performed by
/// [`RateLimit::try_new`]. Serialization uses the same mirror so the layered
/// configuration round-trips: `trusted_proxies` and `client_ip_header` are
/// written as strings, not structured JSON, because the `config` crate merges
/// the persisted snapshot back through the same string-typed fields.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "RateLimitConfig", into = "RateLimitConfig")]
#[non_exhaustive]
pub struct RateLimit {
    /// Number of requests a client address may spend before throttling.
    burst_size: u32,
    /// Header the trusted proxy overwrites with the client address.
    client_ip_header: ClientIpHeader,
    /// Seconds after which one element of the quota is replenished.
    period_seconds: u64,
    /// Addresses of the reverse proxies whose client-address headers are
    /// trusted. Empty means the forwarded headers are never trusted and the
    /// client key is always the peer address.
    trusted_proxies: Vec<IpAddr>,
}

/// Unchecked deserialization mirror of [`RateLimit`].
///
/// Serde builds this snapshot and hands it to the [`TryFrom`] implementation,
/// which validates it through [`RateLimit::try_new`]. Serialization goes the
/// other way — through [`From`] — so the two directions agree on the wire form.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct RateLimitConfig {
    /// Number of requests a client address may spend before throttling.
    #[serde(default = "default_burst_size")]
    burst_size: u32,
    /// Header the trusted proxy overwrites with the client address.
    #[serde(default = "default_client_ip_header")]
    client_ip_header: String,
    /// Seconds after which one element of the quota is replenished.
    #[serde(default = "default_period_seconds")]
    period_seconds: u64,
    /// Comma-separated addresses of the trusted reverse proxies.
    #[serde(default)]
    trusted_proxies: String,
}

impl TryFrom<RateLimitConfig> for RateLimit {
    type Error = RateLimitError;

    fn try_from(config: RateLimitConfig) -> Result<Self, Self::Error> {
        Self::try_new(
            config.burst_size,
            ClientIpHeader::parse(&config.client_ip_header)?,
            config.period_seconds,
            parse_trusted_proxies(&config.trusted_proxies)?,
        )
    }
}

impl From<&RateLimit> for RateLimitConfig {
    fn from(rate_limit: &RateLimit) -> Self {
        Self {
            burst_size: rate_limit.burst_size,
            client_ip_header: rate_limit.client_ip_header.as_str().to_owned(),
            period_seconds: rate_limit.period_seconds,
            trusted_proxies: rate_limit.trusted_proxies_value(),
        }
    }
}

impl From<RateLimit> for RateLimitConfig {
    fn from(rate_limit: RateLimit) -> Self {
        Self::from(&rate_limit)
    }
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            burst_size: DEFAULT_BURST_SIZE,
            client_ip_header: DEFAULT_CLIENT_IP_HEADER,
            period_seconds: DEFAULT_PERIOD_SECONDS,
            trusted_proxies: Vec::new(),
        }
    }
}

impl RateLimit {
    /// Return the number of requests a client address may spend before
    /// throttling.
    #[must_use]
    pub fn burst_size(&self) -> u32 {
        self.burst_size
    }

    /// Return the header the trusted proxy overwrites with the client address.
    #[must_use]
    pub fn client_ip_header(&self) -> ClientIpHeader {
        self.client_ip_header
    }

    /// Return the seconds after which one element of the quota is replenished.
    #[must_use]
    pub fn period_seconds(&self) -> u64 {
        self.period_seconds
    }

    /// Update the number of requests a client address may spend before
    /// throttling.
    ///
    /// # Errors
    ///
    /// Returns [`RateLimitError::InvalidBurstSize`] when `burst_size` is zero.
    pub fn set_burst_size(&mut self, burst_size: u32) -> Result<(), RateLimitError> {
        self.burst_size = Self::validate_burst_size(burst_size)?;
        Ok(())
    }

    /// Update the header the trusted proxy overwrites with the client address.
    pub fn set_client_ip_header(&mut self, client_ip_header: ClientIpHeader) {
        self.client_ip_header = client_ip_header;
    }

    /// Update the seconds after which one element of the quota is replenished.
    ///
    /// # Errors
    ///
    /// Returns [`RateLimitError::InvalidPeriodSeconds`] when `period_seconds`
    /// is zero.
    pub fn set_period_seconds(&mut self, period_seconds: u64) -> Result<(), RateLimitError> {
        self.period_seconds = Self::validate_period_seconds(period_seconds)?;
        Ok(())
    }

    /// Update the addresses of the trusted reverse proxies.
    pub fn set_trusted_proxies(&mut self, trusted_proxies: Vec<IpAddr>) {
        self.trusted_proxies = trusted_proxies;
    }

    /// Return the addresses of the trusted reverse proxies.
    #[must_use]
    pub fn trusted_proxies(&self) -> &[IpAddr] {
        &self.trusted_proxies
    }

    /// Return the trusted proxies rendered as the persisted comma-separated
    /// list.
    #[must_use]
    pub fn trusted_proxies_value(&self) -> String {
        self.trusted_proxies
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<String>>()
            .join(&TRUSTED_PROXIES_SEPARATOR.to_string())
    }

    /// Initialise a new `RateLimit`, validating `burst_size` and
    /// `period_seconds`.
    ///
    /// # Errors
    ///
    /// Returns [`RateLimitError::InvalidBurstSize`] when `burst_size` is zero,
    /// and [`RateLimitError::InvalidPeriodSeconds`] when `period_seconds` is
    /// zero.
    pub fn try_new(
        burst_size: u32,
        client_ip_header: ClientIpHeader,
        period_seconds: u64,
        trusted_proxies: Vec<IpAddr>,
    ) -> Result<Self, RateLimitError> {
        let validated_burst_size = Self::validate_burst_size(burst_size)?;
        let validated_period_seconds = Self::validate_period_seconds(period_seconds)?;
        Ok(Self {
            burst_size: validated_burst_size,
            client_ip_header,
            period_seconds: validated_period_seconds,
            trusted_proxies,
        })
    }

    /// Validate `burst_size`.
    ///
    /// # Errors
    ///
    /// Returns [`RateLimitError::InvalidBurstSize`] when `burst_size` is zero.
    fn validate_burst_size(burst_size: u32) -> Result<u32, RateLimitError> {
        if burst_size == 0 {
            return Err(RateLimitError::InvalidBurstSize);
        }
        Ok(burst_size)
    }

    /// Validate `period_seconds`.
    ///
    /// # Errors
    ///
    /// Returns [`RateLimitError::InvalidPeriodSeconds`] when `period_seconds`
    /// is zero.
    fn validate_period_seconds(period_seconds: u64) -> Result<u64, RateLimitError> {
        if period_seconds == 0 {
            return Err(RateLimitError::InvalidPeriodSeconds);
        }
        Ok(period_seconds)
    }
}

/// Error returned when initialising or updating a `Security` value object.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SecurityError {
    /// The `JWT` settings are invalid.
    #[error("invalid jwt configuration")]
    InvalidJwt(#[from] JwtError),
}

/// Security-related settings.
///
/// Deserialization is routed through `SecurityConfig` and [`TryFrom`] so that
/// the layered configuration cannot bypass the validation performed by
/// [`Security::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "SecurityConfig")]
#[non_exhaustive]
pub struct Security {
    /// `JWT` settings.
    jwt: Jwt,
    /// Whether the Root Admin default password is still logged to standard
    /// output on runtime start.
    log_root_admin_password: bool,
    /// Login rate-limiting settings.
    rate_limit: RateLimit,
}

/// Unchecked deserialization mirror of [`Security`].
///
/// Serde builds this snapshot and hands it to the [`TryFrom`] implementation,
/// which validates it through [`Security::try_new`].
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SecurityConfig {
    /// `JWT` settings.
    #[serde(default)]
    jwt: Jwt,
    /// Whether the Root Admin default password is still logged to standard
    /// output on runtime start.
    #[serde(default = "default_log_root_admin_password")]
    log_root_admin_password: bool,
    /// Login rate-limiting settings.
    #[serde(default)]
    rate_limit: RateLimit,
}

impl TryFrom<SecurityConfig> for Security {
    type Error = SecurityError;

    fn try_from(config: SecurityConfig) -> Result<Self, Self::Error> {
        let mut security = Self::try_new(config.jwt, config.log_root_admin_password)?;
        security.set_rate_limit(config.rate_limit);
        Ok(security)
    }
}

impl Default for Security {
    fn default() -> Self {
        Self {
            jwt: Jwt::default(),
            log_root_admin_password: true,
            rate_limit: RateLimit::default(),
        }
    }
}

impl Security {
    /// Return the `JWT` settings.
    #[must_use]
    pub fn jwt(&self) -> &Jwt {
        &self.jwt
    }

    /// Return a mutable reference to the `JWT` settings.
    #[must_use]
    pub fn jwt_mut(&mut self) -> &mut Jwt {
        &mut self.jwt
    }

    /// Return whether the Root Admin default password is still logged to
    /// standard output on runtime start.
    #[must_use]
    pub fn log_root_admin_password(&self) -> bool {
        self.log_root_admin_password
    }

    /// Initialise a new `Security`.
    #[must_use]
    pub fn new(jwt: Jwt, log_root_admin_password: bool) -> Self {
        Self {
            jwt,
            log_root_admin_password,
            rate_limit: RateLimit::default(),
        }
    }

    /// Return the login rate-limiting settings.
    #[must_use]
    pub fn rate_limit(&self) -> &RateLimit {
        &self.rate_limit
    }

    /// Update whether the Root Admin default password is logged to standard
    /// output on runtime start.
    pub fn set_log_root_admin_password(&mut self, log_root_admin_password: bool) {
        self.log_root_admin_password = log_root_admin_password;
    }

    /// Update the login rate-limiting settings.
    pub fn set_rate_limit(&mut self, rate_limit: RateLimit) {
        self.rate_limit = rate_limit;
    }

    /// Initialise a new `Security`, validating the `JWT` settings.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityError::InvalidJwt`] when the `JWT` settings are
    /// invalid.
    pub fn try_new(jwt: Jwt, log_root_admin_password: bool) -> Result<Self, SecurityError> {
        Ok(Self {
            jwt,
            log_root_admin_password,
            rate_limit: RateLimit::default(),
        })
    }
}

/// The application configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct Configuration {
    /// Asset-related settings.
    asset: Asset,
    /// Logging-related settings.
    logging: Logging,
    /// Persistence-related settings.
    persistence: Persistence,
    /// Security-related settings.
    security: Security,
}

impl Configuration {
    /// Return the asset-related settings.
    #[must_use]
    pub fn asset(&self) -> &Asset {
        &self.asset
    }

    /// Return the logging-related settings.
    #[must_use]
    pub fn logging(&self) -> &Logging {
        &self.logging
    }

    /// Initialise a new `Configuration`.
    #[must_use]
    pub fn new(
        persistence: Persistence,
        security: Security,
        logging: Logging,
        asset: Asset,
    ) -> Self {
        Self {
            asset,
            logging,
            persistence,
            security,
        }
    }

    /// Return the persistence-related settings.
    #[must_use]
    pub fn persistence(&self) -> &Persistence {
        &self.persistence
    }

    /// Return a mutable reference to the persistence-related settings.
    #[must_use]
    pub fn persistence_mut(&mut self) -> &mut Persistence {
        &mut self.persistence
    }

    /// Return the security-related settings.
    #[must_use]
    pub fn security(&self) -> &Security {
        &self.security
    }

    /// Return a mutable reference to the security-related settings.
    #[must_use]
    pub fn security_mut(&mut self) -> &mut Security {
        &mut self.security
    }
}

/// Parse the comma-separated trusted-proxy list.
///
/// # Errors
///
/// Returns [`RateLimitError::InvalidTrustedProxy`] when a non-empty entry is
/// not a valid IP address.
pub fn parse_trusted_proxies(value: &str) -> Result<Vec<IpAddr>, RateLimitError> {
    value
        .split(TRUSTED_PROXIES_SEPARATOR)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| IpAddr::from_str(entry).map_err(|_| RateLimitError::InvalidTrustedProxy))
        .collect()
}

/// Default value for [`AssetStorage::root`].
fn default_storage_root() -> String {
    DEFAULT_STORAGE_ROOT.to_owned()
}

/// Default value for [`AssetUpload::chunk_size_bytes`].
fn default_chunk_size_bytes() -> u64 {
    DEFAULT_CHUNK_SIZE_BYTES
}

/// Default value for [`AssetUpload::expiry_seconds`].
fn default_expiry_seconds() -> u64 {
    DEFAULT_EXPIRY_SECONDS
}

/// Default value for [`AssetUpload::max_file_size_bytes`].
fn default_max_file_size_bytes() -> u64 {
    DEFAULT_MAX_FILE_SIZE_BYTES
}

/// Default value for [`Logging::ansi`].
fn default_log_ansi() -> bool {
    DEFAULT_LOG_ANSI
}

/// Default value for [`Logging::level`].
fn default_log_level() -> String {
    DEFAULT_LOG_LEVEL.to_owned()
}

/// Default value for [`Logging::max_files`].
fn default_log_max_files() -> u32 {
    DEFAULT_LOG_MAX_FILES
}

/// Default value for [`Logging::rotation`].
fn default_log_rotation() -> Rotation {
    DEFAULT_LOG_ROTATION
}

/// Default value for [`Jwt::ttl`].
fn default_jwt_ttl() -> u64 {
    DEFAULT_JWT_TTL
}

/// Default value for [`Sqlite3::max_connections`].
fn default_sqlite3_max_connections() -> u32 {
    DEFAULT_SQLITE3_MAX_CONNECTIONS
}

/// Default value for [`RateLimit::burst_size`].
fn default_burst_size() -> u32 {
    DEFAULT_BURST_SIZE
}

/// Default value for [`RateLimit::client_ip_header`].
fn default_client_ip_header() -> String {
    DEFAULT_CLIENT_IP_HEADER.as_str().to_owned()
}

/// Default value for [`RateLimit::period_seconds`].
fn default_period_seconds() -> u64 {
    DEFAULT_PERIOD_SECONDS
}

/// Default value for [`Security::log_root_admin_password`].
fn default_log_root_admin_password() -> bool {
    true
}
