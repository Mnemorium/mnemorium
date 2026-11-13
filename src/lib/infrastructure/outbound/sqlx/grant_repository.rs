use sqlx::{Sqlite, Transaction};

use crate::domain::NumericID;
use crate::domain::model::{Grant, Principal};
use crate::domain::port::{GrantFilter, GrantRepository, RepositoryError};

use super::model::GrantRow;
use super::resource_repository::stored_grant;

/// Upsert of a user grant, conflicting on its partial unique index.
const USER_GRANT_UPSERT: &str = "INSERT INTO grant (
        principal_kind, principal_id, principal_role, resource_id,
        role, child_kind, effect
    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
    ON CONFLICT (principal_id, resource_id, role, child_kind, effect)
        WHERE principal_kind = 'USER'
    DO UPDATE SET principal_kind = excluded.principal_kind
    RETURNING grant_id, principal_kind, principal_id, principal_role,
              resource_id, role, child_kind, effect";

/// Upsert of a role grant, conflicting on its partial unique index.
const ROLE_GRANT_UPSERT: &str = "INSERT INTO grant (
        principal_kind, principal_id, principal_role, resource_id,
        role, child_kind, effect
    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
    ON CONFLICT (principal_role, resource_id, role, child_kind, effect)
        WHERE principal_kind = 'ROLE'
    DO UPDATE SET principal_kind = excluded.principal_kind
    RETURNING grant_id, principal_kind, principal_id, principal_role,
              resource_id, role, child_kind, effect";

/// Repository reading and writing grants, backed by a `SQLite` transaction.
pub(crate) struct SqlxGrantRepository<'transaction> {
    /// Transaction the repository reads from and writes to.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxGrantRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub(crate) fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }
}

