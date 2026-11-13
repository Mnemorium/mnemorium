use super::user::Role;
use crate::domain::NumericID;

/// The subject a grant applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub(crate) enum Principal {
    /// Every user holding a built-in role.
    Role(Role),
    /// The root, which holds no stored grant.
    Root,
    /// One user account.
    User(NumericID),
}

impl Principal {
    /// Return whether the principal is the root.
    #[must_use]
    pub(crate) fn is_root(self) -> bool {
        self == Self::Root
    }
}
