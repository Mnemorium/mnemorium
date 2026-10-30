/// Artwork a provider reports, as image content.
///
/// The bytes are the downloaded image and the media type is the one sniffed
/// from them; a provider address never travels in metadata, so no layer above
/// the adapter depends on where the image came from.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Artwork {
    /// Content of the image, as downloaded.
    bytes: Vec<u8>,
    /// Media type sniffed from `bytes`.
    media_type: String,
}

impl Artwork {
    /// Return the content of the image.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Return the media type sniffed from the content.
    #[must_use]
    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    /// Create new artwork.
    #[must_use]
    pub fn new(bytes: Vec<u8>, media_type: String) -> Self {
        Self { bytes, media_type }
    }
}
