use hmac::{Hmac, KeyInit as _, Mac as _};
use sha2::Sha256;

/// Length of a derived setup token, in characters.
pub(crate) const SETUP_TOKEN_LENGTH: usize = 8;

/// Position of the display separator inside a setup token.
const DISPLAY_SEPARATOR_POSITION: usize = 4;

/// Prefix separating the setup-token derivation from every other pepper use.
const DOMAIN_SEPARATION_PREFIX: &[u8] = b"mnemorium/root-admin-setup/v1";

/// Crockford base32 alphabet: `0`-`9` and `A`-`Z` without `I`, `L`, `O` and `U`.
const CROCKFORD_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Domain service deriving and checking the Root Admin setup token.
///
/// The derivation is a pure function of the instance's **Site pepper** and a
/// stored nonce, so it is stateless and shared by the bootstrap and the setup
/// use case. The token never exists in the datastore: the grant stores only the
/// nonce the derivation consumes.
#[derive(Debug, Default)]
#[non_exhaustive]
pub(crate) struct SetupTokenDeriver;

impl SetupTokenDeriver {
    /// Derive the token for `nonce` under `pepper`.
    ///
    /// The token is eight Crockford base32 characters: the first 40 bits of
    /// `HMAC-SHA256(pepper, domain-separation-prefix || nonce)`.
    #[must_use]
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "the accumulator holds at most twelve bits and every shift is bounded by the 40-bit digest"
    )]
    pub(crate) fn derive(pepper: &str, nonce: &str) -> String {
        let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(pepper.as_bytes()) else {
            return String::new();
        };
        mac.update(DOMAIN_SEPARATION_PREFIX);
        mac.update(nonce.as_bytes());
        let digest = mac.finalize().into_bytes();

        let mut token = String::with_capacity(SETUP_TOKEN_LENGTH);
        let mut accumulator: u32 = 0;
        let mut available_bits: u32 = 0;
        for byte in digest.iter().take(5) {
            accumulator = (accumulator << 8u32) | u32::from(*byte);
            available_bits += 8u32;
            while available_bits >= 5u32 && token.len() < SETUP_TOKEN_LENGTH {
                let shift = available_bits - 5u32;
                let index = (accumulator >> shift) & 0x1Fu32;
                if let Some(symbol) = usize::try_from(index)
                    .ok()
                    .and_then(|position| CROCKFORD_ALPHABET.get(position))
                {
                    token.push(char::from(*symbol));
                }
                available_bits -= 5u32;
            }
        }
        token
    }

    /// Render `token` as the displayed `XXXX-XXXX` form.
    #[must_use]
    pub(crate) fn display(token: &str) -> String {
        let mut shown = String::with_capacity(SETUP_TOKEN_LENGTH + 1);
        for (position, character) in token.chars().enumerate() {
            if position == DISPLAY_SEPARATOR_POSITION {
                shown.push('-');
            }
            shown.push(character);
        }
        shown
    }

    /// Fold a submitted token to the derived form.
    ///
    /// Case is ignored, the separator and surrounding whitespace are dropped,
    /// and `I`/`L` are read as `1` while `O` is read as `0`.
    #[must_use]
    pub(crate) fn normalize(submitted: &str) -> String {
        submitted
            .chars()
            .filter(|character| *character != '-' && !character.is_whitespace())
            .map(|character| match character.to_ascii_uppercase() {
                'I' | 'L' => '1',
                'O' => '0',
                other => other,
            })
            .collect()
    }

    /// Return whether `submitted` names the grant derived from `nonce`.
    ///
    /// The comparison is constant time over the derived length, so a caller
    /// cannot learn which character differed.
    #[must_use]
    pub(crate) fn verify(pepper: &str, nonce: &str, submitted: &str) -> bool {
        let expected = Self::derive(pepper, nonce);
        let normalized = Self::normalize(submitted);
        constant_time_eq(expected.as_bytes(), normalized.as_bytes())
    }
}

