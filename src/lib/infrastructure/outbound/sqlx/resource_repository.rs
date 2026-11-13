use std::collections::HashSet;

use sqlx::{Sqlite, Transaction};

use crate::domain::NumericID;
use crate::domain::model::{
    ChildKind, Grant, GrantEffect, MAX_AUTHORIZATION_DEPTH, Principal, RelationSource, Resource,
    ResourceAccess, ResourceKind, ResourceRef, ResourceRelationKind, ResourceRole, Role, User,
    Visibility,
};
use crate::domain::port::{NewResource, RepositoryError, ResourceFilter, ResourceRepository};

use super::error_mapping::stored_data_invalid;
use super::model::{GrantRow, RelationRow, ResourceRow};

/// Repository reading the resource graph and persisting its grants, backed by a
/// `SQLite` transaction.
pub(crate) struct SqlxResourceRepository<'transaction> {
    /// Transaction the repository reads from and writes to.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxResourceRepository<'transaction> {
    /// Load the ancestors of `resource`, nearest first.
    async fn ancestors(&mut self, resource: &Resource) -> Result<Vec<Resource>, RepositoryError> {
        let mut ancestors = Vec::new();
        let mut visited = HashSet::new();
        let mut current = resource.auth_parent_id();
        while let Some(parent_id) = current {
            if ancestors.len() >= MAX_AUTHORIZATION_DEPTH || !visited.insert(parent_id) {
                return Err(stored_data_invalid("resource_chain"));
            }
            let Some(parent) = self.by_id(parent_id).await? else {
                return Err(stored_data_invalid("resource_parent"));
            };
            current = parent.auth_parent_id();
            ancestors.push(parent);
        }
        Ok(ancestors)
    }

    /// Load one resource row by its surrogate identifier.
    async fn by_id(&mut self, resource_id: NumericID) -> Result<Option<Resource>, RepositoryError> {
        let row = sqlx::query_as::<_, ResourceRow>(
            "SELECT resource_id, kind, entity_id, owner_id, auth_parent_id, visibility
             FROM resource WHERE resource_id = ?1",
        )
        .bind(resource_id)
        .fetch_optional(&mut **self.transaction)
        .await?;
        row.as_ref().map(stored_resource).transpose()
    }

    /// Load the grants stored on `resource_ids` that apply to `caller`.
    async fn grants_on(
        &mut self,
        resource_ids: &[NumericID],
        caller: &User,
    ) -> Result<Vec<Grant>, RepositoryError> {
        let mut grants = Vec::new();
        for resource_id in resource_ids {
            let rows = sqlx::query_as::<_, GrantRow>(
                "SELECT grant_id, principal_kind, principal_id, principal_role,
                        resource_id, role, child_kind, effect
                 FROM grant
                 WHERE resource_id = ?1
                   AND (
                       (?2 IS NULL AND ?3 IS NULL)
                       OR (?2 IS NOT NULL AND principal_kind = 'USER' AND principal_id = ?2)
                       OR (?3 IS NOT NULL AND principal_kind = 'ROLE' AND principal_role = ?3)
                   )
                 ORDER BY grant_id",
            )
            .bind(resource_id)
            .bind(caller.id())
            .bind(caller.role().as_wire())
            .fetch_all(&mut **self.transaction)
            .await?;
            for row in rows {
                grants.push(stored_grant(row)?);
            }
        }
        Ok(grants)
    }

    /// Load the resources matching `filter`, ordered by identifier.
    async fn matching_resources(
        &mut self,
        filter: &ResourceFilter,
    ) -> Result<Vec<Resource>, RepositoryError> {
        let rows = sqlx::query_as::<_, ResourceRow>(
            "SELECT resource_id, kind, entity_id, owner_id, auth_parent_id, visibility
             FROM resource
             WHERE (?1 IS NULL OR kind = ?1)
               AND (?2 IS NULL OR entity_id = ?2)
               AND (?3 IS NULL OR owner_id = ?3)
               AND (?4 IS NULL OR visibility = ?4)
               AND (?5 IS NULL OR resource_id = ?5)
             ORDER BY resource_id",
        )
        .bind(filter.kind.map(ResourceKind::as_wire))
        .bind(filter.entity_id)
        .bind(filter.owner_id)
        .bind(filter.visibility.map(Visibility::as_wire))
        .bind(filter.resource_id)
        .fetch_all(&mut **self.transaction)
        .await?;
        rows.iter().map(stored_resource).collect()
    }

    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub(crate) fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }

    /// Load the relation containers that reference `resource`.
    async fn relation_sources(
        &mut self,
        resource: &Resource,
        caller: &User,
    ) -> Result<Vec<RelationSource>, RepositoryError> {
        let rows = sqlx::query_as::<_, RelationRow>(
            "SELECT from_resource_id, kind
             FROM resource_relation WHERE to_resource_id = ?1
             ORDER BY from_resource_id, kind",
        )
        .bind(resource.resource_id())
        .fetch_all(&mut **self.transaction)
        .await?;

        let mut sources = Vec::with_capacity(rows.len());
        for row in rows {
            ResourceRelationKind::from_wire(&row.kind)
                .ok_or_else(|| stored_data_invalid("resource_relation_kind"))?;
            let Some(container) = self.by_id(row.from_resource_id).await? else {
                return Err(stored_data_invalid("resource_relation_from"));
            };
            let ancestors = self.ancestors(&container).await?;
            let mut ids = Vec::with_capacity(ancestors.len().saturating_add(1));
            ids.push(container.resource_id());
            ids.extend(ancestors.iter().map(Resource::resource_id));
            let grants = self.grants_on(&ids, caller).await?;
            sources.push(RelationSource::new(container, ancestors, grants));
        }
        Ok(sources)
    }

    /// Resolve an address to its resource identifier.
    async fn resolve(
        &mut self,
        address: ResourceRef,
    ) -> Result<Option<NumericID>, RepositoryError> {
        let row = sqlx::query_as::<_, (NumericID,)>(
            "SELECT resource_id FROM resource WHERE kind = ?1 AND entity_id = ?2",
        )
        .bind(address.kind().as_wire())
        .bind(address.id())
        .fetch_optional(&mut **self.transaction)
        .await?;
        Ok(row.map(|(id,)| id))
    }
}

