use crate::domain::NumericID;

/// Required length of `MusicRecording::isrc_code`.
const ISRC_CODE_LENGTH: usize = 12;

/// Error returned when initialising or updating a `MusicRecording`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum MusicRecordingError {
    /// The `isrc_code` is not exactly [`ISRC_CODE_LENGTH`] characters long.
    #[error("isrc_code must be exactly {ISRC_CODE_LENGTH} characters long")]
    InvalidIsrcCode,
    /// The `name` is empty.
    #[error("name must not be empty")]
    InvalidName,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    #[expect(
        dead_code,
        reason = "the error enum keeps the catch-all variant the rulebook's error shape declares; no caller constructs it yet"
    )]
    Unknown(#[source] anyhow::Error),
}

/// Placement of a recording within the container that sequences it.
///
/// A recording is one entity; a track is the placement of that recording in a
/// medium or a playlist. The index is the placement's address within its
/// container, so two occurrences of one recording are distinct values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum TrackIndex {
    /// The recording is placed in an album medium, at this index.
    MediumIndex(u8),
    /// The recording is placed in a playlist, at this index.
    PlaylistIndex(u32),
    /// The recording holds no placement.
    Unplaced,
}

impl TrackIndex {
    /// Return the medium index, when the placement is a medium placement.
    #[must_use]
    pub(crate) fn medium_index(self) -> Option<u8> {
        match self {
            Self::MediumIndex(index) => Some(index),
            Self::PlaylistIndex(_) | Self::Unplaced => None,
        }
    }

    /// Return the playlist index, when the placement is a playlist placement.
    #[must_use]
    pub(crate) fn playlist_index(self) -> Option<u32> {
        match self {
            Self::PlaylistIndex(index) => Some(index),
            Self::MediumIndex(_) | Self::Unplaced => None,
        }
    }
}

/// A music recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MusicRecording {
    /// Identifier of the audio backing the recording.
    audio_id: NumericID,
    /// Identifier of the file the backing audio references.
    file_id: NumericID,
    /// Unique identifier of the recording.
    id: NumericID,
    /// Placement of the recording within its container.
    index: TrackIndex,
    /// ISRC code of the recording, when set.
    isrc_code: Option<String>,
    /// Identifiers of the music groups associated with the recording, when set.
    music_group_ids: Option<Vec<NumericID>>,
    /// Name of the recording, when set.
    name: Option<String>,
}

impl MusicRecording {
    /// Return the identifier of the backing audio.
    #[must_use]
    pub(crate) fn audio_id(&self) -> NumericID {
        self.audio_id
    }

    /// Return the identifier of the file the backing audio references.
    #[must_use]
    pub(crate) fn file_id(&self) -> NumericID {
        self.file_id
    }

    /// Return the unique identifier.
    #[must_use]
    pub(crate) fn id(&self) -> NumericID {
        self.id
    }

    /// Return the placement of the recording within its container.
    #[must_use]
    pub(crate) fn index(&self) -> TrackIndex {
        self.index
    }

    /// Return the ISRC code, if any.
    #[must_use]
    pub(crate) fn isrc_code(&self) -> Option<&str> {
        self.isrc_code.as_deref()
    }

    /// Return the identifiers of the associated music groups, if any.
    #[must_use]
    pub(crate) fn music_group_ids(&self) -> Option<&[NumericID]> {
        self.music_group_ids.as_deref()
    }

    /// Return the name, if any.
    #[must_use]
    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Update the `isrc_code`.
    ///
    /// # Errors
    ///
    /// Returns [`MusicRecordingError::InvalidIsrcCode`] when `isrc_code` is
    /// not exactly [`ISRC_CODE_LENGTH`] characters long.
    pub(crate) fn set_isrc_code(
        &mut self,
        isrc_code: Option<String>,
    ) -> Result<(), MusicRecordingError> {
        self.isrc_code = Self::validate_isrc_code(isrc_code)?;
        Ok(())
    }

    /// Update the name.
    ///
    /// # Errors
    ///
    /// Returns [`MusicRecordingError::InvalidName`] when `name` is empty.
    pub(crate) fn set_name(&mut self, name: Option<String>) -> Result<(), MusicRecordingError> {
        self.name = Self::validate_name(name)?;
        Ok(())
    }

    /// Initialise a new `MusicRecording`, validating `name` and `isrc_code`.
    ///
    /// # Errors
    ///
    /// Returns [`MusicRecordingError::InvalidName`] when `name` is empty, and
    /// [`MusicRecordingError::InvalidIsrcCode`] when `isrc_code` is not exactly
    /// [`ISRC_CODE_LENGTH`] characters long.
    pub(crate) fn try_new(
        id: NumericID,
        name: Option<String>,
        isrc_code: Option<String>,
        audio_id: NumericID,
        file_id: NumericID,
        music_group_ids: Option<Vec<NumericID>>,
        index: TrackIndex,
    ) -> Result<Self, MusicRecordingError> {
        let validated_name = Self::validate_name(name)?;
        let validated_isrc_code = Self::validate_isrc_code(isrc_code)?;
        let normalized_music_group_ids = match music_group_ids {
            Some(ids) if ids.is_empty() => None,
            ids => ids,
        };
        Ok(Self {
            audio_id,
            file_id,
            id,
            index,
            isrc_code: validated_isrc_code,
            music_group_ids: normalized_music_group_ids,
            name: validated_name,
        })
    }

    /// Validate `isrc_code`.
    ///
    /// # Errors
    ///
    /// Returns [`MusicRecordingError::InvalidIsrcCode`] when `isrc_code` is
    /// not exactly [`ISRC_CODE_LENGTH`] characters long.
    fn validate_isrc_code(
        isrc_code: Option<String>,
    ) -> Result<Option<String>, MusicRecordingError> {
        match isrc_code {
            None => Ok(None),
            Some(raw) => {
                if raw.chars().count() == ISRC_CODE_LENGTH {
                    Ok(Some(raw))
                } else {
                    Err(MusicRecordingError::InvalidIsrcCode)
                }
            }
        }
    }

    /// Validate `name`.
    ///
    /// # Errors
    ///
    /// Returns [`MusicRecordingError::InvalidName`] when `name` is empty.
    fn validate_name(name: Option<String>) -> Result<Option<String>, MusicRecordingError> {
        match name {
            None => Ok(None),
            Some(raw) => {
                if raw.is_empty() {
                    Err(MusicRecordingError::InvalidName)
                } else {
                    Ok(Some(raw))
                }
            }
        }
    }
}
