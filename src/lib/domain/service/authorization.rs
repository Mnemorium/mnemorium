//! The authorization policy: one decision over an action and a resource.
//!
//! This is a domain service: it depends only on domain models and values and
//! holds no I/O. Loading the caller, the caller's permissions and the resource
//! is application orchestration and stays in the application layer; emitting
//! the `authz_failed` event and mapping the denial belong to the use case.

use crate::domain::model::permission::Action;
use crate::domain::model::permission::Permission;
use crate::domain::model::permission::Protected;
use crate::domain::model::permission::ResourceRef;
use crate::domain::model::permission::RootAccess;
use crate::domain::model::user::User;

/// The rendering of a denial to the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Decision {
    /// The caller may perform the action.
    Allow,
    /// The caller may not perform the action.
    Deny(Denial),
}

impl Decision {
    /// Return whether the decision allows the action.
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow)
    }
}

/// A denied action and how the caller must see the refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Denial {
    /// The denied action.
    action: Action,
    /// Whether the refusal conceals the addressed resource (`404`).
    conceal: bool,
}

impl Denial {
    /// Return the denied action.
    #[must_use]
    pub fn action(&self) -> Action {
        self.action
    }

    /// Return whether the refusal conceals the addressed resource.
    ///
    /// A concealed denial answers `404 Not Found`, indistinguishable from a
    /// missing resource; a revealed denial answers `403 Forbidden`.
    #[must_use]
    pub fn conceals(&self) -> bool {
        self.conceal
    }
}

/// Domain service holding the authorization policy.
#[derive(Debug)]
#[non_exhaustive]
pub struct Authorization;

impl Authorization {
    /// Return whether the scope `action` resolves to covers `resource`.
    fn allows<P: Protected + ?Sized>(
        caller: &User,
        permissions: &[Permission],
        action: Action,
        resource: &P,
    ) -> bool {
        if caller.is_root_admin() && action.root_access() == RootAccess::All {
            return true;
        }
        if action == Action::UserRead
            && resource.resource_ref().map(ResourceRef::id) == Some(caller.id())
        {
            return true;
        }
        if action == Action::UserUpdate
            && resource.is_privileged_account()
            && resource.resource_ref().map(ResourceRef::id) != Some(caller.id())
            && !caller.is_root_admin()
        {
            return false;
        }
        let scope = permissions
            .iter()
            .find(|permission| permission.user_id() == caller.id() && permission.action() == action)
            .map_or_else(
                || action.role_scope(caller.role()),
                |permission| permission.scope().clone(),
            );
        scope.covers(caller.id(), resource)
    }

    /// Decide whether `caller` may perform `action` on `resource`.
    ///
    /// The effective scope is the Root Admin rule, then the caller's override
    /// for `action`, then the caller's role preset. A management or moderation
    /// denial conceals the resource when the caller cannot read it.
    #[must_use]
    pub fn decide<P: Protected + ?Sized>(
        caller: &User,
        permissions: &[Permission],
        action: Action,
        resource: &P,
    ) -> Decision {
        if Self::allows(caller, permissions, action, resource) {
            return Decision::Allow;
        }

        let conceal = action.conceals_denial()
            || action
                .read_action()
                .is_some_and(|read| !Self::allows(caller, permissions, read, resource));
        Decision::Deny(Denial { action, conceal })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::error::Error;

    use chrono::NaiveDateTime;

    use crate::domain::alias::NumericID;
    use crate::domain::model::gallery::Gallery;
    use crate::domain::model::integrity_hash::IntegrityHash;
    use crate::domain::model::music_playlist::MusicPlaylist;
    use crate::domain::model::permission::ACTIONS;
    use crate::domain::model::permission::Action;
    use crate::domain::model::permission::Global;
    use crate::domain::model::permission::Permission;
    use crate::domain::model::permission::ResourceKind;
    use crate::domain::model::permission::ResourceRef;
    use crate::domain::model::permission::Scope;
    use crate::domain::model::upload::ChunkBitmap;
    use crate::domain::model::upload::Upload;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::test_helpers::DIGEST;
    use crate::test_helpers::sample_album;
    use crate::test_helpers::sample_movie;
    use crate::test_helpers::sample_series;

    use super::Authorization;
    use super::Decision;

    fn user(id: NumericID, role: Role) -> Result<User, Box<dyn Error>> {
        Ok(User::try_new(
            id,
            "caller".to_owned(),
            None,
            id.max(1),
            role,
        )?)
    }

    fn gallery(id: NumericID, owner_id: Option<NumericID>, is_public: bool) -> Gallery {
        let now = NaiveDateTime::default();
        Gallery::new(id, owner_id, "Gallery".to_owned(), is_public, now, now)
    }

    fn playlist(id: NumericID, owner_id: NumericID, is_public: bool) -> MusicPlaylist {
        let now = NaiveDateTime::default();
        MusicPlaylist::new(
            id,
            "Mix".to_owned(),
            now,
            now,
            owner_id,
            is_public,
            None,
            Vec::new(),
        )
    }

    fn upload(id: NumericID, owner_id: NumericID) -> Result<Upload, Box<dyn Error>> {
        Ok(Upload::try_new(
            id,
            owner_id,
            "song.flac".to_owned(),
            8,
            "audio/flac".to_owned(),
            4,
            IntegrityHash::try_new(DIGEST.to_owned())?,
            ChunkBitmap::from_bytes(vec![0], 2)?,
            false,
            NaiveDateTime::default(),
        )?)
    }

    fn override_scope(
        user_id: NumericID,
        action: Action,
        scope: Scope,
    ) -> Result<Permission, Box<dyn Error>> {
        Ok(Permission::try_new(user_id, action, scope)?)
    }

    #[test]
    fn every_action_is_fully_described() {
        // Arrange
        let wire_names: BTreeSet<&str> = ACTIONS.iter().map(|action| action.as_wire()).collect();

        // Act & Assert
        assert_eq!(
            wire_names.len(),
            ACTIONS.len(),
            "the catalog must list each action once"
        );
        assert!(
            ACTIONS
                .iter()
                .all(|action| Action::from_wire(action.as_wire()) == Some(*action)),
            "every action must round-trip through its stored name"
        );
        assert!(
            ACTIONS.iter().all(|action| action
                .role_scope(Role::Standard)
                .is_supported_by(action.supported_scopes())),
            "every Standard preset must be a supported scope"
        );
        assert!(
            ACTIONS.iter().all(|action| action
                .role_scope(Role::Admin)
                .is_supported_by(action.supported_scopes())),
            "every Admin preset must be a supported scope"
        );
        assert!(
            ACTIONS
                .iter()
                .all(|action| !action.verb().is_empty() && !action.target().is_empty()),
            "every action must name an event verb and target"
        );
    }

    #[test]
    fn default_standard_caller_reads_a_public_gallery() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(9, Role::Standard)?;
        let resource = gallery(2, Some(3), true);

        // Act
        let decision = Authorization::decide(&caller, &[], Action::GalleryRead, &resource);

        // Assert
        assert!(decision.is_allowed());
        Ok(())
    }

