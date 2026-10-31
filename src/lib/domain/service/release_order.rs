//! Ordering rule over editions and release candidates.
//!
//! The provider's own order is the only ranking the system reports, so the list
//! rule is a **stable partition**: editions whose every medium is a compact
//! disc move to the front and every edition keeps the provider's relative
//! order inside its bucket. [`ReleaseOrder::compare_editions`] is the separate
//! tie-break order a future ingest uses to choose among the compact-disc
//! editions, and never a re-ranking of the returned list.

use std::cmp::Ordering;

use crate::domain::model::music_candidate::MusicCandidate;
use crate::domain::model::music_metadata::Edition;
use crate::domain::model::music_metadata::MediumSummary;
use crate::domain::model::music_metadata::ReleaseStatus;

/// Domain service holding the edition ordering rule.
///
/// The service is stateless: it receives the editions or candidates it operates
/// on as parameters and holds no I/O.
#[derive(Debug)]
#[non_exhaustive]
pub struct ReleaseOrder;

impl ReleaseOrder {
    /// Compare two editions by the selection tie-breaks.
    ///
    /// An official edition precedes every other status, an earlier date precedes
    /// a later one and an absent date, and the provider's identifier settles the
    /// rest. The comparison chooses among the editions of one album; it does not
    /// reorder the list [`ReleaseOrder::order_editions`] returns.
    #[must_use]
    pub fn compare_editions(left: &Edition, right: &Edition) -> Ordering {
        let left_official = left.status().is_some_and(ReleaseStatus::is_official);
        let right_official = right.status().is_some_and(ReleaseStatus::is_official);
        (!left_official)
            .cmp(&(!right_official))
            .then_with(|| match (left.release_date(), right.release_date()) {
                (Some(left_date), Some(right_date)) => left_date.cmp(&right_date),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            })
            .then_with(|| left.reference().id().cmp(right.reference().id()))
    }

    /// Order the release candidates of one search, compact-disc releases first.
    ///
    /// Only release candidates move: an album or a recording keeps its position
    /// and the provider's relative order, and the release candidates keep theirs
    /// inside each bucket.
    #[must_use]
    pub fn order_candidates(candidates: Vec<MusicCandidate>) -> Vec<MusicCandidate> {
        let mut slots: Vec<Option<MusicCandidate>> = candidates.into_iter().map(Some).collect();
        let mut release_positions: Vec<usize> = Vec::new();
        let mut releases: Vec<MusicCandidate> = Vec::new();
        for (position, slot) in slots.iter_mut().enumerate() {
            let is_release = slot
                .as_ref()
                .is_some_and(|candidate| matches!(candidate, MusicCandidate::Release { .. }));
            if is_release && let Some(candidate) = slot.take() {
                release_positions.push(position);
                releases.push(candidate);
            }
        }
        let (compact_discs, others): (Vec<MusicCandidate>, Vec<MusicCandidate>) = releases
            .into_iter()
            .partition(|candidate| all_compact_discs(candidate.mediums()));
        for (position, candidate) in release_positions
            .into_iter()
            .zip(compact_discs.into_iter().chain(others))
        {
            if let Some(slot) = slots.get_mut(position) {
                *slot = Some(candidate);
            }
        }
        slots.into_iter().flatten().collect()
    }

