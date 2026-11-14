//! The one application-side authorizer every use case calls.
//!
//! It searches the resource with its authorization-parent chain, the grants
//! that apply to it and its relation containers in the caller's transaction,
//! calls the pure
//! domain engine, emits the single `authz_failed` event of a denial, and maps
//! the denial to a concealment decision. It is the only place under
//! `src/lib/application` that logs `authz_failed`.

use tracing::warn;

use crate::domain::model::{Action, ChildKind, ResourceRef, User};
use crate::domain::port::{RepositoryError, ResourceFilter, ResourceRepository};
use crate::domain::service::{AuthorizationEngine, EngineDecision};

/// Why an authorization check refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Denied {
    /// Whether the refusal conceals the addressed resource (`404`).
    conceal: bool,
}

impl Denied {
    /// Return whether the refusal conceals the addressed resource.
    #[must_use]
    pub(crate) fn conceals(self) -> bool {
        self.conceal
    }
}

/// Error returned by an authorization check.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum AuthorizationError {
    /// The caller may not perform the action.
    #[error("the caller may not perform the action")]
    Denied(Denied),
    /// The decision could not be read for a non-specific reason.
    #[error("the authorization decision could not be read: {0}")]
    Unknown(#[source] anyhow::Error),
}

impl AuthorizationError {
    /// Return whether the refusal conceals the addressed resource.
    #[must_use]
    pub(crate) fn conceals(&self) -> bool {
        match *self {
            Self::Denied(denied) => denied.conceals(),
            Self::Unknown(_) => false,
        }
    }
}

impl From<RepositoryError> for AuthorizationError {
    fn from(error: RepositoryError) -> Self {
        Self::Unknown(error.into())
    }
}

/// Decide whether `caller` may perform `action` on `address`.
///
/// A missing resource, a storage error and a deny all return
/// [`AuthorizationError::Denied`]: the first two deny by default and conceal
/// the resource.
pub(crate) async fn authorize<R: ResourceRepository + ?Sized>(
    resources: &mut R,
    caller: &User,
    address: ResourceRef,
    action: Action,
    child_kind: Option<ChildKind>,
) -> Result<EngineDecision, AuthorizationError> {
    let filter = ResourceFilter {
        entity_id: Some(address.id()),
        kind: Some(address.kind()),
        ..ResourceFilter::default()
    };
    let found = match resources.search(&filter, caller).await {
        Ok(found) => found.into_iter().next(),
        Err(_error) => None,
    };
    let Some(access) = found else {
        emit_denial(caller, action, address);
        return Err(AuthorizationError::Denied(Denied { conceal: true }));
    };

    let decision = AuthorizationEngine::authorize(caller, &access, action, child_kind);
    if decision.is_allowed() {
        return Ok(decision);
    }

    emit_denial(caller, action, address);
    let conceal = if action.is_read() {
        true
    } else {
        !AuthorizationEngine::authorize(caller, &access, Action::Discover, None).is_allowed()
    };
    Err(AuthorizationError::Denied(Denied { conceal }))
}

/// Decide whether `caller` may perform `action`, without emitting an event.
///
/// A listing filters the items a caller may not discover; dropping an item is
/// not a denial, so this check stays silent. A missing resource and a storage
/// error answer `false`, which the caller treats as a filtered item.
pub(crate) async fn allows<R: ResourceRepository + ?Sized>(
    resources: &mut R,
    caller: &User,
    address: ResourceRef,
    action: Action,
    child_kind: Option<ChildKind>,
) -> bool {
    let filter = ResourceFilter {
        entity_id: Some(address.id()),
        kind: Some(address.kind()),
        ..ResourceFilter::default()
    };
    match resources.search(&filter, caller).await {
        Ok(found) => found.first().is_some_and(|access| {
            AuthorizationEngine::authorize(caller, access, action, child_kind).is_allowed()
        }),
        Err(_) => false,
    }
}

/// Emit the one `authz_failed` event of a denial.
fn emit_denial(caller: &User, action: Action, address: ResourceRef) {
    warn!(
        target: "security",
        event = "authz_failed",
        actor = caller.id(),
        action = action.as_wire(),
        target = %format!("{}:{}", address.kind().as_wire(), address.id()),
        "denied an action"
    );
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use tracing_test::traced_test;

    use super::*;
    use crate::domain::model::{Resource, ResourceAccess, ResourceKind, Role, User, Visibility};
    use crate::domain::port::MockResourceRepository;

    /// The caller every case authorizes as.
    fn caller() -> Result<User, Box<dyn Error>> {
        Ok(User::try_new(
            1,
            "alice".to_owned(),
            None,
            1,
            Role::Standard,
        )?)
    }

    #[tokio::test]
    #[traced_test]
    async fn a_domain_rejection_is_recorded_once_by_its_caller() -> Result<(), Box<dyn Error>> {
        // Arrange: the domain engine rejects a private gallery owned by another
        // user, which it returns as a decision value.
        let mut resources = MockResourceRepository::new();
        resources.expect_search().times(1).returning(|filter, _| {
            let resource = Resource::from_stored(
                7,
                ResourceKind::Gallery,
                filter.entity_id.unwrap_or_default(),
                Some(99),
                None,
                Visibility::Private,
            );
            Box::pin(async move {
                Ok(vec![ResourceAccess::new(
                    resource,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                )])
            })
        });

        // Act
        let decision = authorize(
            &mut resources,
            &caller()?,
            ResourceRef::new(ResourceKind::Gallery, 7),
            Action::View,
            None,
        )
        .await;

        // Assert: the caller records the rejection exactly once and the domain
        // emits nothing of its own.
        assert!(matches!(decision, Err(AuthorizationError::Denied(_))));
        logs_assert(assert_one_authz_failure);
        Ok(())
    }

    /// Assert the captured output is the caller's single `authz_failed` event
    /// carrying its actor, action and target fields.
    fn assert_one_authz_failure(lines: &[&str]) -> Result<(), String> {
        let ours: Vec<&str> = lines
            .iter()
            .copied()
            .filter(|line| line.contains("event=\"authz_failed\""))
            .collect();
        if ours.len() != 1 {
            return Err(format!(
                "the caller must record exactly one authz_failed (found {})",
                ours.len()
            ));
        }
        let Some(line) = ours.first() else {
            return Err("the caller recorded no authz_failed event".to_owned());
        };
        for field in [
            "security:",
            "WARN",
            "actor=1",
            "action=\"view\"",
            "target=GALLERY:7",
        ] {
            if !line.contains(field) {
                return Err(format!("the denial event must carry {field}"));
            }
        }
        Ok(())
    }
}
