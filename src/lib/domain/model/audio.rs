use crate::domain::alias::NumericID;
use crate::domain::model::audio_channel::AudioChannel;

/// Error returned when initialising or updating an `Audio`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AudioError {
    /// The `bit_depth` is not a positive power of two.
    #[error("bit_depth must be a positive power of two")]
    InvalidBitDepth,
    /// The `block_size` is not a positive power of two.
    #[error("block_size must be a positive power of two")]
    InvalidBlockSize,
    /// The `duration_ms` is negative.
    #[error("duration_ms must not be negative")]
    InvalidDurationMs,
    /// The `name` is empty.
    #[error("name must not be empty")]
    InvalidName,
    /// The `sample_rate_hz` is negative.
    #[error("sample_rate_hz must not be negative")]
    InvalidSampleRateHz,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// An audio file.
#[derive(Debug, Clone, PartialEq)]
pub struct Audio {
    /// Bit depth of the audio.
    bit_depth: i64,
    /// Block size of the audio.
    block_size: i64,
    /// Channel layout of the audio.
    channel: AudioChannel,
    /// Codec of the audio.
    codec: String,
    /// Duration of the audio, in milliseconds.
    duration_ms: f64,
    /// Identifier of the backing file.
    file_id: NumericID,
    /// Unique identifier of the audio.
    id: NumericID,
    /// Name of the audio.
    name: String,
    /// Sample rate of the audio, in hertz.
    sample_rate_hz: f64,
}

impl Audio {
    /// Return the bit depth.
    #[must_use]
    pub fn bit_depth(&self) -> i64 {
        self.bit_depth
    }

    /// Return the block size.
    #[must_use]
    pub fn block_size(&self) -> i64 {
        self.block_size
    }

    /// Return the channel layout.
    #[must_use]
    pub fn channel(&self) -> &AudioChannel {
        &self.channel
    }

    /// Return the codec.
    #[must_use]
    pub fn codec(&self) -> &str {
        &self.codec
    }

    /// Return the duration, in milliseconds.
    #[must_use]
    pub fn duration_ms(&self) -> f64 {
        self.duration_ms
    }

    /// Return the identifier of the backing file.
    #[must_use]
    pub fn file_id(&self) -> NumericID {
        self.file_id
    }

    /// Return the unique identifier.
    #[must_use]
    pub fn id(&self) -> NumericID {
        self.id
    }

    /// Return the name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the sample rate, in hertz.
    #[must_use]
    pub fn sample_rate_hz(&self) -> f64 {
        self.sample_rate_hz
    }

    /// Update the codec.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::InvalidName`] when `codec` is empty.
    pub fn set_codec(&mut self, codec: String) -> Result<(), AudioError> {
        self.codec = Self::validate_non_empty(codec)?;
        Ok(())
    }

    /// Update the name.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::InvalidName`] when `name` is empty.
    pub fn set_name(&mut self, name: String) -> Result<(), AudioError> {
        self.name = Self::validate_non_empty(name)?;
        Ok(())
    }

    /// Initialise a new `Audio`, validating `name`, `duration_ms`,
    /// `sample_rate_hz`, `bit_depth` and `block_size`.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::InvalidName`] when `name` is empty,
    /// [`AudioError::InvalidDurationMs`] when `duration_ms` is negative,
    /// [`AudioError::InvalidSampleRateHz`] when `sample_rate_hz` is negative,
    /// and [`AudioError::InvalidBitDepth`] or [`AudioError::InvalidBlockSize`]
    /// when the corresponding value is not a positive power of two.
    #[expect(
        clippy::too_many_arguments,
        reason = "try_new mirrors all audio columns"
    )]
    pub fn try_new(
        id: NumericID,
        name: String,
        duration_ms: f64,
        codec: String,
        sample_rate_hz: f64,
        channel: AudioChannel,
        bit_depth: i64,
        block_size: i64,
        file_id: NumericID,
    ) -> Result<Self, AudioError> {
        let validated_name = Self::validate_non_empty(name)?;
        let validated_codec = Self::validate_non_empty(codec)?;
        if duration_ms < 0.0f64 {
            return Err(AudioError::InvalidDurationMs);
        }
        if sample_rate_hz < 0.0f64 {
            return Err(AudioError::InvalidSampleRateHz);
        }
        Self::validate_power_of_two(bit_depth, AudioError::InvalidBitDepth)?;
        Self::validate_power_of_two(block_size, AudioError::InvalidBlockSize)?;
        Ok(Self {
            bit_depth,
            block_size,
            channel,
            codec: validated_codec,
            duration_ms,
            file_id,
            id,
            name: validated_name,
            sample_rate_hz,
        })
    }

    /// Validate `value` is non-empty.
    ///
    /// # Errors
    ///
    /// Returns `err` when `value` is empty.
    fn validate_non_empty(value: String) -> Result<String, AudioError> {
        if value.is_empty() {
            Err(AudioError::InvalidName)
        } else {
            Ok(value)
        }
    }

    /// Validate `value` is a positive power of two.
    ///
    /// # Errors
    ///
    /// Returns `err` when `value` is not a positive power of two.
    fn validate_power_of_two(value: i64, err: AudioError) -> Result<(), AudioError> {
        if value > 0 && value.unsigned_abs().is_power_of_two() {
            Ok(())
        } else {
            Err(err)
        }
    }
}
