//! `MusicBrainz` adapter: maps the service's search and lookup payloads onto the
//! canonical music port.
//!
//! The service asks for a descriptive `User-Agent` and documents a request
//! limit, so the adapter carries the header on every request, spaces its
//! requests through one slot reservation and retries a rate-limited, timed-out
//! or transiently failed request within a bounded budget. A failure is
//! classified here and logged once, on give-up, as the catalogued `port_fault`;
//! the raw transport error, whose `Display` can carry the request address, is
//! never propagated or logged (`OBS-002`, `OBS-003`).
//!
//! Artwork reaches the Cover Art Archive through its JSON index: the shared
//! artwork client refuses redirects, and the archive's image endpoint answers
//! with one, so the adapter reads the index and then fetches the address it
//! names.

use std::time::Duration;

use chrono::NaiveDate;
use reqwest::Client;
use reqwest::StatusCode;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;
use tokio::time::Instant;
use tokio::time::sleep_until;

use crate::application::security_event;
use crate::domain::model::artwork::Artwork;
use crate::domain::model::media_candidate::ExternalRef;
use crate::domain::model::media_candidate::ProviderId;
use crate::domain::model::music_album::ProductionType;
use crate::domain::model::music_candidate::MusicCandidate;
use crate::domain::model::music_candidate::MusicTarget;
use crate::domain::model::music_metadata::ArtistCredit;
use crate::domain::model::music_metadata::Edition;
use crate::domain::model::music_metadata::MediumFormat;
use crate::domain::model::music_metadata::MediumSummary;
use crate::domain::model::music_metadata::MusicMetadata;
use crate::domain::model::music_metadata::ReleaseStatus;
use crate::domain::model::music_query::MusicKind;
use crate::domain::model::music_query::MusicQuery;
use crate::domain::port::error::ExternalServiceError;
use crate::domain::port::music_metadata_provider::MusicMetadataProvider;
use crate::domain::service::release_order::ReleaseOrder;
use crate::infrastructure::outbound::reqwest::ArtworkClient;
use crate::infrastructure::outbound::reqwest::REQUEST_TIMEOUT;
use crate::infrastructure::outbound::reqwest::classify_status;
use crate::infrastructure::outbound::reqwest::classify_transport_error;
use crate::infrastructure::outbound::reqwest::log_provider_fault;
use crate::infrastructure::outbound::reqwest::read_payload;
use crate::infrastructure::outbound::reqwest::request_url;

/// Base address of the `MusicBrainz` API.
const DEFAULT_API_BASE_URL: &str = "https://musicbrainz.org/ws/2";

/// Base address of the Cover Art Archive.
const DEFAULT_ARCHIVE_BASE_URL: &str = "https://coverartarchive.org";

/// Hosts the Cover Art Archive serves album artwork from.
///
/// The archive publishes each image under `coverartarchive.org` and may hand
/// the download on to `archive.org`, so both hosts are the archive's.
const MUSICBRAINZ_ARTWORK_HOSTS: &[&str] = &["archive.org", "coverartarchive.org"];

/// Shortest interval between two requests to the service, from its own limit.
const DEFAULT_MIN_INTERVAL: Duration = Duration::from_secs(1);

/// Number of attempts one request may spend, the first one included.
const DEFAULT_ATTEMPTS: u32 = 3;

/// Security-event operation name owned by this adapter.
const OPERATION: &str = "music_metadata";

/// Characters `MusicBrainz`'s search syntax reserves.
const RESERVED: &[char] = &[
    '+', '-', '&', '|', '!', '(', ')', '{', '}', '[', ']', '^', '"', '~', '*', '?', ':', '\\', '/',
];

/// Descriptive `User-Agent` the service requires, naming the maintainer.
const USER_AGENT: &str = concat!(
    "Mnemorium/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/Mnemorium/mnemorium)"
);

/// One artist credit entry of a `MusicBrainz` payload.
#[derive(Debug, Deserialize)]
struct ArtistCreditPayload {
    /// Phrase the service places after the name, when it reports one.
    #[serde(default)]
    joinphrase: Option<String>,
    /// Name the service reports.
    #[serde(default)]
    name: Option<String>,
}

/// One genre entry of a `MusicBrainz` payload.
#[derive(Debug, Deserialize)]
struct GenrePayload {
    /// Genre name as the service spells it.
    #[serde(default)]
    name: String,
}

/// One image entry of a Cover Art Archive index.
#[derive(Debug, Deserialize)]
struct ArtworkImage {
    /// Whether the image is the front cover.
    #[serde(default)]
    front: bool,
    /// Address of the image.
    #[serde(default)]
    image: String,
}

/// The Cover Art Archive's JSON index for one album or release.
#[derive(Debug, Deserialize)]
struct ArtworkIndex {
    /// Images the archive lists.
    #[serde(default)]
    images: Vec<ArtworkImage>,
}

/// One medium entry of a `MusicBrainz` payload.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct MediumPayload {
    /// Format of the medium.
    #[serde(default)]
    format: Option<String>,
    /// Album-scoped position of the medium.
    #[serde(default)]
    position: Option<u32>,
    /// Number of tracks the medium carries.
    #[serde(default)]
    track_count: Option<u32>,
}

/// The release group a release belongs to.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ReleaseGroupRefPayload {
    /// Identifier the service issued for the release group.
    #[serde(default)]
    id: String,
}

/// One search hit for an album.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ReleaseGroupHit {
    /// Artist credits the service reports.
    #[serde(default)]
    artist_credit: Vec<ArtistCreditPayload>,
    /// Date of first release, as the service spells it.
    #[serde(default)]
    first_release_date: Option<String>,
    /// Identifier the service issued for the work.
    #[serde(default)]
    id: String,
    /// Title the service reports.
    #[serde(default)]
    title: String,
}

/// One search hit or edition entry for a release.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ReleaseHit {
    /// Artist credits the service reports.
    #[serde(default)]
    artist_credit: Vec<ArtistCreditPayload>,
    /// Country of release, as the service spells it.
    #[serde(default)]
    country: Option<String>,
    /// Date of release, as the service spells it.
    #[serde(default)]
    date: Option<String>,
    /// Identifier the service issued for the release.
    #[serde(default)]
    id: String,
    /// Mediums of the release, in the service's order.
    ///
    /// A lookup spells the list `media`; a search spells it `medium-list`.
    #[serde(default, alias = "media")]
    medium_list: Vec<MediumPayload>,
    /// Release group the release belongs to.
    #[serde(default)]
    release_group: Option<ReleaseGroupRefPayload>,
    /// Status the service reports.
    #[serde(default)]
    status: Option<String>,
    /// Title the service reports.
    #[serde(default)]
    title: String,
}

/// One search hit for a recording.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RecordingHit {
    /// Artist credits the service reports.
    #[serde(default)]
    artist_credit: Vec<ArtistCreditPayload>,
    /// Date of first release, as the service spells it.
    #[serde(default)]
    first_release_date: Option<String>,
    /// Identifier the service issued for the recording.
    #[serde(default)]
    id: String,
    /// Title the service reports.
    #[serde(default)]
    title: String,
}

/// The results of one `MusicBrainz` search.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct SearchPayload {
    /// Recording hits, in the service's order.
    #[serde(default)]
    recordings: Vec<RecordingHit>,
    /// Album hits, in the service's order.
    #[serde(default)]
    release_groups: Vec<ReleaseGroupHit>,
    /// Release hits, in the service's order.
    #[serde(default)]
    releases: Vec<ReleaseHit>,
}

