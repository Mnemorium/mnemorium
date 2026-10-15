use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use crate::domain::alias::NumericID;

/// Rotation period of the log files, matching the `chk_configuration_log_rotation`
/// check constraint in the `configuration` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "UPPERCASE")]
#[non_exhaustive]
pub enum LogRotation {
    Daily,
    Hourly,
    Minutely,
    Never,
}

/// Data model for the `configuration` table.
#[derive(Clone, sqlx::FromRow)]
pub struct Configuration {
    pub asset_storage_root: String,
    pub asset_upload_chunk_size_bytes: i64,
    pub asset_upload_expiry_seconds: i64,
    pub asset_upload_max_file_size_bytes: i64,
    #[sqlx(primary_key)]
    pub configuration_id: NumericID,
    pub is_log_ansi: bool,
    pub is_root_admin_password_logged: bool,
    pub jwt_secret: String,
    pub jwt_ttl: i64,
    pub log_level: String,
    pub log_max_files: i64,
    pub log_rotation: LogRotation,
    pub pepper: String,
    pub rate_limit_burst_size: i64,
    pub rate_limit_client_ip_header: String,
    pub rate_limit_period_seconds: i64,
    pub rate_limit_trusted_proxies: String,
    pub sqlite3_max_connections: i64,
    pub sqlite3_path: String,
}

impl Debug for Configuration {
    /// Redact the secret-bearing columns (`OBS-004`): the JWT signing secret
    /// and the pepper never reach a log sink, while every identifier and plain
    /// setting stays readable.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("Configuration")
            .field("asset_storage_root", &self.asset_storage_root)
            .field(
                "asset_upload_chunk_size_bytes",
                &self.asset_upload_chunk_size_bytes,
            )
            .field(
                "asset_upload_expiry_seconds",
                &self.asset_upload_expiry_seconds,
            )
            .field(
                "asset_upload_max_file_size_bytes",
                &self.asset_upload_max_file_size_bytes,
            )
            .field("configuration_id", &self.configuration_id)
            .field("is_log_ansi", &self.is_log_ansi)
            .field(
                "is_root_admin_password_logged",
                &self.is_root_admin_password_logged,
            )
            .field("jwt_secret", &"**")
            .field("jwt_ttl", &self.jwt_ttl)
            .field("log_level", &self.log_level)
            .field("log_max_files", &self.log_max_files)
            .field("log_rotation", &self.log_rotation)
            .field("pepper", &"**")
            .field("rate_limit_burst_size", &self.rate_limit_burst_size)
            .field(
                "rate_limit_client_ip_header",
                &self.rate_limit_client_ip_header,
            )
            .field("rate_limit_period_seconds", &self.rate_limit_period_seconds)
            .field(
                "rate_limit_trusted_proxies",
                &self.rate_limit_trusted_proxies,
            )
            .field("sqlite3_max_connections", &self.sqlite3_max_connections)
            .field("sqlite3_path", &self.sqlite3_path)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::Configuration;
    use super::LogRotation;

    #[test]
    fn configuration_debug_redacts_the_secrets_but_keeps_the_path() {
        // Arrange: a `configuration` row whose secret-bearing columns hold
        // unique sentinels.
        let row = Configuration {
            asset_storage_root: "/storage".to_owned(),
            asset_upload_chunk_size_bytes: 4,
            asset_upload_expiry_seconds: 3600,
            asset_upload_max_file_size_bytes: 1_000_000,
            configuration_id: 0,
            is_log_ansi: false,
            is_root_admin_password_logged: false,
            jwt_secret: "LEAK_JWT_SECRET".to_owned(),
            jwt_ttl: 3600,
            log_level: "info".to_owned(),
            log_max_files: 7,
            log_rotation: LogRotation::Daily,
            pepper: "LEAK_PEPPER".to_owned(),
            rate_limit_burst_size: 10,
            rate_limit_client_ip_header: "x-forwarded-for".to_owned(),
            rate_limit_period_seconds: 60,
            rate_limit_trusted_proxies: String::new(),
            sqlite3_max_connections: 1,
            sqlite3_path: "mnemorium.db".to_owned(),
        };

        // Act
        let debug = format!("{row:?}");

        // Assert
        assert!(
            !debug.contains("LEAK_JWT_SECRET"),
            "Debug output must not contain the JWT signing secret, got: {debug}"
        );
        assert!(
            !debug.contains("LEAK_PEPPER"),
            "Debug output must not contain the pepper, got: {debug}"
        );
        assert!(
            debug.contains("mnemorium.db"),
            "Debug output must still contain a plain setting, got: {debug}"
        );
    }
}
