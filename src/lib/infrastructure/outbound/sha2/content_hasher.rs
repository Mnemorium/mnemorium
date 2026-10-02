use sha2::Digest as _;
use sha2::Sha256;

use crate::domain::model::integrity_hash::IntegrityHash;
use crate::domain::model::integrity_hash::SHA256_HEX_LENGTH;
use crate::domain::port::content_hasher::ContentHasher;
use crate::domain::port::content_hasher::ContentHasherSession;
use crate::domain::port::error::ContentHasherError;

/// Content hasher backed by the SHA-256 algorithm.
#[derive(Debug, Default)]
pub struct Sha2ContentHasher;

impl ContentHasher for Sha2ContentHasher {
    fn hasher(&self) -> Box<dyn ContentHasherSession> {
        Box::new(Sha2ContentHasherSession {
            hasher: Sha256::new(),
        })
    }
}

/// One incremental SHA-256 hashing session.
struct Sha2ContentHasherSession {
    /// The SHA-256 state fed by every `update`.
    hasher: Sha256,
}

impl ContentHasherSession for Sha2ContentHasherSession {
    fn finalize(self: Box<Self>) -> Result<IntegrityHash<SHA256_HEX_LENGTH>, ContentHasherError> {
        let hex = hex_encode(&self.hasher.finalize());
        IntegrityHash::try_new(hex).map_err(|error| ContentHasherError::Unknown(error.into()))
    }

    fn update(&mut self, bytes: &[u8]) {
        self.hasher.update(bytes);
    }
}

/// Hexadecimal-encode `bytes`, lowercase.
#[expect(
    clippy::single_call_fn,
    reason = "the digest encoding is named for readability"
)]
fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut hex = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        let _result = write!(hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::domain::model::integrity_hash::IntegrityHash;
    use crate::domain::model::integrity_hash::SHA256_HEX_LENGTH;
    use crate::domain::port::content_hasher::ContentHasher as _;
    use crate::domain::port::error::ContentHasherError;

    use super::Sha2ContentHasher;

    /// SHA-256 of `b"1234"`, in lowercase hexadecimal.
    const DIGEST_1234: &str = "03ac674216f3e15c761ee1a5e255f067953623c8b388b4459e13f978d7c846f4";

    /// SHA-256 of the empty input, in lowercase hexadecimal.
    const DIGEST_EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    /// Stream `chunks` through a fresh hasher and return the digest.
    fn hash(chunks: &[&[u8]]) -> Result<IntegrityHash<SHA256_HEX_LENGTH>, ContentHasherError> {
        let mut session = Sha2ContentHasher.hasher();
        for chunk in chunks {
            session.update(chunk);
        }
        session.finalize()
    }

    #[test]
    fn hasher_hashes_incrementally() -> Result<(), Box<dyn Error>> {
        assert_eq!(hash(&[b"12", b"34"])?.as_str(), hash(&[b"1234"])?.as_str());
        Ok(())
    }

    #[test]
    fn hasher_hashes_the_empty_input() -> Result<(), Box<dyn Error>> {
        assert_eq!(hash(&[])?.as_str(), DIGEST_EMPTY);
        Ok(())
    }

    #[test]
    fn hasher_matches_the_known_sha256_vector() -> Result<(), Box<dyn Error>> {
        assert_eq!(hash(&[b"1234"])?.as_str(), DIGEST_1234);
        Ok(())
    }
}