/// One album lookup payload.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct AlbumLookupPayload {
    /// Artist credits the service reports.
    #[serde(default)]
    artist_credit: Vec<ArtistCreditPayload>,
    /// Date of first release, as the service spells it.
    #[serde(default)]
    first_release_date: Option<String>,
    /// Genres from the service's curated vocabulary.
    #[serde(default)]
    genres: Vec<GenrePayload>,
    /// Primary type the service reports.
    #[serde(default)]
    primary_type: Option<String>,
    /// Editions the service lists; a lookup spells the list `releases`.
    #[serde(default)]
    releases: Vec<ReleaseHit>,
    /// Secondary types the service reports.
    #[serde(default)]
    secondary_types: Vec<String>,
    /// Title the service reports.
    #[serde(default)]
    title: String,
}

/// One release lookup payload.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ReleaseLookupPayload {
    /// Artist credits the service reports.
    #[serde(default)]
    artist_credit: Vec<ArtistCreditPayload>,
    /// Country of release, as the service spells it.
    #[serde(default)]
    country: Option<String>,
    /// Date of release, as the service spells it.
    #[serde(default)]
    date: Option<String>,
    /// Genres from the service's curated vocabulary.
    #[serde(default)]
    genres: Vec<GenrePayload>,
    /// Identifier the service issued for the release.
    #[serde(default)]
    id: String,
    /// Mediums of the release, in the service's order.
    #[serde(default)]
    media: Vec<MediumPayload>,
    /// Release group the release belongs to.
    #[serde(default)]
    release_group: Option<ReleaseGroupRefPayload>,
    /// Status the service reports.
    #[serde(default)]
    status: Option<String>,
    /// Title the service reports.
    #[serde(default)]
    title: String,
}

/// One recording lookup payload.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RecordingLookupPayload {
    /// Artist credits the service reports.
    #[serde(default)]
    artist_credit: Vec<ArtistCreditPayload>,
    /// Date of first release, as the service spells it.
    #[serde(default)]
    first_release_date: Option<String>,
    /// Genres from the service's curated vocabulary.
    #[serde(default)]
    genres: Vec<GenrePayload>,
    /// ISRC codes the service reports.
    #[serde(default)]
    isrcs: Vec<String>,
    /// Title the service reports.
    #[serde(default)]
    title: String,
}

/// Music metadata provider backed by the `MusicBrainz` API.
pub struct MusicBrainzMetadataProvider {
    /// Base address of the service's API.
    api_base: String,
    /// Base address of the Cover Art Archive.
    archive_base: String,
    /// Client downloading the artwork under the shared policy.
    artwork: ArtworkClient,
    /// Number of attempts one request may spend.
    attempts: u32,
    /// Client carrying the timeout and the descriptive `User-Agent`.
    client: Client,
    /// Shortest interval between two requests.
    min_interval: Duration,
    /// Earliest instant at which the next request may be sent.
    next_slot: Mutex<Option<Instant>>,
}

impl MusicBrainzMetadataProvider {
    /// Return no artwork and emit the one catalogued event for `reason`.
    fn artwork_absent(reason: &str) -> Option<Artwork> {
        security_event::metadata_artwork_failed(ProviderId::MusicBrainz.as_str(), reason);
        None
    }

    /// Fetch the archive's index for `path` and download the image it names.
    ///
    /// The archive answers the index with a redirect to its download host and
    /// names each image in plain http on its own host, so the index is read only
    /// once its final address is a host the artwork policy trusts, and the image
    /// is fetched through the policy's allow-listed redirect hop.
    async fn artwork_for(&self, path: &str) -> Option<Artwork> {
        let address = format!("{}{path}", self.archive_base);
        let Ok(response) = self.client.get(address).send().await else {
            return Self::artwork_absent("transport");
        };
        if !response.status().is_success() {
            return Self::artwork_absent("status");
        }
        if !self
            .artwork
            .accepts_host(response.url().host_str().unwrap_or_default())
        {
            return Self::artwork_absent("host");
        }
        let Ok(index) = read_payload::<ArtworkIndex>(response).await else {
            return Self::artwork_absent("payload");
        };
        let Some(image) = index
            .images
            .iter()
            .find(|image| image.front)
            .or_else(|| index.images.first())
        else {
            return Self::artwork_absent("absent");
        };
        self.artwork
            .fetch_redirected(ProviderId::MusicBrainz, &image.image)
            .await
    }

    /// Classify a failure, log the one `port_fault`, and return it unchanged.
    fn fault(error: ExternalServiceError) -> ExternalServiceError {
        log_provider_fault(OPERATION, error.classification());
        error
    }

    /// Send one request once the next slot is reserved.
    async fn get_payload<T: DeserializeOwned>(
        &self,
        path: &str,
        pairs: &[(&str, &str)],
    ) -> Result<Option<T>, ExternalServiceError> {
        let mut all: Vec<(&str, &str)> = pairs.to_vec();
        all.push(("fmt", "json"));
        let Some(url) = request_url(&self.api_base, path, &all) else {
            return Err(Self::fault(ExternalServiceError::InvalidRequest));
        };
        let mut attempt: u32 = 0;
        loop {
            self.reserve_slot().await;
            let outcome = self.send_once::<T>(url.clone()).await;
            match outcome {
                Ok(payload) => return Ok(payload),
                Err(error) => {
                    attempt = attempt.saturating_add(1);
                    let retryable = matches!(
                        &error,
                        ExternalServiceError::CommunicationFailure
                            | ExternalServiceError::DependencyFailure
                            | ExternalServiceError::RateLimited
                            | ExternalServiceError::RetryableFailure
                            | ExternalServiceError::Timeout
                            | ExternalServiceError::Unavailable
                    );
                    if attempt >= self.attempts || !retryable {
                        return Err(Self::fault(error));
                    }
                }
            }
        }
    }

    /// Create a provider talking to the production endpoints.
    #[must_use]
    pub fn new() -> Self {
        Self::with_endpoints(
            DEFAULT_API_BASE_URL.to_owned(),
            DEFAULT_ARCHIVE_BASE_URL.to_owned(),
            REQUEST_TIMEOUT,
            DEFAULT_MIN_INTERVAL,
            DEFAULT_ATTEMPTS,
        )
    }

    /// Reserve the next request slot, waiting until it arrives.
    async fn reserve_slot(&self) {
        let send_at = {
            let mut slot = self.next_slot.lock().await;
            let now = Instant::now();
            let send_at = slot.map_or(now, |previous| previous.max(now));
            *slot = Some(send_at.checked_add(self.min_interval).unwrap_or(send_at));
            send_at
        };
        if send_at > Instant::now() {
            sleep_until(send_at).await;
        }
    }

    /// Send one request and decode an optional payload.
    async fn send_once<T: DeserializeOwned>(
        &self,
        url: reqwest::Url,
    ) -> Result<Option<T>, ExternalServiceError> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|failure| classify_transport_error(&failure))?;
        let status = response.status();
        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        // The service reports a rate limit as `429` and, under load, as `503`;
        // both are classified before the shared helper maps every 5xx to a
        // dependency failure.
        if matches!(status.as_u16(), 429 | 503) {
            return Err(ExternalServiceError::RateLimited);
        }
        classify_status(status)?;
        read_payload::<T>(response).await.map(Some)
    }

    /// Replace the artwork client, so a test can stand the archive in for
    /// `wiremock`.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_artwork_client(mut self, artwork: ArtworkClient) -> Self {
        self.artwork = artwork;
        self
    }

    /// Create a provider with every address and policy input as parameters.
    #[must_use]
    pub fn with_endpoints(
        api_base: String,
        archive_base: String,
        timeout: Duration,
        min_interval: Duration,
        attempts: u32,
    ) -> Self {
        let client = Client::builder()
            .timeout(timeout)
            .user_agent(USER_AGENT)
            .build()
            // A builder failure means the TLS backend cannot be initialised at
            // all, which the first request then reports as a classified fault.
            .unwrap_or_default();
        Self {
            api_base,
            archive_base,
            artwork: ArtworkClient::new(MUSICBRAINZ_ARTWORK_HOSTS),
            attempts: attempts.max(1),
            client,
            min_interval,
            next_slot: Mutex::new(None),
        }
    }
}

