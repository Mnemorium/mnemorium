/// Scope of a stored image or video.
///
/// The first attachment decides the scope: a library cover makes the asset
/// `Library`, while gallery media or a playlist cover makes it `User`. The
/// scope is `None` until the first attachment and never changes afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum Scope {
    /// Artwork owned by the library and shared by catalogue resources.
    Library,
    /// Media owned by one caller: gallery media or a playlist cover.
    User,
}
