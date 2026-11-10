use std::future::Future;
use std::pin::Pin;

use crate::domain::NumericID;
use crate::domain::port::ContentLink;

/// Command to fetch one typed audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct GetAudioCommand {
    /// Identifier of the audio to fetch.
    audio_id: NumericID,
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
}

impl GetAudioCommand {
    /// Return the identifier of the audio to fetch.
    #[must_use]
    pub(crate) fn audio_id(&self) -> NumericID {
        self.audio_id
    }

    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Create a new fetch-audio command.
    #[must_use]
    pub(crate) fn new(caller_id: NumericID, audio_id: NumericID) -> Self {
        Self {
            audio_id,
            caller_id,
        }
    }
}

/// Response of a successful audio lookup: one audio and its backing file.
#[non_exhaustive]
pub(crate) struct GetAudioResponse {
    /// Bit depth, absent when the content does not report one.
    bit_depth: Option<i64>,
    /// Block size, absent when the content does not report one.
    block_size: Option<i64>,
    /// Codec of the audio.
    codec: String,
    /// Playback credential scoped to the backing file.
    content_link: ContentLink,
    /// Duration, in milliseconds, absent when the content does not report one.
    duration_ms: Option<f64>,
    /// Identifier of the backing file.
    file_id: NumericID,
    /// Unique identifier of the audio.
    id: NumericID,
    /// Name of the audio.
    name: String,
    /// Sample rate, in hertz, absent when the content does not report one.
    sample_rate_hz: Option<f64>,
}

impl GetAudioResponse {
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

    /// Return the playback credential scoped to the backing file.
    #[must_use]
    pub(crate) fn content_link(&self) -> &ContentLink {
        &self.content_link
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

    /// Return the unique identifier of the audio.
    #[must_use]
    pub(crate) fn id(&self) -> NumericID {
        self.id
    }

    /// Return the name of the audio.
    #[must_use]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Create a new fetch-audio response.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the response's fields one-to-one"
    )]
    pub(crate) fn new(
        id: NumericID,
        name: String,
        duration_ms: Option<f64>,
        codec: String,
        sample_rate_hz: Option<f64>,
        bit_depth: Option<i64>,
        block_size: Option<i64>,
        file_id: NumericID,
        content_link: ContentLink,
    ) -> Self {
        Self {
            bit_depth,
            block_size,
            codec,
            content_link,
            duration_ms,
            file_id,
            id,
            name,
            sample_rate_hz,
        }
    }

    /// Return the sample rate, in hertz, if the content reports one.
    #[must_use]
    pub(crate) fn sample_rate_hz(&self) -> Option<f64> {
        self.sample_rate_hz
    }
}

/// Error returned when fetching a typed audio fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum GetAudioError {
    /// The audio does not exist, or the caller may not read it.
    ///
    /// Both outcomes are reported the same way so a caller cannot probe for
    /// audio it may not read.
    #[error("the audio does not exist")]
    NoSuchAudio,
    /// The caller does not exist.
    #[error("the caller does not exist")]
    NoSuchCaller,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case fetching one typed audio.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait GetAudioUseCase: Send + Sync {
    /// Fetch the requested audio.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn GetAudioUseCase>`.
    fn execute<'future>(
        &'future self,
        command: GetAudioCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetAudioResponse, GetAudioError>> + Send + 'future>>;
}
