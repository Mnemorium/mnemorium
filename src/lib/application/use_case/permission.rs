//! The permission use cases behind the `/identity/permission` routes.

use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::slice;
use std::sync::Arc;

use crate::application::authorization::authorize;
use crate::application::port::{
    GetPermissionSummaryCommand, GetPermissionSummaryUseCase, GetResourcePermissionCommand,
    GetResourcePermissionUseCase, GrantRequest, GrantedResource, PermissionError,
    PermissionSummary, PutResourcePermissionCommand, PutResourcePermissionUseCase,
    ResourcePermission, RevokeShareCommand, RevokeShareUseCase, ShareResourceCommand,
    ShareResourceUseCase,
};
use crate::domain::NumericID;
use crate::domain::model::{
    Action, ChildKind, Grant, GrantEffect, Principal, ResourceAccess, ResourceKind, ResourceRef,
    ResourceRole, User,
};
use crate::domain::port::{
    GrantFilter, GrantRepository as _, IdentityUnitOfWork, ResourceFilter, ResourceRepository as _,
    UnitOfWork as _, UnitOfWorkFactory, UserFilter, UserRepository as _, UserUnitOfWork,
};
use crate::domain::service::AuthorizationEngine;

/// The summary use case.
#[derive(Debug)]
pub(crate) struct GetPermissionSummary<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetPermissionSummary<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetPermissionSummaryUseCase for GetPermissionSummary<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: GetPermissionSummaryCommand,
    ) -> Pin<Box<dyn Future<Output = Result<PermissionSummary, PermissionError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| PermissionError::Unknown(error.into()))?;

            let result = async {
                let caller = load_caller(&mut unit_of_work, command.caller_id).await?;
                let target = load_user(&mut unit_of_work, command.user_id).await?;
                let self_read = caller.id() == target.id();
                if !self_read {
                    require(
                        &mut unit_of_work,
                        &caller,
                        ResourceRef::new(ResourceKind::User, target.id()),
                        Action::ManagePermissions,
                    )
                    .await?;
                }

                let explanation = match command.decision {
                    None => None,
                    Some(query) => {
                        require(
                            &mut unit_of_work,
                            &caller,
                            ResourceRef::new(ResourceKind::User, target.id()),
                            Action::ManagePermissions,
                        )
                        .await?;
                        let access =
                            load_resource(&mut unit_of_work, &target, query.resource).await?;
                        let decision =
                            AuthorizationEngine::authorize(&target, &access, query.action, None);
                        Some(decision.explanation().clone())
                    }
                };

                let mut grants = unit_of_work
                    .grants()
                    .search(&GrantFilter {
                        principal: Some(Principal::User(target.id())),
                        ..GrantFilter::default()
                    })
                    .await
                    .map_err(|error| PermissionError::Unknown(error.into()))?;
                grants.extend(
                    unit_of_work
                        .grants()
                        .search(&GrantFilter {
                            principal: Some(Principal::Role(target.role())),
                            ..GrantFilter::default()
                        })
                        .await
                        .map_err(|error| PermissionError::Unknown(error.into()))?,
                );

                let mut effective = Vec::with_capacity(grants.len());
                for grant in grants {
                    let found = unit_of_work
                        .resources()
                        .search(
                            &ResourceFilter {
                                resource_id: Some(grant.resource_id()),
                                ..ResourceFilter::default()
                            },
                            &target,
                        )
                        .await
                        .map_err(|error| PermissionError::Unknown(error.into()))?;
                    if let Some(access) = found.into_iter().next() {
                        let resource = access.resource();
                        effective.push(GrantedResource {
                            grant,
                            resource: ResourceRef::new(resource.kind(), resource.entity_id()),
                        });
                    }
                }

                Ok(PermissionSummary {
                    detailed: caller.is_root_admin() || !self_read,
                    explanation,
                    grants: effective,
                    role: target.role(),
                    user_id: target.id(),
                })
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| PermissionError::Unknown(error.into()))?;
                    Ok(value)
                }
                Err(error) => {
                    let _rollback = unit_of_work.rollback().await;
                    Err(error)
                }
            }
        })
    }
}

