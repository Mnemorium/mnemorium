use std::collections::HashMap;

use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;
use crate::domain::model::music_recording::MusicRecording;
use crate::domain::model::permission::Protected;
use crate::domain::model::permission::ResourceKind;
use crate::domain::model::permission::ResourceRef;

/// Error returned when initialising or updating a `MusicPlaylist`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MusicPlaylistError {
    /// The requested order is not a complete permutation of the playlist's
    /// current track indices.
    #[error("the requested order is not a valid permutation of the playlist tracks")]
    InvalidOrder,
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
    /// Identifier of the file holding the playlist's artwork, when set.
    cover_file_id: Option<NumericID>,
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
    /// Return the identifier of the file holding the playlist's artwork, if any.
    #[must_use]
    pub fn cover_file_id(&self) -> Option<NumericID> {
        self.cover_file_id
    }

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

    /// Initialise a new `MusicPlaylist`.
    ///
    /// The caller validates the name with the domain name policy before
    /// invoking this constructor (`ARCH-005`).
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the playlist's fields one-to-one"
    )]
    pub fn new(
        id: NumericID,
        name: String,
        created_at: NaiveDateTime,
        last_modified_at: NaiveDateTime,
        user_id: NumericID,
        is_public: bool,
        cover_file_id: Option<NumericID>,
        tracks: Vec<MusicRecording>,
    ) -> Self {
        Self {
            cover_file_id,
            created_at,
            id,
            is_public,
            last_modified_at,
            name,
            tracks,
            user_id,
        }
    }

    /// Update whether the playlist is visible to every user.
    pub fn set_is_public(&mut self, is_public: bool) {
        self.is_public = is_public;
    }

    /// Update the name of the playlist.
    ///
    /// The caller validates the name with the domain name policy before
    /// invoking this method (`ARCH-005`).
    pub fn set_name(&mut self, name: String) {
        self.name = name;
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

    /// Return the identifier of the user owning the playlist.
    #[must_use]
    pub fn user_id(&self) -> NumericID {
        self.user_id
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

impl Protected for MusicPlaylist {
    fn is_privileged_account(&self) -> bool {
        false
    }

    fn is_publicly_visible(&self) -> bool {
        self.is_public()
    }

    fn owner_id(&self) -> Option<NumericID> {
        Some(self.user_id)
    }

    fn resource_ref(&self) -> Option<ResourceRef> {
        Some(ResourceRef::new(ResourceKind::MusicPlaylist, self.id))
    }
}
