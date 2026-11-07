/// The numeric identifier every aggregate uses.
pub(crate) type NumericID = i64;

/// Error returned when constructing a validated scalar value.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum ScalarError {
    /// The value is negative.
    #[error("value must not be negative")]
    Negative,
    /// The value is not a finite number.
    #[error("value must be finite")]
    NotFinite,
    /// The value is zero or negative.
    #[error("value must be greater than zero")]
    NotPositive,
}

/// A finite, non-negative floating-point measurement.
///
/// The value is valid by construction, so a measurement the domain carries in
/// this type needs no second validation when it is persisted. A NaN, an
/// infinity and a negative value are all rejected.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NonNegativeF64(f64);

impl NonNegativeF64 {
    /// Initialise a `NonNegativeF64`.
    ///
    /// # Errors
    ///
    /// Returns [`ScalarError::NotFinite`] when `value` is NaN or infinite, and
    /// [`ScalarError::Negative`] when it is negative.
    pub(crate) fn try_new(value: f64) -> Result<Self, ScalarError> {
        if !value.is_finite() {
            return Err(ScalarError::NotFinite);
        }
        if value < 0.0f64 {
            return Err(ScalarError::Negative);
        }
        Ok(Self(value))
    }

    /// Return the underlying value.
    #[must_use]
    pub(crate) fn value(self) -> f64 {
        self.0
    }
}

/// A positive integer measurement.
///
/// The value is valid by construction, so a measurement the domain carries in
/// this type needs no second validation when it is persisted. Zero and a
/// negative value are both rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PositiveI64(i64);

impl PositiveI64 {
    /// Initialise a `PositiveI64`.
    ///
    /// # Errors
    ///
    /// Returns [`ScalarError::NotPositive`] when `value` is zero or negative.
    pub(crate) fn try_new(value: i64) -> Result<Self, ScalarError> {
        if value <= 0 {
            return Err(ScalarError::NotPositive);
        }
        Ok(Self(value))
    }

    /// Return the underlying value.
    #[must_use]
    pub(crate) fn value(self) -> i64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::nan(f64::NAN)]
    #[case::positive_infinity(f64::INFINITY)]
    #[case::negative_infinity(f64::NEG_INFINITY)]
    fn non_negative_f64_try_new_rejects_non_finite_value(#[case] value: f64) {
        // Act
        let result = NonNegativeF64::try_new(value);

        // Assert
        assert!(matches!(result, Err(ScalarError::NotFinite)));
    }

    #[test]
    fn non_negative_f64_try_new_rejects_negative_value() {
        // Act
        let result = NonNegativeF64::try_new(-1.0f64);

        // Assert
        assert!(matches!(result, Err(ScalarError::Negative)));
    }

    #[rstest]
    #[case::zero(0.0f64)]
    #[case::positive(44_100.0f64)]
    fn non_negative_f64_try_new_accepts_non_negative_value(#[case] value: f64) {
        // Act
        let scalar = NonNegativeF64::try_new(value);

        // Assert
        assert_eq!(scalar.map(NonNegativeF64::value).ok(), Some(value));
    }

    #[rstest]
    #[case::zero(0)]
    #[case::negative(-1)]
    fn positive_i64_try_new_rejects_non_positive_value(#[case] value: i64) {
        // Act
        let result = PositiveI64::try_new(value);

        // Assert
        assert!(matches!(result, Err(ScalarError::NotPositive)));
    }

    #[test]
    fn positive_i64_try_new_accepts_positive_value() {
        // Act
        let scalar = PositiveI64::try_new(1_920);

        // Assert
        assert_eq!(scalar.map(PositiveI64::value).ok(), Some(1_920));
    }
}