/// The resource permission read use case.
pub(crate) struct GetResourcePermission<F> {
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> GetResourcePermission<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> GetResourcePermissionUseCase for GetResourcePermission<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: GetResourcePermissionCommand,
    ) -> Pin<Box<dyn Future<Output = Result<ResourcePermission, PermissionError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| PermissionError::Unknown(error.into()))?;

            let result = async {
                let caller = load_caller(&mut unit_of_work, command.caller_id).await?;
                let access = load_resource(&mut unit_of_work, &caller, command.address).await?;
                require_sharing(&mut unit_of_work, &caller, command.address).await?;
                let resource = access.resource();
                let grants = stored_grants(&mut unit_of_work, resource.resource_id()).await?;
                Ok(ResourcePermission {
                    address: command.address,
                    grants,
                    owner_id: resource.owner_id(),
                    visibility: resource.visibility(),
                })
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| PermissionError::Unknown(error.into()))?;
                    Ok(value)
                }
                Err(error) => {
                    let _rollback = unit_of_work.rollback().await;
                    Err(error)
                }
            }
        })
    }
}

/// The resource permission replacement use case.
pub(crate) struct PutResourcePermission<F> {
    /// Factory opening the unit of work wrapping the write.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> PutResourcePermission<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> PutResourcePermissionUseCase for PutResourcePermission<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: PutResourcePermissionCommand,
    ) -> Pin<Box<dyn Future<Output = Result<ResourcePermission, PermissionError>> + Send + 'future>>
    {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| PermissionError::Unknown(error.into()))?;

            let result = async {
                let caller = load_caller(&mut unit_of_work, command.caller_id).await?;
                let access = load_resource(&mut unit_of_work, &caller, command.address).await?;
                require(
                    &mut unit_of_work,
                    &caller,
                    command.address,
                    Action::ManagePermissions,
                )
                .await?;
                let resource = access.resource();
                let grants = build_grants(resource.resource_id(), &command.grants)?;
                require_known_principals(&mut unit_of_work, &grants).await?;

                // The set is validated before the first write, so a rejected
                // replacement leaves the previous set untouched.
                let saved = unit_of_work
                    .resources()
                    .save(resource.with_visibility(command.visibility), &grants)
                    .await
                    .map_err(|error| PermissionError::Unknown(error.into()))?;
                let stored = stored_grants(&mut unit_of_work, saved.resource_id()).await?;

                Ok(ResourcePermission {
                    address: command.address,
                    grants: stored,
                    owner_id: saved.owner_id(),
                    visibility: saved.visibility(),
                })
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| PermissionError::Unknown(error.into()))?;
                    Ok(value)
                }
                Err(error) => {
                    let _rollback = unit_of_work.rollback().await;
                    Err(error)
                }
            }
        })
    }
}

/// The revoke use case.
pub(crate) struct RevokeShare<F> {
    /// Factory opening the unit of work wrapping the write.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> RevokeShare<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> RevokeShareUseCase for RevokeShare<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: RevokeShareCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), PermissionError>> + Send + 'future>> {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| PermissionError::Unknown(error.into()))?;

            let result = async {
                let caller = load_caller(&mut unit_of_work, command.caller_id).await?;
                let access = load_resource(&mut unit_of_work, &caller, command.address).await?;
                require_sharing(&mut unit_of_work, &caller, command.address).await?;
                let removed = unit_of_work
                    .grants()
                    .delete(command.grant_id, access.resource().resource_id())
                    .await
                    .map_err(|error| PermissionError::Unknown(error.into()))?;
                if !removed {
                    return Err(PermissionError::NotFound);
                }
                Ok(())
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| PermissionError::Unknown(error.into()))?;
                    Ok(value)
                }
                Err(error) => {
                    let _rollback = unit_of_work.rollback().await;
                    Err(error)
                }
            }
        })
    }
}

