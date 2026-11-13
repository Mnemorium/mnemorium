use crate::domain::NumericID;

/// Error returned when initialising a `MusicGroup`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum MusicGroupError {
    /// The `stage_name` is empty.
    #[error("stage_name must not be empty")]
    InvalidStageName,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    #[expect(
        dead_code,
        reason = "the error enum keeps the catch-all variant the rulebook's error shape declares; no caller constructs it yet"
    )]
    Unknown(#[source] anyhow::Error),
}

/// A music group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MusicGroup {
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
    pub(crate) fn cover_image_id(&self) -> Option<NumericID> {
        self.cover_image_id
    }

    /// Return the unique identifier.
    #[must_use]
    pub(crate) fn id(&self) -> NumericID {
        self.id
    }

    /// Return the stage name.
    #[must_use]
    pub(crate) fn stage_name(&self) -> &str {
        &self.stage_name
    }

    /// Initialise a new `MusicGroup`, validating `stage_name`.
    ///
    /// # Errors
    ///
    /// Returns [`MusicGroupError::InvalidStageName`] when `stage_name` is
    /// empty.
    pub(crate) fn try_new(
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
