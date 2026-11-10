use crate::domain::NumericID;

/// Role of an application user, matching the `chk_user_role` check
/// constraint in the `user` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "UPPERCASE")]
#[non_exhaustive]
pub(crate) enum Role {
    /// A user who may administer the instance.
    Admin,
    /// A regular user.
    Standard,
}

/// Data model for the `user` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct User {
    /// Identifier of the `credential` row holding the password hash.
    pub credential_id: NumericID,
    /// Optional contact address.
    pub email: Option<String>,
    /// Role the user holds.
    pub role: Role,
    /// Primary key of the `user` row.
    #[sqlx(primary_key)]
    pub user_id: NumericID,
    /// Unique login name.
    pub username: String,
}
