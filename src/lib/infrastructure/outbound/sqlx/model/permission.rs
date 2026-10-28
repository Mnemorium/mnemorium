use crate::domain::alias::NumericID;

/// Data model for the `permission` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PermissionRow {
    /// Stored name of the overridden action.
    pub action_name: String,
    /// Unique identifier of the permission.
    pub permission_id: NumericID,
    /// Stored scope of the override.
    pub scope: String,
    /// Identifier of the user the permission applies to.
    pub user_id: NumericID,
}

/// Data model for the `permission_resource` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PermissionResourceRow {
    /// Identifier of the named aggregate.
    pub resource_id: NumericID,
    /// Stored kind of the named aggregate.
    pub resource_type: String,
}
