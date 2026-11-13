use super::action::Action;

/// The contributor bundle.
const CONTRIBUTOR: [Action; 5] = [
    Action::Discover,
    Action::ViewMetadata,
    Action::View,
    Action::Stream,
    Action::CreateChild,
];

/// The downloader bundle.
const DOWNLOADER: [Action; 5] = [
    Action::Discover,
    Action::ViewMetadata,
    Action::View,
    Action::Stream,
    Action::Download,
];

/// The editor bundle.
const EDITOR: [Action; 7] = [
    Action::Discover,
    Action::ViewMetadata,
    Action::View,
    Action::Stream,
    Action::CreateChild,
    Action::EditMetadata,
    Action::ReplaceMedia,
];

/// The manager bundle.
const MANAGER: [Action; 9] = [
    Action::Discover,
    Action::ViewMetadata,
    Action::View,
    Action::Stream,
    Action::CreateChild,
    Action::EditMetadata,
    Action::ReplaceMedia,
    Action::Share,
    Action::Delete,
];

/// The owner bundle.
const OWNER: [Action; 10] = [
    Action::Discover,
    Action::ViewMetadata,
    Action::View,
    Action::Stream,
    Action::CreateChild,
    Action::EditMetadata,
    Action::ReplaceMedia,
    Action::Share,
    Action::Delete,
    Action::ManagePermissions,
];

/// The viewer bundle.
const VIEWER: [Action; 4] = [
    Action::Discover,
    Action::ViewMetadata,
    Action::View,
    Action::Stream,
];

/// One bundle of actions granted on a resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub(crate) enum ResourceRole {
    /// `Viewer` plus `CreateChild`.
    Contributor,
    /// `Viewer` plus `Download`.
    Downloader,
    /// `Contributor` plus `EditMetadata` and `ReplaceMedia`.
    Editor,
    /// `Editor` plus `Share` and `Delete`.
    Manager,
    /// `Manager` plus `ManagePermissions`; implicit from ownership.
    Owner,
    /// Reads: `Discover`, `ViewMetadata`, `View` and `Stream`.
    Viewer,
}

impl ResourceRole {
    /// Return the actions the bundle contains.
    #[must_use]
    pub(crate) fn actions(self) -> &'static [Action] {
        match self {
            Self::Viewer => &VIEWER,
            Self::Downloader => &DOWNLOADER,
            Self::Contributor => &CONTRIBUTOR,
            Self::Editor => &EDITOR,
            Self::Manager => &MANAGER,
            Self::Owner => &OWNER,
        }
    }

    /// Render the role as its stored representation (`VIEWER`).
    #[must_use]
    pub(crate) fn as_wire(self) -> &'static str {
        match self {
            Self::Viewer => "VIEWER",
            Self::Downloader => "DOWNLOADER",
            Self::Contributor => "CONTRIBUTOR",
            Self::Editor => "EDITOR",
            Self::Manager => "MANAGER",
            Self::Owner => "OWNER",
        }
    }

    /// Parse a role from its stored representation (`VIEWER`).
    #[must_use]
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        match value {
            "VIEWER" => Some(Self::Viewer),
            "DOWNLOADER" => Some(Self::Downloader),
            "CONTRIBUTOR" => Some(Self::Contributor),
            "EDITOR" => Some(Self::Editor),
            "MANAGER" => Some(Self::Manager),
            "OWNER" => Some(Self::Owner),
            _ => None,
        }
    }

    /// Return whether the bundle contains `action`.
    #[must_use]
    pub(crate) fn includes(self, action: Action) -> bool {
        self.actions().contains(&action)
    }

    /// Return whether the role may be granted to a user principal.
    ///
    /// `Owner` is implicit from ownership and is never a user grant; a seeded
    /// role grant may still carry it.
    #[must_use]
    pub(crate) fn is_grantable_by_user(self) -> bool {
        self != Self::Owner
    }
}
