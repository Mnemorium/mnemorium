use chrono::NaiveDate;

use crate::domain::model::artwork::Artwork;
use crate::domain::model::genre::Genre;

/// Metadata a provider reports for a film, a show or an episode.
///
/// The shape does not depend on the provider that answered. Absence is `None`
/// or an empty list: a date, a country or a genre the provider does not report
/// is absent rather than fabricated, and no provider URL, score, popularity,
/// runtime or language travels here because the catalogue has no place for
/// them.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MediaMetadata {
    /// One episode of a show.
    Episode {
        /// Date the episode first aired, when the provider reports one.
        air_date: Option<NaiveDate>,
        /// Artwork the provider reports, when the download succeeds.
        artwork: Option<Artwork>,
        /// Index of the episode within its season.
        episode: u32,
        /// Plot summary the provider reports.
        plot: String,
        /// Index of the season within the show.
        season: u32,
        /// Title the provider reports.
        title: String,
    },
    /// One film.
    Movie {
        /// Artwork the provider reports, when the download succeeds.
        artwork: Option<Artwork>,
        /// Country of origin as an uppercase ISO 3166-1 alpha-2 code, when the
        /// provider reports a mappable one.
        country_of_origin: Option<String>,
        /// Genres from the shared vocabulary, ordered canonically.
        genres: Vec<Genre>,
        /// Plot summary the provider reports.
        plot: String,
        /// Date the film was released, when the provider reports one.
        release_date: Option<NaiveDate>,
        /// Title the provider reports.
        title: String,
    },
    /// One show.
    Series {
        /// Artwork the provider reports, when the download succeeds.
        artwork: Option<Artwork>,
        /// Country of origin as an uppercase ISO 3166-1 alpha-2 code, when the
        /// provider reports a mappable one.
        country_of_origin: Option<String>,
        /// Date the show first aired, when the provider reports one.
        first_air_date: Option<NaiveDate>,
        /// Genres from the shared vocabulary, ordered canonically.
        genres: Vec<Genre>,
        /// Plot summary the provider reports.
        plot: String,
        /// Title the provider reports.
        title: String,
    },
}
