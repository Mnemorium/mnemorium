use std::collections::{HashMap, HashSet};
use std::iter;

use sqlx::{QueryBuilder, Sqlite, Transaction};

use crate::domain::NumericID;
use crate::domain::model::{
    ChildKind, Grant, GrantEffect, MAX_AUTHORIZATION_DEPTH, Principal, RelationSource, Resource,
    ResourceAccess, ResourceKind, ResourceRef, ResourceRelationKind, ResourceRole, Role, User,
    Visibility,
};
use crate::domain::port::{NewResource, RepositoryError, ResourceFilter, ResourceRepository};

use super::error_mapping::stored_data_invalid;
use super::model::{GrantRow, RelationEdgeRow, RelationRow, ResourceRow};
use super::query_filter::push_filter;

/// Addresses one batched resolution hands a single statement.
///
/// The statement binds two variables per address, and `SQLite` caps a
/// statement at 32 766, so a collection larger than this is resolved in
/// successive batches rather than one oversized query.
const SEARCH_MANY_CHUNK: usize = 1_000;

/// Repository reading the resource graph and persisting its grants, backed by a
/// `SQLite` transaction.
pub(crate) struct SqlxResourceRepository<'transaction> {
    /// Transaction the repository reads from and writes to.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

impl<'transaction> SqlxResourceRepository<'transaction> {
    /// Load the resources `addresses` name, one statement for the set.
    async fn addressed_resources(
        &mut self,
        addresses: &[ResourceRef],
    ) -> Result<Vec<Resource>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new("WITH wanted(kind, entity_id) AS (");
        builder.push_values(addresses, |mut row, address| {
            row.push_bind(address.kind().as_wire());
            row.push_bind(address.id());
        });
        builder.push(
            ") SELECT resource.resource_id, resource.kind, resource.entity_id,
                      resource.owner_id, resource.auth_parent_id, resource.visibility
             FROM resource
             JOIN wanted ON wanted.kind = resource.kind AND wanted.entity_id = resource.entity_id",
        );
        let rows = builder
            .build_query_as::<ResourceRow>()
            .fetch_all(&mut **self.transaction)
            .await?;
        rows.iter().map(stored_resource).collect()
    }

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

    /// Load the grants that apply to `caller` on `seed_ids` or their ancestors,
    /// grouped by the resource they sit on and ordered by identifier.
    async fn grants_in_closure(
        &mut self,
        seed_ids: &[NumericID],
        caller: &User,
    ) -> Result<HashMap<NumericID, Vec<Grant>>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new("WITH RECURSIVE seeds(resource_id) AS (");
        builder.push_values(seed_ids, |mut row, resource_id| {
            row.push_bind(resource_id);
        });
        builder.push(
            "), closure(resource_id) AS (
                 SELECT resource_id FROM seeds
                 UNION
                 SELECT resource.auth_parent_id
                 FROM resource
                 JOIN closure ON resource.resource_id = closure.resource_id
                 WHERE resource.auth_parent_id IS NOT NULL
             )
             SELECT grant_id, principal_kind, principal_id, principal_role,
                    resource_id, role, child_kind, effect
             FROM grant
             WHERE resource_id IN (SELECT resource_id FROM closure)
               AND (
                   (principal_kind = 'USER' AND principal_id = ",
        );
        builder.push_bind(caller.id());
        builder.push(
            ")
                   OR (principal_kind = 'ROLE' AND principal_role = ",
        );
        builder.push_bind(caller.role().as_wire());
        builder.push(")) ORDER BY grant_id");
        let rows = builder
            .build_query_as::<GrantRow>()
            .fetch_all(&mut **self.transaction)
            .await?;
        let mut grants: HashMap<NumericID, Vec<Grant>> = HashMap::new();
        for row in rows {
            let grant = stored_grant(row)?;
            grants.entry(grant.resource_id()).or_default().push(grant);
        }
        Ok(grants)
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
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT resource_id, kind, entity_id, owner_id, auth_parent_id, visibility FROM resource",
        );
        push_resource_predicate(&mut builder, filter);
        builder.push(" ORDER BY resource_id");
        let rows = builder
            .build_query_as::<ResourceRow>()
            .fetch_all(&mut **self.transaction)
            .await?;
        rows.iter().map(stored_resource).collect()
    }

    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub(crate) fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }

    /// Load the relation edges that point at `resource_ids`.
    async fn relation_edges(
        &mut self,
        resource_ids: &[NumericID],
    ) -> Result<Vec<RelationEdgeRow>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT from_resource_id, kind, to_resource_id
             FROM resource_relation WHERE to_resource_id IN (",
        );
        let mut separated = builder.separated(", ");
        for resource_id in resource_ids {
            separated.push_bind(resource_id);
        }
        separated.push_unseparated(") ORDER BY from_resource_id, kind");
        let rows = builder
            .build_query_as::<RelationEdgeRow>()
            .fetch_all(&mut **self.transaction)
            .await?;
        Ok(rows)
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

    /// Load `seed_ids` and every ancestor they reach, keyed by identifier.
    ///
    /// One recursive walk replaces the parent-by-parent reads [`ancestors`]
    /// performs, which repeat the same shared containers for every sibling.
    async fn resource_closure(
        &mut self,
        seed_ids: &[NumericID],
    ) -> Result<HashMap<NumericID, Resource>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new("WITH RECURSIVE seeds(resource_id) AS (");
        builder.push_values(seed_ids, |mut row, resource_id| {
            row.push_bind(resource_id);
        });
        builder.push(
            "), closure(resource_id) AS (
                 SELECT resource_id FROM seeds
                 UNION
                 SELECT resource.auth_parent_id
                 FROM resource
                 JOIN closure ON resource.resource_id = closure.resource_id
                 WHERE resource.auth_parent_id IS NOT NULL
             )
             SELECT resource_id, kind, entity_id, owner_id, auth_parent_id, visibility
             FROM resource
             WHERE resource_id IN (SELECT resource_id FROM closure)",
        );
        let rows = builder
            .build_query_as::<ResourceRow>()
            .fetch_all(&mut **self.transaction)
            .await?;
        let mut closure = HashMap::with_capacity(rows.len());
        for row in &rows {
            let resource = stored_resource(row)?;
            closure.insert(resource.resource_id(), resource);
        }
        Ok(closure)
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

    async fn search_many(
        &mut self,
        addresses: &[ResourceRef],
        caller: &User,
    ) -> Result<Vec<ResourceAccess>, RepositoryError> {
        let mut found = Vec::with_capacity(addresses.len());
        for chunk in addresses.chunks(SEARCH_MANY_CHUNK) {
            let matched = self.addressed_resources(chunk).await?;
            let by_address: HashMap<ResourceRef, Resource> = matched
                .into_iter()
                .map(|resource| {
                    (
                        ResourceRef::new(resource.kind(), resource.entity_id()),
                        resource,
                    )
                })
                .collect();
            let ordered: Vec<Resource> = chunk
                .iter()
                .filter_map(|address| by_address.get(address).cloned())
                .collect();
            if ordered.is_empty() {
                continue;
            }

            let matched_ids: Vec<NumericID> = ordered.iter().map(Resource::resource_id).collect();
            let edges = self.relation_edges(&matched_ids).await?;
            let mut seeds = matched_ids;
            seeds.extend(edges.iter().map(|edge| edge.from_resource_id));
            seeds.sort_unstable();
            seeds.dedup();
            let closure = self.resource_closure(&seeds).await?;
            let grants = self.grants_in_closure(&seeds, caller).await?;

            for resource in &ordered {
                let ancestors = ancestors_from(&closure, resource)?;
                let resource_grants = chain_grants(&grants, resource, &ancestors);
                let relations = relation_sources_from(&closure, &grants, &edges, resource)?;
                found.push(ResourceAccess::new(
                    resource.clone(),
                    ancestors,
                    resource_grants,
                    relations,
                ));
            }
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

/// Append `filter` as equality clauses on `resource`.
///
/// `WHERE 1 = 1` keeps every following clause an `AND`, so `push_filter` applies
/// uniformly; `SQLite` folds the constant away and still picks the index. The
/// `(?1 IS NULL OR …)` shape this replaces could not: a parameter compared
/// against `NULL` hides the column from the planner, so every authorization
/// decision scanned the whole table.
fn push_resource_predicate(builder: &mut QueryBuilder<Sqlite>, filter: &ResourceFilter) {
    builder.push(" WHERE 1 = 1");
    let mut first = false;
    if let Some(kind) = filter.kind {
        push_filter(&mut first, builder, "kind = ", kind.as_wire());
    }
    if let Some(entity_id) = filter.entity_id {
        push_filter(&mut first, builder, "entity_id = ", entity_id);
    }
    if let Some(owner_id) = filter.owner_id {
        push_filter(&mut first, builder, "owner_id = ", owner_id);
    }
    if let Some(visibility) = filter.visibility {
        push_filter(&mut first, builder, "visibility = ", visibility.as_wire());
    }
    if let Some(resource_id) = filter.resource_id {
        push_filter(&mut first, builder, "resource_id = ", resource_id);
    }
}

/// Build the ancestor chain of `resource` from an already-loaded closure,
/// nearest first.
///
/// A chain longer than [`MAX_AUTHORIZATION_DEPTH`], a cycle and a missing parent
/// are stored-data failures, exactly as they are on the row-by-row path.
fn ancestors_from(
    closure: &HashMap<NumericID, Resource>,
    resource: &Resource,
) -> Result<Vec<Resource>, RepositoryError> {
    let mut ancestors = Vec::new();
    let mut visited = HashSet::new();
    let mut current = resource.auth_parent_id();
    while let Some(parent_id) = current {
        if ancestors.len() >= MAX_AUTHORIZATION_DEPTH || !visited.insert(parent_id) {
            return Err(stored_data_invalid("resource_chain"));
        }
        let Some(parent) = closure.get(&parent_id) else {
            return Err(stored_data_invalid("resource_parent"));
        };
        current = parent.auth_parent_id();
        ancestors.push(parent.clone());
    }
    Ok(ancestors)
}

/// Collect the grants that sit on `resource` or one of its ancestors.
fn chain_grants(
    grants: &HashMap<NumericID, Vec<Grant>>,
    resource: &Resource,
    ancestors: &[Resource],
) -> Vec<Grant> {
    let mut chain = Vec::new();
    for resource_id in
        iter::once(resource.resource_id()).chain(ancestors.iter().map(Resource::resource_id))
    {
        if let Some(found) = grants.get(&resource_id) {
            chain.extend(found.iter().copied());
        }
    }
    chain
}

/// Build the relation containers that reference `resource` from the batched
/// edge, resource and grant sets.
fn relation_sources_from(
    closure: &HashMap<NumericID, Resource>,
    grants: &HashMap<NumericID, Vec<Grant>>,
    edges: &[RelationEdgeRow],
    resource: &Resource,
) -> Result<Vec<RelationSource>, RepositoryError> {
    let mut sources = Vec::new();
    for edge in edges
        .iter()
        .filter(|edge| edge.to_resource_id == resource.resource_id())
    {
        ResourceRelationKind::from_wire(&edge.kind)
            .ok_or_else(|| stored_data_invalid("resource_relation_kind"))?;
        let Some(container) = closure.get(&edge.from_resource_id) else {
            return Err(stored_data_invalid("resource_relation_from"));
        };
        let ancestors = ancestors_from(closure, container)?;
        let container_grants = chain_grants(grants, container, &ancestors);
        sources.push(RelationSource::new(
            container.clone(),
            ancestors,
            container_grants,
        ));
    }
    Ok(sources)
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
    use crate::infrastructure::outbound::sqlx::test_helpers::*;
    use rstest::rstest;
    use std::error::Error;
    use std::io;

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

    #[rstest]
    #[case::kind_and_entity(ResourceFilter {
        kind: Some(ResourceKind::Gallery),
        entity_id: Some(1_010),
        ..ResourceFilter::default()
    })]
    #[case::resource_id(ResourceFilter {
        resource_id: Some(1),
        ..ResourceFilter::default()
    })]
    #[tokio::test]
    async fn the_authorization_lookup_seeks_instead_of_scanning(
        #[case] filter: ResourceFilter,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange: a table the planner has no reason to scan, and no `ANALYZE`
        // statistics to bias it either way. The migration backfills a gallery
        // resource per seeded gallery, so the synthetic rows start above it.
        let mut transaction = begin_transaction().await?;
        sqlx::query(
            "WITH RECURSIVE seed(entity_id) AS (
                 SELECT 1000
                 UNION ALL
                 SELECT entity_id + 1 FROM seed WHERE entity_id < 1499
             )
             INSERT INTO resource (kind, entity_id, owner_id, visibility)
             SELECT 'GALLERY', entity_id, NULL, 'PRIVATE' FROM seed",
        )
        .execute(&mut *transaction)
        .await?;

        // Act
        let mut builder = QueryBuilder::<Sqlite>::new(
            "EXPLAIN QUERY PLAN
             SELECT resource_id, kind, entity_id, owner_id, auth_parent_id, visibility
             FROM resource",
        );
        push_resource_predicate(&mut builder, &filter);
        let plan = builder
            .build_query_as::<(i64, i64, i64, String)>()
            .fetch_all(&mut *transaction)
            .await?;
        let details: Vec<&str> = plan.iter().map(|row| row.3.as_str()).collect();

        // Assert
        assert!(
            details
                .iter()
                .any(|detail| detail.starts_with("SEARCH resource USING")),
            "an equality filter must seek, not scan: {filter:?} → {details:?}"
        );
        assert!(
            !details
                .iter()
                .any(|detail| detail.contains("SCAN resource")),
            "a scan of `resource` decides every authorization: {filter:?} → {details:?}"
        );
        Ok(())
    }

    /// Reduce an access to the identifiers a decision reads, so two reads are
    /// comparable without depending on how each built its values.
    fn access_shape(
        access: &ResourceAccess,
    ) -> (NumericID, Vec<NumericID>, Vec<NumericID>, Vec<NumericID>) {
        (
            access.resource().resource_id(),
            access
                .ancestors()
                .iter()
                .map(Resource::resource_id)
                .collect(),
            access.grants().iter().map(Grant::id).collect(),
            access
                .relations()
                .iter()
                .map(|source| source.container().resource_id())
                .collect(),
        )
    }

    #[tokio::test]
    async fn the_batched_read_matches_the_row_by_row_read() -> Result<(), Box<dyn Error>> {
        // Arrange: a gallery under the seeded library, a grant on it, and a
        // playlist that references it through a relation.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 5).await?;
        let mut repository = SqlxResourceRepository::new(&mut transaction);
        let gallery = repository
            .create(NewResource {
                auth_parent: Some(ResourceRef::new(ResourceKind::Library, 0)),
                entity_id: 10,
                kind: ResourceKind::Gallery,
                owner_id: Some(5),
                visibility: Visibility::Private,
            })
            .await?;
        let container = repository
            .create(NewResource {
                auth_parent: Some(ResourceRef::new(ResourceKind::Library, 0)),
                entity_id: 20,
                kind: ResourceKind::MusicPlaylist,
                owner_id: Some(5),
                visibility: Visibility::Private,
            })
            .await?;
        SqlxGrantRepository::new(&mut transaction)
            .save_grant(Grant::try_new(
                Principal::User(5),
                gallery.resource_id(),
                ResourceRole::Viewer,
                ChildKind::All,
                GrantEffect::Allow,
            )?)
            .await?;
        sqlx::query(
            "INSERT INTO resource_relation (from_resource_id, to_resource_id, kind)
             VALUES (?1, ?2, 'PLAYLIST_ENTRY')",
        )
        .bind(container.resource_id())
        .bind(gallery.resource_id())
        .execute(&mut *transaction)
        .await?;
        let caller = User::try_new(5, "user5".to_owned(), None, 5, Role::Standard)?;
        let address = ResourceRef::new(ResourceKind::Gallery, 10);

        // Act
        let by_row = {
            let mut row_repository = SqlxResourceRepository::new(&mut transaction);
            row_repository
                .search(
                    &ResourceFilter {
                        entity_id: Some(10),
                        kind: Some(ResourceKind::Gallery),
                        ..ResourceFilter::default()
                    },
                    &caller,
                )
                .await?
        };
        let batched = {
            let mut batch_repository = SqlxResourceRepository::new(&mut transaction);
            batch_repository.search_many(&[address], &caller).await?
        };

        // Assert
        assert_eq!(by_row.len(), 1);
        assert_eq!(batched.len(), 1);
        assert_eq!(
            batched.first().map(access_shape),
            by_row.first().map(access_shape),
            "the batched read must decide exactly what the row-by-row read decides"
        );
        Ok(())
    }

    #[tokio::test]
    async fn the_batched_read_keeps_the_caller_order_and_omits_a_missing_address()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction, 5).await?;
        let mut repository = SqlxResourceRepository::new(&mut transaction);
        repository
            .create(NewResource {
                auth_parent: None,
                entity_id: 10,
                kind: ResourceKind::Gallery,
                owner_id: Some(5),
                visibility: Visibility::Private,
            })
            .await?;
        repository
            .create(NewResource {
                auth_parent: None,
                entity_id: 11,
                kind: ResourceKind::Gallery,
                owner_id: Some(5),
                visibility: Visibility::Private,
            })
            .await?;
        let caller = User::try_new(5, "user5".to_owned(), None, 5, Role::Standard)?;

        // Act
        let batched = repository
            .search_many(
                &[
                    ResourceRef::new(ResourceKind::Gallery, 11),
                    ResourceRef::new(ResourceKind::Gallery, 999),
                    ResourceRef::new(ResourceKind::Gallery, 10),
                ],
                &caller,
            )
            .await?;

        // Assert
        assert_eq!(
            batched
                .iter()
                .map(|access| access.resource().entity_id())
                .collect::<Vec<_>>(),
            vec![11, 10],
            "results follow the caller's order, and a missing resource is absent"
        );
        Ok(())
    }
}
