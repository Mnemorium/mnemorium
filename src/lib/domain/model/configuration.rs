#![allow(
    dead_code,
    reason = "the configuration value objects keep the provider accessors, setters and mutators their contract declares; several have no caller yet"
)]

use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::net::IpAddr;
use std::str::FromStr as _;

/// Default size of every chunk except the last, in bytes.
pub(crate) const DEFAULT_CHUNK_SIZE_BYTES: u64 = 5_242_880;

/// Default lifetime of an upload session, in seconds.
const DEFAULT_EXPIRY_SECONDS: u64 = 86_400;

/// Default maximum size of a single uploaded file, in bytes (100 GiB).
const DEFAULT_MAX_FILE_SIZE_BYTES: u64 = 107_374_182_400;

/// Default interval between two upload-reaper runs, in seconds.
const DEFAULT_REAP_INTERVAL_SECONDS: u64 = 900;

/// Default maximum number of upload sessions a reaper phase handles per run.
const DEFAULT_REAP_BATCH_LENGTH: usize = 100;

/// Largest accepted upload-reaper interval, in seconds (one day).
const MAX_REAP_INTERVAL_SECONDS: u64 = 86_400;

/// Reserved target carrying security events; a verbosity directive must never
/// target it, or it could suppress a catalog event (`OBS-007`).
const SECURITY_TARGET: &str = "security";

/// Default verbosity filter applied to the `tracing` instrumentation.
const DEFAULT_LOG_LEVEL: &str = "debug,sqlx=warn";

/// Default value for [`Logging::ansi`].
const DEFAULT_LOG_ANSI: bool = false;

/// Default value for [`Logging::max_files`].
const DEFAULT_LOG_MAX_FILES: u32 = 7;

/// Default value for [`Logging::rotation`].
const DEFAULT_LOG_ROTATION: Rotation = Rotation::Daily;

/// Default wait for an absent library root at startup, in seconds.
const DEFAULT_MEDIA_ROOT_WAIT_SECONDS: u64 = 30;

/// Largest accepted wait for the library root, in seconds (one day).
const MAX_MEDIA_ROOT_WAIT_SECONDS: u64 = 86_400;

/// Default value for [`Jwt::ttl`], in seconds.
const DEFAULT_JWT_TTL_SECONDS: u64 = 3600;

/// Default value for [`Sqlite3::max_connections`].
///
/// Fixed rather than derived from the host: the value must be deterministic so
/// the domain performs no OS probe, and `SQLite` serializes writers regardless.
const DEFAULT_SQLITE3_MAX_CONNECTIONS: u32 = 8;

/// Default number of requests a [`RateLimit`] burst admits.
const DEFAULT_BURST: u32 = 5;

/// Default header carrying the client address, used when no proxy is trusted.
const DEFAULT_CLIENT_IP_HEADER: ClientIpHeader = ClientIpHeader::XForwardedFor;

/// Default value for [`RateLimit::period_seconds`].
const DEFAULT_PERIOD_SECONDS: u64 = 12;

/// Separator between addresses in the [`RateLimit::trusted_proxies`] list.
const TRUSTED_PROXIES_SEPARATOR: char = ',';

/// Identifier of an external metadata provider.
///
/// The identifier is provider-tagged rather than positional: a reference one
/// provider issues means nothing to another, so every reference and every
/// configured link carries the provider that owns it. The supported providers
/// are configuration vocabulary; no other domain model names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub(crate) enum ProviderId {
    /// `MusicBrainz`, which serves music metadata.
    MusicBrainz,
    /// The Open Movie Database.
    Omdb,
    /// The Movie Database.
    Tmdb,
}

impl ProviderId {
    /// Return the provider name as the configuration declares it.
    #[must_use]
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::MusicBrainz => "musicbrainz",
            Self::Omdb => "omdb",
            Self::Tmdb => "tmdb",
        }
    }

    /// Parse the provider name a configuration entry declares.
    ///
    /// The comparison is case-insensitive and ignores surrounding whitespace, so
    /// `TMDB` and ` tmdb ` both name the same provider.
    #[must_use]
    pub(crate) fn parse(value: &str) -> Option<Self> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "musicbrainz" => Some(Self::MusicBrainz),
            "omdb" => Some(Self::Omdb),
            "tmdb" => Some(Self::Tmdb),
            _ => None,
        }
    }

    /// Return whether this provider serves music rather than video.
    ///
    /// The predicate names the medium a provider answers for, so a
    /// configuration section admits only the providers it can use: the music
    /// section takes a music provider and the video section takes a video one.
    #[must_use]
    pub(crate) fn serves_music(self) -> bool {
        match self {
            Self::MusicBrainz => true,
            Self::Omdb | Self::Tmdb => false,
        }
    }

    /// Return whether this provider serves video metadata rather than music.
    ///
    /// The predicate names the medium a provider answers for, so a
    /// configuration section admits only the providers it can use: the video
    /// section takes a video provider and the music section takes a music one.
    #[must_use]
    pub(crate) fn serves_video(self) -> bool {
        match self {
            Self::MusicBrainz => false,
            Self::Omdb | Self::Tmdb => true,
        }
    }
}

