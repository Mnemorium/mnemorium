/// Error returned when initialising an `AudioChannel`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AudioChannelError {
    /// The name is empty.
    #[error("name must not be empty")]
    InvalidName,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// An audio channel value object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioChannel {
    /// Human-readable description of the layout.
    description: String,
    /// Name of the audio channel layout.
    name: String,
    /// Number of channels in the layout.
    nb_channel: i64,
}

impl AudioChannel {
    /// Return the description.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Return the name of the audio channel layout.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the number of channels.
    #[must_use]
    pub fn nb_channel(&self) -> i64 {
        self.nb_channel
    }

    /// Initialise a new `AudioChannel`, validating `name` is non-empty.
    ///
    /// # Errors
    ///
    /// Returns [`AudioChannelError::InvalidName`] when `name` is empty.
    pub fn try_new(
        name: String,
        nb_channel: i64,
        description: String,
    ) -> Result<Self, AudioChannelError> {
        if name.is_empty() {
            return Err(AudioChannelError::InvalidName);
        }
        Ok(Self {
            description,
            name,
            nb_channel,
        })
    }
}
