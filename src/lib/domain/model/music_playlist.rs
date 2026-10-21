use std::collections::HashMap;

use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;
use crate::domain::model::music_recording::MusicRecording;
use crate::domain::service::name_policy::MAX_NAME_LENGTH;
use crate::domain::service::name_policy::NamePolicy;
use crate::domain::service::name_policy::NamePolicyError;

/// Error returned when initialising or updating a `MusicPlaylist`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MusicPlaylistError {
    /// The requested order is not a complete permutation of the playlist's
    /// current track indices.
    #[error("the requested order is not a valid permutation of the playlist tracks")]
    InvalidOrder,
    /// The name is empty or made only of whitespace.
    #[error("playlist name must not be empty")]
    NameEmpty,
    /// The name is longer than [`MAX_NAME_LENGTH`] characters.
    #[error("playlist name must be at most {MAX_NAME_LENGTH} characters long")]
    NameTooLong,
    /// The recording is already present in the playlist.
    #[error("the recording is already present in the playlist")]
    RecordingAlreadyPresent,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// A music playlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicPlaylist {
    /// Date and time of the creation of the playlist.
    created_at: NaiveDateTime,
    /// Unique identifier of the playlist.
    id: NumericID,
    /// Whether the playlist is visible to every user.
    is_public: bool,
    /// Date and time of the last modification of the playlist.
    last_modified_at: NaiveDateTime,
    /// Name of the playlist.
    name: String,
    /// Tracks of the playlist, in order.
    tracks: Vec<MusicRecording>,
    /// Identifier of the user owning the playlist.
    user_id: NumericID,
}

impl MusicPlaylist {
    /// Return the date and time of the creation of the playlist.
    #[must_use]
    pub fn created_at(&self) -> NaiveDateTime {
        self.created_at
    }

    /// Reject `recording_id` when the playlist already sequences that recording.
    ///
    /// # Errors
    ///
    /// Returns [`MusicPlaylistError::RecordingAlreadyPresent`] when one of the
    /// playlist's tracks references `recording_id`.
    pub fn ensure_recording_absent(
        &self,
        recording_id: NumericID,
    ) -> Result<(), MusicPlaylistError> {
        if self.tracks.iter().any(|track| track.id() == recording_id) {
            return Err(MusicPlaylistError::RecordingAlreadyPresent);
        }
        Ok(())
    }

    /// Return the unique identifier.
    #[must_use]
    pub fn id(&self) -> NumericID {
        self.id
    }

    /// Return whether `user_id` owns the playlist.
    #[must_use]
    pub fn is_owned_by(&self, user_id: NumericID) -> bool {
        self.user_id == user_id
    }

    /// Return whether the playlist is visible to every user.
    #[must_use]
    pub fn is_public(&self) -> bool {
        self.is_public
    }

    /// Return the date and time of the last modification of the playlist.
    #[must_use]
    pub fn last_modified_at(&self) -> NaiveDateTime {
        self.last_modified_at
    }

    /// Return the name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Update whether the playlist is visible to every user.
    pub fn set_is_public(&mut self, is_public: bool) {
        self.is_public = is_public;
    }

    /// Update the name of the playlist.
    ///
    /// # Errors
    ///
    /// Returns [`MusicPlaylistError::NameEmpty`] when `name` is empty or made
    /// only of whitespace, and [`MusicPlaylistError::NameTooLong`] when it is
    /// longer than [`MAX_NAME_LENGTH`] characters.
    pub fn set_name(&mut self, name: String) -> Result<(), MusicPlaylistError> {
        self.name = Self::validate_name(name)?;
        Ok(())
    }

    /// Record a modification of the playlist at `last_modified_at`.
    pub fn touch(&mut self, last_modified_at: NaiveDateTime) {
        self.last_modified_at = last_modified_at;
    }

    /// Return the tracks, in order.
    #[must_use]
    pub fn tracks(&self) -> &[MusicRecording] {
        &self.tracks
    }

    /// Initialise a new `MusicPlaylist`, validating `name`.
    ///
    /// # Errors
    ///
    /// Returns [`MusicPlaylistError::NameEmpty`] when `name` is empty or made
    /// only of whitespace, and [`MusicPlaylistError::NameTooLong`] when it is
    /// longer than [`MAX_NAME_LENGTH`] characters.
    pub fn try_new(
        id: NumericID,
        name: String,
        created_at: NaiveDateTime,
        last_modified_at: NaiveDateTime,
        user_id: NumericID,
        is_public: bool,
        tracks: Vec<MusicRecording>,
    ) -> Result<Self, MusicPlaylistError> {
        let validated_name = Self::validate_name(name)?;
        Ok(Self {
            created_at,
            id,
            is_public,
            last_modified_at,
            name: validated_name,
            tracks,
            user_id,
        })
    }

    /// Return the identifier of the user owning the playlist.
    #[must_use]
    pub fn user_id(&self) -> NumericID {
        self.user_id
    }

    /// Validate and normalize `name`.
    ///
    /// # Errors
    ///
    /// Returns [`MusicPlaylistError::NameEmpty`] when `name` is empty or made
    /// only of whitespace, and [`MusicPlaylistError::NameTooLong`] when it is
    /// longer than [`MAX_NAME_LENGTH`] characters.
    fn validate_name(name: String) -> Result<String, MusicPlaylistError> {
        NamePolicy::validate(name).map_err(|error| match error {
            NamePolicyError::Empty => MusicPlaylistError::NameEmpty,
            NamePolicyError::TooLong => MusicPlaylistError::NameTooLong,
        })
    }

    /// Validate that `order` is a complete permutation of the playlist's
    /// current track indices, and return the new `0..n-1` placement sequence.
    ///
    /// The returned pairs name the new index and the recording that moves to
    /// it.
    ///
    /// # Errors
    ///
    /// Returns [`MusicPlaylistError::InvalidOrder`] when `order` omits a current
    /// index, repeats an index, or names an index the playlist does not hold.
    pub fn validate_order(
        &self,
        order: &[u32],
    ) -> Result<Vec<(u32, NumericID)>, MusicPlaylistError> {
        let current: HashMap<u32, NumericID> = self
            .tracks
            .iter()
            .map(|track| {
                track
                    .index()
                    .playlist_index()
                    .map(|index| (index, track.id()))
                    .ok_or(MusicPlaylistError::InvalidOrder)
            })
            .collect::<Result<_, _>>()?;
        if order.len() != current.len() {
            return Err(MusicPlaylistError::InvalidOrder);
        }

        let mut seen = Vec::with_capacity(order.len());
        let mut reordered = Vec::with_capacity(order.len());
        for (new_index, old_index) in order.iter().enumerate() {
            if seen.contains(old_index) {
                return Err(MusicPlaylistError::InvalidOrder);
            }
            let recording_id = current
                .get(old_index)
                .copied()
                .ok_or(MusicPlaylistError::InvalidOrder)?;
            seen.push(*old_index);
            reordered.push((
                u32::try_from(new_index).map_err(|_| MusicPlaylistError::InvalidOrder)?,
                recording_id,
            ));
        }
        Ok(reordered)
    }
}
