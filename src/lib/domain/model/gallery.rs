use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;
use crate::domain::model::permission::{Protected, ResourceKind, ResourceRef};

/// Identifier of the seeded, undeletable default gallery.
const DEFAULT_GALLERY_ID: NumericID = 0;

/// A gallery: an ordered collection of media items.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::struct_field_names,
    reason = "`gallery_id` is the primary key name mandated by the SQL section and the domain field naming"
)]
pub(crate) struct Gallery {
    /// Date and time at which the gallery was created.
    created_at: NaiveDateTime,
    /// Unique identifier of the gallery.
    gallery_id: NumericID,
    /// Whether any authenticated user may read and moderate the gallery.
    is_public: bool,
    /// Date and time of the last modification of the gallery.
    last_modified_at: NaiveDateTime,
    /// Name of the gallery.
    name: String,
    /// Identifier of the owning user, absent for the system gallery.
    user_id: Option<NumericID>,
}

impl Gallery {
    /// Return the date and time at which the gallery was created.
    #[must_use]
    pub(crate) fn created_at(&self) -> NaiveDateTime {
        self.created_at
    }

    /// Return the unique identifier of the gallery.
    #[must_use]
    pub(crate) fn gallery_id(&self) -> NumericID {
        self.gallery_id
    }

    /// Return whether `user_id` owns the gallery.
    #[must_use]
    #[expect(
        dead_code,
        reason = "part of the type's declared contract; no caller exercises it yet"
    )]
    pub(crate) fn is_owned_by(&self, user_id: NumericID) -> bool {
        self.user_id == Some(user_id)
    }

    /// Return whether any authenticated user may read and moderate the
    /// gallery.
    #[must_use]
    pub(crate) fn is_public(&self) -> bool {
        self.is_public
    }

    /// Return whether the gallery is the seeded default gallery.
    #[must_use]
    pub(crate) fn is_system(&self) -> bool {
        self.gallery_id == DEFAULT_GALLERY_ID
    }

    /// Return the date and time of the last modification of the gallery.
    #[must_use]
    pub(crate) fn last_modified_at(&self) -> NaiveDateTime {
        self.last_modified_at
    }

    /// Return the name of the gallery.
    #[must_use]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Initialise a new `Gallery`.
    ///
    /// The caller validates the name with the domain name policy before
    /// invoking this constructor (`ARCH-005`).
    #[must_use]
    pub(crate) fn new(
        gallery_id: NumericID,
        user_id: Option<NumericID>,
        name: String,
        is_public: bool,
        created_at: NaiveDateTime,
        last_modified_at: NaiveDateTime,
    ) -> Self {
        Self {
            created_at,
            gallery_id,
            is_public,
            last_modified_at,
            name,
            user_id,
        }
    }

    /// Update the name of the gallery.
    ///
    /// The caller validates the name with the domain name policy before
    /// invoking this method (`ARCH-005`).
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "part of the type's declared contract; no caller exercises it yet"
        )
    )]
    pub(crate) fn set_name(&mut self, name: String) {
        self.name = name;
    }

    /// Record a modification of the gallery at `last_modified_at`.
    pub(crate) fn touch(&mut self, last_modified_at: NaiveDateTime) {
        self.last_modified_at = last_modified_at;
    }

    /// Return the identifier of the owning user, if any.
    #[must_use]
    pub(crate) fn user_id(&self) -> Option<NumericID> {
        self.user_id
    }
}

impl Protected for Gallery {
    fn is_privileged_account(&self) -> bool {
        false
    }

    fn is_publicly_visible(&self) -> bool {
        self.is_public()
    }

    fn owner_id(&self) -> Option<NumericID> {
        self.user_id
    }

    fn resource_ref(&self) -> Option<ResourceRef> {
        Some(ResourceRef::new(ResourceKind::Gallery, self.gallery_id))
    }
}
