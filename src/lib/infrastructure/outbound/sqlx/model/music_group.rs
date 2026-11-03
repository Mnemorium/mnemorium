use crate::domain::alias::NumericID;

/// Row of the `music_group` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MusicGroupRow {
    /// Identifier of the image holding the group's artwork, when set.
    pub cover_image_id: Option<NumericID>,
    /// Unique identifier of the group.
    #[sqlx(primary_key)]
    pub music_group_id: NumericID,
    /// Stage name of the group.
    pub stage_name: String,
}