impl ResourceRepository for SqlxResourceRepository<'_> {
    async fn create(&mut self, resource: NewResource) -> Result<Resource, RepositoryError> {
        let parent_id = match resource.auth_parent {
            Some(parent_address) => self.resolve(parent_address).await?,
            None => None,
        };
        let row = sqlx::query_as::<_, ResourceRow>(
            "INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
             VALUES (?1, ?2, ?3, ?4, ?5)
             RETURNING resource_id, kind, entity_id, owner_id, auth_parent_id, visibility",
        )
        .bind(resource.kind.as_wire())
        .bind(resource.entity_id)
        .bind(resource.owner_id)
        .bind(parent_id)
        .bind(resource.visibility.as_wire())
        .fetch_one(&mut **self.transaction)
        .await?;
        stored_resource(&row)
    }

    async fn delete(&mut self, address: ResourceRef) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM resource WHERE kind = ?1 AND entity_id = ?2")
            .bind(address.kind().as_wire())
            .bind(address.id())
            .execute(&mut **self.transaction)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn reassign_owner(
        &mut self,
        address: ResourceRef,
        owner_id: Option<NumericID>,
        auth_parent_id: Option<NumericID>,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query(
            "UPDATE resource SET owner_id = ?1, auth_parent_id = ?2
             WHERE kind = ?3 AND entity_id = ?4",
        )
        .bind(owner_id)
        .bind(auth_parent_id)
        .bind(address.kind().as_wire())
        .bind(address.id())
        .execute(&mut **self.transaction)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn save(
        &mut self,
        resource: Resource,
        grants: &[Grant],
    ) -> Result<Resource, RepositoryError> {
        let row = sqlx::query_as::<_, ResourceRow>(
            "INSERT INTO resource (
                 resource_id, kind, entity_id, owner_id, auth_parent_id, visibility
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (resource_id) DO UPDATE SET
                 kind = excluded.kind,
                 entity_id = excluded.entity_id,
                 owner_id = excluded.owner_id,
                 auth_parent_id = excluded.auth_parent_id,
                 visibility = excluded.visibility
             RETURNING resource_id, kind, entity_id, owner_id, auth_parent_id, visibility",
        )
        .bind(resource.resource_id())
        .bind(resource.kind().as_wire())
        .bind(resource.entity_id())
        .bind(resource.owner_id())
        .bind(resource.auth_parent_id())
        .bind(resource.visibility().as_wire())
        .fetch_one(&mut **self.transaction)
        .await?;
        let saved = stored_resource(&row)?;
        sqlx::query("DELETE FROM grant WHERE resource_id = ?1 AND principal_kind = 'USER'")
            .bind(saved.resource_id())
            .execute(&mut **self.transaction)
            .await?;
        for grant in grants {
            insert_grant(self.transaction, *grant).await?;
        }
        Ok(saved)
    }

    async fn search(
        &mut self,
        filter: &ResourceFilter,
        caller: &User,
    ) -> Result<Vec<ResourceAccess>, RepositoryError> {
        let resources = self.matching_resources(filter).await?;
        let mut found = Vec::with_capacity(resources.len());
        for resource in resources {
            let ancestors = self.ancestors(&resource).await?;
            let mut ids = Vec::with_capacity(ancestors.len().saturating_add(1));
            ids.push(resource.resource_id());
            ids.extend(ancestors.iter().map(Resource::resource_id));
            let grants = self.grants_on(&ids, caller).await?;
            let relations = self.relation_sources(&resource, caller).await?;
            found.push(ResourceAccess::new(resource, ancestors, grants, relations));
        }
        Ok(found)
    }
}