/// Compare two byte strings without short-circuiting on the first difference.
fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (left_byte, right_byte) in left.iter().zip(right) {
        difference |= left_byte ^ right_byte;
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::error::Error;

    const PEPPER: &str = "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";

    fn alphabet_contains(character: char) -> bool {
        CROCKFORD_ALPHABET
            .iter()
            .any(|symbol| char::from(*symbol) == character)
    }

    // proves: SCN-AUTHENTICATION-002-A — Business rule
    #[test]
    fn setup_token_deriver_samples_follow_declared_shape() -> Result<(), Box<dyn Error>> {
        // Act & Assert
        (move |pepper: &str, samples: usize| -> Result<(), Box<dyn Error>> {
            for sample in 0..samples {
                let nonce = format!("{sample:064x}");
                let token = SetupTokenDeriver::derive(pepper, &nonce);

                assert_eq!(token.chars().count(), SETUP_TOKEN_LENGTH);
                assert!(
                    token.chars().all(alphabet_contains),
                    "{token:?} carries a character outside the declared alphabet"
                );
                let shown = SetupTokenDeriver::display(&token);
                assert_eq!(shown.chars().count(), SETUP_TOKEN_LENGTH + 1);
                assert_eq!(shown.chars().nth(DISPLAY_SEPARATOR_POSITION), Some('-'));
                assert_eq!(
                    SetupTokenDeriver::normalize(&shown).chars().count(),
                    SETUP_TOKEN_LENGTH
                );
            }
            Ok(())
        })(PEPPER, 1_000)
    }

    #[test]
    fn setup_token_deriver_same_inputs_derive_the_same_token() {
        // Arrange
        let nonce = "a".repeat(64);

        // Act
        let first = SetupTokenDeriver::derive(PEPPER, &nonce);
        let second = SetupTokenDeriver::derive(PEPPER, &nonce);

        // Assert
        assert_eq!(first, second);
    }

    #[test]
    fn setup_token_deriver_a_different_nonce_changes_the_token() {
        // Arrange
        let first_nonce = "a".repeat(64);
        let second_nonce = "b".repeat(64);

        // Act
        let first = SetupTokenDeriver::derive(PEPPER, &first_nonce);
        let second = SetupTokenDeriver::derive(PEPPER, &second_nonce);

        // Assert
        assert_ne!(first, second);
    }

    #[test]
    fn setup_token_deriver_a_different_pepper_changes_the_token() {
        // Arrange
        let nonce = "a".repeat(64);
        let other_pepper = "c".repeat(64);

        // Act
        let first = SetupTokenDeriver::derive(PEPPER, &nonce);
        let second = SetupTokenDeriver::derive(&other_pepper, &nonce);

        // Assert
        assert_ne!(first, second);
    }

    #[test]
    fn setup_token_deriver_normalization_reads_case_dash_and_confusable_characters() {
        // Arrange
        let nonce = "e".repeat(64);
        let token = SetupTokenDeriver::derive(PEPPER, &nonce);
        let folded = token.to_ascii_lowercase();
        let unsigned = folded.replace('-', "");
        let confusable = unsigned.replace('0', "o").replace('1', "i");
        let confusable_l = unsigned.replace('0', "o").replace('1', "l");

        // Act & Assert
        assert!(SetupTokenDeriver::verify(PEPPER, &nonce, &folded));
        assert!(SetupTokenDeriver::verify(PEPPER, &nonce, &unsigned));
        assert!(SetupTokenDeriver::verify(PEPPER, &nonce, &confusable));
        assert!(SetupTokenDeriver::verify(PEPPER, &nonce, &confusable_l));
        assert!(SetupTokenDeriver::verify(
            PEPPER,
            &nonce,
            &format!(" {} ", SetupTokenDeriver::display(&token))
        ));
    }

    #[test]
    fn setup_token_deriver_a_wrong_token_is_refused() {
        // Arrange
        let nonce = "f".repeat(64);
        let token = SetupTokenDeriver::derive(PEPPER, &nonce);
        let wrong_token = (|token: &str| {
            let mut characters = token.chars();
            let replacement = match characters.next() {
                Some('0') => '1',
                _ => '0',
            };
            format!("{replacement}{}", characters.collect::<String>())
        })(&token);

        // Act & Assert
        assert!(!SetupTokenDeriver::verify(PEPPER, &nonce, &wrong_token));
        assert!(!SetupTokenDeriver::verify(PEPPER, &nonce, ""));
    }

    #[test]
    fn constant_time_comparison_reports_equality_without_early_exit() {
        // Act & Assert
        assert!(constant_time_eq(b"ABCDEFGH", b"ABCDEFGH"));
        assert!(!constant_time_eq(b"ABCDEFGH", b"ABCDEFGI"));
        assert!(!constant_time_eq(b"ABCDEFGH", b"ABCDEFG"));
    }
}
