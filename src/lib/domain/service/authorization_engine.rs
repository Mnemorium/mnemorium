//! The authorization decision engine.
//!
//! This is a pure domain service: it receives the caller, the addressed
//! resource, its authorization-parent chain, the matching grants and the
//! relation containers that reference it, and returns a decision with its
//! explanation. It holds no I/O and no port; loading the access bundle,
//! emitting the `authz_failed` event and mapping the denial belong to the
//! application layer.

use crate::domain::NumericID;
use crate::domain::model::{
    Action, ChildKind, Grant, GrantEffect, Resource, ResourceAccess, ResourceKind, ResourceRef,
    ResourceRole, User, Visibility,
};

/// Why a decision allowed or denied an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum Reason {
    /// No grant, ownership, role, visibility or relation lend matched.
    Default,
    /// A matching grant carried the `Deny` effect.
    Deny,
    /// A matching grant allowed the action.
    Grant,
    /// The caller read its own `User` resource.
    OwnAccount,
    /// The caller owns the resource or one of its ancestors.
    Owner,
    /// A relation container lent its read actions.
    Relation,
    /// The caller is the root.
    Root,
    /// The upload owner-only rule decided the request.
    Upload,
    /// The resource's visibility tier allowed the action.
    Visibility,
}

/// The recorded cause of a decision.
///
/// The explanation is what the permission API and the `authz_failed` log
/// event read; it names the reason, the grants that matched, the denying
/// grant, the authorization path and the resource's visibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Explanation {
    /// Whether the decision allowed the action.
    allowed: bool,
    /// The addressed resource followed by its authorization parents.
    auth_path: Vec<ResourceRef>,
    /// Identifier of the grant that denied the action, when one did.
    denied_by: Option<NumericID>,
    /// Identifiers of every grant that matched the action.
    matched: Vec<NumericID>,
    /// Why the decision allowed or denied the action.
    reason: Reason,
    /// Visibility of the addressed resource.
    visibility: Visibility,
}

impl Reason {
    /// Render the reason as its wire representation (`owner`).
    #[must_use]
    pub(crate) fn as_wire(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Deny => "deny",
            Self::Grant => "grant",
            Self::OwnAccount => "own_account",
            Self::Owner => "owner",
            Self::Relation => "relation",
            Self::Root => "root",
            Self::Upload => "upload",
            Self::Visibility => "visibility",
        }
    }
}

impl Explanation {
    /// Return whether the decision allowed the action.
    #[must_use]
    pub(crate) fn allowed(&self) -> bool {
        self.allowed
    }

    /// Return the addressed resource followed by its authorization parents.
    #[must_use]
    pub(crate) fn auth_path(&self) -> &[ResourceRef] {
        &self.auth_path
    }

    /// Return the identifier of the grant that denied the action.
    #[must_use]
    pub(crate) fn denied_by(&self) -> Option<NumericID> {
        self.denied_by
    }

    /// Return the identifiers of every grant that matched the action.
    #[must_use]
    pub(crate) fn matched(&self) -> &[NumericID] {
        &self.matched
    }

    /// Return why the decision allowed or denied the action.
    #[must_use]
    pub(crate) fn reason(&self) -> Reason {
        self.reason
    }

    /// Return the visibility of the addressed resource.
    #[must_use]
    pub(crate) fn visibility(&self) -> Visibility {
        self.visibility
    }
}

/// One authorization decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Decision {
    /// Why the decision allowed or denied the action.
    explanation: Explanation,
}

impl Decision {
    /// Return the recorded cause of the decision.
    #[must_use]
    pub(crate) fn explanation(&self) -> &Explanation {
        &self.explanation
    }

    /// Return whether the decision allowed the action.
    #[must_use]
    pub(crate) fn is_allowed(&self) -> bool {
        self.explanation.allowed
    }
}

/// The authorization decision engine.
#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct Authorization;

impl Authorization {
    /// Decide whether `caller` may perform `action` on the addressed resource.
    #[must_use]
    pub(crate) fn authorize(
        caller: &User,
        access: &ResourceAccess,
        action: Action,
        child_kind: Option<ChildKind>,
    ) -> Decision {
        Self::evaluate(caller, access, action, child_kind, true)
    }

