use crate::domain::{NonNegativeF64, NumericID};

/// Error returned when initialising or updating an `Audio`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum AudioError {
    /// The `bit_depth` is not positive.
    #[error("bit_depth must be positive")]
    InvalidBitDepth,
    /// The `block_size` is not positive.
    #[error("block_size must be positive")]
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
    #[expect(
        dead_code,
        reason = "the error enum keeps the catch-all variant the rulebook's error shape declares; no caller constructs it yet"
    )]
    Unknown(#[source] anyhow::Error),
}

/// An audio file.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Audio {
    /// Bit depth of the audio, absent when the content does not report one.
    bit_depth: Option<i64>,
    /// Block size of the audio, absent when the content does not report one.
    block_size: Option<i64>,
    /// Codec of the audio.
    codec: String,
    /// Duration of the audio, in milliseconds, absent when the content does not
    /// report one.
    duration_ms: Option<f64>,
    /// Identifier of the backing file.
    file_id: NumericID,
    /// Unique identifier of the audio.
    id: NumericID,
    /// Name of the audio.
    name: String,
    /// Sample rate of the audio, in hertz, absent when the content does not
    /// report one.
    sample_rate_hz: Option<f64>,
}

impl Audio {
    /// Return the bit depth, if the content reports one.
    #[must_use]
    pub(crate) fn bit_depth(&self) -> Option<i64> {
        self.bit_depth
    }

    /// Return the block size, if the content reports one.
    #[must_use]
    pub(crate) fn block_size(&self) -> Option<i64> {
        self.block_size
    }

    /// Return the codec.
    #[must_use]
    pub(crate) fn codec(&self) -> &str {
        &self.codec
    }

    /// Return the duration, in milliseconds, if the content reports one.
    #[must_use]
    pub(crate) fn duration_ms(&self) -> Option<f64> {
        self.duration_ms
    }

    /// Return the identifier of the backing file.
    #[must_use]
    pub(crate) fn file_id(&self) -> NumericID {
        self.file_id
    }

    /// Build an `Audio` from probed metadata, which is valid by construction.
    ///
    /// The probed measurements carry the validated scalar types, so the
    /// conversion has no error branch.
    #[must_use]
    pub(crate) fn from_probed(
        id: NumericID,
        name: String,
        probed: ProbedAudio,
        file_id: NumericID,
    ) -> Self {
        Self {
            bit_depth: probed.bit_depth,
            block_size: probed.block_size,
            codec: probed.codec,
            duration_ms: probed.duration_ms.map(NonNegativeF64::value),
            file_id,
            id,
            name,
            sample_rate_hz: probed.sample_rate_hz.map(NonNegativeF64::value),
        }
    }

    /// Return the unique identifier.
    #[must_use]
    pub(crate) fn id(&self) -> NumericID {
        self.id
    }

    /// Return the name.
    #[must_use]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Return the sample rate, in hertz, if the content reports one.
    #[must_use]
    pub(crate) fn sample_rate_hz(&self) -> Option<f64> {
        self.sample_rate_hz
    }

    /// Update the codec.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::InvalidName`] when `codec` is empty.
    #[expect(
        dead_code,
        reason = "part of the type's declared contract; no caller exercises it yet"
    )]
    fn set_codec(&mut self, codec: String) -> Result<(), AudioError> {
        self.codec = Self::validate_non_empty(codec)?;
        Ok(())
    }

    /// Update the name.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::InvalidName`] when `name` is empty.
    #[expect(
        dead_code,
        reason = "part of the type's declared contract; no caller exercises it yet"
    )]
    pub(crate) fn set_name(&mut self, name: String) -> Result<(), AudioError> {
        self.name = Self::validate_non_empty(name)?;
        Ok(())
    }

    /// Initialise a new `Audio`, validating `name`, `duration_ms`,
    /// `sample_rate_hz`, `bit_depth` and `block_size`.
    ///
    /// An absent `duration_ms`, `sample_rate_hz`, `bit_depth` or `block_size`
    /// is valid: none exists for every codec, and "unknown" is recorded rather
    /// than fabricated.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::InvalidName`] when `name` is empty,
    /// [`AudioError::InvalidDurationMs`] when a present `duration_ms` is
    /// negative, [`AudioError::InvalidSampleRateHz`] when a present
    /// `sample_rate_hz` is negative, and [`AudioError::InvalidBitDepth`] or
    /// [`AudioError::InvalidBlockSize`] when a present value is not positive.
    #[expect(
        clippy::too_many_arguments,
        reason = "try_new mirrors all audio columns"
    )]
    pub(crate) fn try_new(
        id: NumericID,
        name: String,
        duration_ms: Option<f64>,
        codec: String,
        sample_rate_hz: Option<f64>,
        bit_depth: Option<i64>,
        block_size: Option<i64>,
        file_id: NumericID,
    ) -> Result<Self, AudioError> {
        let validated_name = Self::validate_non_empty(name)?;
        let validated_codec = Self::validate_non_empty(codec)?;
        if duration_ms.is_some_and(|value| value < 0.0f64) {
            return Err(AudioError::InvalidDurationMs);
        }
        if sample_rate_hz.is_some_and(|value| value < 0.0f64) {
            return Err(AudioError::InvalidSampleRateHz);
        }
        if let Some(value) = bit_depth {
            Self::validate_positive(value, AudioError::InvalidBitDepth)?;
        }
        if let Some(value) = block_size {
            Self::validate_positive(value, AudioError::InvalidBlockSize)?;
        }
        Ok(Self {
            bit_depth,
            block_size,
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

    /// Validate `value` is positive.
    ///
    /// # Errors
    ///
    /// Returns `err` when `value` is not positive.
    fn validate_positive(value: i64, err: AudioError) -> Result<(), AudioError> {
        if value > 0 { Ok(()) } else { Err(err) }
    }
}

/// Metadata extracted from an audio file.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub(crate) struct ProbedAudio {
    /// Bit depth of the audio, absent when the content does not report one.
    pub bit_depth: Option<i64>,
    /// Block size of the audio, absent when the content does not report one.
    pub block_size: Option<i64>,
    /// Codec of the audio.
    pub codec: String,
    /// Duration of the audio, in milliseconds, absent when the content does not
    /// report one.
    pub duration_ms: Option<NonNegativeF64>,
    /// Sample rate of the audio, in hertz, absent when the content does not
    /// report one.
    pub sample_rate_hz: Option<NonNegativeF64>,
}
