/// A field a partial-update command may leave alone, clear, or replace.
///
/// A `PATCH` body distinguishes a field it does not name (keep the stored
/// value) from one it sends as `null` (clear the stored value), and both from
/// one it sends with a value (replace the stored value). This type carries that
/// three-way choice, which a nested `Option` expresses only ambiguously.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum FieldUpdate<T> {
    /// The request does not name the field; the stored value is kept.
    #[default]
    Absent,
    /// The request names the field as `null`; the stored value is cleared.
    Clear,
    /// The request names a value; the stored value is replaced.
    Set(T),
}

impl<T> FieldUpdate<T> {
    /// Resolve the update against the stored value.
    ///
    /// An absent field keeps `stored`, a cleared field drops it, and a set field
    /// replaces it.
    #[must_use]
    pub fn resolve(self, stored: Option<T>) -> Option<T> {
        match self {
            Self::Absent => stored,
            Self::Clear => None,
            Self::Set(value) => Some(value),
        }
    }

    /// Map the value the request names, keeping an absent or cleared field.
    ///
    /// # Errors
    ///
    /// Returns the error `map` produces for a set value; an absent or cleared
    /// field never fails.
    pub fn try_map<U, E, F>(self, map: F) -> Result<FieldUpdate<U>, E>
    where
        F: FnOnce(T) -> Result<U, E>,
    {
        match self {
            Self::Absent => Ok(FieldUpdate::Absent),
            Self::Clear => Ok(FieldUpdate::Clear),
            Self::Set(value) => map(value).map(FieldUpdate::Set),
        }
    }
}