    /// Build a decision from the rule that decided it and the matched grants.
    fn decision(
        access: &ResourceAccess,
        allowed: bool,
        reason: Reason,
        matched: Vec<NumericID>,
        denied_by: Option<NumericID>,
    ) -> Decision {
        let resource = access.resource();
        let mut auth_path = Vec::with_capacity(access.ancestors().len().saturating_add(1));
        auth_path.push(ResourceRef::new(resource.kind(), resource.entity_id()));
        auth_path.extend(
            access
                .ancestors()
                .iter()
                .map(|ancestor| ResourceRef::new(ancestor.kind(), ancestor.entity_id())),
        );
        Decision {
            explanation: Explanation {
                allowed,
                auth_path,
                denied_by,
                matched,
                reason,
                visibility: resource.visibility(),
            },
        }
    }

    /// Evaluate one access bundle, optionally lending through relations.
    ///
    /// The order is fixed: upload owner-only, the root, the own-account read,
    /// any matching `Deny`, any matching `Allow` from ownership, a grant, the
    /// role, visibility or a relation's read lend, and otherwise `Deny`.
    fn evaluate(
        caller: &User,
        access: &ResourceAccess,
        action: Action,
        child_kind: Option<ChildKind>,
        lend_relations: bool,
    ) -> Decision {
        let resource = access.resource();

        // An upload is decided owner-only before every other rule, including
        // the root's bypass. No grant widens it.
        if resource.kind() == ResourceKind::Upload {
            return Self::decision(
                access,
                resource.owner_id() == Some(caller.id()),
                Reason::Upload,
                Vec::new(),
                None,
            );
        }

        if caller.is_root_admin() {
            return Self::decision(access, true, Reason::Root, Vec::new(), None);
        }

        if resource.kind() == ResourceKind::User
            && resource.entity_id() == caller.id()
            && action.is_read()
        {
            return Self::decision(access, true, Reason::OwnAccount, Vec::new(), None);
        }

        let matching: Vec<&Grant> = access
            .grants()
            .iter()
            .filter(|grant| Self::on_chain(access, grant))
            .filter(|grant| grant.applies_to(caller.id(), caller.role()))
            .filter(|grant| grant.matches(action, child_kind))
            .collect();
        let matched: Vec<NumericID> = matching.iter().map(|grant| grant.id()).collect();

        // A deny wins over every allow, wherever it sits on the chain.
        if let Some(deny) = matching
            .iter()
            .find(|grant| grant.effect() == GrantEffect::Deny)
        {
            return Self::decision(access, false, Reason::Deny, matched, Some(deny.id()));
        }

        if Self::owner_allows(caller, access, action) {
            return Self::decision(access, true, Reason::Owner, matched, None);
        }

        if matching
            .iter()
            .any(|grant| grant.effect() == GrantEffect::Allow)
        {
            return Self::decision(access, true, Reason::Grant, matched, None);
        }

        if Self::visibility_allows(resource.visibility(), action) {
            return Self::decision(access, true, Reason::Visibility, matched, None);
        }

        if lend_relations
            && Self::is_lent_action(action)
            && access.relations().iter().any(|relation| {
                let container = ResourceAccess::new(
                    relation.container().clone(),
                    relation.ancestors().to_vec(),
                    relation.grants().to_vec(),
                    Vec::new(),
                );
                Self::evaluate(caller, &container, Action::View, None, false).is_allowed()
            })
        {
            return Self::decision(access, true, Reason::Relation, matched, None);
        }

        Self::decision(access, false, Reason::Default, matched, None)
    }

    /// Return whether a relation may lend `action`.
    #[must_use]
    fn is_lent_action(action: Action) -> bool {
        matches!(
            action,
            Action::Discover | Action::ViewMetadata | Action::View | Action::Stream
        )
    }

    /// Return whether a grant belongs to the addressed resource or an ancestor.
    #[must_use]
    fn on_chain(access: &ResourceAccess, grant: &Grant) -> bool {
        grant.resource_id() == access.resource().resource_id()
            || access
                .ancestors()
                .iter()
                .any(|ancestor| ancestor.resource_id() == grant.resource_id())
    }

    /// Return whether the implicit owner allows `action`.
    #[must_use]
    fn owner_allows(caller: &User, access: &ResourceAccess, action: Action) -> bool {
        access
            .resource()
            .owner_id()
            .or_else(|| access.ancestors().iter().find_map(Resource::owner_id))
            .is_some_and(|owner_id| owner_id == caller.id() && ResourceRole::Owner.includes(action))
    }

