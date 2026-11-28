//! Ordering rule over editions and release candidates.
//!
//! The provider's own order is the only ranking the system reports, so the list
//! rule is a **stable partition**: editions whose every medium is a compact
//! disc move to the front and every edition keeps the provider's relative
//! order inside its bucket. [`ReleaseOrder::compare_editions`] is the separate
//! tie-break order a future ingest uses to choose among the compact-disc
//! editions, and never a re-ranking of the returned list.

use crate::domain::model::{Edition, MediumSummary};

/// Domain service holding the edition ordering rule.
///
/// The service is stateless: it receives the editions or candidates it operates
/// on as parameters and holds no I/O.
#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct ReleaseOrder;

impl ReleaseOrder {
    /// Order the editions of one album, compact discs first.
    ///
    /// The partition is stable: the provider's relative order survives inside
    /// each bucket, so the returned list adds no ranking of its own.
    #[must_use]
    pub(crate) fn order_editions(editions: Vec<Edition>) -> Vec<Edition> {
        let (compact_discs, others): (Vec<Edition>, Vec<Edition>) = editions
            .into_iter()
            .partition(|edition| all_compact_discs(edition.mediums()));
        compact_discs.into_iter().chain(others).collect()
    }
}

/// Return whether `mediums` is non-empty and every medium is a compact disc.
///
/// An edition that reports no medium is not a compact-disc edition, and neither
/// is one holding a compact disc beside another format.
fn all_compact_discs(mediums: &[MediumSummary]) -> bool {
    !mediums.is_empty()
        && mediums
            .iter()
            .all(|medium| medium.format().is_compact_disc())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::{ExternalRef, MediumFormat, ProviderId, ReleaseStatus};
    use rstest::fixture;

    use chrono::NaiveDate;

    /// Build a medium summary of `format` at `position`.
    fn medium(format: MediumFormat, position: u32) -> MediumSummary {
        MediumSummary::new(format, position, 10)
    }

    /// Build an edition with `id`, `status`, `date` and `formats`.
    fn edition(
        id: &str,
        status: Option<ReleaseStatus>,
        date: Option<NaiveDate>,
        formats: &[MediumFormat],
    ) -> Edition {
        let mediums = formats
            .iter()
            .enumerate()
            .map(|(position, format)| {
                let index = u32::try_from(position)
                    .unwrap_or_default()
                    .saturating_add(1);
                medium(format.clone(), index)
            })
            .collect();
        Edition::new(
            ExternalRef::new(ProviderId::MusicBrainz, id.to_owned()),
            format!("Edition {id}"),
            date,
            None,
            status,
            mediums,
        )
    }

    /// Build a compact disc, the format the partition looks for.
    #[fixture]
    fn cd() -> MediumFormat {
        MediumFormat::Cd
    }

    /// Build a vinyl medium, the format the partition sets aside.
    #[fixture]
    fn vinyl() -> MediumFormat {
        MediumFormat::Other("Vinyl".to_owned())
    }
    /// The reference ids of `editions`, in order.
    fn edition_ids(editions: &[Edition]) -> Vec<&str> {
        editions
            .iter()
            .map(|edition| edition.reference().id())
            .collect()
    }

    // proves: SCN-METADATA-ENRICHMENT-020-A — Business rule
    #[test]
    fn order_editions_places_compact_discs_first() {
        // Arrange
        let editions = vec![
            edition("vinyl", None, None, &[vinyl()]),
            edition("cd", None, None, &[cd()]),
        ];

        // Act
        let ordered = ReleaseOrder::order_editions(editions);

        // Assert
        assert_eq!(edition_ids(&ordered), vec!["cd", "vinyl"]);
    }

    // proves: SCN-METADATA-ENRICHMENT-020-B — Business rule
    #[test]
    fn order_editions_keeps_the_provider_order_within_a_bucket() {
        // Arrange
        let editions = vec![
            edition("cd-a", None, None, &[cd()]),
            edition("cd-b", None, None, &[cd()]),
            edition("vinyl-a", None, None, &[vinyl()]),
            edition("vinyl-b", None, None, &[vinyl()]),
        ];

        // Act
        let ordered = ReleaseOrder::order_editions(editions);

        // Assert
        assert_eq!(
            edition_ids(&ordered),
            vec!["cd-a", "cd-b", "vinyl-a", "vinyl-b"]
        );
    }

    // proves: SCN-METADATA-ENRICHMENT-020-C — Business rule
    #[test]
    fn a_mixed_format_edition_is_not_a_compact_disc() {
        // Arrange
        let editions = vec![
            edition("mixed", None, None, &[cd(), vinyl()]),
            edition("cd", None, None, &[cd()]),
        ];

        // Act
        let ordered = ReleaseOrder::order_editions(editions);

        // Assert
        assert_eq!(edition_ids(&ordered), vec!["cd", "mixed"]);
    }

    // proves: SCN-METADATA-ENRICHMENT-020-D — Business rule
    #[test]
    fn an_edition_without_a_medium_is_not_a_compact_disc() {
        // Arrange
        let editions = vec![
            edition("unknown", None, None, &[]),
            edition("cd", None, None, &[cd()]),
        ];

        // Act
        let ordered = ReleaseOrder::order_editions(editions);

        // Assert
        assert_eq!(edition_ids(&ordered), vec!["cd", "unknown"]);
    }

    #[test]
    fn order_editions_keeps_an_empty_list_empty() {
        // Act
        let ordered = ReleaseOrder::order_editions(Vec::new());

        // Assert
        assert!(ordered.is_empty());
    }
}
