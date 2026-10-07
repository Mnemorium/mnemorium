use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::gallery::Gallery;
use crate::domain::model::gallery_item::GalleryItem;
use crate::domain::model::gallery_item::GalleryItemMedia;
use crate::domain::port::error::RepositoryError;

/// Search filters for [`GalleryRepository::search`]. Every field is optional;
/// an all-`None` filter returns every gallery.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct GalleryFilter {
    /// Filter on the gallery identifier.
    pub id: Option<NumericID>,
    /// Filter on whether the gallery is public.
    pub is_public: Option<bool>,
    /// Filter on the gallery name.
    pub name: Option<String>,
    /// Filter on the identifier of the owning user.
    pub user_id: Option<NumericID>,
}

/// Search filters for [`GalleryRepository::search_items`]. Every field is
/// optional; an all-`None` filter returns every item.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct GalleryItemFilter {
    /// Filter on the identifier of the owning gallery.
    pub gallery_id: Option<NumericID>,
    /// Filter on the item identifier.
    pub id: Option<NumericID>,
    /// Filter on the referenced medium.
    pub media: Option<GalleryItemMedia>,
}

/// Port for persisting and querying `Gallery` and its `GalleryItem`s.
#[cfg_attr(test, mockall::automock)]
pub trait GalleryRepository: Send + Sync {
    /// Insert a new `item` into its gallery, returning the persisted item with
    /// its final identifier.
    ///
    /// The identifier of `item` is ignored: the repository assigns a fresh
    /// identity.
    fn add_item(
        &mut self,
        item: GalleryItem,
    ) -> impl Future<Output = Result<GalleryItem, RepositoryError>> + Send;

    /// Insert a new `gallery`, returning the persisted gallery with its final
    /// identifier.
    ///
    /// The identifier of `gallery` is ignored: the repository assigns a fresh
    /// identity.
    fn create(
        &mut self,
        gallery: Gallery,
    ) -> impl Future<Output = Result<Gallery, RepositoryError>> + Send;

    /// Delete the gallery identified by `id`.
    ///
    /// Returns `Ok(true)` when a gallery matched `id` and was deleted, and
    /// `Ok(false)` when no gallery matched. A missing gallery is a valid
    /// outcome, not an error.
    fn delete(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Delete the gallery item identified by `id`.
    ///
    /// Returns `Ok(true)` when an item matched `id` and was deleted, and
    /// `Ok(false)` when no item matched. A missing item is a valid outcome, not
    /// an error.
    fn delete_item(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Return the index the next item added to gallery `gallery_id` would take,
    /// that is one past the highest index currently used, or zero when the
    /// gallery is empty.
    fn next_item_index(
        &mut self,
        gallery_id: NumericID,
    ) -> impl Future<Output = Result<i64, RepositoryError>> + Send;

    /// Insert or update an existing `gallery` targeted by its identifier,
    /// returning the persisted gallery.
    fn save(
        &mut self,
        gallery: Gallery,
    ) -> impl Future<Output = Result<Gallery, RepositoryError>> + Send;

    /// Search galleries matching `filter`, returned as `Vec<Gallery>`.
    ///
    /// Returns an empty list when no gallery matches; a missing match is a
    /// valid outcome, not an error.
    fn search(
        &mut self,
        filter: &GalleryFilter,
    ) -> impl Future<Output = Result<Vec<Gallery>, RepositoryError>> + Send;

    /// Search items matching `filter`, returned as `Vec<GalleryItemDetail>`.
    ///
    /// Returns an empty list when no item matches; a missing match is a valid
    /// outcome, not an error.
    fn search_items(
        &mut self,
        filter: &GalleryItemFilter,
    ) -> impl Future<Output = Result<Vec<GalleryItemDetail>, RepositoryError>> + Send;
}

/// A `GalleryItem` together with the metadata of its backing file.
///
/// A read model assembled from `gallery_item LEFT JOIN image/video LEFT JOIN
/// file`, carrying the fields the item representation needs that the
/// [`GalleryItem`] aggregate does not hold.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GalleryItemDetail {
    /// Identifier of the backing file.
    file_id: NumericID,
    /// The gallery item itself.
    item: GalleryItem,
    /// Name of the item: the image name, or the file path for a video.
    name: String,
}

impl GalleryItemDetail {
    /// Return the identifier of the backing file.
    #[must_use]
    pub fn file_id(&self) -> NumericID {
        self.file_id
    }

    /// Return the gallery item.
    #[must_use]
    pub fn item(&self) -> &GalleryItem {
        &self.item
    }

    /// Return the name of the item.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Create a new gallery-item detail.
    #[must_use]
    pub fn new(item: GalleryItem, file_id: NumericID, name: String) -> Self {
        Self {
            file_id,
            item,
            name,
        }
    }
}

#[cfg(test)]
impl<T: GalleryRepository + ?Sized> GalleryRepository for &mut T {
    fn add_item(
        &mut self,
        item: GalleryItem,
    ) -> impl Future<Output = Result<GalleryItem, RepositoryError>> + Send {
        (**self).add_item(item)
    }

    fn create(
        &mut self,
        gallery: Gallery,
    ) -> impl Future<Output = Result<Gallery, RepositoryError>> + Send {
        (**self).create(gallery)
    }

    fn delete(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete(id)
    }

    fn delete_item(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete_item(id)
    }

    fn next_item_index(
        &mut self,
        gallery_id: NumericID,
    ) -> impl Future<Output = Result<i64, RepositoryError>> + Send {
        (**self).next_item_index(gallery_id)
    }

    fn save(
        &mut self,
        gallery: Gallery,
    ) -> impl Future<Output = Result<Gallery, RepositoryError>> + Send {
        (**self).save(gallery)
    }

    fn search(
        &mut self,
        filter: &GalleryFilter,
    ) -> impl Future<Output = Result<Vec<Gallery>, RepositoryError>> + Send {
        (**self).search(filter)
    }

    fn search_items(
        &mut self,
        filter: &GalleryItemFilter,
    ) -> impl Future<Output = Result<Vec<GalleryItemDetail>, RepositoryError>> + Send {
        (**self).search_items(filter)
    }
}