    /// Return whether the resource's visibility grants `action`.
    #[must_use]
    fn visibility_allows(visibility: Visibility, action: Action) -> bool {
        match visibility {
            Visibility::Private => false,
            Visibility::Restricted => matches!(action, Action::Discover | Action::ViewMetadata),
            Visibility::Public => ResourceRole::Viewer.includes(action),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    use crate::domain::NumericID;
    use crate::domain::model::{
        ChildKind, Grant, GrantEffect, Principal, RelationSource, Resource, ResourceAccess,
        ResourceKind, ResourceRole, Role, User, Visibility,
    };

    fn user(id: NumericID, role: Role) -> Result<User, Box<dyn Error>> {
        Ok(User::try_new(
            id,
            "caller".to_owned(),
            None,
            id.max(1),
            role,
        )?)
    }

    fn resource(
        id: NumericID,
        kind: ResourceKind,
        owner: Option<NumericID>,
        visibility: Visibility,
    ) -> Resource {
        Resource::from_stored(id, kind, id, owner, None, visibility)
    }

    fn access(
        resource: Resource,
        grants: Vec<Grant>,
        relations: Vec<RelationSource>,
    ) -> ResourceAccess {
        ResourceAccess::new(resource, Vec::new(), grants, relations)
    }

    fn grant(
        principal: Principal,
        resource_id: NumericID,
        role: ResourceRole,
        effect: GrantEffect,
    ) -> Grant {
        Grant::from_stored(0, principal, resource_id, role, ChildKind::All, effect)
    }

    #[test]
    fn owner_allows_manage_permissions() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(3, Role::Standard)?;
        let access = access(
            resource(10, ResourceKind::Gallery, Some(3), Visibility::Private),
            Vec::new(),
            Vec::new(),
        );

        // Act
        let decision = Authorization::authorize(&caller, &access, Action::ManagePermissions, None);

        // Assert
        assert!(decision.is_allowed());
        Ok(())
    }

    #[test]
    fn an_owner_reaches_a_child_by_inheritance() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(3, Role::Standard)?;
        let parent = resource(10, ResourceKind::Gallery, Some(3), Visibility::Private);
        let child = resource(11, ResourceKind::GalleryItem, None, Visibility::Private);
        let access = ResourceAccess::new(child, vec![parent], Vec::new(), Vec::new());

        // Act
        let decision = Authorization::authorize(&caller, &access, Action::EditMetadata, None);

        // Assert
        assert!(decision.is_allowed());
        Ok(())
    }

    #[test]
    fn a_deny_defeats_a_peer_allow() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(3, Role::Standard)?;
        let grants = vec![
            grant(
                Principal::User(3),
                10,
                ResourceRole::Viewer,
                GrantEffect::Allow,
            ),
            grant(
                Principal::User(3),
                10,
                ResourceRole::Viewer,
                GrantEffect::Deny,
            ),
        ];
        let access = access(
            resource(10, ResourceKind::Gallery, Some(9), Visibility::Private),
            grants,
            Vec::new(),
        );

        // Act
        let decision = Authorization::authorize(&caller, &access, Action::View, None);