    #[test]
    fn default_standard_caller_cannot_read_another_private_gallery() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(9, Role::Standard)?;
        let resource = gallery(2, Some(3), false);

        // Act
        let decision = Authorization::decide(&caller, &[], Action::GalleryRead, &resource);

        // Assert
        assert!(!decision.is_allowed());
        assert!(matches!(decision, Decision::Deny(denial) if denial.conceals()));
        Ok(())
    }

    #[test]
    fn owned_scope_hides_another_public_gallery() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(9, Role::Standard)?;
        let resource = gallery(2, Some(3), true);
        let permissions = [override_scope(9, Action::GalleryRead, Scope::Owned)?];

        // Act
        let decision = Authorization::decide(&caller, &permissions, Action::GalleryRead, &resource);

        // Assert
        assert!(!decision.is_allowed());
        Ok(())
    }

    #[test]
    fn all_scope_covers_a_private_gallery() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(9, Role::Standard)?;
        let resource = gallery(2, Some(3), false);
        let permissions = [override_scope(9, Action::GalleryRead, Scope::All)?];

        // Act
        let decision = Authorization::decide(&caller, &permissions, Action::GalleryRead, &resource);

        // Assert
        assert!(decision.is_allowed());
        Ok(())
    }

    #[test]
    fn listed_scope_covers_exactly_the_named_gallery() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(9, Role::Standard)?;
        let named = gallery(2, Some(3), false);
        let other = gallery(3, Some(4), true);
        let permissions = [override_scope(
            9,
            Action::GalleryRead,
            Scope::Listed(vec![ResourceRef::new(ResourceKind::Gallery, 2)]),
        )?];

        // Act
        let named_decision =
            Authorization::decide(&caller, &permissions, Action::GalleryRead, &named);
        let other_decision =
            Authorization::decide(&caller, &permissions, Action::GalleryRead, &other);

        // Assert
        assert!(named_decision.is_allowed());
        assert!(!other_decision.is_allowed());
        Ok(())
    }

    #[test]
    fn none_scope_hides_a_public_playlist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(9, Role::Standard)?;
        let resource = playlist(2, 3, true);
        let permissions = [override_scope(9, Action::PlaylistRead, Scope::None)?];

        // Act
        let decision =
            Authorization::decide(&caller, &permissions, Action::PlaylistRead, &resource);

        // Assert
        assert!(!decision.is_allowed());
        assert!(matches!(decision, Decision::Deny(denial) if denial.conceals()));
        Ok(())
    }

    #[test]
    fn root_admin_reads_a_private_gallery() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(0, Role::Admin)?;
        let resource = gallery(2, Some(3), false);

        // Act
        let decision = Authorization::decide(&caller, &[], Action::GalleryRead, &resource);

        // Assert
        assert!(decision.is_allowed());
        Ok(())
    }

    #[test]
    fn root_admin_manages_another_playlist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(0, Role::Admin)?;
        let resource = playlist(2, 3, false);

        // Act
        let decision = Authorization::decide(&caller, &[], Action::PlaylistManage, &resource);

        // Assert
        assert!(decision.is_allowed());
        Ok(())
    }

    #[test]
    fn root_admin_cannot_read_another_upload() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(0, Role::Admin)?;
        let resource = upload(2, 3)?;

        // Act
        let decision = Authorization::decide(&caller, &[], Action::UploadRead, &resource);

        // Assert
        assert!(!decision.is_allowed());
        assert!(matches!(decision, Decision::Deny(denial) if denial.conceals()));
        Ok(())
    }

    #[test]
    fn owner_reads_its_upload() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(3, Role::Standard)?;
        let resource = upload(2, 3)?;

        // Act
        let decision = Authorization::decide(&caller, &[], Action::UploadWrite, &resource);

        // Assert
        assert!(decision.is_allowed());
        Ok(())
    }

    #[test]
    fn visible_playlist_moderation_is_revealed() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(9, Role::Standard)?;
        let resource = playlist(2, 3, true);
        let permissions = [override_scope(9, Action::PlaylistModerate, Scope::None)?];

        // Act
        let decision =
            Authorization::decide(&caller, &permissions, Action::PlaylistModerate, &resource);

        // Assert
        assert!(matches!(decision, Decision::Deny(denial) if !denial.conceals()));
        Ok(())
    }

    #[test]
    fn invisible_playlist_moderation_is_concealed() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(9, Role::Standard)?;
        let resource = playlist(2, 3, false);

        // Act
        let decision = Authorization::decide(&caller, &[], Action::PlaylistModerate, &resource);

        // Assert
        assert!(matches!(decision, Decision::Deny(denial) if denial.conceals()));
        Ok(())
    }

    #[test]
    fn standard_caller_reads_its_own_account() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(9, Role::Standard)?;
        let target = user(9, Role::Standard)?;

        // Act
        let decision = Authorization::decide(&caller, &[], Action::UserRead, &target);

        // Assert
        assert!(decision.is_allowed());
        Ok(())
    }

    #[test]
    fn standard_caller_cannot_read_another_account() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(9, Role::Standard)?;
        let target = user(8, Role::Standard)?;

        // Act
        let decision = Authorization::decide(&caller, &[], Action::UserRead, &target);

        // Assert
        assert!(matches!(decision, Decision::Deny(denial) if !denial.conceals()));
        Ok(())
    }

    #[test]
    fn admin_cannot_update_another_admin() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(7, Role::Admin)?;
        let target = user(8, Role::Admin)?;

        // Act
        let decision = Authorization::decide(&caller, &[], Action::UserUpdate, &target);

        // Assert
        assert!(matches!(decision, Decision::Deny(denial) if !denial.conceals()));
        Ok(())
    }

    #[test]
    fn admin_updates_a_standard_user() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(7, Role::Admin)?;
        let target = user(8, Role::Standard)?;

        // Act
        let decision = Authorization::decide(&caller, &[], Action::UserUpdate, &target);

        // Assert
        assert!(decision.is_allowed());
        Ok(())
    }

    #[test]
    fn root_admin_updates_another_admin() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(0, Role::Admin)?;
        let target = user(8, Role::Admin)?;

        // Act
        let decision = Authorization::decide(&caller, &[], Action::UserUpdate, &target);

        // Assert
        assert!(decision.is_allowed());
        Ok(())
    }

    #[test]
    fn standard_caller_cannot_create_a_user() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(9, Role::Standard)?;

        // Act
        let decision = Authorization::decide(&caller, &[], Action::UserCreate, &Global);

        // Assert
        assert!(matches!(decision, Decision::Deny(denial) if !denial.conceals()));
        Ok(())
    }

    #[test]
    fn granted_standard_caller_creates_a_user() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(9, Role::Standard)?;
        let permissions = [override_scope(9, Action::UserCreate, Scope::All)?];

        // Act
        let decision = Authorization::decide(&caller, &permissions, Action::UserCreate, &Global);

        // Assert
        assert!(decision.is_allowed());
        Ok(())
    }

    #[test]
    fn admin_cannot_create_an_admin() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(7, Role::Admin)?;

        // Act
        let decision = Authorization::decide(&caller, &[], Action::UserCreateAdmin, &Global);

        // Assert
        assert!(!decision.is_allowed());
        Ok(())
    }

    #[test]
    fn catalog_read_covers_a_film_and_a_listed_album_is_visible() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(9, Role::Standard)?;
        let movie = sample_movie(5)?;
        let series = sample_series(6, Vec::new())?;
        let album = sample_album(7, Vec::new())?;
        let permissions = [override_scope(
            9,
            Action::CatalogRead,
            Scope::Listed(vec![ResourceRef::new(ResourceKind::MusicAlbum, 7)]),
        )?];

        // Act
        let movie_decision =
            Authorization::decide(&caller, &permissions, Action::CatalogRead, &movie);
        let series_decision =
            Authorization::decide(&caller, &permissions, Action::CatalogRead, &series);
        let album_decision =
            Authorization::decide(&caller, &permissions, Action::CatalogRead, &album);

        // Assert
        assert!(!movie_decision.is_allowed());
        assert!(!series_decision.is_allowed());
        assert!(album_decision.is_allowed());
        Ok(())
    }
}
