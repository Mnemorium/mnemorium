use sqlx::QueryBuilder;
use sqlx::Sqlite;
use sqlx::Transaction;

use crate::domain::alias::NumericID;
use crate::domain::model::permission::Action;
use crate::domain::model::permission::Permission;
use crate::domain::model::permission::ResourceKind;
use crate::domain::model::permission::ResourceRef;
use crate::domain::model::permission::Scope;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::permission_repository::PermissionFilter;
use crate::domain::port::permission_repository::PermissionRepository;

use super::error_mapping::stored_data_invalid;
use super::model::permission::PermissionResourceRow;
use super::model::permission::PermissionRow;
use super::query_filter::push_filter;

/// Repository persisting permission overrides, backed by a `SQLite`
/// transaction.
pub struct SqlxPermissionRepository<'transaction> {
    /// Transaction the repository reads from and writes to.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxPermissionRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }
}

impl PermissionRepository for SqlxPermissionRepository<'_> {
    async fn delete(
        &mut self,
        user_id: NumericID,
        action: Action,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM permission WHERE user_id = ?1 AND action_name = ?2")
            .bind(user_id)
            .bind(action.as_wire())
            .execute(&mut **self.transaction)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn delete_all(&mut self, user_id: NumericID) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM permission WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut **self.transaction)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn save(&mut self, permission: Permission) -> Result<Permission, RepositoryError> {
        // A permission is identified by its surrogate `permission_id`; the
        // `(user_id, action_name)` pair stays unique, and the sub-select
        // resolves it to the row the upsert targets.
        let row = sqlx::query_as::<_, PermissionRow>(
            "INSERT INTO permission (permission_id, user_id, action_name, scope)
             VALUES (
                 (
                     SELECT permission_id
                     FROM permission
                     WHERE user_id = ?1 AND action_name = ?2
                 ),
                 ?1,
                 ?2,
                 ?3
             )
             ON CONFLICT (permission_id) DO UPDATE SET
                 scope = excluded.scope
             RETURNING permission_id, user_id, action_name, scope",
        )
        .bind(permission.user_id())
        .bind(permission.action().as_wire())
        .bind(permission.scope().as_wire())
        .fetch_one(&mut **self.transaction)
        .await?;

        sqlx::query("DELETE FROM permission_resource WHERE permission_id = ?")
            .bind(row.permission_id)
            .execute(&mut **self.transaction)
            .await?;

        let listed = permission.scope().clone();
        if let Scope::Listed(resources) = listed {
            for resource in resources {
                sqlx::query(
                    "INSERT INTO permission_resource (
                        permission_id,
                        resource_type,
                        resource_id
                     ) VALUES (?1, ?2, ?3)",
                )
                .bind(row.permission_id)
                .bind(resource.kind().as_wire())
                .bind(resource.id())
                .execute(&mut **self.transaction)
                .await?;
            }
        }

        let resources = self.resources(row.permission_id).await?;
        domain_permission(&row, resources)
    }

    async fn search(
        &mut self,
        filter: &PermissionFilter,
    ) -> Result<Vec<Permission>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT permission_id, user_id, action_name, scope FROM permission",
        );
        let mut first = true;
        if let Some(action) = filter.action {
            push_filter(&mut first, &mut builder, "action_name = ", action.as_wire());
        }
        if let Some(user_id) = filter.user_id {
            push_filter(&mut first, &mut builder, "user_id = ", user_id);
        }
        builder.push(" ORDER BY user_id, action_name");

        let rows = builder
            .build_query_as::<PermissionRow>()
            .fetch_all(&mut **self.transaction)
            .await?;

        let mut permissions = Vec::with_capacity(rows.len());
        for row in rows {
            let resources = self.resources(row.permission_id).await?;
            permissions.push(domain_permission(&row, resources)?);
        }
        Ok(permissions)
    }
}

impl SqlxPermissionRepository<'_> {
    /// Return the aggregates a stored `LISTED` permission names.
    async fn resources(
        &mut self,
        permission_id: NumericID,
    ) -> Result<Vec<ResourceRef>, RepositoryError> {
        let rows = sqlx::query_as::<_, PermissionResourceRow>(
            "SELECT resource_type, resource_id
             FROM permission_resource
             WHERE permission_id = ?1
             ORDER BY resource_type, resource_id",
        )
        .bind(permission_id)
        .fetch_all(&mut **self.transaction)
        .await?;

        rows.into_iter()
            .map(|row| {
                let kind = ResourceKind::from_wire(&row.resource_type)
                    .ok_or_else(|| stored_data_invalid("unknown_permission_resource_type"))?;
                Ok(ResourceRef::new(kind, row.resource_id))
            })
            .collect()
    }
}

