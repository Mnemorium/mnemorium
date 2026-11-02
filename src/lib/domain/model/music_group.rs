use crate::domain::alias::NumericID;

/// Error returned when initialising a `MusicGroup`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MusicGroupError {
    /// The `stage_name` is empty.
    #[error("stage_name must not be empty")]
    InvalidStageName,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// A music group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicGroup {
    /// Identifier of the image holding the group's artwork, when set.
    cover_image_id: Option<NumericID>,
    /// Unique identifier of the group.
    id: NumericID,
    /// Stage name of the group.
    stage_name: String,
}

impl MusicGroup {
    /// Return the identifier of the image holding the group's artwork, if any.
    #[must_use]
    pub fn cover_image_id(&self) -> Option<NumericID> {
        self.cover_image_id
    }

    /// Return the unique identifier.
    #[must_use]
    pub fn id(&self) -> NumericID {
        self.id
    }

    /// Return the stage name.
    #[must_use]
    pub fn stage_name(&self) -> &str {
        &self.stage_name
    }

    /// Initialise a new `MusicGroup`, validating `stage_name`.
    ///
    /// # Errors
    ///
    /// Returns [`MusicGroupError::InvalidStageName`] when `stage_name` is
    /// empty.
    pub fn try_new(
        id: NumericID,
        stage_name: String,
        cover_image_id: Option<NumericID>,
    ) -> Result<Self, MusicGroupError> {
        if stage_name.is_empty() {
            return Err(MusicGroupError::InvalidStageName);
        }
        Ok(Self {
            cover_image_id,
            id,
            stage_name,
        })
    }
}