impl GrantRepository for SqlxGrantRepository<'_> {
    async fn delete(
        &mut self,
        grant_id: NumericID,
        resource_id: NumericID,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM grant WHERE grant_id = ?1 AND resource_id = ?2")
            .bind(grant_id)
            .bind(resource_id)
            .execute(&mut **self.transaction)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn save_grant(&mut self, grant: Grant) -> Result<Grant, RepositoryError> {
        let (kind, id, role) = match grant.principal() {
            Principal::Role(role) => ("ROLE", None, Some(role.as_wire())),
            Principal::Root => return Err(RepositoryError::DataIntegrityViolation),
            Principal::User(id) => ("USER", Some(id), None),
        };
        // The grant's natural key is a partial unique index, so each statement
        // names its own index predicate as the conflict target.
        let statement = if kind == "USER" {
            USER_GRANT_UPSERT
        } else {
            ROLE_GRANT_UPSERT
        };
        let row = sqlx::query_as::<_, GrantRow>(statement)
            .bind(kind)
            .bind(id)
            .bind(role)
            .bind(grant.resource_id())
            .bind(grant.role().as_wire())
            .bind(grant.child_kind().as_wire())
            .bind(grant.effect().as_wire())
            .fetch_one(&mut **self.transaction)
            .await?;
        stored_grant(row)
    }

    async fn search(&mut self, filter: &GrantFilter) -> Result<Vec<Grant>, RepositoryError> {
        let (kind, user_id, role) = match filter.principal {
            None => (None, None, None),
            Some(Principal::Role(role)) => (Some("ROLE"), None, Some(role.as_wire())),
            Some(Principal::Root) => (Some("ROOT"), None, None),
            Some(Principal::User(id)) => (Some("USER"), Some(id), None),
        };
        let rows = sqlx::query_as::<_, GrantRow>(
            "SELECT grant_id, principal_kind, principal_id, principal_role,
                    resource_id, role, child_kind, effect
             FROM grant
             WHERE (?1 IS NULL OR resource_id = ?1)
               AND (
                   ?2 IS NULL
                   OR (?2 = 'USER' AND principal_kind = 'USER' AND principal_id = ?3)
                   OR (?2 = 'ROLE' AND principal_kind = 'ROLE' AND principal_role = ?4)
               )
             ORDER BY grant_id",
        )
        .bind(filter.resource_id)
        .bind(kind)
        .bind(user_id)
        .bind(role)
        .fetch_all(&mut **self.transaction)
        .await?;
        rows.into_iter().map(stored_grant).collect()
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;
    use crate::domain::model::{
        ChildKind, GrantEffect, Principal, ResourceKind, ResourceRole, Role,
    };
    use crate::infrastructure::outbound::sqlx::test_helpers::begin_transaction;

    /// Insert one resource and return its surrogate identifier.
    async fn seed_resource(
        transaction: &mut Transaction<'static, Sqlite>,
        entity_id: NumericID,
    ) -> Result<NumericID, sqlx::Error> {
        let (resource_id,) = sqlx::query_as::<_, (NumericID,)>(
            "INSERT INTO resource (kind, entity_id, owner_id, visibility)
             VALUES (?1, ?2, NULL, 'PRIVATE')
             RETURNING resource_id",
        )
        .bind(ResourceKind::Gallery.as_wire())
        .bind(entity_id)
        .fetch_one(&mut **transaction)
        .await?;
        Ok(resource_id)
    }

    fn viewer(user_id: NumericID, resource_id: NumericID) -> Result<Grant, Box<dyn Error>> {
        Ok(Grant::try_new(
            Principal::User(user_id),
            resource_id,
            ResourceRole::Viewer,
            ChildKind::All,
            GrantEffect::Allow,
        )?)
    }

    #[tokio::test]
    async fn save_then_search_round_trips_a_grant() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let resource_id = seed_resource(&mut transaction, 10).await?;
        let mut repository = SqlxGrantRepository::new(&mut transaction);

        // Act
        let saved = repository.save_grant(viewer(5, resource_id)?).await?;
        let found = repository
            .search(&GrantFilter {
                principal: Some(Principal::User(5)),
                resource_id: Some(resource_id),
            })
            .await?;

        // Assert
        assert_eq!(found.len(), 1);
        assert_eq!(found.first().map(Grant::id), Some(saved.id()));
        assert_eq!(found.first().map(Grant::role), Some(ResourceRole::Viewer));
        Ok(())
    }

    #[tokio::test]
    async fn saving_the_same_grant_twice_updates_it_in_place() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let resource_id = seed_resource(&mut transaction, 10).await?;
        let mut repository = SqlxGrantRepository::new(&mut transaction);

        // Act
        let first = repository.save_grant(viewer(5, resource_id)?).await?;
        let second = repository.save_grant(viewer(5, resource_id)?).await?;
        let found = repository
            .search(&GrantFilter {
                principal: None,
                resource_id: Some(resource_id),
            })
            .await?;

        // Assert
        assert_eq!(first.id(), second.id());
        assert_eq!(found.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn the_datastore_refuses_an_owner_grant_naming_a_user() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let resource_id = seed_resource(&mut transaction, 10).await?;
        let repository = SqlxGrantRepository::new(&mut transaction);

        // Act
        let result = sqlx::query(
            "INSERT INTO grant (
                 principal_kind, principal_id, principal_role, resource_id,
                 role, child_kind, effect
             ) VALUES ('USER', 5, NULL, ?1, 'OWNER', 'ALL', 'ALLOW')",
        )
        .bind(resource_id)
        .execute(&mut **repository.transaction)
        .await;

        // Assert
        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn delete_removes_only_the_addressed_grant() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let resource_id = seed_resource(&mut transaction, 10).await?;
        let other_id = seed_resource(&mut transaction, 11).await?;
        let mut repository = SqlxGrantRepository::new(&mut transaction);
        let saved = repository.save_grant(viewer(5, resource_id)?).await?;
        let foreign = repository.save_grant(viewer(5, other_id)?).await?;

        // Act
        let removed = repository.delete(saved.id(), resource_id).await?;
        let wrong = repository.delete(foreign.id(), resource_id).await?;
        let found = repository
            .search(&GrantFilter {
                principal: None,
                resource_id: Some(resource_id),
            })
            .await?;

        // Assert
        assert!(removed);
        assert!(!wrong);
        assert!(found.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn search_filters_by_principal_role() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let resource_id = seed_resource(&mut transaction, 10).await?;
        let mut repository = SqlxGrantRepository::new(&mut transaction);
        repository
            .save_grant(Grant::try_new(
                Principal::Role(Role::Standard),
                resource_id,
                ResourceRole::Viewer,
                ChildKind::All,
                GrantEffect::Allow,
            )?)
            .await?;

        // Act
        let found = repository
            .search(&GrantFilter {
                principal: Some(Principal::Role(Role::Standard)),
                resource_id: Some(resource_id),
            })
            .await?;

        // Assert
        assert_eq!(found.len(), 1);
        assert_eq!(
            found.first().map(Grant::principal),
            Some(Principal::Role(Role::Standard))
        );
        Ok(())
    }
}
