use sqlx::{QueryBuilder, Sqlite, Transaction};

use crate::domain::NumericID;
use crate::domain::model::{Grant, Principal};
use crate::domain::port::{GrantFilter, GrantRepository, RepositoryError};

use super::model::GrantRow;
use super::query_filter::push_filter;
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
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT grant_id, principal_kind, principal_id, principal_role,
                    resource_id, role, child_kind, effect
             FROM grant",
        );
        let mut first = true;
        if let Some(resource_id) = filter.resource_id {
            push_filter(&mut first, &mut builder, "resource_id = ", resource_id);
        }
        match filter.principal {
            // `chk_grant_principal_kind` stores only `USER` and `ROLE` rows, so
            // the Root Admin — who holds every action without a grant — matches
            // none.
            Some(Principal::Root) => return Ok(Vec::new()),
            None => {}
            Some(Principal::Role(role)) => {
                push_filter(&mut first, &mut builder, "principal_kind = ", "ROLE");
                push_filter(
                    &mut first,
                    &mut builder,
                    "principal_role = ",
                    role.as_wire(),
                );
            }
            Some(Principal::User(user_id)) => {
                push_filter(&mut first, &mut builder, "principal_kind = ", "USER");
                push_filter(&mut first, &mut builder, "principal_id = ", user_id);
            }
        }
        builder.push(" ORDER BY grant_id");
        let rows = builder
            .build_query_as::<GrantRow>()
            .fetch_all(&mut **self.transaction)
            .await?;
        rows.into_iter().map(stored_grant).collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::infrastructure::outbound::sqlx::test_helpers::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::error::Error;

    use super::*;
    use crate::domain::model::{ChildKind, GrantEffect, Principal, ResourceRole, Role};
    use crate::infrastructure::outbound::sqlx::test_helpers::begin_transaction;

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

    #[tokio::test]
    async fn the_authorization_migration_backfills_every_entity() -> Result<(), Box<dyn Error>> {
        // Arrange: a database from before the resource graph, holding one account,
        // one public gallery and one private playlist.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        let migrator = sqlx::migrate!("./migrations");
        migrator.run_to(20_261_105_090_000, &pool).await?;
        sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (5, 'hash-5')")
            .execute(&pool)
            .await?;
        sqlx::query(
            "INSERT INTO user (user_id, role, username, email, credential_id)
             VALUES (5, 'STANDARD', 'user5', NULL, 5)",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "INSERT INTO gallery (gallery_id, name, created_at, last_modified_at, is_public, user_id)
             VALUES (10, 'holiday', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 1, 5)",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "INSERT INTO music_playlist (music_playlist_id, name, user_id, is_public)
             VALUES (12, 'mix', 5, 0)",
        )
        .execute(&pool)
        .await?;

        // Act
        migrator.run(&pool).await?;

        // Assert
        let gallery: (Option<i64>, String) = sqlx::query_as(
            "SELECT owner_id, visibility FROM resource WHERE kind = 'GALLERY' AND entity_id = 10",
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(gallery, (Some(5), "PUBLIC".to_owned()));

        let playlist: (Option<i64>, String) = sqlx::query_as(
            "SELECT owner_id, visibility FROM resource
             WHERE kind = 'MUSIC_PLAYLIST' AND entity_id = 12",
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(playlist, (Some(5), "PRIVATE".to_owned()));

        let user: (Option<i64>, String, i64) = sqlx::query_as(
            "SELECT owner_id, visibility, auth_parent_id FROM resource
             WHERE kind = 'USER' AND entity_id = 5",
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(user, (None, "PRIVATE".to_owned(), 1));

        let (grants,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM grant")
            .fetch_one(&pool)
            .await?;
        assert!(
            grants >= 9,
            "the seeded role grants must be stored, got {grants}"
        );

        pool.close().await;
        Ok(())
    }
}
