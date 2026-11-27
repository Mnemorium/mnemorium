use fast_qr::QRBuilder;

/// Error returned when a payload cannot be rendered as a QR code.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum QrRenderError {
    /// The payload is larger than the encoder accepts.
    #[error("the QR payload is too large to encode")]
    PayloadTooLarge,
}

/// Render a payload as a terminal QR block.
///
/// The adapter is pure and has one call site in the composition root beside the
/// token line; nothing injects it, so it needs no port.
#[derive(Debug, Default)]
#[non_exhaustive]
pub(crate) struct QrRenderer;

impl QrRenderer {
    /// Render `payload` as the Unicode block matrix the terminal displays.
    ///
    /// # Errors
    ///
    /// Returns [`QrRenderError::PayloadTooLarge`] when `payload` exceeds the
    /// encoder's capacity.
    pub(crate) fn render(payload: &str) -> Result<String, QrRenderError> {
        QRBuilder::new(payload)
            .build()
            .map(|code| code.to_str())
            .map_err(|_| QrRenderError::PayloadTooLarge)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::error::Error;

    /// A setup token in the shape the composition root renders.
    const TOKEN: &str = "ABCD-EFGH";

    #[test]
    fn rendered_block_is_not_empty() -> Result<(), Box<dyn Error>> {
        // Act
        let rendered = QrRenderer::render(TOKEN)?;

        // Assert
        assert!(!rendered.trim().is_empty());
        Ok(())
    }

    #[test]
    fn rendered_block_is_stable_for_a_fixed_token() -> Result<(), Box<dyn Error>> {
        // Act
        let first = QrRenderer::render(TOKEN)?;
        let second = QrRenderer::render(TOKEN)?;

        // Assert
        assert_eq!(first, second);
        Ok(())
    }
}