impl Default for MusicBrainzMetadataProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MusicMetadataProvider for MusicBrainzMetadataProvider {
    async fn lookup(
        &self,
        target: &MusicTarget,
    ) -> Result<Option<MusicMetadata>, ExternalServiceError> {
        let reference = target.reference();
        if reference.provider() != ProviderId::MusicBrainz {
            return Ok(None);
        }
        let id = reference.id();
        if id.is_empty()
            || !id
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-')
        {
            return Ok(None);
        }
        match target.kind() {
            MusicKind::Album => {
                let path = format!("/release-group/{id}");
                let Some(payload) = self
                    .get_payload::<AlbumLookupPayload>(
                        &path,
                        &[("inc", "artist-credits+genres+releases+release-groups")],
                    )
                    .await?
                else {
                    return Ok(None);
                };
                let artwork = self.artwork_for(&format!("/release-group/{id}")).await;
                let editions = payload
                    .releases
                    .into_iter()
                    .map(|hit| {
                        Edition::new(
                            ExternalRef::new(ProviderId::MusicBrainz, hit.id),
                            hit.title,
                            date(hit.date.as_deref()),
                            country(hit.country.as_deref()),
                            hit.status.as_deref().and_then(ReleaseStatus::from_name),
                            mediums(&hit.medium_list),
                        )
                    })
                    .collect();
                // A secondary type takes precedence over the primary one, and an
                // unmappable secondary type leaves the production type absent
                // rather than falling back to the primary.
                let production_type = if payload.secondary_types.is_empty() {
                    payload
                        .primary_type
                        .as_deref()
                        .and_then(mapped_production_type)
                } else {
                    payload
                        .secondary_types
                        .iter()
                        .find_map(|name| mapped_production_type(name))
                };
                Ok(Some(MusicMetadata::Album {
                    artwork,
                    artist_credits: credits(&payload.artist_credit),
                    editions: ReleaseOrder::order_editions(editions),
                    first_release_date: date(payload.first_release_date.as_deref()),
                    genres: genres(&payload.genres),
                    production_type,
                    title: payload.title,
                }))
            }
            MusicKind::Release => {
                let path = format!("/release/{id}");
                let Some(payload) = self
                    .get_payload::<ReleaseLookupPayload>(
                        &path,
                        &[("inc", "artist-credits+genres+recordings+release-groups")],
                    )
                    .await?
                else {
                    return Ok(None);
                };
                let artwork = self.artwork_for(&format!("/release/{id}")).await;
                let edition = Edition::new(
                    ExternalRef::new(ProviderId::MusicBrainz, payload.id),
                    payload.title,
                    date(payload.date.as_deref()),
                    country(payload.country.as_deref()),
                    payload.status.as_deref().and_then(ReleaseStatus::from_name),
                    mediums(&payload.media),
                );
                Ok(Some(MusicMetadata::Release {
                    album: payload
                        .release_group
                        .map(|group| ExternalRef::new(ProviderId::MusicBrainz, group.id)),
                    artwork,
                    artist_credits: credits(&payload.artist_credit),
                    edition,
                    genres: genres(&payload.genres),
                }))
            }
            MusicKind::Recording => {
                let path = format!("/recording/{id}");
                let Some(payload) = self
                    .get_payload::<RecordingLookupPayload>(
                        &path,
                        &[("inc", "artist-credits+genres+isrcs+releases")],
                    )
                    .await?
                else {
                    return Ok(None);
                };
                Ok(Some(MusicMetadata::Recording {
                    artist_credits: credits(&payload.artist_credit),
                    first_release_date: date(payload.first_release_date.as_deref()),
                    genres: genres(&payload.genres),
                    isrcs: payload
                        .isrcs
                        .into_iter()
                        .filter(|code| !code.trim().is_empty())
                        .collect(),
                    title: payload.title,
                }))
            }
        }
    }

    async fn search(
        &self,
        query: &MusicQuery,
    ) -> Result<Vec<MusicCandidate>, ExternalServiceError> {
        let kind = query.kind();
        let (path, field, year_field) = match kind {
            MusicKind::Album => ("/release-group", "releasegroup", "firstreleasedate"),
            MusicKind::Release => ("/release", "release", "date"),
            MusicKind::Recording => ("/recording", "recording", "firstreleasedate"),
        };
        let mut search = quoted_field(field, query.title());
        if let Some(artist) = query
            .artist()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            search.push_str(" AND ");
            search.push_str(&quoted_field("artist", artist));
        }
        if let Some(year_value) = query.year() {
            search.push_str(" AND ");
            search.push_str(year_field);
            search.push(':');
            search.push_str(&year_value.to_string());
        }
        let Some(payload) = self
            .get_payload::<SearchPayload>(path, &[("query", &search)])
            .await?
        else {
            return Ok(Vec::new());
        };
        Ok(match kind {
            MusicKind::Album => payload
                .release_groups
                .into_iter()
                .map(|hit| MusicCandidate::Album {
                    artist: first_artist(&hit.artist_credit),
                    reference: ExternalRef::new(ProviderId::MusicBrainz, hit.id),
                    title: hit.title,
                    year: year(hit.first_release_date.as_deref()),
                })
                .collect(),
            MusicKind::Release => payload
                .releases
                .into_iter()
                .map(|hit| MusicCandidate::Release {
                    album: hit
                        .release_group
                        .map(|group| ExternalRef::new(ProviderId::MusicBrainz, group.id)),
                    artist: first_artist(&hit.artist_credit),
                    mediums: mediums(&hit.medium_list),
                    reference: ExternalRef::new(ProviderId::MusicBrainz, hit.id),
                    status: hit.status.as_deref().and_then(ReleaseStatus::from_name),
                    title: hit.title,
                    year: year(hit.date.as_deref()),
                })
                .collect(),
            MusicKind::Recording => payload
                .recordings
                .into_iter()
                .map(|hit| MusicCandidate::Recording {
                    artist: first_artist(&hit.artist_credit),
                    reference: ExternalRef::new(ProviderId::MusicBrainz, hit.id),
                    title: hit.title,
                    year: year(hit.first_release_date.as_deref()),
                })
                .collect(),
        })
    }
}

/// Escape `value` and place it in the quoted `field` of a Lucene expression.
fn quoted_field(field: &str, value: &str) -> String {
    let mut quoted =
        String::with_capacity(value.len().saturating_add(field.len()).saturating_add(4));
    quoted.push_str(field);
    quoted.push_str(":\"");
    for character in value.chars() {
        if RESERVED.contains(&character) {
            quoted.push('\\');
        }
        quoted.push(character);
    }
    quoted.push('"');
    quoted
}

/// Map the artist credits the service reports.
fn credits(entries: &[ArtistCreditPayload]) -> Vec<ArtistCredit> {
    entries
        .iter()
        .filter_map(|entry| {
            let name = entry.name.as_deref()?.trim();
            (!name.is_empty()).then(|| {
                ArtistCredit::new(
                    name.to_owned(),
                    entry
                        .joinphrase
                        .as_deref()
                        .map(str::trim)
                        .filter(|phrase| !phrase.is_empty())
                        .map(str::to_owned),
                )
            })
        })
        .collect()
}