/// Error returned when initialising or updating an asset value object.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum AssetError {
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
    /// The reaper batch size is zero.
    #[error("asset upload reap_batch_size must be greater than zero")]
    ReapBatchSizeZero,
    /// The reaper interval is outside the accepted range.
    #[error("asset upload reap_interval_seconds must be between 1 and 86400")]
    ReapIntervalOutOfRange,
}

/// Upload-related settings of the Asset bounded context.
///
/// Constructed through [`AssetUpload::try_new`], which validates every setting;
/// the configuration adapters merge their layer into the resolved value and
/// rebuild it, so no layer can bypass the validation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub(crate) struct AssetUpload {
    /// Size of every chunk except the last, in bytes.
    chunk_size_bytes: u64,
    /// Lifetime of an upload session, in seconds.
    expiry_seconds: u64,
    /// Maximum size of a single uploaded file, in bytes.
    max_file_size_bytes: u64,
    /// Maximum number of upload sessions the reaper handles per phase.
    reap_batch_size: usize,
    /// Interval between two upload-reaper runs, in seconds.
    reap_interval_seconds: u64,
}

impl AssetUpload {
    /// Return the size of every chunk except the last, in bytes.
    #[must_use]
    pub(crate) fn chunk_size_bytes(&self) -> u64 {
        self.chunk_size_bytes
    }

    /// Return the lifetime of an upload session, in seconds.
    #[must_use]
    pub(crate) fn expiry_seconds(&self) -> u64 {
        self.expiry_seconds
    }

    /// Return the maximum size of a single uploaded file, in bytes.
    #[must_use]
    pub(crate) fn max_file_size_bytes(&self) -> u64 {
        self.max_file_size_bytes
    }

    /// Return the maximum number of upload sessions the reaper handles per
    /// phase.
    #[must_use]
    pub(crate) fn reap_batch_size(&self) -> usize {
        self.reap_batch_size
    }

    /// Return the interval between two upload-reaper runs, in seconds.
    #[must_use]
    pub(crate) fn reap_interval_seconds(&self) -> u64 {
        self.reap_interval_seconds
    }

    /// Update the size of every chunk except the last, in bytes.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::ChunkSizeZero`] when `chunk_size_bytes` is zero.
    fn set_chunk_size_bytes(&mut self, chunk_size_bytes: u64) -> Result<(), AssetError> {
        self.chunk_size_bytes = Self::validate_chunk_size_bytes(chunk_size_bytes)?;
        Ok(())
    }

