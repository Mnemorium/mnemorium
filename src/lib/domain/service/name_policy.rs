/// Maximum number of characters allowed in a resource name.
pub const MAX_NAME_LENGTH: usize = 100;

/// Error returned when validating a name against the name policy.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum NamePolicyError {
    /// The name is empty or made only of whitespace.
    #[error("name must not be empty")]
    Empty,
    /// The name is longer than [`MAX_NAME_LENGTH`] characters.
    #[error("name must be at most {MAX_NAME_LENGTH} characters long")]
    TooLong,
}

/// Domain service enforcing the name policy shared by named Library resources.
#[derive(Debug)]
#[non_exhaustive]
pub struct NamePolicy;

impl NamePolicy {
    /// Validate `name`, returning its trimmed form.
    ///
    /// # Errors
    ///
    /// Returns [`NamePolicyError::Empty`] when `name` is empty or made only of
    /// whitespace, and [`NamePolicyError::TooLong`] when it is longer than
    /// [`MAX_NAME_LENGTH`] characters after trimming.
    pub fn validate(name: String) -> Result<String, NamePolicyError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(NamePolicyError::Empty);
        }
        if trimmed.chars().count() > MAX_NAME_LENGTH {
            return Err(NamePolicyError::TooLong);
        }
        // Reuse the caller's allocation when the name is already normalized.
        if trimmed.len() == name.len() {
            Ok(name)
        } else {
            Ok(trimmed.to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::MAX_NAME_LENGTH;
    use super::NamePolicy;
    use super::NamePolicyError;

    #[rstest]
    #[case::plain("Holidays", "Holidays")]
    #[case::surrounding_whitespace("  Holidays  ", "Holidays")]
    #[case::inner_whitespace("Late  night", "Late  night")]
    fn name_policy_accepts_and_trims_valid_names(#[case] name: &str, #[case] expected: &str) {
        // Act
        let result = NamePolicy::validate(name.to_owned());

        // Assert
        assert_eq!(result, Ok(expected.to_owned()));
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

    #[test]
    fn name_policy_measures_length_after_trimming() {
        // Arrange: the surrounding whitespace must not count towards the limit.
        let name = format!(" {} ", "x".repeat(MAX_NAME_LENGTH));

        // Act
        let result = NamePolicy::validate(name);

        // Assert
        assert!(result.is_ok());
    }
}
