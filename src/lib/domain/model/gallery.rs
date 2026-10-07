use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;

/// Identifier of the seeded, undeletable default gallery.
pub const DEFAULT_GALLERY_ID: NumericID = 0;

/// Maximum number of characters allowed in a gallery name.
pub const MAX_GALLERY_NAME_LENGTH: usize = 100;

/// Error returned when initialising or updating a `Gallery`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GalleryError {
    /// The name is empty or made only of whitespace.
    #[error("gallery name must not be empty")]
    NameEmpty,
    /// The name is longer than [`MAX_GALLERY_NAME_LENGTH`] characters.
    #[error("gallery name must be at most {MAX_GALLERY_NAME_LENGTH} characters long")]
    NameTooLong,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// A gallery: an ordered collection of media items.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::struct_field_names,
    reason = "`gallery_id` is the primary key name mandated by the SQL section and the domain field naming"
)]
pub struct Gallery {
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
    pub fn created_at(&self) -> NaiveDateTime {
        self.created_at
    }

    /// Return the unique identifier of the gallery.
    #[must_use]
    pub fn gallery_id(&self) -> NumericID {
        self.gallery_id
    }

    /// Return whether `user_id` owns the gallery.
    #[must_use]
    pub fn is_owned_by(&self, user_id: NumericID) -> bool {
        self.user_id == Some(user_id)
    }

    /// Return whether any authenticated user may read and moderate the
    /// gallery.
    #[must_use]
    pub fn is_public(&self) -> bool {
        self.is_public
    }

    /// Return whether the gallery is the seeded default gallery.
    #[must_use]
    pub fn is_system(&self) -> bool {
        self.gallery_id == DEFAULT_GALLERY_ID
    }

    /// Return the date and time of the last modification of the gallery.
    #[must_use]
    pub fn last_modified_at(&self) -> NaiveDateTime {
        self.last_modified_at
    }

    /// Return the name of the gallery.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Update the name of the gallery.
    ///
    /// # Errors
    ///
    /// Returns [`GalleryError::NameEmpty`] when `name` is empty or made only of
    /// whitespace, and [`GalleryError::NameTooLong`] when it is longer than
    /// [`MAX_GALLERY_NAME_LENGTH`] characters.
    pub fn set_name(&mut self, name: String) -> Result<(), GalleryError> {
        self.name = Self::validate_name(name)?;
        Ok(())
    }

    /// Record a modification of the gallery at `last_modified_at`.
    pub fn touch(&mut self, last_modified_at: NaiveDateTime) {
        self.last_modified_at = last_modified_at;
    }

    /// Initialise a new `Gallery`, validating `name`.
    ///
    /// # Errors
    ///
    /// Returns [`GalleryError::NameEmpty`] when `name` is empty or made only of
    /// whitespace, and [`GalleryError::NameTooLong`] when it is longer than
    /// [`MAX_GALLERY_NAME_LENGTH`] characters.
    pub fn try_new(
        gallery_id: NumericID,
        user_id: Option<NumericID>,
        name: String,
        is_public: bool,
        created_at: NaiveDateTime,
        last_modified_at: NaiveDateTime,
    ) -> Result<Self, GalleryError> {
        let validated_name = Self::validate_name(name)?;
        Ok(Self {
            created_at,
            gallery_id,
            is_public,
            last_modified_at,
            name: validated_name,
            user_id,
        })
    }

    /// Return the identifier of the owning user, if any.
    #[must_use]
    pub fn user_id(&self) -> Option<NumericID> {
        self.user_id
    }

    /// Validate `name`.
    ///
    /// # Errors
    ///
    /// Returns [`GalleryError::NameEmpty`] when `name` is empty or made only of
    /// whitespace, and [`GalleryError::NameTooLong`] when it is longer than
    /// [`MAX_GALLERY_NAME_LENGTH`] characters.
    fn validate_name(name: String) -> Result<String, GalleryError> {
        if name.trim().is_empty() {
            return Err(GalleryError::NameEmpty);
        }
        if name.chars().count() > MAX_GALLERY_NAME_LENGTH {
            return Err(GalleryError::NameTooLong);
        }
        Ok(name)
    }
}