/// Return the first artist name the service reports.
fn first_artist(entries: &[ArtistCreditPayload]) -> Option<String> {
    entries
        .iter()
        .filter_map(|entry| entry.name.as_deref())
        .map(str::trim)
        .find(|name| !name.is_empty())
        .map(str::to_owned)
}

/// Map the genres the service curates, normalized and canonically ordered.
///
/// Only the curated `genres` vocabulary is read; the service's user tags are a
/// different list and are dropped, so a non-genre tag never reaches metadata.
fn genres(entries: &[GenrePayload]) -> Vec<String> {
    let mut genres: Vec<String> = entries
        .iter()
        .map(|entry| normalize(entry.name.as_str()))
        .filter(|name| !name.is_empty())
        .collect();
    genres.sort_unstable();
    genres.dedup();
    genres
}

/// Normalize a vocabulary name so equivalent spellings resolve alike.
fn normalize(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .replace(['-', '/'], " ")
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Map one production-type spelling onto the catalogue's list.
fn mapped_production_type(name: &str) -> Option<ProductionType> {
    match normalize(name).as_str() {
        "album" => Some(ProductionType::Studio),
        "compilation" => Some(ProductionType::Compilation),
        "demo" => Some(ProductionType::Demo),
        "dj mix" => Some(ProductionType::DjMix),
        "live" => Some(ProductionType::Live),
        "mixtape street" => Some(ProductionType::Mixtape),
        "remix" => Some(ProductionType::Remix),
        "soundtrack" => Some(ProductionType::Soundtrack),
        _ => None,
    }
}

/// Map the mediums the service reports, in its order.
fn mediums(entries: &[MediumPayload]) -> Vec<MediumSummary> {
    entries
        .iter()
        .filter_map(|entry| {
            let index = entry.position?;
            let track_count = entry.track_count?;
            let format = match entry
                .format
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                Some(value) if value.eq_ignore_ascii_case("cd") => MediumFormat::Cd,
                Some(value) => MediumFormat::Other(value.to_owned()),
                None => MediumFormat::Other(String::new()),
            };
            Some(MediumSummary::new(format, index, track_count))
        })
        .collect()
}

/// Parse a complete date, treating a partial date or a placeholder as absent.
fn date(value: Option<&str>) -> Option<NaiveDate> {
    let text = value?.trim();
    NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()
}

/// Map a country onto an uppercase ISO 3166-1 alpha-2 code.
///
/// A placeholder such as `N/A` or `[Worldwide]` is absent rather than guessed.
fn country(value: Option<&str>) -> Option<String> {
    let text = value?.trim();
    (text.len() == 2 && text.bytes().all(|byte| byte.is_ascii_alphabetic()))
        .then(|| text.to_ascii_uppercase())
}

