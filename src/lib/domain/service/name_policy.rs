/// Maximum number of characters allowed in a resource name.
pub(crate) const MAX_NAME_LENGTH: usize = 100;

/// Error returned when validating a name against the name policy.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum NamePolicyError {
    /// The name is empty or made only of whitespace.
    #[error("name must not be empty")]
    Empty,
    /// The name begins or ends with whitespace.
    #[error("name must not begin or end with whitespace")]
    SurroundingWhitespace,
    /// The name is longer than [`MAX_NAME_LENGTH`] characters.
    #[error("name must be at most {MAX_NAME_LENGTH} characters long")]
    TooLong,
}

/// Domain service enforcing the name policy shared by named Library resources.
///
/// The policy validates; it never repairs. A value that breaks a rule is
/// rejected, and a value that satisfies every rule is stored exactly as
/// submitted — surrounding whitespace is rejected, never trimmed, and interior
/// whitespace stays part of the name.
#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct NamePolicy;

impl NamePolicy {
    /// Validate `name`, returning it unchanged.
    ///
    /// # Errors
    ///
    /// Returns [`NamePolicyError::Empty`] when `name` is empty or made only of
    /// whitespace, [`NamePolicyError::SurroundingWhitespace`] when it begins or
    /// ends with whitespace, and [`NamePolicyError::TooLong`] when it is longer
    /// than [`MAX_NAME_LENGTH`] characters.
    pub(crate) fn validate(name: String) -> Result<String, NamePolicyError> {
        if name.trim().is_empty() {
            return Err(NamePolicyError::Empty);
        }
        if name.trim() != name
            || name
                .starts_with(|character: char| character.is_whitespace() || character == '\u{feff}')
            || name
                .ends_with(|character: char| character.is_whitespace() || character == '\u{feff}')
        {
            return Err(NamePolicyError::SurroundingWhitespace);
        }
        if name.chars().count() > MAX_NAME_LENGTH {
            return Err(NamePolicyError::TooLong);
        }
        Ok(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::plain("Holidays")]
    #[case::inner_whitespace("Late  night")]
    #[case::inner_zero_width_no_break_space("Late\u{feff}night")]
    fn name_policy_accepts_and_keeps_valid_names(#[case] name: &str) {
        // Act
        let result = NamePolicy::validate(name.to_owned());

        // Assert
        assert_eq!(result, Ok(name.to_owned()));
    }

    #[rstest]
    #[case::empty("")]
    #[case::whitespace_only("   ")]
    fn name_policy_rejects_empty_names(#[case] name: &str) {
        // Act
        let result = NamePolicy::validate(name.to_owned());

        // Assert
        assert_eq!(result, Err(NamePolicyError::Empty));
    }

    #[rstest]
    #[case::surrounding_spaces("  Holidays  ")]
    #[case::leading_space(" Holidays")]
    #[case::trailing_space("Holidays ")]
    #[case::single_character_padded(" x ")]
    #[case::trailing_newline("Holidays\n")]
    #[case::next_line("Holidays\u{85}")]
    #[case::zero_width_no_break_space("Holidays\u{feff}")]
    fn name_policy_rejects_surrounding_whitespace(#[case] name: &str) {
        // Act
        let result = NamePolicy::validate(name.to_owned());

        // Assert
        assert_eq!(result, Err(NamePolicyError::SurroundingWhitespace));
    }

    #[test]
    fn name_policy_accepts_exactly_the_maximum_length() {
        // Arrange
        let name = "x".repeat(MAX_NAME_LENGTH);

        // Act
        let result = NamePolicy::validate(name.clone());

        // Assert
        assert_eq!(result, Ok(name));
    }

    #[test]
    fn name_policy_rejects_too_long_names() {
        // Arrange
        let name = "x".repeat(MAX_NAME_LENGTH.saturating_add(1));

        // Act
        let result = NamePolicy::validate(name.clone());

        // Assert
        assert_eq!(result, Err(NamePolicyError::TooLong));
    }
}