/// Insert one grant and return its stored row.
pub(crate) async fn insert_grant(
    transaction: &mut Transaction<'static, Sqlite>,
    grant: Grant,
) -> Result<Grant, RepositoryError> {
    let (kind, id, role) = match grant.principal() {
        Principal::Role(role) => ("ROLE", None, Some(role.as_wire())),
        Principal::Root => return Err(RepositoryError::DataIntegrityViolation),
        Principal::User(id) => ("USER", Some(id), None),
    };
    let row = sqlx::query_as::<_, GrantRow>(
        "INSERT INTO grant (
            principal_kind, principal_id, principal_role, resource_id,
            role, child_kind, effect
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         RETURNING grant_id, principal_kind, principal_id, principal_role,
                   resource_id, role, child_kind, effect",
    )
    .bind(kind)
    .bind(id)
    .bind(role)
    .bind(grant.resource_id())
    .bind(grant.role().as_wire())
    .bind(grant.child_kind().as_wire())
    .bind(grant.effect().as_wire())
    .fetch_one(&mut **transaction)
    .await?;
    stored_grant(row)
}

/// Convert a stored resource row into the domain model.
fn stored_resource(row: &ResourceRow) -> Result<Resource, RepositoryError> {
    let Some(kind) = ResourceKind::from_wire(&row.kind) else {
        return Err(stored_data_invalid("resource_kind"));
    };
    let Some(visibility) = Visibility::from_wire(&row.visibility) else {
        return Err(stored_data_invalid("resource_visibility"));
    };
    Ok(Resource::from_stored(
        row.resource_id,
        kind,
        row.entity_id,
        row.owner_id,
        row.auth_parent_id,
        visibility,
    ))
}

