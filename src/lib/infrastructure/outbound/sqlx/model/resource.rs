use crate::domain::NumericID;

/// Data model for the `resource` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct ResourceRow {
    /// Identifier of the authorization parent.
    pub auth_parent_id: Option<NumericID>,
    /// Identifier of the entity in its own table.
    pub entity_id: NumericID,
    /// Stored kind of the resource.
    pub kind: String,
    /// Identifier of the owning user.
    pub owner_id: Option<NumericID>,
    /// Surrogate identifier of the resource.
    pub resource_id: NumericID,
    /// Stored visibility of the resource.
    pub visibility: String,
}

/// Data model for the `grant` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct GrantRow {
    /// Stored child kind.
    pub child_kind: String,
    /// Stored effect.
    pub effect: String,
    /// Surrogate identifier of the grant.
    pub grant_id: NumericID,
    /// Identifier of the principal user, when the principal is a user.
    pub principal_id: Option<NumericID>,
    /// Stored principal kind.
    pub principal_kind: String,
    /// Stored principal role, when the principal is a role.
    pub principal_role: Option<String>,
    /// Identifier of the addressed resource.
    pub resource_id: NumericID,
    /// Stored resource role.
    pub role: String,
}

/// Data model for one relation edge and its container kind.
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct RelationRow {
    /// Identifier of the container resource.
    pub from_resource_id: NumericID,
    /// Stored kind of the relation.
    pub kind: String,
}
