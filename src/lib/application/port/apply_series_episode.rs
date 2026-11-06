use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;
use crate::domain::model::episode::Episode;
use crate::domain::model::external_ref::ExternalRef;

/// Command to apply external metadata to one episode of a show.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ApplySeriesEpisodeCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Season-scoped index of the episode to write.
    episode_index: u32,
    /// Reference to the show at the provider.
    reference: ExternalRef,
    /// Series-scoped index of the season holding the episode.
    season_index: u32,
    /// Identifier of the show holding the season.
    series_id: NumericID,
}

impl ApplySeriesEpisodeCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the season-scoped index of the episode to write.
    #[must_use]
    pub(crate) fn episode_index(&self) -> u32 {
        self.episode_index
    }

    /// Create a new apply-episode command.
    #[must_use]
    pub(crate) fn new(
        caller_id: NumericID,
        series_id: NumericID,
        season_index: u32,
        episode_index: u32,
        reference: ExternalRef,
    ) -> Self {
        Self {
            caller_id,
            episode_index,
            reference,
            season_index,
            series_id,
        }
    }

    /// Return the reference to the show at the provider.
    #[must_use]
    pub(crate) fn reference(&self) -> &ExternalRef {
        &self.reference
    }

    /// Return the series-scoped index of the season holding the episode.
    #[must_use]
    pub(crate) fn season_index(&self) -> u32 {
        self.season_index
    }

    /// Return the identifier of the show holding the season.
    #[must_use]
    pub(crate) fn series_id(&self) -> NumericID {
        self.series_id
    }
}

/// Response of a successful episode apply: the stored episode.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub(crate) struct ApplySeriesEpisodeResponse {
    /// The episode that was stored.
    episode: Episode,
    /// Season-scoped index of the episode.
    episode_index: u32,
    /// Series-scoped index of the season holding the episode.
    season_index: u32,
    /// Identifier of the show holding the season.
    series_id: NumericID,
}

impl ApplySeriesEpisodeResponse {
    /// Return the episode that was stored.
    #[must_use]
    pub(crate) fn episode(&self) -> &Episode {
        &self.episode
    }

    /// Create a new applied-episode response.
    #[must_use]
    pub(crate) fn new(
        series_id: NumericID,
        season_index: u32,
        episode_index: u32,
        episode: Episode,
    ) -> Self {
        Self {
            episode,
            episode_index,
            season_index,
            series_id,
        }
    }

    /// Return the series-scoped index of the season holding the episode.
    #[must_use]
    pub(crate) fn season_index(&self) -> u32 {
        self.season_index
    }

    /// Return the identifier of the show holding the season.
    #[must_use]
    pub(crate) fn series_id(&self) -> NumericID {
        self.series_id
    }
}

/// Error returned when applying external metadata to an episode.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum ApplySeriesEpisodeError {
    /// The write conflicts with the stored catalogue.
    #[error("the applied metadata conflicts with the stored catalogue")]
    Conflict,
    /// The caller may not manage the show.
    #[error("the caller may not manage the library")]
    Forbidden,
    /// The provider reports a work the episode apply cannot write.
    #[error("the provider reports a work that cannot be applied to the episode")]
    InvalidReference,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// The show holds no episode at the requested position.
    #[error("an episode with this index does not exist in the season")]
    NoSuchEpisode,
    /// The show holds no season at the requested index.
    #[error("a season with this index does not exist in the show")]
    NoSuchSeason,
    /// No show matches the requested identifier.
    #[error("a show with this identifier does not exist")]
    NoSuchSeries,
    /// The provider reports another episode position than the one addressed.
    #[error("the provider reports another episode position")]
    PositionMismatch,
    /// The provider failed while resolving the reference.
    #[error("the video metadata provider failed")]
    ProviderFailure,
    /// No video metadata provider is configured.
    #[error("no video metadata provider is configured")]
    ProviderUnavailable,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
    /// The provider does not know the reference.
    #[error("the video metadata provider does not know the reference")]
    UnknownReference,
}

/// Use case for applying external metadata to one episode.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait ApplySeriesEpisodeUseCase: Send + Sync {
    /// Resolve the reference and write the addressed episode.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as
    /// `Arc<dyn ApplySeriesEpisodeUseCase>`.
    fn execute<'future>(
        &'future self,
        command: ApplySeriesEpisodeCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ApplySeriesEpisodeResponse, ApplySeriesEpisodeError>>
                + Send
                + 'future,
        >,
    >;
}