        // Assert
        assert!(!decision.is_allowed());
        Ok(())
    }

    #[test]
    fn an_ancestor_deny_defeats_a_child_allow() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(3, Role::Standard)?;
        let parent = resource(10, ResourceKind::Gallery, Some(9), Visibility::Private);
        let child = resource(11, ResourceKind::GalleryItem, None, Visibility::Private);
        let grants = vec![
            grant(
                Principal::User(3),
                10,
                ResourceRole::Viewer,
                GrantEffect::Deny,
            ),
            grant(
                Principal::User(3),
                11,
                ResourceRole::Viewer,
                GrantEffect::Allow,
            ),
        ];
        let access = ResourceAccess::new(child, vec![parent], grants, Vec::new());

        // Act
        let decision = Authorization::authorize(&caller, &access, Action::View, None);

        // Assert
        assert!(!decision.is_allowed());
        Ok(())
    }

    #[test]
    fn a_child_deny_defeats_an_inherited_allow() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(3, Role::Standard)?;
        let parent = resource(10, ResourceKind::Gallery, Some(9), Visibility::Private);
        let child = resource(11, ResourceKind::GalleryItem, None, Visibility::Private);
        let grants = vec![
            grant(
                Principal::User(3),
                10,
                ResourceRole::Viewer,
                GrantEffect::Allow,
            ),
            grant(
                Principal::User(3),
                11,
                ResourceRole::Viewer,
                GrantEffect::Deny,
            ),
        ];
        let access = ResourceAccess::new(child, vec![parent], grants, Vec::new());

        // Act
        let decision = Authorization::authorize(&caller, &access, Action::View, None);

        // Assert
        assert!(!decision.is_allowed());
        Ok(())
    }

    #[test]
    fn a_role_grant_applies_to_the_role() -> Result<(), Box<dyn Error>> {
        // Arrange
        let standard = user(3, Role::Standard)?;
        let admin = user(4, Role::Admin)?;
        let grants = vec![grant(
            Principal::Role(Role::Admin),
            10,
            ResourceRole::Viewer,
            GrantEffect::Allow,
        )];
        let access = access(
            resource(10, ResourceKind::Gallery, Some(9), Visibility::Private),
            grants,
            Vec::new(),
        );

        // Act
        let standard_decision = Authorization::authorize(&standard, &access, Action::View, None);
        let admin_decision = Authorization::authorize(&admin, &access, Action::View, None);

        // Assert
        assert!(!standard_decision.is_allowed());
        assert!(admin_decision.is_allowed());
        Ok(())
    }

    #[test]
    fn a_private_resource_conceals_and_a_restricted_one_exposes_metadata()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(3, Role::Standard)?;
        let private = access(
            resource(10, ResourceKind::Gallery, Some(9), Visibility::Private),
            Vec::new(),
            Vec::new(),
        );
        let restricted = access(
            resource(11, ResourceKind::Gallery, Some(9), Visibility::Restricted),
            Vec::new(),
            Vec::new(),
        );

        // Act
        let private_decision = Authorization::authorize(&caller, &private, Action::View, None);
        let metadata = Authorization::authorize(&caller, &restricted, Action::ViewMetadata, None);
        let view = Authorization::authorize(&caller, &restricted, Action::View, None);

        // Assert
        assert!(!private_decision.is_allowed());
        assert!(metadata.is_allowed());
        assert!(!view.is_allowed());
        Ok(())
    }

    #[test]
    fn public_grants_the_viewer_actions_but_not_download() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(3, Role::Standard)?;
        let access = access(
            resource(10, ResourceKind::MusicPlaylist, Some(9), Visibility::Public),
            Vec::new(),
            Vec::new(),
        );

        // Act
        let view = Authorization::authorize(&caller, &access, Action::View, None);
        let stream = Authorization::authorize(&caller, &access, Action::Stream, None);
        let download = Authorization::authorize(&caller, &access, Action::Download, None);

        // Assert
        assert!(view.is_allowed());
        assert!(stream.is_allowed());
        assert!(!download.is_allowed());
        Ok(())
    }

    #[test]
    fn the_root_bypasses_every_deny_but_not_an_upload() -> Result<(), Box<dyn Error>> {
        // Arrange
        let root = user(0, Role::Admin)?;
        let grants = vec![grant(
            Principal::User(0),
            10,
            ResourceRole::Viewer,
            GrantEffect::Deny,
        )];
        let gallery = access(
            resource(10, ResourceKind::Gallery, Some(9), Visibility::Private),
            grants,
            Vec::new(),
        );
        let upload = access(
            resource(11, ResourceKind::Upload, Some(9), Visibility::Private),
            Vec::new(),
            Vec::new(),
        );

        // Act
        let gallery_decision = Authorization::authorize(&root, &gallery, Action::Delete, None);
        let upload_decision = Authorization::authorize(&root, &upload, Action::View, None);

        // Assert
        assert!(gallery_decision.is_allowed());
        assert!(!upload_decision.is_allowed());
        Ok(())
    }

    #[test]
    fn an_upload_is_owner_only() -> Result<(), Box<dyn Error>> {
        // Arrange
        let owner = user(3, Role::Standard)?;
        let other = user(4, Role::Standard)?;
        let access = access(
            resource(10, ResourceKind::Upload, Some(3), Visibility::Private),
            Vec::new(),
            Vec::new(),
        );

        // Act
        let owner_decision = Authorization::authorize(&owner, &access, Action::ReplaceMedia, None);
        let other_decision = Authorization::authorize(&other, &access, Action::View, None);

        // Assert
        assert!(owner_decision.is_allowed());
        assert!(!other_decision.is_allowed());
        Ok(())
    }

    #[test]
    fn a_caller_reads_its_own_account_without_a_grant() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(3, Role::Standard)?;
        let own = access(
            resource(3, ResourceKind::User, None, Visibility::Private),
            Vec::new(),
            Vec::new(),
        );
        let other = access(
            resource(4, ResourceKind::User, None, Visibility::Private),
            Vec::new(),
            Vec::new(),
        );

        // Act
        let own_decision = Authorization::authorize(&caller, &own, Action::ViewMetadata, None);
        let other_decision = Authorization::authorize(&caller, &other, Action::ViewMetadata, None);

        // Assert
        assert!(own_decision.is_allowed());
        assert!(!other_decision.is_allowed());
        Ok(())
    }

    #[test]
    fn a_relation_lends_a_read_action_only() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(3, Role::Standard)?;
        let playlist = resource(10, ResourceKind::MusicPlaylist, Some(9), Visibility::Public);
        let recording = access(
            resource(11, ResourceKind::MusicRecording, None, Visibility::Private),
            Vec::new(),
            vec![RelationSource::new(playlist, Vec::new(), Vec::new())],
        );

        // Act
        let stream = Authorization::authorize(&caller, &recording, Action::Stream, None);
        let edit = Authorization::authorize(&caller, &recording, Action::EditMetadata, None);

        // Assert
        assert!(stream.is_allowed());
        assert!(!edit.is_allowed());
        Ok(())
    }

    #[test]
    fn a_deny_on_the_target_defeats_a_relation_lend() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(3, Role::Standard)?;
        let playlist = resource(10, ResourceKind::MusicPlaylist, Some(9), Visibility::Public);
        let grants = vec![grant(
            Principal::User(3),
            11,
            ResourceRole::Viewer,
            GrantEffect::Deny,
        )];
        let recording = access(
            resource(11, ResourceKind::MusicRecording, None, Visibility::Private),
            grants,
            vec![RelationSource::new(playlist, Vec::new(), Vec::new())],
        );

        // Act
        let stream = Authorization::authorize(&caller, &recording, Action::Stream, None);

        // Assert
        assert!(!stream.is_allowed());
        Ok(())
    }

    #[test]
    fn a_create_child_grant_names_its_child_kind() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(3, Role::Standard)?;
        let grants = vec![Grant::from_stored(
            0,
            Principal::User(3),
            10,
            ResourceRole::Contributor,
            ChildKind::Gallery,
            GrantEffect::Allow,
        )];
        let access = access(
            resource(
                10,
                ResourceKind::GalleryCollection,
                None,
                Visibility::Private,
            ),
            grants,
            Vec::new(),
        );

        // Act
        let gallery = Authorization::authorize(
            &caller,
            &access,
            Action::CreateChild,
            Some(ChildKind::Gallery),
        );
        let film = Authorization::authorize(
            &caller,
            &access,
            Action::CreateChild,
            Some(ChildKind::Movie),
        );

        // Assert
        assert!(gallery.is_allowed());
        assert!(!film.is_allowed());
        Ok(())
    }

    #[test]
    fn every_action_renders_its_stable_stored_name() {
        // Act & Assert
        assert_eq!(Action::Discover.as_wire(), "discover");
        assert_eq!(Action::ViewMetadata.as_wire(), "view_metadata");
        assert_eq!(Action::View.as_wire(), "view");
        assert_eq!(Action::Stream.as_wire(), "stream");
        assert_eq!(Action::Download.as_wire(), "download");
        assert_eq!(Action::CreateChild.as_wire(), "create_child");
        assert_eq!(Action::EditMetadata.as_wire(), "edit_metadata");
        assert_eq!(Action::ReplaceMedia.as_wire(), "replace_media");
        assert_eq!(Action::Share.as_wire(), "share");
        assert_eq!(Action::ManagePermissions.as_wire(), "manage_permissions");
        assert_eq!(Action::Delete.as_wire(), "delete");
        assert_eq!(Action::ChangeRole.as_wire(), "change_role");
        assert_eq!(Action::ReplaceCredential.as_wire(), "replace_credential");
    }

    #[test]
    fn the_role_bundles_contain_their_documented_actions() {
        // Act & Assert
        assert!(
            ResourceRole::Viewer.includes(Action::Discover)
                && ResourceRole::Viewer.includes(Action::ViewMetadata)
                && ResourceRole::Viewer.includes(Action::View)
                && ResourceRole::Viewer.includes(Action::Stream)
        );
        assert!(ResourceRole::Downloader.includes(Action::Download));
        assert!(ResourceRole::Contributor.includes(Action::CreateChild));
        assert!(
            ResourceRole::Editor.includes(Action::EditMetadata)
                && ResourceRole::Editor.includes(Action::ReplaceMedia)
        );
        assert!(
            ResourceRole::Manager.includes(Action::Share)
                && ResourceRole::Manager.includes(Action::Delete)
        );
        assert!(ResourceRole::Owner.includes(Action::ManagePermissions));
        assert!(!ResourceRole::Manager.includes(Action::ManagePermissions));
    }
}
