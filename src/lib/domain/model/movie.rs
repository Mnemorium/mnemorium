use chrono::NaiveDate;

use crate::domain::alias::NumericID;
use crate::domain::model::permission::Protected;
use crate::domain::model::permission::ResourceKind;
use crate::domain::model::permission::ResourceRef;
use crate::domain::model::video::Video;

/// Error returned when initialising a `Movie`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MovieError {
    /// A genre identifier is empty.
    #[error("a genre identifier must not be empty")]
    InvalidGenre,
    /// The `name` is empty.
    #[error("name must not be empty")]
    InvalidName,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// A film the server holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Movie {
    /// Country of origin of the film.
    country_of_origin: String,
    /// Identifier of the file holding the film's artwork, when set.
    cover_file_id: Option<NumericID>,
    /// Identifiers of the genres the film is associated with.
    genre_ids: Vec<String>,
    /// Unique identifier of the film.
    id: NumericID,
    /// Name of the film.
    name: String,
    /// Plot of the film.
    plot: String,
    /// Date of the release of the film.
    release_date: NaiveDate,
    /// Video backing the film.
    video: Video,
}

impl Movie {
    /// Return the country of origin of the film.
    #[must_use]
    pub fn country_of_origin(&self) -> &str {
        &self.country_of_origin
    }

    /// Return the identifier of the file holding the film's artwork, if any.
    #[must_use]
    pub fn cover_file_id(&self) -> Option<NumericID> {
        self.cover_file_id
    }

    /// Return the identifiers of the genres the film is associated with.
    #[must_use]
    pub fn genre_ids(&self) -> &[String] {
        &self.genre_ids
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

    /// Return the plot.
    #[must_use]
    pub fn plot(&self) -> &str {
        &self.plot
    }

    /// Return the date of the release.
    #[must_use]
    pub fn release_date(&self) -> NaiveDate {
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
    pub fn try_new(
        id: NumericID,
        name: String,
        plot: String,
        country_of_origin: String,
        release_date: NaiveDate,
        genre_ids: Vec<String>,
        cover_file_id: Option<NumericID>,
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
            cover_file_id,
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
    pub fn video(&self) -> &Video {
        &self.video
    }
}

impl Protected for Movie {
    fn is_privileged_account(&self) -> bool {
        false
    }

    fn is_publicly_visible(&self) -> bool {
        true
    }

    fn owner_id(&self) -> Option<NumericID> {
        None
    }

    fn resource_ref(&self) -> Option<ResourceRef> {
        Some(ResourceRef::new(ResourceKind::Movie, self.id))
    }
}