/// The share use case.
pub(crate) struct ShareResource<F> {
    /// Factory opening the unit of work wrapping the write.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory> ShareResource<F> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(unit_of_work_factory: Arc<F>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl<F> ShareResourceUseCase for ShareResource<F>
where
    F: UnitOfWorkFactory,
    F::Uow: IdentityUnitOfWork + UserUnitOfWork,
{
    fn execute<'future>(
        &'future self,
        command: ShareResourceCommand,
    ) -> Pin<Box<dyn Future<Output = Result<Grant, PermissionError>> + Send + 'future>> {
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| PermissionError::Unknown(error.into()))?;

            let result = async {
                let caller = load_caller(&mut unit_of_work, command.caller_id).await?;
                let access = load_resource(&mut unit_of_work, &caller, command.address).await?;
                require_sharing(&mut unit_of_work, &caller, command.address).await?;
                // A share may only widen read access: a higher role is an
                // escalation, not an invalid representation.
                if !matches!(
                    command.role,
                    ResourceRole::Viewer | ResourceRole::Downloader
                ) {
                    return Err(PermissionError::Forbidden);
                }
                if !matches!(command.principal, Principal::User(_)) {
                    return Err(PermissionError::Invalid("principal_not_storable"));
                }
                let grant = Grant::try_new(
                    command.principal,
                    access.resource().resource_id(),
                    command.role,
                    ChildKind::All,
                    GrantEffect::Allow,
                )
                .map_err(|_error| PermissionError::Invalid("grant_not_storable"))?;
                require_known_principals(&mut unit_of_work, slice::from_ref(&grant)).await?;
                let stored = unit_of_work
                    .grants()
                    .save_grant(grant)
                    .await
                    .map_err(|error| PermissionError::Unknown(error.into()))?;
                Ok(stored)
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| PermissionError::Unknown(error.into()))?;
                    Ok(value)
                }
                Err(error) => {
                    let _rollback = unit_of_work.rollback().await;
                    Err(error)
                }
            }
        })
    }
}

/// Load the authenticated caller.
async fn load_caller<U>(unit_of_work: &mut U, caller_id: NumericID) -> Result<User, PermissionError>
where
    U: UserUnitOfWork,
{
    unit_of_work
        .users()
        .search(&UserFilter {
            id: Some(caller_id),
            ..UserFilter::default()
        })
        .await
        .map_err(|error| PermissionError::Unknown(error.into()))?
        .into_iter()
        .next()
        .ok_or(PermissionError::Forbidden)
}

/// Load one user by identifier.
async fn load_user<U>(unit_of_work: &mut U, user_id: NumericID) -> Result<User, PermissionError>
where
    U: UserUnitOfWork,
{
    unit_of_work
        .users()
        .search(&UserFilter {
            id: Some(user_id),
            ..UserFilter::default()
        })
        .await
        .map_err(|error| PermissionError::Unknown(error.into()))?
        .into_iter()
        .next()
        .ok_or(PermissionError::NotFound)
}

/// Load the addressed resource, concealing a missing one.
async fn load_resource<U>(
    unit_of_work: &mut U,
    caller: &User,
    address: ResourceRef,
) -> Result<ResourceAccess, PermissionError>
where
    U: IdentityUnitOfWork,
{
    unit_of_work
        .resources()
        .search(
            &ResourceFilter {
                entity_id: Some(address.id()),
                kind: Some(address.kind()),
                ..ResourceFilter::default()
            },
            caller,
        )
        .await
        .map_err(|error| PermissionError::Unknown(error.into()))?
        .into_iter()
        .next()
        .ok_or(PermissionError::NotFound)
}

/// Refuse the request unless `caller` may perform `action` on `address`.
async fn require<U>(
    unit_of_work: &mut U,
    caller: &User,
    address: ResourceRef,
    action: Action,
) -> Result<(), PermissionError>
where
    U: IdentityUnitOfWork,
{
    authorize(&mut unit_of_work.resources(), caller, address, action, None)
        .await
        .map(|_decision| ())
        .map_err(|_error| PermissionError::Forbidden)
}

/// Refuse the request unless `caller` may share or manage `address`.
async fn require_sharing<U>(
    unit_of_work: &mut U,
    caller: &User,
    address: ResourceRef,
) -> Result<(), PermissionError>
where
    U: IdentityUnitOfWork,
{
    if require(unit_of_work, caller, address, Action::Share)
        .await
        .is_ok()
    {
        return Ok(());
    }
    require(unit_of_work, caller, address, Action::ManagePermissions).await
}

/// Read every grant stored on `resource_id`.
async fn stored_grants<U>(
    unit_of_work: &mut U,
    resource_id: NumericID,
) -> Result<Vec<Grant>, PermissionError>
where
    U: IdentityUnitOfWork,
{
    unit_of_work
        .grants()
        .search(&GrantFilter {
            resource_id: Some(resource_id),
            ..GrantFilter::default()
        })
        .await
        .map_err(|error| PermissionError::Unknown(error.into()))
}