/// Return the year of `value` when it starts with four decimal digits.
fn year(value: Option<&str>) -> Option<u32> {
    value?
        .get(..4)
        .and_then(|digits| digits.parse::<u32>().ok())
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::error::Error;
    use std::sync::Mutex as StdMutex;
    use std::time::Duration;
    use std::time::Instant as StdInstant;

    use chrono::NaiveDate;
    use reqwest::StatusCode;
    use rstest::rstest;
    use tracing_test::traced_test;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::Request;
    use wiremock::Respond;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;
    use wiremock::matchers::query_param;

    use crate::domain::model::media_candidate::ExternalRef;
    use crate::domain::model::media_candidate::ProviderId;
    use crate::domain::model::music_album::ProductionType;
    use crate::domain::model::music_candidate::MusicCandidate;
    use crate::domain::model::music_candidate::MusicTarget;
    use crate::domain::model::music_metadata::ArtistCredit;
    use crate::domain::model::music_metadata::MediumFormat;
    use crate::domain::model::music_metadata::MediumSummary;
    use crate::domain::model::music_metadata::MusicMetadata;
    use crate::domain::model::music_metadata::ReleaseStatus;
    use crate::domain::model::music_query::MusicKind;
    use crate::domain::model::music_query::MusicQuery;
    use crate::domain::model::music_query::MusicQueryError;
    use crate::domain::port::music_metadata_provider::MusicMetadataProvider as _;
    use crate::infrastructure::outbound::reqwest::ArtworkClient;

    use super::MUSICBRAINZ_ARTWORK_HOSTS;
    use super::MusicBrainzMetadataProvider;
    use super::USER_AGENT;
    use super::country;
    use super::date;
    use super::genres;
    use super::mapped_production_type;

    /// The album search payload `wiremock` serves.
    const SEARCH_RELEASE_GROUP: &str =
        include_str!("fixtures/musicbrainz/search_release_group.json");

    /// The release search payload `wiremock` serves.
    const SEARCH_RELEASE: &str = include_str!("fixtures/musicbrainz/search_release.json");

    /// The recording search payload `wiremock` serves.
    const SEARCH_RECORDING: &str = include_str!("fixtures/musicbrainz/search_recording.json");

    /// The album lookup payload `wiremock` serves.
    const RELEASE_GROUP: &str = include_str!("fixtures/musicbrainz/release_group.json");

    /// The release lookup payload `wiremock` serves.
    const RELEASE: &str = include_str!("fixtures/musicbrainz/release.json");

    /// The recording lookup payload `wiremock` serves.
    const RECORDING: &str = include_str!("fixtures/musicbrainz/recording.json");

    /// An album search payload that reports no hit.
    const EMPTY_RELEASE_GROUPS: &str = include_str!("fixtures/musicbrainz/search_empty.json");

    /// An album search hit that reports neither an artist nor a date.
    const SEARCH_WITHOUT_ARTIST: &str =
        include_str!("fixtures/musicbrainz/search_without_artist.json");

    /// An album payload whose secondary type qualifies the primary one.
    const RELEASE_GROUP_LIVE_SECONDARY: &str =
        include_str!("fixtures/musicbrainz/release_group_live_secondary.json");

    /// An album payload whose secondary type the catalogue cannot map.
    const RELEASE_GROUP_UNMAPPABLE_SECONDARY: &str =
        include_str!("fixtures/musicbrainz/release_group_unmappable_secondary.json");

    /// The Cover Art Archive index template `wiremock` serves.
    const COVER_ART_INDEX: &str = include_str!("fixtures/musicbrainz/cover_art_index.json");

    /// An archive index naming an image the archive redirects to its download host.
    const COVER_ART_INDEX_REDIRECT: &str =
        include_str!("fixtures/musicbrainz/cover_art_index_redirect.json");

    /// An album payload that lists neither an edition nor a genre.
    const RELEASE_GROUP_WITHOUT_EDITIONS: &str =
        include_str!("fixtures/musicbrainz/release_group_without_editions.json");

    /// A body that is not JSON.
    const MALFORMED: &[u8] = include_bytes!("fixtures/malformed.json");

    /// The 1×1 image `wiremock` serves as artwork.
    const ARTWORK: &[u8] = include_bytes!("fixtures/artwork/pixel.png");

    /// The album identifier every fixture uses.
    const ALBUM_ID: &str = "f5093c06-23e3-404f-aeaa-40f72885ee3a";

    /// The compact-disc release identifier every fixture uses.
    const CD_RELEASE_ID: &str = "a8b6c4d2-2222-4a2b-9c3d-000000000002";

    /// The vinyl release identifier every fixture uses.
    const VINYL_RELEASE_ID: &str = "a8b6c4d2-1111-4a2b-9c3d-000000000001";

    /// The recording identifier every fixture uses.
    const RECORDING_ID: &str = "8f7c9c2a-3333-4a2b-9c3d-000000000003";

    /// Timeout the adapter tests give one request.
    const TEST_TIMEOUT: Duration = Duration::from_secs(5);

    /// Attempt budget the adapter tests configure.
    const TEST_ATTEMPTS: u32 = 3;

    /// A responder serving `statuses` in order and then `body`.
    struct Sequenced {
        /// Body served with every response.
        body: &'static str,
        /// Statuses served in order; the last one repeats once drained.
        statuses: StdMutex<VecDeque<u16>>,
    }

    impl Sequenced {
        /// Build a responder from `statuses` and `body`.
        fn new(statuses: &[u16], body: &'static str) -> Self {
            Self {
                body,
                statuses: StdMutex::new(statuses.iter().copied().collect()),
            }
        }
    }

    impl Respond for Sequenced {
        fn respond(&self, _request: &Request) -> ResponseTemplate {
            let status = self
                .statuses
                .lock()
                .ok()
                .and_then(|mut statuses| statuses.pop_front())
                .unwrap_or(200);
            ResponseTemplate::new(status).set_body_string(self.body)
        }
    }

    /// A provider pointed at `server`, with the archive under its own path.
    fn provider(server: &MockServer) -> MusicBrainzMetadataProvider {
        MusicBrainzMetadataProvider::with_endpoints(
            server.uri(),
            format!("{}/archive", server.uri()),
            TEST_TIMEOUT,
            Duration::ZERO,
            TEST_ATTEMPTS,
        )
        .with_artwork_client(ArtworkClient::for_test("127.0.0.1"))
    }

    /// A provider pointed at `server` that makes one attempt per request.
    fn single_attempt_provider(server: &MockServer) -> MusicBrainzMetadataProvider {
        MusicBrainzMetadataProvider::with_endpoints(
            server.uri(),
            format!("{}/archive", server.uri()),
            TEST_TIMEOUT,
            Duration::ZERO,
            1,
        )
        .with_artwork_client(ArtworkClient::for_test("127.0.0.1"))
    }

    /// A provider pointed at `server` that spaces its requests by `interval`.
    fn spaced_provider(server: &MockServer, interval: Duration) -> MusicBrainzMetadataProvider {
        MusicBrainzMetadataProvider::with_endpoints(
            server.uri(),
            format!("{}/archive", server.uri()),
            TEST_TIMEOUT,
            interval,
            TEST_ATTEMPTS,
        )
        .with_artwork_client(ArtworkClient::for_test("127.0.0.1"))
    }

    /// The query a test searches for.
    ///
    /// # Errors
    ///
    /// Returns the constructor's error when the title is empty, which no test
    /// passes.
    fn query(kind: MusicKind, title: &str) -> Result<MusicQuery, MusicQueryError> {
        MusicQuery::try_new(kind, title.to_owned(), None, None)
    }

    /// Mount a JSON answer on `route`.
    async fn mount_json(server: &MockServer, route: &str, body: &str) {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(server)
            .await;
    }

    /// Return the first candidate of a search.
    ///
    /// # Errors
    ///
    /// Returns an error when the search returned nothing.
    fn first(candidates: &[MusicCandidate]) -> Result<&MusicCandidate, Box<dyn Error>> {
        candidates
            .first()
            .ok_or_else(|| "the search must return a candidate".into())
    }

    #[tokio::test]
    async fn musicbrainz_sends_the_user_agent_and_the_format() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/recording"))
            .and(header("User-Agent", USER_AGENT))
            .and(query_param("fmt", "json"))
            .and(query_param("query", "recording:\"So What\""))
            .respond_with(ResponseTemplate::new(200).set_body_string(SEARCH_RECORDING))
            .expect(1)
            .mount(&server)
            .await;
        let provider = provider(&server);

        // Act
        let candidates = provider
            .search(&query(MusicKind::Recording, "So What")?)
            .await?;

        // Assert
        assert_eq!(candidates.len(), 1);
        Ok(())
    }

    #[test]
    fn musicbrainz_artwork_policy_accepts_the_archive_and_rejects_the_api() {
        // Arrange
        let client = ArtworkClient::new(MUSICBRAINZ_ARTWORK_HOSTS);

        // Act & Assert
        assert!(client.accepts_host("coverartarchive.org"));
        assert!(client.accepts_host("archive.org"));
        assert!(
            client.accepts_host("dn710607.ca.archive.org"),
            "the archive serves each image from its own CDN subdomain"
        );
        assert!(!client.accepts_host("musicbrainz.org"));
        assert!(
            !client.accepts_host("notarchive.org"),
            "a look-alike domain is not an archive subdomain"
        );
        assert!(
            !client.accepts_host("api.themoviedb.org"),
            "another provider's host must not be accepted"
        );
    }

    #[rstest]
    #[case::quote("He said \"Hi\"", "releasegroup:\"He said \\\"Hi\\\"\"")]
    #[case::conjunction("Rock AND Roll", "releasegroup:\"Rock AND Roll\"")]
    #[case::field_operator("Love: Live", "releasegroup:\"Love\\: Live\"")]
    #[case::parenthesised("Album (Deluxe)", "releasegroup:\"Album \\(Deluxe\\)\"")]
    #[tokio::test]
    async fn musicbrainz_escapes_each_query_value(
        #[case] title: &str,
        #[case] expected: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/release-group"))
            .and(query_param("query", expected))
            .respond_with(ResponseTemplate::new(200).set_body_string(EMPTY_RELEASE_GROUPS))
            .expect(1)
            .mount(&server)
            .await;
        let provider = provider(&server);

        // Act
        let candidates = provider.search(&query(MusicKind::Album, title)?).await?;

        // Assert
        assert!(candidates.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn musicbrainz_search_maps_a_release_group_hit() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        mount_json(&server, "/release-group", SEARCH_RELEASE_GROUP).await;
        let provider = provider(&server);

        // Act
        let candidates = provider
            .search(&query(MusicKind::Album, "Kind of Blue")?)
            .await?;

        // Assert
        assert_eq!(candidates.len(), 2, "both hits must be kept, in order");
        let candidate = first(&candidates)?;
        assert_eq!(candidate.kind(), MusicKind::Album);
        assert_eq!(candidate.title(), "Kind of Blue");
        assert_eq!(candidate.artist(), Some("Miles Davis"));
        assert_eq!(candidate.year(), Some(1959));
        assert_eq!(candidate.reference().provider(), ProviderId::MusicBrainz);
        assert_eq!(candidate.reference().id(), ALBUM_ID);
        assert_eq!(
            candidates.last().and_then(MusicCandidate::year),
            Some(2008),
            "the provider order must be preserved"
        );
        Ok(())
    }

    #[tokio::test]
    async fn musicbrainz_search_maps_a_release_hit_with_its_mediums() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let server = MockServer::start().await;
        mount_json(&server, "/release", SEARCH_RELEASE).await;
        let provider = provider(&server);

        // Act
        let candidates = provider
            .search(&query(MusicKind::Release, "Kind of Blue")?)
            .await?;

        // Assert
        assert_eq!(candidates.len(), 2);
        let candidate = first(&candidates)?;
        assert_eq!(candidate.kind(), MusicKind::Release);
        assert_eq!(candidate.status(), Some(ReleaseStatus::Official));
        assert_eq!(
            candidate.album().map(ExternalRef::id),
            Some(ALBUM_ID),
            "a release candidate must name its album"
        );
        assert_eq!(candidate.mediums().len(), 1);
        assert_eq!(
            candidate.mediums().first().map(MediumSummary::format),
            Some(&MediumFormat::Other("12\" Vinyl".to_owned()))
        );
        assert_eq!(
            candidates
                .get(1)
                .and_then(|release| release.mediums().first())
                .map(MediumSummary::format)
                .map(MediumFormat::is_compact_disc),
            Some(true)
        );
        Ok(())
    }

    #[tokio::test]
    async fn musicbrainz_search_maps_a_recording_hit() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        mount_json(&server, "/recording", SEARCH_RECORDING).await;
        let provider = provider(&server);

        // Act
        let candidates = provider
            .search(&query(MusicKind::Recording, "So What")?)
            .await?;

        // Assert
        assert_eq!(candidates.len(), 1);
        let candidate = first(&candidates)?;
        assert_eq!(candidate.kind(), MusicKind::Recording);
        assert_eq!(candidate.title(), "So What");
        assert_eq!(candidate.artist(), Some("Miles Davis"));
        assert_eq!(candidate.year(), Some(1959));
        assert_eq!(candidate.reference().id(), RECORDING_ID);
        Ok(())
    }

    #[tokio::test]
    async fn musicbrainz_search_leaves_an_unreported_artist_absent() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        mount_json(&server, "/release-group", SEARCH_WITHOUT_ARTIST).await;
        let provider = provider(&server);

        // Act
        let candidates = provider
            .search(&query(MusicKind::Album, "Untitled Work")?)
            .await?;

        // Assert
        let candidate = first(&candidates)?;
        assert_eq!(candidate.artist(), None, "an absent artist stays absent");
        assert_eq!(candidate.year(), None, "an absent date stays absent");
        Ok(())
    }

    #[tokio::test]
    async fn musicbrainz_lookup_maps_an_album_and_orders_its_editions() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let server = MockServer::start().await;
        mount_json(
            &server,
            &format!("/release-group/{ALBUM_ID}"),
            RELEASE_GROUP,
        )
        .await;
        Mock::given(method("GET"))
            .and(path(format!("/archive/release-group/{ALBUM_ID}")))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let provider = provider(&server);
        let target = MusicTarget::Album(ExternalRef::new(
            ProviderId::MusicBrainz,
            ALBUM_ID.to_owned(),
        ));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert
        let Some(MusicMetadata::Album {
            artwork,
            artist_credits,
            editions,
            first_release_date,
            genres: mapped_genres,
            production_type,
            title,
        }) = metadata
        else {
            return Err("the album lookup must return album metadata".into());
        };
        assert_eq!(title, "Kind of Blue");
        assert_eq!(artwork, None, "an absent index yields no artwork");
        assert_eq!(
            artist_credits.first().map(ArtistCredit::name),
            Some("Miles Davis")
        );
        assert_eq!(
            first_release_date,
            Some(NaiveDate::from_ymd_opt(1959, 8, 17).ok_or("the date must exist")?)
        );
        assert_eq!(production_type, Some(ProductionType::Studio));
        assert_eq!(mapped_genres, vec!["jazz", "modal jazz"]);
        assert!(
            !mapped_genres.iter().any(|genre| genre == "seen live"),
            "a non-genre tag must be dropped"
        );
        assert_eq!(editions.len(), 2);
        assert_eq!(
            editions.first().map(|edition| edition.reference().id()),
            Some(CD_RELEASE_ID),
            "the compact-disc edition must come first"
        );
        assert_eq!(
            editions.last().map(|edition| edition.reference().id()),
            Some(VINYL_RELEASE_ID)
        );
        assert_eq!(
            editions.first().map(|edition| edition.mediums().len()),
            Some(2)
        );
        assert_eq!(
            editions.first().and_then(|edition| edition.country()),
            Some("GB")
        );
        Ok(())
    }

    #[tokio::test]
    async fn musicbrainz_lookup_maps_a_release_and_reports_absent_values()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        mount_json(&server, &format!("/release/{CD_RELEASE_ID}"), RELEASE).await;
        Mock::given(method("GET"))
            .and(path(format!("/archive/release/{CD_RELEASE_ID}")))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let provider = provider(&server);
        let target = MusicTarget::Release(ExternalRef::new(
            ProviderId::MusicBrainz,
            CD_RELEASE_ID.to_owned(),
        ));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert
        let Some(MusicMetadata::Release {
            album,
            artwork,
            artist_credits,
            edition,
            genres: mapped_genres,
        }) = metadata
        else {
            return Err("the release lookup must return release metadata".into());
        };
        assert_eq!(edition.title(), "Kind of Blue");
        assert_eq!(edition.release_date(), None, "a partial date is absent");
        assert_eq!(edition.country(), None, "a placeholder country is absent");
        assert_eq!(edition.status(), Some(ReleaseStatus::Official));
        assert_eq!(edition.mediums().len(), 2);
        assert_eq!(artwork, None);
        assert_eq!(
            album.map(|reference| reference.id().to_owned()),
            Some(ALBUM_ID.to_owned())
        );
        assert_eq!(
            artist_credits.first().map(ArtistCredit::name),
            Some("Miles Davis")
        );
        assert_eq!(mapped_genres, vec!["cool jazz", "jazz"]);
        Ok(())
    }

    #[tokio::test]
    async fn musicbrainz_lookup_maps_a_recording() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        mount_json(&server, &format!("/recording/{RECORDING_ID}"), RECORDING).await;
        let provider = provider(&server);
        let target = MusicTarget::Recording(ExternalRef::new(
            ProviderId::MusicBrainz,
            RECORDING_ID.to_owned(),
        ));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert
        let Some(MusicMetadata::Recording {
            artist_credits,
            first_release_date,
            genres: mapped_genres,
            isrcs,
            title,
        }) = metadata
        else {
            return Err("the recording lookup must return recording metadata".into());
        };
        assert_eq!(title, "So What");
        assert_eq!(
            first_release_date,
            Some(NaiveDate::from_ymd_opt(1959, 8, 17).ok_or("the date must exist")?)
        );
        assert_eq!(
            artist_credits.first().map(ArtistCredit::name),
            Some("Miles Davis")
        );
        assert_eq!(mapped_genres, vec!["bebop", "jazz"]);
        assert_eq!(isrcs, vec!["USSM15900001", "USSM15900002"]);
        Ok(())
    }

    #[rstest]
    #[case::album("Album", Some(ProductionType::Studio))]
    #[case::compilation("compilation", Some(ProductionType::Compilation))]
    #[case::demo("Demo", Some(ProductionType::Demo))]
    #[case::dj_mix("DJ-mix", Some(ProductionType::DjMix))]
    #[case::live("Live", Some(ProductionType::Live))]
    #[case::mixtape("Mixtape/Street", Some(ProductionType::Mixtape))]
    #[case::remix("Remix", Some(ProductionType::Remix))]
    #[case::soundtrack("soundtrack", Some(ProductionType::Soundtrack))]
    #[case::unmappable("Spokenword", None)]
    fn musicbrainz_production_type_table(
        #[case] name: &str,
        #[case] expected: Option<ProductionType>,
    ) {
        // Act
        let mapped = mapped_production_type(name);

        // Assert
        assert_eq!(mapped, expected);
    }

    /// Look up the album payload `body` describes and return its production type.
    async fn album_production_type(body: &str) -> Result<Option<ProductionType>, Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        mount_json(&server, &format!("/release-group/{ALBUM_ID}"), body).await;
        Mock::given(method("GET"))
            .and(path(format!("/archive/release-group/{ALBUM_ID}")))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let provider = provider(&server);
        let target = MusicTarget::Album(ExternalRef::new(
            ProviderId::MusicBrainz,
            ALBUM_ID.to_owned(),
        ));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert
        let Some(MusicMetadata::Album {
            production_type, ..
        }) = metadata
        else {
            return Err("the album lookup must return album metadata".into());
        };
        Ok(production_type)
    }

    #[tokio::test]
    async fn musicbrainz_secondary_production_type_takes_precedence() -> Result<(), Box<dyn Error>>
    {
        // Act
        let mapped = album_production_type(RELEASE_GROUP_LIVE_SECONDARY).await?;

        // Assert
        assert_eq!(mapped, Some(ProductionType::Live));
        Ok(())
    }

    #[tokio::test]
    async fn musicbrainz_unmappable_secondary_type_stays_absent() -> Result<(), Box<dyn Error>> {
        // Act
        let mapped = album_production_type(RELEASE_GROUP_UNMAPPABLE_SECONDARY).await?;

        // Assert
        assert_eq!(mapped, None, "the secondary type must not fall back");
        Ok(())
    }

    #[test]
    fn musicbrainz_genre_normalisation_folds_case_and_drops_empty_entries() {
        // Arrange
        let entries = vec![
            super::GenrePayload {
                name: "  Cool   Jazz ".to_owned(),
            },
            super::GenrePayload {
                name: "JAZZ".to_owned(),
            },
            super::GenrePayload {
                name: String::new(),
            },
        ];

        // Act
        let mapped = genres(&entries);

        // Assert
        assert_eq!(mapped, vec!["cool jazz", "jazz"]);
    }

    #[test]
    fn musicbrainz_country_and_date_placeholders_are_absent() {
        // Arrange
        let placeholder = Some("N/A");
        let partial_date = Some("1959");

        // Act & Assert
        assert_eq!(country(placeholder), None);
        assert_eq!(country(Some("[Worldwide]")), None);
        assert_eq!(country(Some("us")), Some("US".to_owned()));
        assert_eq!(date(partial_date), None);
        assert_eq!(date(placeholder), None);
        assert_eq!(
            date(Some("1959-08-17")),
            NaiveDate::from_ymd_opt(1959, 8, 17)
        );
    }

    #[rstest]
    #[case::rate_limited(StatusCode::TOO_MANY_REQUESTS, "rate_limited")]
    #[case::service_unavailable(StatusCode::SERVICE_UNAVAILABLE, "rate_limited")]
    #[case::invalid_request(StatusCode::BAD_REQUEST, "invalid_request")]
    #[case::unauthorized(StatusCode::UNAUTHORIZED, "unauthorized")]
    #[case::dependency_failure(StatusCode::INTERNAL_SERVER_ERROR, "dependency_failure")]
    #[tokio::test]
    async fn musicbrainz_classifies_each_status(
        #[case] status: StatusCode,
        #[case] expected: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/release-group"))
            .respond_with(ResponseTemplate::new(status))
            .expect(1)
            .mount(&server)
            .await;
        let provider = single_attempt_provider(&server);

        // Act
        let result = provider
            .search(&query(MusicKind::Album, "Kind of Blue")?)
            .await;

        // Assert
        assert_eq!(
            result.err().map(|error| error.classification()),
            Some(expected)
        );
        Ok(())
    }

    #[tokio::test]
    async fn musicbrainz_lookup_returns_none_for_an_unknown_mbid() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(format!("/release-group/{ALBUM_ID}")))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&server)
            .await;
        let provider = provider(&server);
        let target = MusicTarget::Album(ExternalRef::new(
            ProviderId::MusicBrainz,
            ALBUM_ID.to_owned(),
        ));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert
        assert_eq!(metadata, None);
        Ok(())
    }

    #[tokio::test]
    async fn musicbrainz_retries_a_rate_limited_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/release-group"))
            .respond_with(Sequenced::new(&[429, 200], SEARCH_RELEASE_GROUP))
            .expect(2)
            .mount(&server)
            .await;
        let provider = provider(&server);

        // Act
        let candidates = provider
            .search(&query(MusicKind::Album, "Kind of Blue")?)
            .await?;

        // Assert
        assert_eq!(candidates.len(), 2, "the retry must return the payload");
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn musicbrainz_exhausts_the_retry_budget_and_logs_once() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/release-group"))
            .respond_with(Sequenced::new(&[429, 429, 429, 200], SEARCH_RELEASE_GROUP))
            .expect(3)
            .mount(&server)
            .await;
        let provider = provider(&server);

        // Act
        let result = provider
            .search(&query(MusicKind::Album, "Kind of Blue")?)
            .await;

        // Assert
        assert_eq!(
            result.err().map(|error| error.classification()),
            Some("rate_limited")
        );
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"port_fault\""))
                .count();
            let carries_content = lines
                .iter()
                .any(|line| line.contains("Kind of Blue") || line.contains("Miles"));
            (count == 1 && !carries_content)
                .then_some(())
                .ok_or_else(|| {
                    format!(
                        "exactly one give-up event carrying no content must be logged, \
                         found {count}"
                    )
                })
        });
        Ok(())
    }

    #[tokio::test]
    async fn musicbrainz_does_not_retry_an_invalid_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/release-group"))
            .respond_with(ResponseTemplate::new(400))
            .expect(1)
            .mount(&server)
            .await;
        let provider = provider(&server);

        // Act
        let result = provider
            .search(&query(MusicKind::Album, "Kind of Blue")?)
            .await;

        // Assert
        assert_eq!(
            result.err().map(|error| error.classification()),
            Some("invalid_request")
        );
        Ok(())
    }

    #[tokio::test]
    async fn musicbrainz_does_not_retry_an_undecodable_payload() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/release-group"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(MALFORMED.to_vec()))
            .expect(1)
            .mount(&server)
            .await;
        let provider = provider(&server);

        // Act
        let result = provider
            .search(&query(MusicKind::Album, "Kind of Blue")?)
            .await;

        // Assert
        assert_eq!(
            result.err().map(|error| error.classification()),
            Some("deserialization_failure")
        );
        Ok(())
    }

    #[tokio::test]
    async fn musicbrainz_spaces_sequential_requests() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/release-group"))
            .respond_with(Sequenced::new(&[200], EMPTY_RELEASE_GROUPS))
            .expect(2)
            .mount(&server)
            .await;
        let interval = Duration::from_millis(80);
        let provider = spaced_provider(&server, interval);

        // Act
        let started = StdInstant::now();
        provider.search(&query(MusicKind::Album, "first")?).await?;
        provider.search(&query(MusicKind::Album, "second")?).await?;

        // Assert
        assert!(
            started.elapsed() >= interval,
            "the second request must wait for the interval"
        );
        Ok(())
    }

    #[tokio::test]
    async fn musicbrainz_serialises_concurrent_requests() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/release-group"))
            .respond_with(Sequenced::new(&[200], EMPTY_RELEASE_GROUPS))
            .expect(2)
            .mount(&server)
            .await;
        let interval = Duration::from_millis(80);
        let provider = spaced_provider(&server, interval);
        let first = query(MusicKind::Album, "first")?;
        let second = query(MusicKind::Album, "second")?;

        // Act
        let started = StdInstant::now();
        let (first_result, second_result) =
            tokio::join!(provider.search(&first), provider.search(&second));

        // Assert
        first_result?;
        second_result?;
        assert!(
            started.elapsed() >= interval,
            "concurrent callers must queue behind the same slot"
        );
        Ok(())
    }

    #[tokio::test]
    async fn musicbrainz_artwork_follows_the_cover_art_json_hop() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        mount_json(
            &server,
            &format!("/release-group/{ALBUM_ID}"),
            RELEASE_GROUP,
        )
        .await;
        let index = COVER_ART_INDEX.replace("{origin}", &server.uri());
        mount_json(
            &server,
            &format!("/archive/release-group/{ALBUM_ID}"),
            &index,
        )
        .await;
        Mock::given(method("GET"))
            .and(path("/cover.jpg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(ARTWORK.to_vec()))
            .expect(1)
            .mount(&server)
            .await;
        let provider = provider(&server);
        let target = MusicTarget::Album(ExternalRef::new(
            ProviderId::MusicBrainz,
            ALBUM_ID.to_owned(),
        ));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert
        let Some(MusicMetadata::Album { artwork, .. }) = metadata else {
            return Err("the album lookup must return album metadata".into());
        };
        let downloaded = artwork.ok_or("the archive image must be downloaded")?;
        assert_eq!(downloaded.bytes(), ARTWORK);
        assert_eq!(downloaded.media_type(), "image/png");
        Ok(())
    }

    #[tokio::test]
    async fn musicbrainz_lookup_maps_an_album_without_editions() -> Result<(), Box<dyn Error>> {
        // Arrange: the payload lists no edition and no genre, and its date is partial.
        let server = MockServer::start().await;
        mount_json(
            &server,
            &format!("/release-group/{ALBUM_ID}"),
            RELEASE_GROUP_WITHOUT_EDITIONS,
        )
        .await;
        Mock::given(method("GET"))
            .and(path(format!("/archive/release-group/{ALBUM_ID}")))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let provider = provider(&server);
        let target = MusicTarget::Album(ExternalRef::new(
            ProviderId::MusicBrainz,
            ALBUM_ID.to_owned(),
        ));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert
        let Some(MusicMetadata::Album {
            editions,
            first_release_date,
            genres: mapped_genres,
            production_type,
            ..
        }) = metadata
        else {
            return Err("the album lookup must return album metadata".into());
        };
        assert!(editions.is_empty(), "no edition listed is an empty list");
        assert!(mapped_genres.is_empty(), "no genre listed is an empty list");
        assert_eq!(first_release_date, None, "a partial date stays absent");
        assert_eq!(production_type, Some(ProductionType::Studio));
        Ok(())
    }

    #[tokio::test]
    async fn musicbrainz_lookup_ignores_another_providers_reference() -> Result<(), Box<dyn Error>>
    {
        // Arrange: a reference issued by a video provider must not be resolved here.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(RELEASE))
            .expect(0)
            .mount(&server)
            .await;
        let provider = provider(&server);
        let target = MusicTarget::Release(ExternalRef::new(ProviderId::Tmdb, "603".to_owned()));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert
        assert_eq!(metadata, None, "only the reference's provider may answer");
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn musicbrainz_lookup_reports_a_provider_failure_without_another_provider()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the only provider fails, so nothing else may be asked.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(format!("/recording/{RECORDING_ID}")))
            .respond_with(ResponseTemplate::new(400))
            .expect(1)
            .mount(&server)
            .await;
        let provider = single_attempt_provider(&server);
        let target = MusicTarget::Recording(ExternalRef::new(
            ProviderId::MusicBrainz,
            RECORDING_ID.to_owned(),
        ));

        // Act
        let result = provider.lookup(&target).await;

        // Assert
        assert_eq!(
            result.err().map(|error| error.classification()),
            Some("invalid_request")
        );
        Ok(())
    }

    #[tokio::test]
    async fn musicbrainz_artwork_follows_the_archives_redirect_hop() -> Result<(), Box<dyn Error>> {
        // Arrange: the index names an address the archive answers with a redirect.
        let server = MockServer::start().await;
        mount_json(
            &server,
            &format!("/release-group/{ALBUM_ID}"),
            RELEASE_GROUP,
        )
        .await;
        let index = COVER_ART_INDEX_REDIRECT.replace("{origin}", &server.uri());
        mount_json(
            &server,
            &format!("/archive/release-group/{ALBUM_ID}"),
            &index,
        )
        .await;
        Mock::given(method("GET"))
            .and(path("/archive-image.jpg"))
            .respond_with(
                ResponseTemplate::new(307)
                    .insert_header("Location", format!("{}/cover.jpg", server.uri())),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/cover.jpg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(ARTWORK.to_vec()))
            .expect(1)
            .mount(&server)
            .await;
        let provider = provider(&server);
        let target = MusicTarget::Album(ExternalRef::new(
            ProviderId::MusicBrainz,
            ALBUM_ID.to_owned(),
        ));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert
        let Some(MusicMetadata::Album { artwork, .. }) = metadata else {
            return Err("the album lookup must return album metadata".into());
        };
        let downloaded = artwork.ok_or("the redirected image must be downloaded")?;
        assert_eq!(downloaded.bytes(), ARTWORK);
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn musicbrainz_artwork_refuses_a_hop_outside_the_allow_list() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the archive points the download at a host it does not own.
        let server = MockServer::start().await;
        mount_json(
            &server,
            &format!("/release-group/{ALBUM_ID}"),
            RELEASE_GROUP,
        )
        .await;
        let index = COVER_ART_INDEX_REDIRECT.replace("{origin}", &server.uri());
        mount_json(
            &server,
            &format!("/archive/release-group/{ALBUM_ID}"),
            &index,
        )
        .await;
        Mock::given(method("GET"))
            .and(path("/archive-image.jpg"))
            .respond_with(
                ResponseTemplate::new(307)
                    .insert_header("Location", "https://elsewhere.test/cover.jpg"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let provider = provider(&server);
        let target = MusicTarget::Album(ExternalRef::new(
            ProviderId::MusicBrainz,
            ALBUM_ID.to_owned(),
        ));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert
        let Some(MusicMetadata::Album { artwork, .. }) = metadata else {
            return Err("the album lookup must return album metadata".into());
        };
        assert_eq!(artwork, None, "a foreign hop must not be followed");
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"metadata_artwork_failed\""))
                .count();
            (count == 1)
                .then_some(())
                .ok_or_else(|| format!("exactly one artwork event must be logged, found {count}"))
        });
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn musicbrainz_artwork_absence_yields_none_and_one_event() -> Result<(), Box<dyn Error>> {
        // Arrange
        let server = MockServer::start().await;
        mount_json(
            &server,
            &format!("/release-group/{ALBUM_ID}"),
            RELEASE_GROUP,
        )
        .await;
        Mock::given(method("GET"))
            .and(path(format!("/archive/release-group/{ALBUM_ID}")))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&server)
            .await;
        let provider = provider(&server);
        let target = MusicTarget::Album(ExternalRef::new(
            ProviderId::MusicBrainz,
            ALBUM_ID.to_owned(),
        ));

        // Act
        let metadata = provider.lookup(&target).await?;

        // Assert
        let Some(MusicMetadata::Album { artwork, .. }) = metadata else {
            return Err("the album lookup must return album metadata".into());
        };
        assert_eq!(artwork, None);
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"metadata_artwork_failed\""))
                .count();
            let carries_content = lines
                .iter()
                .any(|line| line.contains("Kind of Blue") || line.contains("Miles"));
            (count == 1 && !carries_content)
                .then_some(())
                .ok_or_else(|| {
                    format!(
                        "exactly one artwork event carrying no content must be logged, \
                         found {count}"
                    )
                })
        });
        Ok(())
    }
}
