//! Authorization predicates shared by the Library use cases.
//!
//! This is a domain service: it depends only on domain models and holds no I/O.
//! Loading the authenticated caller is application orchestration and stays in
//! the application layer.

use crate::domain::model::gallery::Gallery;
use crate::domain::model::music_playlist::MusicPlaylist;
use crate::domain::model::user::User;

/// Domain service holding the Library visibility and ownership rules.
#[derive(Debug)]
#[non_exhaustive]
pub struct LibraryAccess;

impl LibraryAccess {
    /// Return whether `caller` may read `gallery`.
    ///
    /// Any authenticated caller may read a public gallery; a private gallery is
    /// readable by its owner or the Root Admin.
    #[must_use]
    pub fn can_read_gallery(gallery: &Gallery, caller: &User) -> bool {
        gallery.is_public() || gallery.is_owned_by(caller.id()) || caller.is_root_admin()
    }

    /// Return whether `caller` may read and moderate `playlist`.
    ///
    /// Any authenticated caller may read and moderate a public playlist; a
    /// private playlist is readable and moderatable by its owner or the Root
    /// Admin.
    #[must_use]
    pub fn can_read_playlist(playlist: &MusicPlaylist, caller: &User) -> bool {
        playlist.is_public() || playlist.is_owned_by(caller.id()) || caller.is_root_admin()
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use chrono::NaiveDateTime;

    use crate::domain::model::music_playlist::MusicPlaylist;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;

    use super::LibraryAccess;

    fn playlist(user_id: i64, is_public: bool) -> MusicPlaylist {
        MusicPlaylist::new(
            1,
            "Mix".to_owned(),
            NaiveDateTime::default(),
            NaiveDateTime::default(),
            user_id,
            is_public,
            Vec::new(),
        )
    }

    #[test]
    fn any_caller_reads_a_public_playlist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let playlist = playlist(3, true);
        let caller = User::try_new(9, "caller".to_owned(), None, 9, Role::Standard)?;

        // Act
        let readable = LibraryAccess::can_read_playlist(&playlist, &caller);

        // Assert
        assert!(readable);
        Ok(())
    }

    #[test]
    fn owner_reads_a_private_playlist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let playlist = playlist(4, false);
        let caller = User::try_new(4, "caller".to_owned(), None, 4, Role::Standard)?;

        // Act
        let readable = LibraryAccess::can_read_playlist(&playlist, &caller);

        // Assert
        assert!(readable);
        Ok(())
    }

    #[test]
    fn root_admin_reads_a_private_playlist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let playlist = playlist(4, false);
        let caller = User::try_new(0, "root".to_owned(), None, 0, Role::Admin)?;

        // Act
        let readable = LibraryAccess::can_read_playlist(&playlist, &caller);

        // Assert
        assert!(readable);
        Ok(())
    }

    #[test]
    fn another_caller_cannot_read_a_private_playlist() -> Result<(), Box<dyn Error>> {
        // Arrange
        let playlist = playlist(4, false);
        let caller = User::try_new(9, "caller".to_owned(), None, 9, Role::Standard)?;

        // Act
        let readable = LibraryAccess::can_read_playlist(&playlist, &caller);

        // Assert
        assert!(!readable);
        Ok(())
    }
}