    /// Order the editions of one album, compact discs first.
    ///
    /// The partition is stable: the provider's relative order survives inside
    /// each bucket, so the returned list adds no ranking of its own.
    #[must_use]
    pub fn order_editions(editions: Vec<Edition>) -> Vec<Edition> {
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
    use std::cmp::Ordering;
    use std::error::Error;

    use chrono::NaiveDate;

    use crate::domain::model::media_candidate::ExternalRef;
    use crate::domain::model::media_candidate::ProviderId;
    use crate::domain::model::music_candidate::MusicCandidate;
    use crate::domain::model::music_metadata::Edition;
    use crate::domain::model::music_metadata::MediumFormat;
    use crate::domain::model::music_metadata::MediumSummary;
    use crate::domain::model::music_metadata::ReleaseStatus;
    use crate::domain::model::music_query::MusicKind;

    use super::ReleaseOrder;

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
    fn cd() -> MediumFormat {
        MediumFormat::Cd
    }

    /// Build a vinyl medium, the format the partition sets aside.
    fn vinyl() -> MediumFormat {
        MediumFormat::Other("Vinyl".to_owned())
    }

    /// Parse a date the tests use.
    ///
    /// # Errors
    ///
    /// Returns the parser's error when the literal is not a date.
    fn date(text: &str) -> Result<NaiveDate, Box<dyn Error>> {
        Ok(NaiveDate::parse_from_str(text, "%Y-%m-%d")?)
    }

    /// Build a release candidate carrying `formats`.
    fn release(id: &str, formats: &[MediumFormat]) -> MusicCandidate {
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
        MusicCandidate::Release {
            album: None,
            artist: None,
            mediums,
            reference: ExternalRef::new(ProviderId::MusicBrainz, id.to_owned()),
            status: None,
            title: format!("Release {id}"),
            year: None,
        }
    }

    /// Build an album candidate.
    fn album(id: &str) -> MusicCandidate {
        MusicCandidate::Album {
            artist: None,
            reference: ExternalRef::new(ProviderId::MusicBrainz, id.to_owned()),
            title: format!("Album {id}"),
            year: None,
        }
    }

    /// Build a recording candidate.
    fn recording(id: &str) -> MusicCandidate {
        MusicCandidate::Recording {
            artist: None,
            reference: ExternalRef::new(ProviderId::MusicBrainz, id.to_owned()),
            title: format!("Recording {id}"),
            year: None,
        }
    }

    /// The reference ids of `editions`, in order.
    fn edition_ids(editions: &[Edition]) -> Vec<&str> {
        editions
            .iter()
            .map(|edition| edition.reference().id())
            .collect()
    }

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

    #[test]
    fn compare_editions_prefers_an_official_status() {
        // Arrange
        let official = edition("official", Some(ReleaseStatus::Official), None, &[cd()]);
        let bootleg = edition("bootleg", Some(ReleaseStatus::Bootleg), None, &[cd()]);

        // Act
        let compared = ReleaseOrder::compare_editions(&official, &bootleg);

        // Assert
        assert_eq!(compared, Ordering::Less);
    }

    #[test]
    fn compare_editions_prefers_the_earliest_date() -> Result<(), Box<dyn Error>> {
        // Arrange
        let early = edition(
            "early",
            Some(ReleaseStatus::Official),
            Some(date("1999-01-01")?),
            &[cd()],
        );
        let late = edition(
            "late",
            Some(ReleaseStatus::Official),
            Some(date("2005-01-01")?),
            &[cd()],
        );

        // Act
        let compared = ReleaseOrder::compare_editions(&early, &late);

        // Assert
        assert_eq!(compared, Ordering::Less);
        Ok(())
    }

    #[test]
    fn compare_editions_settles_a_tie_by_identifier() -> Result<(), Box<dyn Error>> {
        // Arrange
        let first = edition(
            "aaa",
            Some(ReleaseStatus::Official),
            Some(date("2000-01-01")?),
            &[cd()],
        );
        let second = edition(
            "bbb",
            Some(ReleaseStatus::Official),
            Some(date("2000-01-01")?),
            &[cd()],
        );

        // Act
        let compared = ReleaseOrder::compare_editions(&first, &second);

        // Assert
        assert_eq!(compared, Ordering::Less);
        Ok(())
    }

    #[test]
    fn order_candidates_moves_only_release_candidates() {
        // Arrange
        let candidates = vec![
            recording("recording"),
            release("vinyl", &[vinyl()]),
            album("album"),
            release("cd", &[cd()]),
        ];

        // Act
        let ordered = ReleaseOrder::order_candidates(candidates);

        // Assert
        let ids: Vec<&str> = ordered
            .iter()
            .map(|candidate| candidate.reference().id())
            .collect();
        assert_eq!(ids, vec!["recording", "cd", "album", "vinyl"]);
        assert_eq!(
            ordered.first().map(MusicCandidate::kind),
            Some(MusicKind::Recording)
        );
        assert_eq!(
            ordered.get(2).map(MusicCandidate::kind),
            Some(MusicKind::Album)
        );
    }
}
