/// Minimum number of characters allowed in a username.
pub(crate) const MIN_USERNAME_LENGTH: usize = 4;

/// Maximum number of characters allowed in a username.
pub(crate) const MAX_USERNAME_LENGTH: usize = 100;

/// Error returned when validating a username.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum UsernamePolicyError {
    /// The username contains a NUL character.
    #[error("username must not contain a NUL character")]
    ContainsNul,
    /// The username contains whitespace anywhere.
    #[error("username must not contain whitespace")]
    ContainsWhitespace,
    /// The username is longer than [`MAX_USERNAME_LENGTH`].
    #[error("username must be at most {MAX_USERNAME_LENGTH} characters long")]
    TooLong,
    /// The username is shorter than [`MIN_USERNAME_LENGTH`].
    #[error("username must be at least {MIN_USERNAME_LENGTH} characters long")]
    TooShort,
}

/// Domain service enforcing the username policy.
///
/// The policy validates; it never repairs. A value that breaks a rule is
/// rejected, and a value that satisfies every rule is stored exactly as
/// submitted — never trimmed, lowercased or otherwise normalized.
#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct UsernamePolicy;

impl UsernamePolicy {
    /// Validate `username` against the policy.
    ///
    /// # Errors
    ///
    /// Returns [`UsernamePolicyError::TooShort`] or
    /// [`UsernamePolicyError::TooLong`] when the character count is outside
    /// `MIN_USERNAME_LENGTH..=MAX_USERNAME_LENGTH`,
    /// [`UsernamePolicyError::ContainsNul`] when the value contains a NUL
    /// character, and [`UsernamePolicyError::ContainsWhitespace`] when any
    /// character is whitespace.
    pub(crate) fn validate(username: &str) -> Result<(), UsernamePolicyError> {
        let length = username.chars().count();
        if length < MIN_USERNAME_LENGTH {
            return Err(UsernamePolicyError::TooShort);
        }
        if length > MAX_USERNAME_LENGTH {
            return Err(UsernamePolicyError::TooLong);
        }
        // The datastore measures a text value up to its first NUL, so a value
        // carrying one cannot satisfy the same length rule it was validated
        // against. The policy rejects it rather than letting storage disagree.
        if username.contains('\0') {
            return Err(UsernamePolicyError::ContainsNul);
        }
        if username
            .chars()
            .any(|character| character.is_whitespace() || character == '\u{feff}')
        {
            return Err(UsernamePolicyError::ContainsWhitespace);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::shortest_allowed("abcd")]
    #[case::lowercase_with_digits_and_symbols("a1_-")]
    #[case::uppercase("Alice")]
    #[case::mixed_case("aliceSmith")]
    #[case::unicode_uppercase("Élise")]
    fn username_policy_accepts_valid_usernames(#[case] username: &str) {
        // Act & Assert
        assert_eq!(UsernamePolicy::validate(username), Ok(()));
    }

    #[test]
    fn username_policy_accepts_exactly_the_maximum_length() {
        // Arrange
        let username = "a".repeat(MAX_USERNAME_LENGTH);

        // Act & Assert
        assert_eq!(UsernamePolicy::validate(&username), Ok(()));
    }

    #[rstest]
    #[case::empty("")]
    #[case::one_character("a")]
    #[case::three_characters("abc")]
    fn username_policy_rejects_short_usernames(#[case] username: &str) {
        // Act & Assert
        assert_eq!(
            UsernamePolicy::validate(username),
            Err(UsernamePolicyError::TooShort)
        );
    }

    #[test]
    fn username_policy_rejects_too_long_usernames() {
        // Arrange
        let username = "a".repeat(MAX_USERNAME_LENGTH.saturating_add(1));

        // Act & Assert
        assert_eq!(
            UsernamePolicy::validate(&username),
            Err(UsernamePolicyError::TooLong)
        );
    }

    #[rstest]
    #[case::leading_space(" alice")]
    #[case::trailing_space("alice ")]
    #[case::interior_space("al ice")]
    #[case::leading_tab("\talice")]
    #[case::trailing_newline("alice\n")]
    #[case::next_line("alice\u{85}")]
    #[case::zero_width_no_break_space("alice\u{feff}")]
    fn username_policy_rejects_whitespace_anywhere(#[case] username: &str) {
        // Act & Assert
        assert_eq!(
            UsernamePolicy::validate(username),
            Err(UsernamePolicyError::ContainsWhitespace)
        );
    }

    #[rstest]
    #[case::leading_nul("\0alice")]
    #[case::interior_nul("al\0ice")]
    fn username_policy_rejects_a_nul_character(#[case] username: &str) {
        // Act & Assert
        assert_eq!(
            UsernamePolicy::validate(username),
            Err(UsernamePolicyError::ContainsNul)
        );
    }

    #[test]
    fn username_policy_rejects_a_whitespace_only_username() {
        // Arrange
        let username = " ".repeat(MIN_USERNAME_LENGTH);

        // Act & Assert
        assert_eq!(
            UsernamePolicy::validate(&username),
            Err(UsernamePolicyError::ContainsWhitespace)
        );
    }
}
