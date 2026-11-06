/// Error returned when validating a country-of-origin code.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum CountryCodeError {
    /// The value is not exactly two ASCII letters.
    #[error("a country code must be exactly two ASCII letters")]
    Invalid,
}

/// Domain service enforcing the ISO 3166-1 alpha-2 country code shape.
#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct CountryCode;

impl CountryCode {
    /// Validate `value` as an ISO 3166-1 alpha-2 code, returning its uppercase
    /// form.
    ///
    /// # Errors
    ///
    /// Returns [`CountryCodeError::Invalid`] when `value` is not exactly two
    /// ASCII letters.
    pub(crate) fn validate(value: &str) -> Result<String, CountryCodeError> {
        if value.len() != 2
            || !value
                .chars()
                .all(|character| character.is_ascii_alphabetic())
        {
            return Err(CountryCodeError::Invalid);
        }
        Ok(value.to_ascii_uppercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::uppercase("US", "US")]
    #[case::lowercase_is_normalized("us", "US")]
    fn country_code_accepts_two_letters(#[case] value: &str, #[case] expected: &str) {
        // Act
        let result = CountryCode::validate(value);

        // Assert
        assert_eq!(result, Ok(expected.to_owned()));
    }

    #[rstest]
    #[case::empty("")]
    #[case::one_letter("U")]
    #[case::three_letters("USA")]
    #[case::digit("U1")]
    #[case::symbol("U$")]
    #[case::non_ascii("ÉU")]
    fn country_code_rejects_anything_else(#[case] value: &str) {
        // Act
        let result = CountryCode::validate(value);

        // Assert
        assert_eq!(result, Err(CountryCodeError::Invalid));
    }
}
