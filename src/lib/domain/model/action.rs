/// One authorizable operation from the fixed catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub(crate) enum Action {
    /// Change a user's role.
    ChangeRole,
    /// Create a child of the addressed creation surface or resource.
    CreateChild,
    /// Remove the resource.
    Delete,
    /// Report that a resource exists and include it in a listing.
    Discover,
    /// Retrieve the original bytes of a resource.
    Download,
    /// Change the metadata of a resource.
    EditMetadata,
    /// Change the visibility or the grant set of a resource.
    ManagePermissions,
    /// Replace a user's credential.
    ReplaceCredential,
    /// Replace the binary content of a resource.
    ReplaceMedia,
    /// Add or revoke a read grant on a resource.
    Share,
    /// Play the audio or video bytes of a resource.
    Stream,
    /// Read the rendered content of a resource.
    View,
    /// Read the metadata of a resource.
    ViewMetadata,
}

impl Action {
    /// Render the action as its stored representation (`view_metadata`).
    #[must_use]
    pub(crate) fn as_wire(self) -> &'static str {
        match self {
            Self::ChangeRole => "change_role",
            Self::CreateChild => "create_child",
            Self::Delete => "delete",
            Self::Discover => "discover",
            Self::Download => "download",
            Self::EditMetadata => "edit_metadata",
            Self::ManagePermissions => "manage_permissions",
            Self::ReplaceMedia => "replace_media",
            Self::ReplaceCredential => "replace_credential",
            Self::Share => "share",
            Self::Stream => "stream",
            Self::View => "view",
            Self::ViewMetadata => "view_metadata",
        }
    }

    /// Parse an action from its stored representation (`view_metadata`).
    #[must_use]
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        Some(match value {
            "change_role" => Self::ChangeRole,
            "create_child" => Self::CreateChild,
            "delete" => Self::Delete,
            "discover" => Self::Discover,
            "download" => Self::Download,
            "edit_metadata" => Self::EditMetadata,
            "manage_permissions" => Self::ManagePermissions,
            "replace_media" => Self::ReplaceMedia,
            "replace_credential" => Self::ReplaceCredential,
            "share" => Self::Share,
            "stream" => Self::Stream,
            "view" => Self::View,
            "view_metadata" => Self::ViewMetadata,
            _ => return None,
        })
    }

    /// Return whether the action observes a resource without changing it.
    #[must_use]
    pub(crate) fn is_read(self) -> bool {
        matches!(
            self,
            Self::Discover | Self::ViewMetadata | Self::View | Self::Stream | Self::Download
        )
    }
}
