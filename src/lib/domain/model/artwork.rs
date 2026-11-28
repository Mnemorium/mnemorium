/// Artwork a provider reports, as image content.
///
/// The bytes are the downloaded image and the media type is the one sniffed
/// from them; a provider address never travels in metadata, so no layer above
/// the adapter depends on where the image came from.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct Artwork {
    /// Content of the image, as downloaded.
    bytes: Vec<u8>,
    /// Media type sniffed from `bytes`.
    media_type: String,
}

impl Artwork {
    /// Create new artwork.
    #[must_use]
    pub(crate) fn new(bytes: Vec<u8>, media_type: String) -> Self {
        Self { bytes, media_type }
    }
}
