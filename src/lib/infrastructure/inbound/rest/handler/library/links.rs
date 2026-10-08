//! HAL links of the Library gallery representations.

use serde::Deserialize;
use serde::Serialize;
use utoipa::ToSchema;

use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::hal::Link;
use crate::infrastructure::inbound::rest::handler::library::gallery_items_href;
use crate::infrastructure::inbound::rest::handler::library::gallery_self_href;
use crate::infrastructure::inbound::rest::handler::library::get_gallery_list::ListGalleriesQuery;

/// HAL links exposed by a gallery representation.
///
/// The `self` link addresses the gallery; the `items` link exposes the alias
/// path where its items are served (`API-037`).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct GalleryLinks {
    /// Link to the item collection of the gallery.
    pub items: Link,
    /// Link to the gallery resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl GalleryLinks {
    /// Build the links of the gallery identified by `gallery_id`.
    #[must_use]
    pub fn for_gallery(gallery_id: NumericID) -> Self {
        Self {
            items: Link::new(&gallery_items_href(gallery_id)),
            self_link: Link::new(&gallery_self_href(gallery_id)),
        }
    }
}

/// HAL links exposed by a gallery collection representation.
///
/// The `self` link addresses the collection view the representation is about,
/// `next` and `prev` navigate that same view when another page exists, and the
/// templated `search` link exposes the collection filters (`API-037`,
/// `API-042`). The active filters and the page size are carried into every link
/// so a filtered or paginated listing stays navigable.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct GalleryListLinks {
    /// Link to the next page, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<Link>,
    /// Link to the previous page, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev: Option<Link>,
    /// Templated link exposing the collection's search filters.
    pub search: Link,
    /// Link to the collection itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl GalleryListLinks {
    /// Build the links of a gallery collection page.
    ///
    /// `query` carries the active filters, `limit` and `offset` are the
    /// effective page, `returned` is the number of galleries in the page and
    /// `total` the number of matching galleries before pagination. `next` is
    /// exposed when the page ends before `total`; `prev` is exposed when the
    /// page does not start at the first gallery.
    #[must_use]
    pub fn for_page(
        query: &ListGalleriesQuery,
        limit: usize,
        offset: usize,
        returned: usize,
        total: usize,
    ) -> Self {
        let next = (offset.saturating_add(returned) < total)
            .then(|| Link::new(&page_href(query, limit, offset.saturating_add(limit))));
        let prev =
            (offset > 0).then(|| Link::new(&page_href(query, limit, offset.saturating_sub(limit))));
        Self {
            next,
            prev,
            search: Link::templated(
                "/api/v1/library/gallery{?name,is_public,owner_id,limit,offset}",
            ),
            self_link: Link::new(&page_href(query, limit, offset)),
        }
    }
}

/// Build the href of the gallery collection view at `limit`/`offset`,
/// preserving the active filters so a filtered page keeps its navigation
/// (`API-037`).
fn page_href(query: &ListGalleriesQuery, limit: usize, offset: usize) -> String {
    let mut href = String::from("/api/v1/library/gallery?");
    if let Some(name) = query.name.as_deref() {
        href.push_str("name=");
        // Percent-encode the filter value with the unreserved character set, so
        // it cannot break out of the query string.
        for byte in name.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    href.push(char::from(byte));
                }
                _ => {
                    let high = char::from_digit(u32::from(byte >> 4u8), 16)
                        .unwrap_or('0')
                        .to_ascii_uppercase();
                    let low = char::from_digit(u32::from(byte & 0x0fu8), 16)
                        .unwrap_or('0')
                        .to_ascii_uppercase();
                    href.push('%');
                    href.push(high);
                    href.push(low);
                }
            }
        }
        href.push('&');
    }
    if let Some(is_public) = query.is_public {
        href.push_str("is_public=");
        href.push_str(if is_public { "true" } else { "false" });
        href.push('&');
    }
    if let Some(owner_id) = query.owner_id {
        href.push_str("owner_id=");
        href.push_str(&owner_id.to_string());
        href.push('&');
    }
    href.push_str("limit=");
    href.push_str(&limit.to_string());
    href.push_str("&offset=");
    href.push_str(&offset.to_string());
    href
}
