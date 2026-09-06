use crate::domain::alias::NumericID;

/// Role of an application user, matching the `chk_user_role` check
/// constraint in the `user` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "UPPERCASE")]
#[non_exhaustive]
pub enum Role {
    Admin,
    Standard,
}

/// Data model for the `user` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub credential_id: NumericID,
    pub email: String,
    pub role: Role,
    #[sqlx(primary_key)]
    pub user_id: NumericID,
    pub username: String,
}
