use crate::domain::alias::NumericID;

/// Rotation period of the log files, matching the `chk_configuration_log_rotation`
/// check constraint in the `configuration` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "UPPERCASE")]
#[non_exhaustive]
pub enum Rotation {
    Daily,
    Hourly,
    Minutely,
    Never,
}

/// Data model for the `configuration` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Configuration {
    #[sqlx(primary_key)]
    pub configuration_id: NumericID,
    pub jwt_secret: String,
    pub jwt_ttl: i64,
    pub log_ansi: bool,
    pub log_level: String,
    pub log_max_files: i64,
    pub log_root_admin_password: bool,
    pub log_rotation: Rotation,
    pub pepper: String,
    pub sqlite3_max_connections: i64,
    pub sqlite3_path: String,
}