    /// Update the lifetime of an upload session, in seconds.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::ExpiryZero`] when `expiry_seconds` is zero.
    fn set_expiry_seconds(&mut self, expiry_seconds: u64) -> Result<(), AssetError> {
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
    fn set_max_file_size_bytes(&mut self, max_file_size_bytes: u64) -> Result<(), AssetError> {
        self.max_file_size_bytes = Self::validate_max_file_size_bytes(max_file_size_bytes)?;
        Ok(())
    }

    /// Update the maximum number of upload sessions the reaper handles per
    /// phase.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::ReapBatchSizeZero`] when `reap_batch_size` is zero.
    fn set_reap_batch_size(&mut self, reap_batch_size: usize) -> Result<(), AssetError> {
        self.reap_batch_size = Self::validate_reap_batch_size(reap_batch_size)?;
        Ok(())
    }

    /// Update the interval between two upload-reaper runs, in seconds.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::ReapIntervalOutOfRange`] when
    /// `reap_interval_seconds` is zero or exceeds one day.
    fn set_reap_interval_seconds(&mut self, reap_interval_seconds: u64) -> Result<(), AssetError> {
        self.reap_interval_seconds = Self::validate_reap_interval_seconds(reap_interval_seconds)?;
        Ok(())
    }

    /// Initialise a new `AssetUpload`, validating every setting.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::ChunkSizeZero`] when `chunk_size_bytes` is zero,
    /// [`AssetError::ExpiryZero`] when `expiry_seconds` is zero,
    /// [`AssetError::MaxFileSizeZero`] when `max_file_size_bytes` is zero,
    /// [`AssetError::MaxFileSizeTooLarge`] when it exceeds [`i64::MAX`],
    /// [`AssetError::ReapBatchSizeZero`] when `reap_batch_size` is zero, and
    /// [`AssetError::ReapIntervalOutOfRange`] when `reap_interval_seconds` is
    /// outside `1..=86400`.
    pub(crate) fn try_new(
        chunk_size_bytes: u64,
        expiry_seconds: u64,
        max_file_size_bytes: u64,
        reap_batch_size: usize,
        reap_interval_seconds: u64,
    ) -> Result<Self, AssetError> {
        let validated_chunk_size = Self::validate_chunk_size_bytes(chunk_size_bytes)?;
        let validated_expiry = Self::validate_expiry_seconds(expiry_seconds)?;
        let validated_max_file_size = Self::validate_max_file_size_bytes(max_file_size_bytes)?;
        let validated_reap_batch_size = Self::validate_reap_batch_size(reap_batch_size)?;
        let validated_reap_interval = Self::validate_reap_interval_seconds(reap_interval_seconds)?;
        Ok(Self {
            chunk_size_bytes: validated_chunk_size,
            expiry_seconds: validated_expiry,
            max_file_size_bytes: validated_max_file_size,
            reap_batch_size: validated_reap_batch_size,
            reap_interval_seconds: validated_reap_interval,
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

    /// Validate `reap_batch_size`.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::ReapBatchSizeZero`] when `reap_batch_size` is zero.
    fn validate_reap_batch_size(reap_batch_size: usize) -> Result<usize, AssetError> {
        if reap_batch_size == 0 {
            return Err(AssetError::ReapBatchSizeZero);
        }
        Ok(reap_batch_size)
    }

    /// Validate `reap_interval_seconds`.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::ReapIntervalOutOfRange`] when
    /// `reap_interval_seconds` is zero or exceeds one day.
    fn validate_reap_interval_seconds(reap_interval_seconds: u64) -> Result<u64, AssetError> {
        if reap_interval_seconds == 0 || reap_interval_seconds > MAX_REAP_INTERVAL_SECONDS {
            return Err(AssetError::ReapIntervalOutOfRange);
        }
        Ok(reap_interval_seconds)
    }
}

impl Default for AssetUpload {
    fn default() -> Self {
        Self {
            chunk_size_bytes: DEFAULT_CHUNK_SIZE_BYTES,
            expiry_seconds: DEFAULT_EXPIRY_SECONDS,
            max_file_size_bytes: DEFAULT_MAX_FILE_SIZE_BYTES,
            reap_batch_size: DEFAULT_REAP_BATCH_LENGTH,
            reap_interval_seconds: DEFAULT_REAP_INTERVAL_SECONDS,
        }
    }
}

/// Asset-related settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub(crate) struct Asset {
    /// Upload settings.
    upload: AssetUpload,
}

impl Asset {
    /// Initialise a new `Asset`.
    #[must_use]
    pub(crate) fn new(upload: AssetUpload) -> Self {
        Self { upload }
    }

    /// Return the upload settings.
    #[must_use]
    pub(crate) fn upload(&self) -> &AssetUpload {
        &self.upload
    }
}

/// Error returned when initialising or updating a `Logging` value object.
#[expect(
    clippy::enum_variant_names,
    reason = "each variant names the logging setting it rejects, so the shared `Level` prefix is the subject"
)]
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum LoggingError {
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
pub(crate) enum Rotation {
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
/// Constructed through [`Logging::try_new`], which validates the verbosity
/// filter; the configuration adapters merge their layer into the resolved value
/// and rebuild it, so no layer can bypass the validation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub(crate) struct Logging {
    /// Whether to colour the standard-output sink with ANSI escape codes.
    ansi: bool,
    /// Verbosity filter applied to the `tracing` instrumentation.
    level: String,
    /// Maximum number of rotated log files to keep; `0` keeps every file.
    max_files: u32,
    /// Rotation period of the file sink.
    rotation: Rotation,
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
    pub(crate) fn ansi(&self) -> bool {
        self.ansi
    }

    /// Return the verbosity filter applied to the `tracing` instrumentation.
    #[must_use]
    pub(crate) fn level(&self) -> &str {
        &self.level
    }

    /// Return the maximum number of rotated log files to keep; `0` keeps every
    /// file.
    #[must_use]
    pub(crate) fn max_files(&self) -> u32 {
        self.max_files
    }

    /// Return the rotation period of the file sink.
    #[must_use]
    pub(crate) fn rotation(&self) -> Rotation {
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
    pub(crate) fn try_new(
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
pub(crate) enum Sqlite3Error {
    /// The maximum number of connections is zero.
    #[error("sqlite3 max_connections must be greater than zero")]
    InvalidMaxConnections,
}

/// `SQLite3` datastore settings.
///
/// Constructed through [`Sqlite3::try_new`], which validates the setting; the
/// configuration adapters merge their layer into the resolved value and rebuild
/// it, so no layer can bypass the validation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub(crate) struct Sqlite3 {
    /// Maximum number of connections to the database.
    max_connections: u32,
}

impl Sqlite3 {
    /// Return the maximum number of connections to the database.
    #[must_use]
    pub(crate) fn max_connections(&self) -> u32 {
        self.max_connections
    }

    /// Update the maximum number of connections to the database.
    ///
    /// # Errors
    ///
    /// Returns [`Sqlite3Error::InvalidMaxConnections`] when `max_connections`
    /// is zero.
    fn set_max_connections(&mut self, max_connections: u32) -> Result<(), Sqlite3Error> {
        self.max_connections = Self::validate_max_connections(max_connections)?;
        Ok(())
    }

    /// Initialise a new `Sqlite3`, validating `max_connections`.
    ///
    /// # Errors
    ///
    /// Returns [`Sqlite3Error::InvalidMaxConnections`] when `max_connections`
    /// is zero.
    pub(crate) fn try_new(max_connections: u32) -> Result<Self, Sqlite3Error> {
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
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub(crate) struct Persistence {
    /// `SQLite3` datastore settings.
    sqlite3: Sqlite3,
}

impl Persistence {
    /// Initialise a new `Persistence`.
    #[must_use]
    pub(crate) fn new(sqlite3: Sqlite3) -> Self {
        Self { sqlite3 }
    }

    /// Return the `SQLite3` datastore settings.
    #[must_use]
    pub(crate) fn sqlite3(&self) -> &Sqlite3 {
        &self.sqlite3
    }

    /// Return a mutable reference to the `SQLite3` datastore settings.
    #[must_use]
    fn sqlite3_mut(&mut self) -> &mut Sqlite3 {
        &mut self.sqlite3
    }
}

/// Error returned when initialising or updating a `Jwt` value object.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum JwtError {
    /// The token lifetime is zero.
    #[error("jwt ttl must be greater than zero")]
    InvalidTtl,
}

/// `JWT` token settings.
///
/// Constructed through [`Jwt::try_new`], which validates the setting; the
/// configuration adapters merge their layer into the resolved value and rebuild
/// it, so no layer can bypass the validation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub(crate) struct Jwt {
    /// Lifetime of a token, in seconds.
    ttl: u64,
}

impl Default for Jwt {
    fn default() -> Self {
        Self {
            ttl: DEFAULT_JWT_TTL_SECONDS,
        }
    }
}

impl Jwt {
    /// Update the lifetime of a token, in seconds.
    ///
    /// # Errors
    ///
    /// Returns [`JwtError::InvalidTtl`] when `ttl` is zero.
    fn set_ttl(&mut self, ttl: u64) -> Result<(), JwtError> {
        self.ttl = Self::validate_ttl(ttl)?;
        Ok(())
    }

    /// Initialise a new `Jwt`, validating `ttl`.
    ///
    /// # Errors
    ///
    /// Returns [`JwtError::InvalidTtl`] when `ttl` is zero.
    pub(crate) fn try_new(ttl: u64) -> Result<Self, JwtError> {
        let validated_ttl = Self::validate_ttl(ttl)?;
        Ok(Self { ttl: validated_ttl })
    }

    /// Return the lifetime of a token, in seconds.
    #[must_use]
    pub(crate) fn ttl(&self) -> u64 {
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
#[expect(
    clippy::enum_variant_names,
    reason = "each variant names the rate-limit setting it rejects, so the shared `Invalid` prefix is the subject"
)]
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum RateLimitError {
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
pub(crate) enum ClientIpHeader {
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
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "STY-RUST-019 fixes the getter receiver as `&self`"
    )]
    pub(crate) fn as_str(&self) -> &'static str {
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
    pub(crate) fn parse(value: &str) -> Result<Self, RateLimitError> {
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
/// Constructed through [`RateLimit::try_new`], which validates every setting;
/// the configuration adapters merge their layer into the resolved value and
/// rebuild it, so no layer can bypass the validation. Serialization routes
/// through [`RateLimitConfig`]: `trusted_proxies` and `client_ip_header` are
/// written as strings because the persisted document carries them as strings.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(into = "RateLimitConfig")]
#[non_exhaustive]
pub(crate) struct RateLimit {
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

/// Serialization mirror of [`RateLimit`].
///
/// Serialization goes through [`From`] so the persisted document keeps the
/// string wire form of `client_ip_header` and `trusted_proxies`.
#[derive(serde::Serialize)]
struct RateLimitConfig {
    /// Number of requests a client address may spend before throttling.
    burst_size: u32,
    /// Header the trusted proxy overwrites with the client address.
    client_ip_header: String,
    /// Seconds after which one element of the quota is replenished.
    period_seconds: u64,
    /// Comma-separated addresses of the trusted reverse proxies.
    trusted_proxies: String,
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
            burst_size: DEFAULT_BURST,
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
    pub(crate) fn burst_size(&self) -> u32 {
        self.burst_size
    }

    /// Return the header the trusted proxy overwrites with the client address.
    #[must_use]
    pub(crate) fn client_ip_header(&self) -> ClientIpHeader {
        self.client_ip_header
    }

    /// Return the seconds after which one element of the quota is replenished.
    #[must_use]
    pub(crate) fn period_seconds(&self) -> u64 {
        self.period_seconds
    }

    /// Update the number of requests a client address may spend before
    /// throttling.
    ///
    /// # Errors
    ///
    /// Returns [`RateLimitError::InvalidBurstSize`] when `burst_size` is zero.
    fn set_burst_size(&mut self, burst_size: u32) -> Result<(), RateLimitError> {
        self.burst_size = Self::validate_burst_size(burst_size)?;
        Ok(())
    }

    /// Update the header the trusted proxy overwrites with the client address.
    fn set_client_ip_header(&mut self, client_ip_header: ClientIpHeader) {
        self.client_ip_header = client_ip_header;
    }

    /// Update the seconds after which one element of the quota is replenished.
    ///
    /// # Errors
    ///
    /// Returns [`RateLimitError::InvalidPeriodSeconds`] when `period_seconds`
    /// is zero.
    fn set_period_seconds(&mut self, period_seconds: u64) -> Result<(), RateLimitError> {
        self.period_seconds = Self::validate_period_seconds(period_seconds)?;
        Ok(())
    }

    /// Update the addresses of the trusted reverse proxies.
    fn set_trusted_proxies(&mut self, trusted_proxies: Vec<IpAddr>) {
        self.trusted_proxies = trusted_proxies;
    }

    /// Return the addresses of the trusted reverse proxies.
    #[must_use]
    pub(crate) fn trusted_proxies(&self) -> &[IpAddr] {
        &self.trusted_proxies
    }

    /// Return the trusted proxies rendered as the persisted comma-separated
    /// list.
    #[must_use]
    fn trusted_proxies_value(&self) -> String {
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
    pub(crate) fn try_new(
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
pub(crate) enum SecurityError {
    /// The `JWT` settings are invalid.
    #[error("invalid jwt configuration")]
    InvalidJwt(#[from] JwtError),
}

/// Security-related settings.
///
/// Constructed through [`Security::try_new`], which validates the `JWT`
/// settings; the configuration adapters merge their layer into the resolved
/// value and rebuild it, so no layer can bypass the validation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub(crate) struct Security {
    /// `JWT` settings.
    jwt: Jwt,
    /// Whether the Root Admin default password is still logged to standard
    /// output on runtime start.
    log_root_admin_password: bool,
    /// Login rate-limiting settings.
    rate_limit: RateLimit,
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
    pub(crate) fn jwt(&self) -> &Jwt {
        &self.jwt
    }

    /// Return a mutable reference to the `JWT` settings.
    #[must_use]
    fn jwt_mut(&mut self) -> &mut Jwt {
        &mut self.jwt
    }

    /// Return whether the Root Admin default password is still logged to
    /// standard output on runtime start.
    #[must_use]
    pub(crate) fn log_root_admin_password(&self) -> bool {
        self.log_root_admin_password
    }

    /// Initialise a new `Security`.
    #[must_use]
    pub(crate) fn new(jwt: Jwt, log_root_admin_password: bool) -> Self {
        Self {
            jwt,
            log_root_admin_password,
            rate_limit: RateLimit::default(),
        }
    }

    /// Return the login rate-limiting settings.
    #[must_use]
    pub(crate) fn rate_limit(&self) -> &RateLimit {
        &self.rate_limit
    }

    /// Update whether the Root Admin default password is logged to standard
    /// output on runtime start.
    fn set_log_root_admin_password(&mut self, log_root_admin_password: bool) {
        self.log_root_admin_password = log_root_admin_password;
    }

    /// Update the login rate-limiting settings.
    fn set_rate_limit(&mut self, rate_limit: RateLimit) {
        self.rate_limit = rate_limit;
    }

    /// Initialise a new `Security`, validating the `JWT` settings.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityError::InvalidJwt`] when the `JWT` settings are
    /// invalid.
    #[expect(
        clippy::unnecessary_wraps,
        reason = "STY-RUST-018 keeps the fallible constructor named `try_new`; `Security` mirrors its sibling configuration value objects and the JWT validation it reports stays fallible at the call site"
    )]
    pub(crate) fn try_new(
        jwt: Jwt,
        log_root_admin_password: bool,
        rate_limit: RateLimit,
    ) -> Result<Self, SecurityError> {
        Ok(Self {
            jwt,
            log_root_admin_password,
            rate_limit,
        })
    }
}

/// Error returned when initialising a video metadata provider or its list.
///
/// The message names the setting; it never carries an API key value
/// (`OBS-004`).
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum VideoProvidersError {
    /// A listed provider has an empty API key.
    #[error("library.metadata.video.providers api_key must not be empty")]
    ApiKeyEmpty,
    /// Two entries share an index.
    #[error("library.metadata.video.providers index must be unique")]
    DuplicateIndex,
    /// Two entries share a provider name.
    #[error("library.metadata.video.providers name must be unique")]
    DuplicateProvider,
    /// An entry's index is zero.
    #[error("library.metadata.video.providers index must be greater than zero")]
    InvalidIndex,
    /// An entry names a provider the system does not support.
    #[error("library.metadata.video.providers name must be a supported provider")]
    UnknownProvider,
    /// An entry names a provider the section cannot use.
    #[error("library.metadata.video.providers name must be a video provider")]
    UnsupportedProvider,
}

/// An external provider's API key.
///
/// The value is configuration, not instance identity, and it never leaves the
/// process: `Debug` prints a placeholder, every holder skips it when it
/// serializes, and only the adapter that builds a request reads the value. The
/// key is config-file-only; the environment layer names no provider setting.
#[derive(Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(transparent)]
#[non_exhaustive]
pub(crate) struct ApiKey(String);

impl ApiKey {
    /// Return the key value for the adapter that builds a request.
    #[must_use]
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }

    /// Return whether the key is empty.
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Create a new API key.
    #[must_use]
    pub(crate) fn new(value: String) -> Self {
        Self(value)
    }
}

impl Debug for ApiKey {
    /// Redact the key (`OBS-004`): the value never reaches a log sink.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("ApiKey(**)")
    }
}

/// One configured video metadata provider.
#[derive(Clone, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub(crate) struct VideoProvider {
    /// API key the adapter presents to the provider; never serialized.
    #[serde(skip_serializing)]
    api_key: ApiKey,
    /// Position of the provider in the failover order; positive and unique.
    index: u32,
    /// Provider the entry configures.
    name: ProviderId,
}

impl VideoProvider {
    /// Return the provider's API key.
    #[must_use]
    pub(crate) fn api_key(&self) -> &ApiKey {
        &self.api_key
    }

    /// Return the position of the provider in the failover order.
    #[must_use]
    pub(crate) fn index(&self) -> u32 {
        self.index
    }

    /// Return the provider the entry configures.
    #[must_use]
    pub(crate) fn name(&self) -> ProviderId {
        self.name
    }

    /// Initialise a new `VideoProvider`, validating every setting.
    ///
    /// # Errors
    ///
    /// Returns [`VideoProvidersError::UnknownProvider`] when `name` is not a
    /// supported provider, [`VideoProvidersError::InvalidIndex`] when `index` is
    /// zero, and [`VideoProvidersError::ApiKeyEmpty`] when `api_key` is empty.
    pub(crate) fn try_new(
        name: &str,
        index: u32,
        api_key: String,
    ) -> Result<Self, VideoProvidersError> {
        let provider = ProviderId::parse(name).ok_or(VideoProvidersError::UnknownProvider)?;
        if provider.serves_music() {
            return Err(VideoProvidersError::UnsupportedProvider);
        }
        if index == 0 {
            return Err(VideoProvidersError::InvalidIndex);
        }
        let key = ApiKey::new(api_key);
        if key.is_empty() {
            return Err(VideoProvidersError::ApiKeyEmpty);
        }
        Ok(Self {
            api_key: key,
            index,
            name: provider,
        })
    }
}

impl Debug for VideoProvider {
    /// Redact the API key (`OBS-004`) while keeping the entry readable.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("VideoProvider")
            .field("api_key", &"**")
            .field("index", &self.index)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// Video metadata settings of the Library bounded context.
///
/// Constructed through [`VideoProviders::try_new`], which validates the list; an
/// empty list is valid and means identification is unavailable.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub(crate) struct VideoProviders {
    /// Configured providers, ordered by their index.
    providers: Vec<VideoProvider>,
}

impl VideoProviders {
    /// Return the configured providers, ordered by their index.
    #[must_use]
    pub(crate) fn providers(&self) -> &[VideoProvider] {
        &self.providers
    }

    /// Initialise a new `VideoProviders`, validating `providers` and ordering
    /// them by index.
    ///
    /// # Errors
    ///
    /// Returns [`VideoProvidersError::DuplicateProvider`] when two entries share
    /// a provider name, and [`VideoProvidersError::DuplicateIndex`] when two
    /// entries share an index.
    pub(crate) fn try_new(providers: Vec<VideoProvider>) -> Result<Self, VideoProvidersError> {
        let mut indices: Vec<u32> = Vec::with_capacity(providers.len());
        let mut names: Vec<ProviderId> = Vec::with_capacity(providers.len());
        for provider in &providers {
            if indices.contains(&provider.index()) {
                return Err(VideoProvidersError::DuplicateIndex);
            }
            if names.contains(&provider.name()) {
                return Err(VideoProvidersError::DuplicateProvider);
            }
            indices.push(provider.index());
            names.push(provider.name());
        }
        let mut sorted = providers;
        sorted.sort_by_key(VideoProvider::index);
        Ok(Self { providers: sorted })
    }
}

/// Error returned when initialising a music metadata provider or its list.
///
/// The message names the setting and never a value (`OBS-004`).
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum MusicProvidersError {
    /// Two entries share an index.
    #[error("library.metadata.music.providers index must be unique")]
    DuplicateIndex,
    /// Two entries share a provider name.
    #[error("library.metadata.music.providers name must be unique")]
    DuplicateProvider,
    /// An entry's index is zero.
    #[error("library.metadata.music.providers index must be greater than zero")]
    InvalidIndex,
    /// An entry names a provider the system does not support.
    #[error("library.metadata.music.providers name must be a supported provider")]
    UnknownProvider,
    /// An entry names a provider the section cannot use.
    #[error("library.metadata.music.providers name must be a music provider")]
    UnsupportedProvider,
}

/// One configured music metadata provider.
///
/// `MusicBrainz` takes no API key, so a music entry carries only its position and
/// name: there is no credential field to leave empty or redact.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub(crate) struct MusicProvider {
    /// Position of the provider in the resolution order; positive and unique.
    index: u32,
    /// Provider the entry configures.
    name: ProviderId,
}

impl MusicProvider {
    /// Return the position of the provider in the resolution order.
    #[must_use]
    pub(crate) fn index(&self) -> u32 {
        self.index
    }

    /// Return the provider the entry configures.
    #[must_use]
    pub(crate) fn name(&self) -> ProviderId {
        self.name
    }

    /// Initialise a new `MusicProvider`, validating every setting.
    ///
    /// # Errors
    ///
    /// Returns [`MusicProvidersError::UnknownProvider`] when `name` is not a
    /// supported provider, [`MusicProvidersError::UnsupportedProvider`] when it
    /// is a provider that serves video rather than music, and
    /// [`MusicProvidersError::InvalidIndex`] when `index` is zero.
    pub(crate) fn try_new(name: &str, index: u32) -> Result<Self, MusicProvidersError> {
        let provider = ProviderId::parse(name).ok_or(MusicProvidersError::UnknownProvider)?;
        if !provider.serves_music() {
            return Err(MusicProvidersError::UnsupportedProvider);
        }
        if index == 0 {
            return Err(MusicProvidersError::InvalidIndex);
        }
        Ok(Self {
            index,
            name: provider,
        })
    }
}

/// Music metadata settings of the Library bounded context.
///
/// Constructed through [`MusicProviders::try_new`], which validates the list; an
/// empty list is valid and means music identification is unavailable.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub(crate) struct MusicProviders {
    /// Configured providers, ordered by their index.
    providers: Vec<MusicProvider>,
}

impl MusicProviders {
    /// Return the configured providers, ordered by their index.
    #[must_use]
    pub(crate) fn providers(&self) -> &[MusicProvider] {
        &self.providers
    }

    /// Initialise a new `MusicProviders`, validating `providers` and ordering
    /// them by index.
    ///
    /// # Errors
    ///
    /// Returns [`MusicProvidersError::DuplicateProvider`] when two entries share
    /// a provider name, and [`MusicProvidersError::DuplicateIndex`] when two
    /// entries share an index.
    pub(crate) fn try_new(providers: Vec<MusicProvider>) -> Result<Self, MusicProvidersError> {
        let mut indices: Vec<u32> = Vec::with_capacity(providers.len());
        let mut names: Vec<ProviderId> = Vec::with_capacity(providers.len());
        for provider in &providers {
            if indices.contains(&provider.index()) {
                return Err(MusicProvidersError::DuplicateIndex);
            }
            if names.contains(&provider.name()) {
                return Err(MusicProvidersError::DuplicateProvider);
            }
            indices.push(provider.index());
            names.push(provider.name());
        }
        let mut sorted = providers;
        sorted.sort_by_key(MusicProvider::index);
        Ok(Self { providers: sorted })
    }
}

/// Metadata settings of the Library bounded context.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub(crate) struct LibraryMetadata {
    /// Music metadata settings.
    music: MusicProviders,
    /// Video metadata settings.
    video: VideoProviders,
}

impl LibraryMetadata {
    /// Return the music metadata settings.
    #[must_use]
    pub(crate) fn music(&self) -> &MusicProviders {
        &self.music
    }

