//! Shared Hypertext Application Language (HAL) link types.
//!
//! See `docs/development/TechnicalDesign.md` § HAL payload guidelines.

use serde::Deserialize;
use serde::Serialize;
use utoipa::ToSchema;

/// Media type of a HAL response, per `API-031`.
pub const HAL_CONTENT_TYPE: &str = "application/hal+json";

/// A single HAL link.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct Link {
    /// Target URI reference of the link.
    pub href: String,
    /// Whether `href` is a URI template, per RFC 6570.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(true))]
    pub templated: Option<bool>,
}

impl Link {
    /// Build a plain link to `href`.
    #[must_use]
    pub fn new(href: &str) -> Self {
        Self {
            href: href.to_owned(),
            templated: None,
        }
    }

    /// Build a link whose `href` is a URI template.
    #[must_use]
    pub fn templated(href: &str) -> Self {
        Self {
            href: href.to_owned(),
            templated: Some(true),
        }
    }
}

/// The `self` link of a HAL representation.
///
/// Every resource representation carries the URI it was served from, so clients
/// never construct or hardcode it themselves (`API-032`, `API-037`).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct SelfLinks {
    /// Link to the resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl SelfLinks {
    /// Build the `self` link pointing at `href`.
    #[must_use]
    pub fn new(href: &str) -> Self {
        Self {
            self_link: Link::new(href),
        }
    }
}
