use crate::domain::alias::NumericID;
use crate::domain::model::episode::Episode;

/// The series-scoped identity of a season.
///
/// A season is addressed by its index within its series; it carries no separate
/// identifier into the API or the representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SeasonIndex(u32);

impl SeasonIndex {
    /// Initialise a new `SeasonIndex`.
    #[must_use]
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    /// Return the series-scoped index of the season.
    #[must_use]
    pub fn value(self) -> u32 {
        self.0
    }
}

/// A season within a series.
#[derive(Debug, Clone, PartialEq)]
pub struct Season {
    /// Identifier of the file holding the season's artwork, when set.
    cover_file_id: Option<NumericID>,
    /// Episodes of the season, in order.
    episodes: Vec<Episode>,
    /// Series-scoped identity of the season.
    index: SeasonIndex,
    /// Name of the season, when set.
    name: Option<String>,
}

impl Season {
    /// Return the identifier of the file holding the season's artwork, if any.
    #[must_use]
    pub fn cover_file_id(&self) -> Option<NumericID> {
        self.cover_file_id
    }

    /// Return the episodes of the season, in order.
    #[must_use]
    pub fn episodes(&self) -> &[Episode] {
        &self.episodes
    }

    /// Return the series-scoped identity of the season.
    #[must_use]
    pub fn index(&self) -> SeasonIndex {
        self.index
    }

    /// Return the name of the season, if any.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Initialise a new `Season`.
    #[must_use]
    pub fn new(
        index: SeasonIndex,
        name: Option<String>,
        cover_file_id: Option<NumericID>,
        episodes: Vec<Episode>,
    ) -> Self {
        Self {
            cover_file_id,
            episodes,
            index,
            name,
        }
    }
}