    /// Initialise a new `LibraryMetadata`.
    #[must_use]
    pub(crate) fn new(music: MusicProviders, video: VideoProviders) -> Self {
        Self { music, video }
    }

    /// Return the video metadata settings.
    #[must_use]
    pub(crate) fn video(&self) -> &VideoProviders {
        &self.video
    }
}

/// Error returned when initialising a [`LibraryStartup`] value object.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum LibraryStartupError {
    /// The wait is longer than one day.
    #[error("library startup media_root_wait_seconds must be at most 86400")]
    MediaRootWaitOutOfRange,
}

/// Startup policy for an absent library root.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub(crate) struct LibraryStartup {
    /// Whether startup refuses to run without the library root.
    fail_without_media_root: bool,
    /// Seconds startup waits for the library root to appear.
    media_root_wait_seconds: u64,
}

impl Default for LibraryStartup {
    fn default() -> Self {
        Self {
            fail_without_media_root: false,
            media_root_wait_seconds: DEFAULT_MEDIA_ROOT_WAIT_SECONDS,
        }
    }
}

impl LibraryStartup {
    /// Return whether startup refuses to run without the library root.
    #[must_use]
    pub(crate) fn fail_without_media_root(&self) -> bool {
        self.fail_without_media_root
    }

    /// Return the seconds startup waits for the library root to appear.
    #[must_use]
    pub(crate) fn media_root_wait_seconds(&self) -> u64 {
        self.media_root_wait_seconds
    }

