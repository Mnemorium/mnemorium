/// Minimum number of characters allowed in a username.
pub const MIN_USERNAME_LENGTH: usize = 4;

/// Maximum number of characters allowed in a username.
pub const MAX_USERNAME_LENGTH: usize = 100;

/// Error returned when validating a username.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum UsernamePolicyError {
    /// The username contains an uppercase letter.
    #[error("username must not contain an uppercase letter")]
    ContainsUppercase,
    /// The username begins or ends with whitespace.
    #[error("username must not begin or end with whitespace")]
    SurroundingWhitespace,
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
/// submitted.
#[derive(Debug)]
#[non_exhaustive]
pub struct UsernamePolicy;

impl UsernamePolicy {
    /// Validate `username` against the policy.
    ///
    /// # Errors
    ///
    /// Returns [`UsernamePolicyError::TooShort`] or
    /// [`UsernamePolicyError::TooLong`] when the character count is outside
    /// `MIN_USERNAME_LENGTH..=MAX_USERNAME_LENGTH`,
    /// [`UsernamePolicyError::ContainsUppercase`] when any character is
    /// uppercase, and [`UsernamePolicyError::SurroundingWhitespace`] when the
    /// value begins or ends with whitespace.
    pub fn validate(username: &str) -> Result<(), UsernamePolicyError> {
        let length = username.chars().count();
        if length < MIN_USERNAME_LENGTH {
            return Err(UsernamePolicyError::TooShort);
        }
        if length > MAX_USERNAME_LENGTH {
            return Err(UsernamePolicyError::TooLong);
        }
        if username.chars().any(char::is_uppercase) {
            return Err(UsernamePolicyError::ContainsUppercase);
        }
        if username.trim() != username {
            return Err(UsernamePolicyError::SurroundingWhitespace);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::MAX_USERNAME_LENGTH;
    use super::MIN_USERNAME_LENGTH;
    use super::UsernamePolicy;
    use super::UsernamePolicyError;

    #[rstest]
    #[case::shortest_allowed("abcd")]
    #[case::lowercase_with_digits_and_symbols("a1_-")]
    #[case::internal_space("al ice")]
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
    #[case::ascii_uppercase("Alice")]
    #[case::unicode_uppercase("Élise")]
    fn username_policy_rejects_uppercase_usernames(#[case] username: &str) {
        // Act & Assert
        assert_eq!(
            UsernamePolicy::validate(username),
            Err(UsernamePolicyError::ContainsUppercase)
        );
    }

    #[rstest]
    #[case::leading_space(" alice")]
    #[case::trailing_space("alice ")]
    #[case::leading_tab("\talice")]
    #[case::trailing_newline("alice\n")]
    fn username_policy_rejects_surrounding_whitespace(#[case] username: &str) {
        // Act & Assert
        assert_eq!(
            UsernamePolicy::validate(username),
            Err(UsernamePolicyError::SurroundingWhitespace)
        );
    }

    #[test]
    fn username_policy_rejects_a_whitespace_only_username() {
        // Arrange
        let username = " ".repeat(MIN_USERNAME_LENGTH);

        // Act & Assert
        assert_eq!(
            UsernamePolicy::validate(&username),
            Err(UsernamePolicyError::SurroundingWhitespace)
        );
    }
}
