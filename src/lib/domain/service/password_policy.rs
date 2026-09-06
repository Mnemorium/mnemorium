/// Minimum number of characters a password must have.
pub const PASSWORD_MIN_LENGTH: usize = 8;

/// Error returned when validating a password against the password policy.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PasswordPolicyError {
    /// The password does not contain at least one symbol.
    #[error("password must contain at least one symbol")]
    PasswordMissingSymbol,
    /// The password is shorter than [`PASSWORD_MIN_LENGTH`].
    #[error("password must be at least {PASSWORD_MIN_LENGTH} characters long")]
    PasswordTooShort,
}

/// Domain service enforcing the password policy.
#[derive(Debug)]
#[non_exhaustive]
pub struct PasswordPolicy {
    /// Minimum number of characters a password must have.
    min_length: usize,
    /// Whether a password must contain at least one symbol.
    require_symbol: bool,
}

impl PasswordPolicy {
    /// Create a new policy using the recommended default parameters.
    #[must_use]
    pub fn new() -> Self {
        Self {
            min_length: PASSWORD_MIN_LENGTH,
            require_symbol: true,
        }
    }

    /// Validate `password` against the policy.
    ///
    /// # Errors
    ///
    /// Returns [`PasswordPolicyError::PasswordTooShort`] when `password` is
    /// shorter than the minimum length, and
    /// [`PasswordPolicyError::PasswordMissingSymbol`] when `password` contains
    /// no non-alphanumeric character.
    pub fn validate(&self, password: &str) -> Result<(), PasswordPolicyError> {
        if password.chars().count() < self.min_length {
            return Err(PasswordPolicyError::PasswordTooShort);
        }
        if self.require_symbol
            && !password
                .chars()
                .any(|character| !character.is_alphanumeric())
        {
            return Err(PasswordPolicyError::PasswordMissingSymbol);
        }
        Ok(())
    }
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::PasswordPolicy;
    use super::PasswordPolicyError;

    #[rstest]
    #[case::with_letter_and_symbol("password!")]
    #[case::with_digit_and_symbol("12345678#")]
    #[case::only_symbols("!#$%&*()-_")]
    #[case::exactly_min_length("p4ssw0rd+")]
    fn password_policy_valid_password_accepted(#[case] password: &str) {
        // Act & Assert
        let result = PasswordPolicy::new().validate(password);
        assert_eq!(result, Ok(()));
    }

    #[rstest]
    #[case::seven_characters("aB3!xYz")]
    #[case::empty("")]
    fn password_policy_short_password_rejected(#[case] password: &str) {
        // Act
        let result = PasswordPolicy::new().validate(password);

        // Assert
        assert_eq!(result, Err(PasswordPolicyError::PasswordTooShort));
    }

    #[rstest]
    #[case::letters_only("password")]
    #[case::digits_only("12345678")]
    #[case::letters_and_digits("password123")]
    fn password_policy_password_without_symbol_rejected(#[case] password: &str) {
        // Act
        let result = PasswordPolicy::new().validate(password);

        // Assert
        assert_eq!(result, Err(PasswordPolicyError::PasswordMissingSymbol));
    }
}