    /// Initialise a new `LibraryStartup`.
    ///
    /// A zero wait is valid: it decides immediately.
    ///
    /// # Errors
    ///
    /// Returns [`LibraryStartupError::MediaRootWaitOutOfRange`] when
    /// `media_root_wait_seconds` exceeds one day, so the startup deadline stays
    /// representable as an `Instant`.
    pub(crate) fn try_new(
        media_root_wait_seconds: u64,
        fail_without_media_root: bool,
    ) -> Result<Self, LibraryStartupError> {
        if media_root_wait_seconds > MAX_MEDIA_ROOT_WAIT_SECONDS {
            return Err(LibraryStartupError::MediaRootWaitOutOfRange);
        }
        Ok(Self {
            fail_without_media_root,
            media_root_wait_seconds,
        })
    }
}

/// Library-related settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub(crate) struct Library {
    /// Metadata settings.
    metadata: LibraryMetadata,
    /// Startup policy for an absent library root.
    startup: LibraryStartup,
}

impl Library {
    /// Return the metadata settings.
    #[must_use]
    pub(crate) fn metadata(&self) -> &LibraryMetadata {
        &self.metadata
    }

    /// Initialise a new `Library`.
    #[must_use]
    pub(crate) fn new(metadata: LibraryMetadata, startup: LibraryStartup) -> Self {
        Self { metadata, startup }
    }

