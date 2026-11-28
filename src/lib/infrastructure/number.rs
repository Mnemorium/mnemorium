//! Checked conversions between the numeric types the adapters share.

/// Convert an integral `f64` into an `i64`, or `None` when it is not one.
///
/// `{:.0}` renders the exact value rather than the shortest decimal that
/// round-trips, so the parse is exact and its range check is the conversion:
/// `as` would truncate, and `f64` implements no `TryFrom` for the integer
/// types.
pub(crate) fn integral_i64(value: f64) -> Option<i64> {
    format!("{value:.0}").parse().ok()
}

/// Convert an integral `f64` into a `u64`, or `None` when it is not one.
///
/// The rendering and range check are those of [`integral_i64`].
pub(crate) fn integral_u64(value: f64) -> Option<u64> {
    format!("{value:.0}").parse().ok()
}

#[cfg(test)]
mod tests {
    use super::{integral_i64, integral_u64};

    #[test]
    fn integral_i64_converts_an_integral_value() {
        // Act
        let converted = integral_i64(4_294_967_296.0f64);

        // Assert
        assert_eq!(converted, Some(4_294_967_296));
    }

    #[test]
    fn integral_i64_keeps_the_exact_integer_past_the_shortest_decimal() {
        // Arrange: the largest `f64` below `i64::MAX` (2^63 - 2^10), whose
        // shortest round-tripping decimal renders as `…775000`.
        let largest = 2.0f64.powi(63) - 1_024.0f64;

        // Act
        let converted = integral_i64(largest);

        // Assert
        assert_eq!(converted, Some(9_223_372_036_854_774_784));
    }

    #[test]
    fn integral_i64_rejects_a_value_outside_the_range() {
        // Act, Assert
        assert_eq!(integral_i64(2.0f64.powi(63)), None);
    }

    #[test]
    fn integral_i64_rejects_a_non_finite_value() {
        // Act, Assert
        assert_eq!(integral_i64(f64::NAN), None);
        assert_eq!(integral_i64(f64::INFINITY), None);
    }

    #[test]
    fn integral_u64_converts_only_a_value_it_can_hold() {
        // Act, Assert
        assert_eq!(integral_u64(4_294_967_296.0f64), Some(4_294_967_296));
        assert_eq!(integral_u64(2.0f64.powi(64)), None);
    }
}