/// Convert a stored grant row into the domain model.
pub(crate) fn stored_grant(row: GrantRow) -> Result<Grant, RepositoryError> {
    let Some(role) = ResourceRole::from_wire(&row.role) else {
        return Err(stored_data_invalid("grant_role"));
    };
    let Some(child_kind) = ChildKind::from_wire(&row.child_kind) else {
        return Err(stored_data_invalid("grant_child_kind"));
    };
    let Some(effect) = GrantEffect::from_wire(&row.effect) else {
        return Err(stored_data_invalid("grant_effect"));
    };
    let principal = match (
        row.principal_kind.as_str(),
        row.principal_id,
        row.principal_role,
    ) {
        ("USER", Some(id), None) => Principal::User(id),
        ("ROLE", None, Some(principal_role)) => {
            let Some(parsed_role) = Role::from_wire(&principal_role) else {
                return Err(stored_data_invalid("grant_principal_role"));
            };
            Principal::Role(parsed_role)
        }
        _ => return Err(stored_data_invalid("grant_principal")),
    };
    Ok(Grant::from_stored(
        row.grant_id,
        principal,
        row.resource_id,
        role,
        child_kind,
        effect,
    ))
}

#[cfg(test)]
mod tests {
    use super::super::SqlxGrantRepository;
    use super::*;
    use crate::domain::model::{
        ChildKind, Grant, GrantEffect, Principal, ResourceRole, Role, User, Visibility,
    };
    use crate::domain::port::GrantRepository as _;
    use crate::infrastructure::outbound::sqlx::test_helpers::begin_transaction;
    use std::error::Error;
    use std::io;

    /// Seed a credential and a standard user so a resource can be owned.
    async fn seed_user(
        transaction: &mut Transaction<'static, Sqlite>,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (?1, ?2)")
            .bind(user_id)
            .bind(format!("hash-{user_id}"))
            .execute(&mut **transaction)
            .await?;
        sqlx::query(
            "INSERT INTO user (user_id, role, username, email, credential_id)
             VALUES (?1, 'STANDARD', ?2, NULL, ?1)",
        )
        .bind(user_id)
        .bind(format!("user{user_id}"))
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn insert_then_load_reads_the_resource_chain_and_grants() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 5).await?;

        // Act
        let created = {
            let mut repository = SqlxResourceRepository::new(&mut transaction);
            repository
                .create(NewResource {
                    auth_parent: Some(ResourceRef::new(ResourceKind::Library, 0)),
                    entity_id: 10,
                    kind: ResourceKind::Gallery,
                    owner_id: Some(5),
                    visibility: Visibility::Private,
                })
                .await?
        };
        let mut grants = SqlxGrantRepository::new(&mut transaction);
        grants
            .save_grant(Grant::try_new(
                Principal::User(5),
                created.resource_id(),
                ResourceRole::Viewer,
                ChildKind::All,
                GrantEffect::Allow,
            )?)
            .await?;
        let mut repository = SqlxResourceRepository::new(&mut transaction);
        let caller = User::try_new(5, "user5".to_owned(), None, 5, Role::Standard)?;
        let found = repository
            .search(
                &ResourceFilter {
                    entity_id: Some(10),
                    kind: Some(ResourceKind::Gallery),
                    ..ResourceFilter::default()
                },
                &caller,
            )
            .await?;

        // Assert
        let access = found
            .into_iter()
            .next()
            .ok_or_else(|| io::Error::other("the gallery resource must resolve"))?;
        assert_eq!(access.resource().kind(), ResourceKind::Gallery);
        assert_eq!(access.ancestors().len(), 2);
        assert_eq!(access.grants().len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn a_duplicate_address_returns_already_exist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let mut repository = SqlxResourceRepository::new(&mut transaction);
        let new_resource = || NewResource {
            auth_parent: Some(ResourceRef::new(ResourceKind::Library, 0)),
            entity_id: 10,
            kind: ResourceKind::Gallery,
            owner_id: None,
            visibility: Visibility::Private,
        };
        repository.create(new_resource()).await?;

        // Act
        let result = repository.create(new_resource()).await;

        // Assert
        assert!(matches!(result, Err(RepositoryError::AlreadyExist)));
        Ok(())
    }