    /// Return the startup policy for an absent library root.
    #[must_use]
    pub(crate) fn startup(&self) -> &LibraryStartup {
        &self.startup
    }
}

/// The application configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub(crate) struct Configuration {
    /// Asset-related settings.
    asset: Asset,
    /// Library-related settings.
    library: Library,
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
    pub(crate) fn asset(&self) -> &Asset {
        &self.asset
    }

    /// Return the library-related settings.
    #[must_use]
    pub(crate) fn library(&self) -> &Library {
        &self.library
    }

    /// Return the logging-related settings.
    #[must_use]
    pub(crate) fn logging(&self) -> &Logging {
        &self.logging
    }

    /// Initialise a new `Configuration`.
    #[must_use]
    pub(crate) fn new(
        persistence: Persistence,
        security: Security,
        logging: Logging,
        asset: Asset,
        library: Library,
    ) -> Self {
        Self {
            asset,
            library,
            logging,
            persistence,
            security,
        }
    }

    /// Return the persistence-related settings.
    #[must_use]
    pub(crate) fn persistence(&self) -> &Persistence {
        &self.persistence
    }

    /// Return a mutable reference to the persistence-related settings.
    #[must_use]
    fn persistence_mut(&mut self) -> &mut Persistence {
        &mut self.persistence
    }

    /// Return the security-related settings.
    #[must_use]
    pub(crate) fn security(&self) -> &Security {
        &self.security
    }

    /// Return a mutable reference to the security-related settings.
    #[must_use]
    fn security_mut(&mut self) -> &mut Security {
        &mut self.security
    }
}

/// Parse the comma-separated trusted-proxy list.
///
/// # Errors
///
/// Returns [`RateLimitError::InvalidTrustedProxy`] when a non-empty entry is
/// not a valid IP address.
pub(crate) fn parse_trusted_proxies(value: &str) -> Result<Vec<IpAddr>, RateLimitError> {
    value
        .split(TRUSTED_PROXIES_SEPARATOR)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| IpAddr::from_str(entry).map_err(|_| RateLimitError::InvalidTrustedProxy))
        .collect()
}
