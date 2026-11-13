use chrono::NaiveDate;

use crate::domain::NumericID;
use crate::domain::model::Video;

/// Error returned when initialising a `Movie`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum MovieError {
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

/// A film the server holds.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Movie {
    /// Country of origin of the film, when known.
    country_of_origin: Option<String>,
    /// Identifier of the image holding the film's artwork, when set.
    cover_image_id: Option<NumericID>,
    /// Identifiers of the genres the film is associated with.
    genre_ids: Vec<String>,
    /// Unique identifier of the film.
    id: NumericID,
    /// Name of the film.
    name: String,
    /// Plot of the film.
    plot: String,
    /// Date of the release of the film, when known.
    release_date: Option<NaiveDate>,
    /// Video backing the film.
    video: Video,
}

impl Movie {
    /// Return the country of origin of the film, if known.
    #[must_use]
    pub(crate) fn country_of_origin(&self) -> Option<&str> {
        self.country_of_origin.as_deref()
    }

    /// Return the identifier of the image holding the film's artwork, if any.
    #[must_use]
    pub(crate) fn cover_image_id(&self) -> Option<NumericID> {
        self.cover_image_id
    }

    /// Return the identifiers of the genres the film is associated with.
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

    /// Return the date of the release, if known.
    #[must_use]
    pub(crate) fn release_date(&self) -> Option<NaiveDate> {
        self.release_date
    }

    /// Initialise a new `Movie`, validating `name` and `genre_ids`.
    ///
    /// # Errors
    ///
    /// Returns [`MovieError::InvalidName`] when `name` is empty, and
    /// [`MovieError::InvalidGenre`] when a genre identifier is empty.
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the film's fields one-to-one"
    )]
    pub(crate) fn try_new(
        id: NumericID,
        name: String,
        plot: String,
        country_of_origin: Option<String>,
        release_date: Option<NaiveDate>,
        genre_ids: Vec<String>,
        cover_image_id: Option<NumericID>,
        video: Video,
    ) -> Result<Self, MovieError> {
        if name.is_empty() {
            return Err(MovieError::InvalidName);
        }
        if genre_ids.iter().any(String::is_empty) {
            return Err(MovieError::InvalidGenre);
        }
        Ok(Self {
            country_of_origin,
            cover_image_id,
            genre_ids,
            id,
            name,
            plot,
            release_date,
            video,
        })
    }

    /// Return the video backing the film.
    #[must_use]
    pub(crate) fn video(&self) -> &Video {
        &self.video
    }

    /// Return the identifier of the video backing the film.
    #[must_use]
    pub(crate) fn video_id(&self) -> NumericID {
        self.video.id()
    }
}
