use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;

/// Data model for the `gallery` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Gallery {
    /// Date and time at which the gallery was created.
    pub created_at: NaiveDateTime,
    /// Unique identifier of the gallery.
    #[sqlx(primary_key)]
    pub gallery_id: NumericID,
    /// Whether any authenticated user may read the gallery.
    pub is_public: bool,
    /// Date and time of the last modification of the gallery.
    pub last_modified_at: NaiveDateTime,
    /// Name of the gallery.
    pub name: String,
    /// Identifier of the owning user, absent for the system gallery.
    pub user_id: Option<NumericID>,
}