/// Map a persisted permission row and its resources back to the domain model.
fn domain_permission(
    row: &PermissionRow,
    resources: Vec<ResourceRef>,
) -> Result<Permission, RepositoryError> {
    let action = Action::from_wire(&row.action_name)
        .ok_or_else(|| stored_data_invalid("unknown_permission_action"))?;
    let stored_scope = Scope::from_wire(&row.scope)
        .ok_or_else(|| stored_data_invalid("unknown_permission_scope"))?;
    let scope = match stored_scope {
        Scope::Listed(_) => Scope::Listed(resources),
        Scope::All | Scope::None | Scope::Owned | Scope::PublicOrOwned => stored_scope,
    };
    Permission::try_new(row.user_id, action, scope)
        .map_err(|_| stored_data_invalid("invalid_permission"))
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use sqlx::Sqlite;
    use sqlx::Transaction;
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::domain::model::permission::Action;
    use crate::domain::model::permission::Permission;
    use crate::domain::model::permission::ResourceKind;
    use crate::domain::model::permission::ResourceRef;
    use crate::domain::model::permission::Scope;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::permission_repository::PermissionFilter;
    use crate::domain::port::permission_repository::PermissionRepository as _;

    use super::PermissionRow;
    use super::SqlxPermissionRepository;
    use super::domain_permission;

    async fn begin_transaction() -> Result<Transaction<'static, Sqlite>, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        pool.begin().await
    }

    async fn seed_user(
        transaction: &mut Transaction<'static, Sqlite>,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (?1, ?2)")
            .bind(user_id)
            .bind(format!("hash{user_id}"))
            .execute(&mut **transaction)
            .await?;
        sqlx::query(
            "INSERT INTO user (user_id, role, username, credential_id)
             VALUES (?1, 'STANDARD', ?2, ?1)",
        )
        .bind(user_id)
        .bind(format!("user{user_id}"))
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    fn permission(
        user_id: i64,
        action: Action,
        scope: Scope,
    ) -> Result<Permission, Box<dyn Error>> {
        Ok(Permission::try_new(user_id, action, scope)?)
    }

    #[tokio::test]
    async fn save_then_search_round_trips_a_listed_scope() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 5).await?;
        let expected = permission(
            5,
            Action::PlaylistRead,
            Scope::Listed(vec![
                ResourceRef::new(ResourceKind::MusicPlaylist, 7),
                ResourceRef::new(ResourceKind::MusicPlaylist, 9),
            ]),
        )?;

        // Act
        let mut repository = SqlxPermissionRepository::new(&mut transaction);
        let persisted = repository.save(expected.clone()).await?;
        let found = repository
            .search(&PermissionFilter {
                user_id: Some(5),
                ..PermissionFilter::default()
            })
            .await?;

        // Assert
        assert_eq!(persisted, expected);
        assert_eq!(found.first().map(Permission::scope), Some(expected.scope()));
        assert_eq!(found.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn save_listed_replaces_the_child_set() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 5).await?;
        let first = permission(
            5,
            Action::PlaylistRead,
            Scope::Listed(vec![
                ResourceRef::new(ResourceKind::MusicPlaylist, 1),
                ResourceRef::new(ResourceKind::MusicPlaylist, 2),
            ]),
        )?;
        let second = permission(
            5,
            Action::PlaylistRead,
            Scope::Listed(vec![ResourceRef::new(ResourceKind::MusicPlaylist, 3)]),
        )?;

        // Act
        let mut repository = SqlxPermissionRepository::new(&mut transaction);
        repository.save(first).await?;
        let persisted = repository.save(second).await?;
        let stored_children: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM permission_resource")
            .fetch_one(&mut *transaction)
            .await?;

        // Assert
        assert_eq!(
            persisted.scope(),
            &Scope::Listed(vec![ResourceRef::new(ResourceKind::MusicPlaylist, 3)])
        );
        assert_eq!(stored_children, 1);
        Ok(())
    }

    #[tokio::test]
    async fn save_twice_updates_the_scope() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 5).await?;

        // Act
        let mut repository = SqlxPermissionRepository::new(&mut transaction);
        repository
            .save(permission(5, Action::GalleryRead, Scope::None)?)
            .await?;
        repository
            .save(permission(5, Action::GalleryRead, Scope::All)?)
            .await?;
        let found = repository
            .search(&PermissionFilter {
                action: Some(Action::GalleryRead),
                ..PermissionFilter::default()
            })
            .await?;

        // Assert
        assert_eq!(found.len(), 1);
        assert_eq!(found.first().map(Permission::scope), Some(&Scope::All));
        Ok(())
    }

    #[tokio::test]
    async fn delete_removes_the_permission_and_its_resources() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 5).await?;
        let mut repository = SqlxPermissionRepository::new(&mut transaction);
        repository
            .save(permission(
                5,
                Action::PlaylistRead,
                Scope::Listed(vec![ResourceRef::new(ResourceKind::MusicPlaylist, 1)]),
            )?)
            .await?;

        // Act
        let deleted = repository.delete(5, Action::PlaylistRead).await?;
        let missing = repository.delete(5, Action::PlaylistRead).await?;
        let stored_children: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM permission_resource")
            .fetch_one(&mut *transaction)
            .await?;

        // Assert
        assert!(deleted);
        assert!(!missing);
        assert_eq!(stored_children, 0);
        Ok(())
    }

    #[tokio::test]
    async fn delete_all_resets_every_permission_of_the_user() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 5).await?;
        seed_user(&mut transaction, 6).await?;
        let mut repository = SqlxPermissionRepository::new(&mut transaction);
        repository
            .save(permission(5, Action::GalleryRead, Scope::All)?)
            .await?;
        repository
            .save(permission(5, Action::PlaylistRead, Scope::None)?)
            .await?;
        repository
            .save(permission(6, Action::GalleryRead, Scope::All)?)
            .await?;

        // Act
        let deleted = repository.delete_all(5).await?;
        let remaining = repository.search(&PermissionFilter::default()).await?;

        // Assert
        assert!(deleted);
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining.first().map(Permission::user_id), Some(6));
        Ok(())
    }

    #[tokio::test]
    async fn save_root_admin_permission_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 0).await?;

        // Act
        let mut repository = SqlxPermissionRepository::new(&mut transaction);
        let result = repository
            .save(permission(0, Action::GalleryRead, Scope::All)?)
            .await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::Conflict)));
        Ok(())
    }

    #[tokio::test]
    async fn search_unknown_action_returns_data_integrity_violation() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 5).await?;
        sqlx::query(
            "INSERT INTO permission (permission_id, user_id, action_name, scope)
             VALUES (1, 5, 'bogus.read', 'ALL')",
        )
        .execute(&mut *transaction)
        .await?;

        // Act
        let mut repository = SqlxPermissionRepository::new(&mut transaction);
        let result = repository.search(&PermissionFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn search_unknown_resource_type_returns_data_integrity_violation()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 5).await?;
        sqlx::query(
            "INSERT INTO permission (permission_id, user_id, action_name, scope)
             VALUES (1, 5, 'gallery.read', 'LISTED')",
        )
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO permission_resource (permission_id, resource_type, resource_id)
             VALUES (1, 'BOGUS', 1)",
        )
        .execute(&mut *transaction)
        .await?;

        // Act
        let mut repository = SqlxPermissionRepository::new(&mut transaction);
        let result = repository.search(&PermissionFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        Ok(())
    }

    #[test]
    fn domain_permission_unknown_scope_returns_data_integrity_violation() {
        // Arrange
        let row = PermissionRow {
            permission_id: 1,
            action_name: "gallery.read".to_owned(),
            scope: "BOGUS".to_owned(),
            user_id: 5,
        };

        // Act
        let result = domain_permission(&row, Vec::new());

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
    }

    #[test]
    fn domain_permission_non_configurable_action_returns_data_integrity_violation() {
        // Arrange
        let row = PermissionRow {
            permission_id: 1,
            action_name: "upload.read".to_owned(),
            scope: "OWNED".to_owned(),
            user_id: 5,
        };

        // Act
        let result = domain_permission(&row, Vec::new());

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
    }
}
