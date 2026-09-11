use crate::domain::alias::NumericID;

/// Data model for the `configuration` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Configuration {
    #[sqlx(primary_key)]
    pub configuration_id: NumericID,
    pub jwt_secret: String,
    pub jwt_ttl: i64,
    pub pepper: String,
    pub sqlite3_max_connections: i64,
    pub sqlite3_path: String,
}