/// Build the requested user grant set, rejecting an invalid one.
fn build_grants(
    resource_id: NumericID,
    requests: &[GrantRequest],
) -> Result<Vec<Grant>, PermissionError> {
    let mut seen = HashSet::with_capacity(requests.len());
    let mut grants = Vec::with_capacity(requests.len());
    for request in requests {
        if !request.role.is_grantable_by_user() {
            return Err(PermissionError::Invalid("role_not_grantable"));
        }
        if request.child_kind == ChildKind::Admin {
            return Err(PermissionError::Invalid("admin_is_not_grantable"));
        }
        if !seen.insert((
            request.principal,
            request.role,
            request.child_kind,
            request.effect,
        )) {
            return Err(PermissionError::Invalid("duplicate_grant"));
        }
        if !matches!(request.principal, Principal::User(_)) {
            // Role grants are seeded by the migration (`REQ-AUTHORIZATION-035`).
            return Err(PermissionError::Invalid("principal_not_storable"));
        }
        grants.push(
            Grant::try_new(
                request.principal,
                resource_id,
                request.role,
                request.child_kind,
                request.effect,
            )
            .map_err(|_error| PermissionError::Invalid("grant_not_storable"))?,
        );
    }
    Ok(grants)
}

/// Assert that every principal a grant names is a known user.
async fn require_known_principals<U>(
    unit_of_work: &mut U,
    grants: &[Grant],
) -> Result<(), PermissionError>
where
    U: UserUnitOfWork,
{
    let mut checked = HashSet::new();
    for grant in grants {
        let Principal::User(user_id) = grant.principal() else {
            return Err(PermissionError::Invalid("principal_not_storable"));
        };
        if !checked.insert(user_id) {
            continue;
        }
        let found = unit_of_work
            .users()
            .search(&UserFilter {
                id: Some(user_id),
                ..UserFilter::default()
            })
            .await
            .map_err(|error| PermissionError::Unknown(error.into()))?;
        if found.is_empty() {
            return Err(PermissionError::Invalid("unknown_principal"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io;

    use super::*;
    use crate::application::port::{
        GetPermissionSummaryCommand, GetResourcePermissionCommand, PermissionDecisionQuery,
        PutResourcePermissionCommand, RevokeShareCommand, ShareResourceCommand,
    };
    use crate::domain::model::{GrantEffect, Resource, ResourceAccess, Role, Visibility};
    use crate::domain::port::{
        MockGrantRepository, MockResourceRepository, MockUserRepository, RepositoryError,
    };
    use crate::test_helpers::*;

    type Summary = GetPermissionSummary<TestFactory>;
    type Policy = GetResourcePermission<TestFactory>;
    type Replace = PutResourcePermission<TestFactory>;
    type Share = ShareResource<TestFactory>;
    type Revoke = RevokeShare<TestFactory>;

    /// One addressable resource with its chain, grants and relations.
    fn access(
        kind: ResourceKind,
        owner: Option<NumericID>,
        visibility: Visibility,
        grants: Vec<Grant>,
    ) -> ResourceAccess {
        ResourceAccess::new(
            Resource::from_stored(1, kind, 10, owner, None, visibility),
            Vec::new(),
            grants,
            Vec::new(),
        )
    }

    /// A user repository resolving only the listed identifiers and roles.
    fn users_with(entries: Vec<(NumericID, Role)>) -> MockUserRepository {
        let mut users = MockUserRepository::new();
        users.expect_search().returning(move |filter| {
            let found = filter.id.and_then(|id| {
                entries
                    .iter()
                    .find(|entry| entry.0 == id)
                    .and_then(|entry| {
                        User::try_new(entry.0, "user".to_owned(), None, entry.0, entry.1).ok()
                    })
            });
            Box::pin(async move { Ok(found.into_iter().collect()) })
        });
        users
    }

    /// A resource repository returning one fixed bundle per search.
    fn resources_with(bundle: ResourceAccess) -> MockResourceRepository {
        let mut resources = MockResourceRepository::new();
        resources.expect_search().returning(move |_, _| {
            let found = bundle.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
        resources
    }

    /// A resource repository returning nothing.
    fn no_matches() -> MockResourceRepository {
        let mut resources = MockResourceRepository::new();
        resources
            .expect_search()
            .returning(|_, _| Box::pin(async { Ok(Vec::new()) }));
        resources
    }

    /// A grant repository returning the listed grants on every search.
    fn grants_with(grants: Vec<Grant>) -> MockGrantRepository {
        let mut repository = MockGrantRepository::new();
        repository.expect_search().returning(move |filter| {
            let found = grants
                .iter()
                .copied()
                .filter(|grant| match filter.principal {
                    Some(principal) => grant.principal() == principal,
                    None => true,
                })
                .collect::<Vec<_>>();
            Box::pin(async move { Ok(found) })
        });
        repository
    }

    fn user_grant(id: NumericID, principal: Principal, resource_id: NumericID) -> Grant {
        Grant::from_stored(
            id,
            principal,
            resource_id,
            ResourceRole::Viewer,
            ChildKind::All,
            GrantEffect::Allow,
        )
    }

    #[tokio::test]
    async fn a_caller_reads_its_own_summary_without_grant_internals() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let grant = user_grant(4, Principal::User(5), 1);
        let unit_of_work = TestUnitOfWork {
            grants: grants_with(vec![grant]),
            resources: resources_with(access(
                ResourceKind::Gallery,
                Some(5),
                Visibility::Private,
                Vec::new(),
            )),
            users: users_with(vec![(5, Role::Standard)]),
            ..TestUnitOfWork::default()
        };
        let use_case = Summary::new(Arc::new(TestFactory::new(unit_of_work)));
        let command = GetPermissionSummaryCommand {
            caller_id: 5,
            decision: None,
            user_id: 5,
        };

        // Act
        let summary = use_case.execute(command).await?;

        // Assert
        assert_eq!(summary.user_id, 5);
        assert_eq!(summary.role, Role::Standard);
        assert_eq!(summary.grants.len(), 1);
        assert!(summary.explanation.is_none());
        assert!(!summary.detailed);
        Ok(())
    }

    #[tokio::test]
    async fn an_admin_reads_a_standard_users_summary() -> Result<(), Box<dyn Error>> {
        // Arrange
        let unit_of_work = TestUnitOfWork {
            grants: grants_with(Vec::new()),
            resources: resources_with(access(
                ResourceKind::User,
                Some(1),
                Visibility::Private,
                Vec::new(),
            )),
            users: users_with(vec![(1, Role::Admin), (2, Role::Standard)]),
            ..TestUnitOfWork::default()
        };
        let use_case = Summary::new(Arc::new(TestFactory::new(unit_of_work)));
        let command = GetPermissionSummaryCommand {
            caller_id: 1,
            decision: None,
            user_id: 2,
        };

        // Act
        let summary = use_case.execute(command).await?;

        // Assert
        assert_eq!(summary.user_id, 2);
        assert!(summary.detailed);
        Ok(())
    }

    #[tokio::test]
    async fn another_users_summary_is_refused_without_manage_permissions()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let unit_of_work = TestUnitOfWork {
            grants: grants_with(Vec::new()),
            resources: resources_with(access(
                ResourceKind::User,
                None,
                Visibility::Private,
                Vec::new(),
            )),
            users: users_with(vec![(2, Role::Standard), (3, Role::Standard)]),
            ..TestUnitOfWork::default()
        };
        let use_case = Summary::new(Arc::new(TestFactory::new(unit_of_work)));
        let command = GetPermissionSummaryCommand {
            caller_id: 2,
            decision: None,
            user_id: 3,
        };

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PermissionError::Forbidden)));
        Ok(())
    }

    #[tokio::test]
    async fn a_manager_receives_the_decision_explanation() -> Result<(), Box<dyn Error>> {
        // Arrange
        let grant = user_grant(4, Principal::User(2), 1);
        let unit_of_work = TestUnitOfWork {
            grants: grants_with(Vec::new()),
            resources: resources_with(access(
                ResourceKind::Gallery,
                Some(1),
                Visibility::Private,
                vec![grant],
            )),
            users: users_with(vec![(1, Role::Admin), (2, Role::Standard)]),
            ..TestUnitOfWork::default()
        };
        let use_case = Summary::new(Arc::new(TestFactory::new(unit_of_work)));
        let command = GetPermissionSummaryCommand {
            caller_id: 1,
            decision: Some(PermissionDecisionQuery {
                action: Action::View,
                resource: ResourceRef::new(ResourceKind::Gallery, 10),
            }),
            user_id: 2,
        };

        // Act
        let summary = use_case.execute(command).await?;

        // Assert
        let explanation = summary
            .explanation
            .ok_or_else(|| io::Error::other("the explanation must be present"))?;
        assert!(explanation.allowed());
        Ok(())
    }

    #[tokio::test]
    async fn a_sharer_reads_the_resource_permission() -> Result<(), Box<dyn Error>> {
        // Arrange
        let grant = user_grant(7, Principal::User(9), 1);
        let unit_of_work = TestUnitOfWork {
            grants: grants_with(vec![grant]),
            resources: resources_with(access(
                ResourceKind::Gallery,
                Some(5),
                Visibility::Private,
                Vec::new(),
            )),
            users: users_with(vec![(5, Role::Standard)]),
            ..TestUnitOfWork::default()
        };
        let use_case = Policy::new(Arc::new(TestFactory::new(unit_of_work)));
        let command = GetResourcePermissionCommand {
            address: ResourceRef::new(ResourceKind::Gallery, 10),
            caller_id: 5,
        };

        // Act
        let permission = use_case.execute(command).await?;

        // Assert
        assert_eq!(
            permission.address,
            ResourceRef::new(ResourceKind::Gallery, 10)
        );
        assert_eq!(permission.grants.len(), 1);
        assert_eq!(permission.owner_id, Some(5));
        assert_eq!(permission.visibility, Visibility::Private);
        Ok(())
    }

    #[tokio::test]
    async fn a_manager_without_manage_permissions_cannot_replace() -> Result<(), Box<dyn Error>> {
        // Arrange
        let unit_of_work = TestUnitOfWork {
            grants: grants_with(Vec::new()),
            resources: resources_with(access(
                ResourceKind::Gallery,
                None,
                Visibility::Private,
                Vec::new(),
            )),
            users: users_with(vec![(6, Role::Standard)]),
            ..TestUnitOfWork::default()
        };
        let use_case = Replace::new(Arc::new(TestFactory::new(unit_of_work)));
        let command = PutResourcePermissionCommand {
            address: ResourceRef::new(ResourceKind::Gallery, 10),
            caller_id: 6,
            grants: Vec::new(),
            visibility: Visibility::Public,
        };

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PermissionError::Forbidden)));
        Ok(())
    }

    #[tokio::test]
    async fn an_owner_replaces_the_visibility_and_grant_set() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut resources = resources_with(access(
            ResourceKind::Gallery,
            Some(5),
            Visibility::Private,
            Vec::new(),
        ));
        resources
            .expect_save()
            .times(1)
            .returning(|resource, grants| {
                let saved = resource.clone();
                let written = grants.len();
                Box::pin(async move {
                    assert_eq!(written, 1);
                    Ok(saved)
                })
            });
        let unit_of_work = TestUnitOfWork {
            grants: grants_with(vec![user_grant(7, Principal::User(9), 1)]),
            resources,
            users: users_with(vec![(5, Role::Standard), (9, Role::Standard)]),
            ..TestUnitOfWork::default()
        };
        let use_case = Replace::new(Arc::new(TestFactory::new(unit_of_work)));
        let command = PutResourcePermissionCommand {
            address: ResourceRef::new(ResourceKind::Gallery, 10),
            caller_id: 5,
            grants: vec![GrantRequest {
                child_kind: ChildKind::All,
                effect: GrantEffect::Allow,
                principal: Principal::User(9),
                role: ResourceRole::Viewer,
            }],
            visibility: Visibility::Public,
        };

        // Act
        let permission = use_case.execute(command).await?;

        // Assert
        assert_eq!(permission.visibility, Visibility::Public);
        assert_eq!(permission.grants.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn a_replacement_naming_the_owner_role_is_refused() -> Result<(), Box<dyn Error>> {
        // Arrange
        let unit_of_work = TestUnitOfWork {
            grants: grants_with(Vec::new()),
            resources: resources_with(access(
                ResourceKind::Gallery,
                Some(5),
                Visibility::Private,
                Vec::new(),
            )),
            users: users_with(vec![(5, Role::Standard), (9, Role::Standard)]),
            ..TestUnitOfWork::default()
        };
        let use_case = Replace::new(Arc::new(TestFactory::new(unit_of_work)));
        let command = PutResourcePermissionCommand {
            address: ResourceRef::new(ResourceKind::Gallery, 10),
            caller_id: 5,
            grants: vec![GrantRequest {
                child_kind: ChildKind::All,
                effect: GrantEffect::Allow,
                principal: Principal::User(9),
                role: ResourceRole::Owner,
            }],
            visibility: Visibility::Private,
        };

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(PermissionError::Invalid("role_not_grantable"))
        ));
        Ok(())
    }

    #[tokio::test]
    async fn a_duplicate_grant_is_refused() -> Result<(), Box<dyn Error>> {
        // Arrange
        let unit_of_work = TestUnitOfWork {
            grants: grants_with(Vec::new()),
            resources: resources_with(access(
                ResourceKind::Gallery,
                Some(5),
                Visibility::Private,
                Vec::new(),
            )),
            users: users_with(vec![(5, Role::Standard), (9, Role::Standard)]),
            ..TestUnitOfWork::default()
        };
        let use_case = Replace::new(Arc::new(TestFactory::new(unit_of_work)));
        let request = GrantRequest {
            child_kind: ChildKind::All,
            effect: GrantEffect::Allow,
            principal: Principal::User(9),
            role: ResourceRole::Viewer,
        };
        let command = PutResourcePermissionCommand {
            address: ResourceRef::new(ResourceKind::Gallery, 10),
            caller_id: 5,
            grants: vec![request, request],
            visibility: Visibility::Private,
        };

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(PermissionError::Invalid("duplicate_grant"))
        ));
        Ok(())
    }

    #[tokio::test]
    async fn a_role_principal_cannot_be_written_over_http() -> Result<(), Box<dyn Error>> {
        // Arrange
        let unit_of_work = TestUnitOfWork {
            grants: grants_with(Vec::new()),
            resources: resources_with(access(
                ResourceKind::Gallery,
                Some(5),
                Visibility::Private,
                Vec::new(),
            )),
            users: users_with(vec![(5, Role::Standard)]),
            ..TestUnitOfWork::default()
        };
        let use_case = Replace::new(Arc::new(TestFactory::new(unit_of_work)));
        let command = PutResourcePermissionCommand {
            address: ResourceRef::new(ResourceKind::Gallery, 10),
            caller_id: 5,
            grants: vec![GrantRequest {
                child_kind: ChildKind::All,
                effect: GrantEffect::Allow,
                principal: Principal::Role(Role::Admin),
                role: ResourceRole::Viewer,
            }],
            visibility: Visibility::Private,
        };

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(PermissionError::Invalid("principal_not_storable"))
        ));
        Ok(())
    }

    #[tokio::test]
    async fn admin_creation_cannot_be_granted() -> Result<(), Box<dyn Error>> {
        // Arrange
        let unit_of_work = TestUnitOfWork {
            grants: grants_with(Vec::new()),
            resources: resources_with(access(
                ResourceKind::UserCollection,
                Some(5),
                Visibility::Private,
                Vec::new(),
            )),
            users: users_with(vec![(5, Role::Standard), (9, Role::Standard)]),
            ..TestUnitOfWork::default()
        };
        let use_case = Replace::new(Arc::new(TestFactory::new(unit_of_work)));
        let command = PutResourcePermissionCommand {
            address: ResourceRef::new(ResourceKind::UserCollection, 10),
            caller_id: 5,
            grants: vec![GrantRequest {
                child_kind: ChildKind::Admin,
                effect: GrantEffect::Allow,
                principal: Principal::User(9),
                role: ResourceRole::Contributor,
            }],
            visibility: Visibility::Private,
        };

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(PermissionError::Invalid("admin_is_not_grantable"))
        ));
        Ok(())
    }

    #[tokio::test]
    async fn a_manager_shares_a_viewer() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut grants = grants_with(Vec::new());
        grants
            .expect_save_grant()
            .times(1)
            .returning(|grant| Box::pin(async move { Ok(grant) }));
        let unit_of_work = TestUnitOfWork {
            grants,
            resources: resources_with(access(
                ResourceKind::Gallery,
                Some(5),
                Visibility::Private,
                Vec::new(),
            )),
            users: users_with(vec![(5, Role::Standard), (9, Role::Standard)]),
            ..TestUnitOfWork::default()
        };
        let use_case = Share::new(Arc::new(TestFactory::new(unit_of_work)));
        let command = ShareResourceCommand {
            address: ResourceRef::new(ResourceKind::Gallery, 10),
            caller_id: 5,
            principal: Principal::User(9),
            role: ResourceRole::Viewer,
        };

        // Act
        let grant = use_case.execute(command).await?;

        // Assert
        assert_eq!(grant.role(), ResourceRole::Viewer);
        assert_eq!(grant.effect(), GrantEffect::Allow);
        Ok(())
    }

    #[tokio::test]
    async fn sharing_cannot_escalate() -> Result<(), Box<dyn Error>> {
        // Arrange
        let unit_of_work = TestUnitOfWork {
            grants: grants_with(Vec::new()),
            resources: resources_with(access(
                ResourceKind::Gallery,
                Some(5),
                Visibility::Private,
                Vec::new(),
            )),
            users: users_with(vec![(5, Role::Standard), (9, Role::Standard)]),
            ..TestUnitOfWork::default()
        };
        let use_case = Share::new(Arc::new(TestFactory::new(unit_of_work)));
        let command = ShareResourceCommand {
            address: ResourceRef::new(ResourceKind::Gallery, 10),
            caller_id: 5,
            principal: Principal::User(9),
            role: ResourceRole::Editor,
        };

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PermissionError::Forbidden)));
        Ok(())
    }

    #[tokio::test]
    async fn a_grant_is_revoked() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut grants = grants_with(Vec::new());
        grants
            .expect_delete()
            .times(1)
            .returning(|grant_id, resource_id| {
                assert_eq!(grant_id, 7);
                assert_eq!(resource_id, 1);
                Box::pin(async { Ok(true) })
            });
        let unit_of_work = TestUnitOfWork {
            grants,
            resources: resources_with(access(
                ResourceKind::Gallery,
                Some(5),
                Visibility::Private,
                Vec::new(),
            )),
            users: users_with(vec![(5, Role::Standard)]),
            ..TestUnitOfWork::default()
        };
        let use_case = Revoke::new(Arc::new(TestFactory::new(unit_of_work)));
        let command = RevokeShareCommand {
            address: ResourceRef::new(ResourceKind::Gallery, 10),
            caller_id: 5,
            grant_id: 7,
        };

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(result.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn revoking_an_unknown_grant_is_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut grants = grants_with(Vec::new());
        grants
            .expect_delete()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(false) }));
        let unit_of_work = TestUnitOfWork {
            grants,
            resources: resources_with(access(
                ResourceKind::Gallery,
                Some(5),
                Visibility::Private,
                Vec::new(),
            )),
            users: users_with(vec![(5, Role::Standard)]),
            ..TestUnitOfWork::default()
        };
        let use_case = Revoke::new(Arc::new(TestFactory::new(unit_of_work)));
        let command = RevokeShareCommand {
            address: ResourceRef::new(ResourceKind::Gallery, 10),
            caller_id: 5,
            grant_id: 7,
        };

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PermissionError::NotFound)));
        Ok(())
    }

    #[tokio::test]
    async fn a_storage_failure_surfaces_as_an_unknown_error() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let unit_of_work = TestUnitOfWork {
            grants: grants_with(Vec::new()),
            resources: no_matches(),
            users,
            ..TestUnitOfWork::default()
        };
        let use_case = Summary::new(Arc::new(TestFactory::new(unit_of_work)));
        let command = GetPermissionSummaryCommand {
            caller_id: 5,
            decision: None,
            user_id: 5,
        };

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PermissionError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn a_missing_caller_is_refused() -> Result<(), Box<dyn Error>> {
        // Arrange
        let unit_of_work = TestUnitOfWork {
            grants: grants_with(Vec::new()),
            resources: no_matches(),
            users: users_with(Vec::new()),
            ..TestUnitOfWork::default()
        };
        let use_case = Summary::new(Arc::new(TestFactory::new(unit_of_work)));
        let command = GetPermissionSummaryCommand {
            caller_id: 5,
            decision: None,
            user_id: 5,
        };

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(PermissionError::Forbidden)));
        Ok(())
    }
}
