//! The authorization policy: one decision over an action and a resource.
//!
//! This is a domain service: it depends only on domain models and values and
//! holds no I/O. Loading the caller, the caller's permissions and the resource
//! is application orchestration and stays in the application layer; emitting
//! the `authz_failed` event and mapping the denial belong to the use case.

use crate::domain::model::asset::AssetContext;
use crate::domain::model::file::File;
use crate::domain::model::permission::Action;
use crate::domain::model::permission::Permission;
use crate::domain::model::permission::Protected;
use crate::domain::model::permission::ResourceRef;
use crate::domain::model::permission::RootAccess;
use crate::domain::model::permission::Scope;
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
        Self::effective_scope(caller, permissions, action).covers(caller.id(), resource)
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

    /// Decide whether `caller` may read the asset owned by the resource named
    /// by `context`.
    ///
    /// `referrer` is the aggregate `context` names: the gallery or the
    /// playlist that owns the asset. Shared catalogue art has no single owner,
    /// so `Global` is passed for [`AssetContext::Library`].
    ///
    /// The decision is the owning resource's read action, never `file.read`:
    /// `file.read` is derived and its scope comes from the owner.
    #[must_use]
    pub fn decide_asset<P: Protected + ?Sized>(
        caller: &User,
        permissions: &[Permission],
        context: AssetContext,
        referrer: &P,
    ) -> Decision {
        match context {
            AssetContext::Library => {
                // Catalogue art is shared and public, so a `LISTED` scope
                // still reaches it: `LISTED` narrows the catalogue metadata,
                // not the artwork (`design.md`, "`Global` for library art").
                // Only a caller shut out of the library entirely is denied.
                if caller.is_root_admin()
                    || !matches!(
                        Self::effective_scope(caller, permissions, Action::LibraryRead),
                        Scope::None | Scope::Owned
                    )
                {
                    return Decision::Allow;
                }
                Decision::Deny(Denial {
                    action: Action::LibraryRead,
                    conceal: true,
                })
            }
            AssetContext::Gallery(reference) => {
                debug_assert_eq!(
                    referrer.resource_ref(),
                    Some(reference),
                    "the referrer must be the gallery the context names"
                );
                Self::decide(caller, permissions, Action::GalleryRead, referrer)
            }
            AssetContext::Playlist(reference) => {
                debug_assert_eq!(
                    referrer.resource_ref(),
                    Some(reference),
                    "the referrer must be the playlist the context names"
                );
                Self::decide(caller, permissions, Action::PlaylistRead, referrer)
            }
        }
    }

    /// Decide whether `caller` may read an asset with no owning resource.
    ///
    /// An asset the library does not own and no gallery or playlist references
    /// is visible only to the caller who owns its file and to the Root Admin.
    /// Every other caller sees the concealed `404 Not Found` of a missing
    /// asset.
    #[must_use]
    pub fn decide_orphan_asset(caller: &User, file: &File) -> Decision {
        if caller.is_root_admin() || file.owner_id() == Some(caller.id()) {
            return Decision::Allow;
        }
        Decision::Deny(Denial {
            action: Action::FileRead,
            conceal: true,
        })
    }

    /// Return the scope `action` resolves to for `caller`.
    ///
    /// The effective scope is the caller's override for `action`, else the
    /// caller's role preset.
    fn effective_scope(caller: &User, permissions: &[Permission], action: Action) -> Scope {
        permissions
            .iter()
            .find(|permission| permission.user_id() == caller.id() && permission.action() == action)
            .map_or_else(
                || action.role_scope(caller.role()),
                |permission| permission.scope().clone(),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::error::Error;

    use chrono::NaiveDate;
    use chrono::NaiveDateTime;

    use crate::domain::alias::NumericID;
    use crate::domain::model::asset::AssetContext;
    use crate::domain::model::file::File;
    use crate::domain::model::gallery::Gallery;
    use crate::domain::model::integrity_hash::IntegrityHash;
    use crate::domain::model::music_playlist::MusicPlaylist;
    use crate::domain::model::permission::ACTIONS;
    use crate::domain::model::permission::Action;
    use crate::domain::model::permission::Global;
    use crate::domain::model::permission::Permission;
    use crate::domain::model::permission::PermissionError;
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

    fn file(id: NumericID, owner_id: NumericID) -> Result<File, Box<dyn Error>> {
        Ok(File::try_new(
            id,
            format!("/files/{id}.png"),
            owner_id,
            false,
            "image/png".to_owned(),
            NaiveDate::default(),
            10,
            NaiveDateTime::default(),
            None,
            None,
        )?)
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
            ACTIONS
                .iter()
                .filter(|action| !action.is_derived())
                .all(|action| action
                    .role_scope(Role::Standard)
                    .is_supported_by(action.supported_scopes())),
            "every Standard preset must be a supported scope"
        );
        assert!(
            ACTIONS
                .iter()
                .filter(|action| !action.is_derived())
                .all(|action| action
                    .role_scope(Role::Admin)
                    .is_supported_by(action.supported_scopes())),
            "every Admin preset must be a supported scope"
        );
        assert!(
            ACTIONS
                .iter()
                .filter(|action| action.is_derived())
                .all(|action| action.resource_kinds().is_empty()
                    && action.supported_scopes().is_empty()),
            "a derived action must carry no scope and no resource kind"
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
    fn library_read_covers_a_film_and_a_listed_album_is_visible() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(9, Role::Standard)?;
        let movie = sample_movie(5)?;
        let series = sample_series(6, Vec::new())?;
        let album = sample_album(7, Vec::new())?;
        let permissions = [override_scope(
            9,
            Action::LibraryRead,
            Scope::Listed(vec![ResourceRef::new(ResourceKind::MusicAlbum, 7)]),
        )?];

        // Act
        let movie_decision =
            Authorization::decide(&caller, &permissions, Action::LibraryRead, &movie);
        let series_decision =
            Authorization::decide(&caller, &permissions, Action::LibraryRead, &series);
        let album_decision =
            Authorization::decide(&caller, &permissions, Action::LibraryRead, &album);

        // Assert
        assert!(!movie_decision.is_allowed());
        assert!(!series_decision.is_allowed());
        assert!(album_decision.is_allowed());
        Ok(())
    }

    #[test]
    fn library_manage_preset_is_admin_only() -> Result<(), Box<dyn Error>> {
        // Arrange
        let album = sample_album(7, Vec::new())?;
        let standard = user(9, Role::Standard)?;
        let admin = user(4, Role::Admin)?;
        let root = user(0, Role::Admin)?;

        // Act
        let standard_decision =
            Authorization::decide(&standard, &[], Action::LibraryManage, &album);
        let admin_decision = Authorization::decide(&admin, &[], Action::LibraryManage, &album);
        let root_decision = Authorization::decide(&root, &[], Action::LibraryManage, &album);

        // Assert
        assert_eq!(
            Action::LibraryManage.role_scope(Role::Standard),
            Scope::None
        );
        assert_eq!(Action::LibraryManage.role_scope(Role::Admin), Scope::All);
        assert!(!standard_decision.is_allowed());
        assert!(admin_decision.is_allowed());
        assert!(root_decision.is_allowed());
        Ok(())
    }

    #[test]
    fn library_manage_denial_reveals_a_readable_item_and_conceals_an_unreadable_one()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(9, Role::Standard)?;
        let album = sample_album(7, Vec::new())?;
        let hidden = [override_scope(9, Action::LibraryRead, Scope::None)?];

        // Act
        let readable = Authorization::decide(&caller, &[], Action::LibraryManage, &album);
        let unreadable = Authorization::decide(&caller, &hidden, Action::LibraryManage, &album);

        // Assert
        assert!(matches!(readable, Decision::Deny(denial) if !denial.conceals()));
        assert!(matches!(unreadable, Decision::Deny(denial) if denial.conceals()));
        Ok(())
    }

    #[test]
    fn library_manage_listed_scope_covers_exactly_the_named_album() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(9, Role::Admin)?;
        let named = sample_album(7, Vec::new())?;
        let other = sample_album(8, Vec::new())?;
        let permissions = [override_scope(
            9,
            Action::LibraryManage,
            Scope::Listed(vec![ResourceRef::new(ResourceKind::MusicAlbum, 7)]),
        )?];

        // Act
        let named_decision =
            Authorization::decide(&caller, &permissions, Action::LibraryManage, &named);
        let other_decision =
            Authorization::decide(&caller, &permissions, Action::LibraryManage, &other);

        // Assert
        assert!(named_decision.is_allowed());
        assert!(matches!(other_decision, Decision::Deny(denial) if !denial.conceals()));
        Ok(())
    }

    #[test]
    fn file_read_is_derived_and_cannot_be_overridden() {
        // Act & Assert
        assert!(Action::FileRead.is_derived());
        assert!(!Action::FileRead.is_configurable());
        assert_eq!(Action::FileRead.role_scope(Role::Standard), Scope::None);
        assert!(Action::FileRead.resource_kinds().is_empty());
        assert!(
            matches!(
                Permission::try_new(9, Action::FileRead, Scope::Owned),
                Err(PermissionError::NotConfigurable)
            ),
            "a permission override must not be able to name a derived action"
        );
    }

    #[test]
    fn a_listed_file_scope_cannot_be_granted() {
        // Act
        let result = Permission::try_new(
            9,
            Action::FileRead,
            Scope::Listed(vec![ResourceRef::new(ResourceKind::File, 4)]),
        );

        // Assert
        assert!(matches!(result, Err(PermissionError::NotConfigurable)));
    }

    #[test]
    fn file_use_preset_is_owned_for_standard_and_all_for_admin() {
        // Act & Assert
        assert_eq!(Action::FileUse.role_scope(Role::Standard), Scope::Owned);
        assert_eq!(Action::FileUse.role_scope(Role::Admin), Scope::All);
    }

    #[test]
    fn library_art_follows_library_read() -> Result<(), Box<dyn Error>> {
        // Arrange
        let caller = user(9, Role::Standard)?;
        let listed = [override_scope(
            9,
            Action::LibraryRead,
            Scope::Listed(vec![ResourceRef::new(ResourceKind::MusicAlbum, 7)]),
        )?];
        let hidden = [override_scope(9, Action::LibraryRead, Scope::None)?];

        // Act
        let default_decision =
            Authorization::decide_asset(&caller, &[], AssetContext::Library, &Global);
        let listed_decision =
            Authorization::decide_asset(&caller, &listed, AssetContext::Library, &Global);
        let hidden_decision =
            Authorization::decide_asset(&caller, &hidden, AssetContext::Library, &Global);

        // Assert
        assert!(default_decision.is_allowed());
        assert!(
            listed_decision.is_allowed(),
            "shared art stays readable to a caller whose catalogue scope is LISTED"
        );
        assert!(matches!(hidden_decision, Decision::Deny(denial) if denial.conceals()));
        Ok(())
    }

    #[test]
    fn a_private_gallery_asset_follows_gallery_read() -> Result<(), Box<dyn Error>> {
        // Arrange
        let owner = user(3, Role::Standard)?;
        let other = user(9, Role::Standard)?;
        let admin = user(7, Role::Admin)?;
        let root = user(0, Role::Admin)?;
        let private = gallery(2, Some(3), false);
        let context = AssetContext::gallery(2);

        // Act
        let owner_decision = Authorization::decide_asset(&owner, &[], context, &private);
        let other_decision = Authorization::decide_asset(&other, &[], context, &private);
        let admin_decision = Authorization::decide_asset(&admin, &[], context, &private);
        let root_decision = Authorization::decide_asset(&root, &[], context, &private);

        // Assert
        assert!(owner_decision.is_allowed());
        assert!(matches!(other_decision, Decision::Deny(denial) if denial.conceals()));
        assert!(
            !admin_decision.is_allowed(),
            "only the Root Admin short-circuits the delegation"
        );
        assert!(root_decision.is_allowed());
        Ok(())
    }

    #[test]
    fn a_playlist_cover_follows_playlist_read() -> Result<(), Box<dyn Error>> {
        // Arrange
        let owner = user(3, Role::Standard)?;
        let other = user(9, Role::Standard)?;
        let private = playlist(2, 3, false);
        let context = AssetContext::playlist(2);

        // Act
        let owner_decision = Authorization::decide_asset(&owner, &[], context, &private);
        let other_decision = Authorization::decide_asset(&other, &[], context, &private);

        // Assert
        assert!(owner_decision.is_allowed());
        assert!(matches!(other_decision, Decision::Deny(denial) if denial.conceals()));
        Ok(())
    }

    #[test]
    fn an_orphan_asset_is_readable_only_by_its_file_owner_and_the_root_admin()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let owner = user(3, Role::Standard)?;
        let other = user(9, Role::Standard)?;
        let root = user(0, Role::Admin)?;
        let asset = file(4, 3)?;

        // Act
        let owner_decision = Authorization::decide_orphan_asset(&owner, &asset);
        let other_decision = Authorization::decide_orphan_asset(&other, &asset);
        let root_decision = Authorization::decide_orphan_asset(&root, &asset);

        // Assert
        assert!(owner_decision.is_allowed());
        assert!(matches!(other_decision, Decision::Deny(denial) if denial.conceals()));
        assert!(root_decision.is_allowed());
        Ok(())
    }
}
