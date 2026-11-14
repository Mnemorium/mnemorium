use chrono::NaiveDate;

use crate::domain::NumericID;
use crate::domain::model::Season;

/// Error returned when initialising a `Series`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum SeriesError {
    /// A genre identifier is empty.
    #[error("a genre identifier must not be empty")]
    InvalidGenre,
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

/// A show the server holds, with its seasons.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Series {
    /// Country of origin of the show, when known.
    country_of_origin: Option<String>,
    /// Identifier of the image holding the show's artwork, when set.
    cover_image_id: Option<NumericID>,
    /// Date of the first airing of the show, when known.
    first_air_date: Option<NaiveDate>,
    /// Identifiers of the genres the show is associated with.
    genre_ids: Vec<String>,
    /// Unique identifier of the show.
    id: NumericID,
    /// Name of the show.
    name: String,
    /// Plot of the show.
    plot: String,
    /// Seasons of the show, in order.
    seasons: Vec<Season>,
}

impl Series {
    /// Return the country of origin of the show, if known.
    #[must_use]
    pub(crate) fn country_of_origin(&self) -> Option<&str> {
        self.country_of_origin.as_deref()
    }

    /// Return the identifier of the image holding the show's artwork, if any.
    #[must_use]
    pub(crate) fn cover_image_id(&self) -> Option<NumericID> {
        self.cover_image_id
    }

    /// Return the date of the first airing, if known.
    #[must_use]
    pub(crate) fn first_air_date(&self) -> Option<NaiveDate> {
        self.first_air_date
    }

    /// Return the identifiers of the genres the show is associated with.
    #[must_use]
    pub(crate) fn genre_ids(&self) -> &[String] {
        &self.genre_ids
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

    /// Return the plot.
    #[must_use]
    pub(crate) fn plot(&self) -> &str {
        &self.plot
    }

    /// Return the seasons, in order.
    #[must_use]
    pub(crate) fn seasons(&self) -> &[Season] {
        &self.seasons
    }

    /// Initialise a new `Series`, validating `name` and `genre_ids`.
    ///
    /// # Errors
    ///
    /// Returns [`SeriesError::InvalidName`] when `name` is empty, and
    /// [`SeriesError::InvalidGenre`] when a genre identifier is empty.
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the show's fields one-to-one"
    )]
    pub(crate) fn try_new(
        id: NumericID,
        name: String,
        plot: String,
        country_of_origin: Option<String>,
        first_air_date: Option<NaiveDate>,
        genre_ids: Vec<String>,
        cover_image_id: Option<NumericID>,
        seasons: Vec<Season>,
    ) -> Result<Self, SeriesError> {
        if name.is_empty() {
            return Err(SeriesError::InvalidName);
        }
        if genre_ids.iter().any(String::is_empty) {
            return Err(SeriesError::InvalidGenre);
        }
        Ok(Self {
            country_of_origin,
            cover_image_id,
            first_air_date,
            genre_ids,
            id,
            name,
            plot,
            seasons,
        })
    }
}