    #[tokio::test]
    async fn delete_removes_the_addressed_resource() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 5).await?;
        {
            let mut repository = SqlxResourceRepository::new(&mut transaction);
            repository
                .create(NewResource {
                    auth_parent: Some(ResourceRef::new(ResourceKind::Library, 0)),
                    entity_id: 10,
                    kind: ResourceKind::Gallery,
                    owner_id: Some(5),
                    visibility: Visibility::Private,
                })
                .await?;
        }

        // Act
        let deleted = {
            let mut repository = SqlxResourceRepository::new(&mut transaction);
            repository
                .delete(ResourceRef::new(ResourceKind::Gallery, 10))
                .await?
        };
        let missing = {
            let mut repository = SqlxResourceRepository::new(&mut transaction);
            repository
                .delete(ResourceRef::new(ResourceKind::Gallery, 11))
                .await?
        };

        // Assert
        assert!(deleted, "the stored resource must be removed");
        assert!(!missing, "an absent resource is a valid no-match outcome");
        let mut repository = SqlxResourceRepository::new(&mut transaction);
        let caller = User::try_new(5, "user5".to_owned(), None, 5, Role::Standard)?;
        let found = repository
            .search(
                &ResourceFilter {
                    entity_id: Some(10),
                    kind: Some(ResourceKind::Gallery),
                    ..ResourceFilter::default()
                },
                &caller,
            )
            .await?;
        assert!(found.is_empty(), "the removed resource must not be found");
        Ok(())
    }

    #[tokio::test]
    async fn reassign_owner_moves_the_resource_to_the_new_owner() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 5).await?;
        seed_user(&mut transaction, 0).await?;
        let created = {
            let mut repository = SqlxResourceRepository::new(&mut transaction);
            repository
                .create(NewResource {
                    auth_parent: None,
                    entity_id: 10,
                    kind: ResourceKind::File,
                    owner_id: Some(5),
                    visibility: Visibility::Private,
                })
                .await?
        };
        let parent = {
            let mut repository = SqlxResourceRepository::new(&mut transaction);
            repository
                .create(NewResource {
                    auth_parent: None,
                    entity_id: 0,
                    kind: ResourceKind::User,
                    owner_id: None,
                    visibility: Visibility::Private,
                })
                .await?
        };

        // Act
        let moved = {
            let mut repository = SqlxResourceRepository::new(&mut transaction);
            repository
                .reassign_owner(
                    ResourceRef::new(ResourceKind::File, 10),
                    Some(0),
                    Some(parent.resource_id()),
                )
                .await?
        };
        let missing = {
            let mut repository = SqlxResourceRepository::new(&mut transaction);
            repository
                .reassign_owner(
                    ResourceRef::new(ResourceKind::File, 11),
                    Some(0),
                    Some(parent.resource_id()),
                )
                .await?
        };

        // Assert
        assert!(moved, "the stored file resource must move to the new owner");
        assert!(!missing, "an absent resource is a valid no-match outcome");
        let mut repository = SqlxResourceRepository::new(&mut transaction);
        let caller = User::try_new(0, "root".to_owned(), None, 0, Role::Admin)?;
        let found = repository
            .search(
                &ResourceFilter {
                    entity_id: Some(created.entity_id()),
                    kind: Some(ResourceKind::File),
                    ..ResourceFilter::default()
                },
                &caller,
            )
            .await?;
        assert_eq!(
            found
                .first()
                .and_then(|access| access.resource().owner_id()),
            Some(0)
        );
        assert_eq!(
            found
                .first()
                .and_then(|access| access.resource().auth_parent_id()),
            Some(parent.resource_id())
        );
        Ok(())
    }
}
