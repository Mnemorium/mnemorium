use std::collections::HashMap;
use std::collections::HashSet;
use std::future::Future;
use std::path::Component;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use crate::application::port::ingest_media::IngestMediaCommand;
use crate::application::port::ingest_media::IngestMediaError;
use crate::application::port::ingest_media::IngestMediaResponse;
use crate::application::port::ingest_media::IngestMediaUseCase;
use crate::application::port::ingest_media::IngestRequest;
use crate::application::security_event;
use crate::application::use_case::upload_layout::media_path;
use crate::domain::alias::NumericID;
use crate::domain::model::audio::Audio;
use crate::domain::model::audio::ProbedAudio;
use crate::domain::model::file::File;
use crate::domain::model::image::Image;
use crate::domain::model::image::ProbedImage;
use crate::domain::model::video::ProbedVideo;
use crate::domain::model::video::Video;
use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::audio_repository::AudioFilter;
use crate::domain::port::audio_repository::AudioRepository as _;
use crate::domain::port::content_type_sniffer::ContentTypeSniffer;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::file_repository::FileFilter;
use crate::domain::port::file_repository::FileRepository as _;
use crate::domain::port::image_repository::ImageFilter;
use crate::domain::port::image_repository::ImageRepository as _;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::media_probe::MediaProbe;
use crate::domain::port::media_source::MediaEntry;
use crate::domain::port::media_source::MediaSource;
use crate::domain::port::mime_type_repository::MimeTypeRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::user_repository::UserFilter;
use crate::domain::port::user_repository::UserRepository as _;
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::port::video_repository::VideoFilter;
use crate::domain::port::video_repository::VideoRepository as _;

/// Identifier of the Root Admin, who owns a file the server did not store.
const ROOT_ADMIN_USER_ID: NumericID = 0;

/// Verdict for one examined file, folded into the aggregate report.
enum Verdict {
    /// The registered file's size or modification instant differs.
    Changed,
    /// A registered file's missing media row was recorded.
    Completed,
    /// The registered file is no longer under the root.
    Missing,
    /// The file and its media row were recorded.
    Registered,
    /// The file was not registered; the ingest treats it as a non-failure.
    Skipped,
    /// The registered file matches its record.
    Untouched,
}

/// Counter accumulator folded into an [`IngestMediaResponse`].
#[derive(Default)]
struct Counts {
    /// Registered files that differ.
    changed: u64,
    /// Missing media rows completed.
    completed: u64,
    /// Registered files no longer found.
    missing: u64,
    /// Files registered.
    registered: u64,
    /// Files skipped.
    skipped: u64,
    /// Registered files that matched.
    untouched: u64,
}

impl Counts {
    /// Fold the counters into the public report.
    fn finish(self) -> IngestMediaResponse {
        IngestMediaResponse::new(
            self.registered,
            self.completed,
            self.untouched,
            self.changed,
            self.missing,
            self.skipped,
        )
    }

    /// Record one verdict.
    fn record(&mut self, verdict: &Verdict) {
        let counter = match *verdict {
            Verdict::Registered => &mut self.registered,
            Verdict::Completed => &mut self.completed,
            Verdict::Untouched => &mut self.untouched,
            Verdict::Changed => &mut self.changed,
            Verdict::Missing => &mut self.missing,
            Verdict::Skipped => &mut self.skipped,
        };
        *counter = counter.saturating_add(1);
    }
}

/// Media family the ingest accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MediaKind {
    /// An `audio/*` file.
    Audio,
    /// An `image/*` file.
    Image,
    /// A `video/*` file.
    Video,
}

/// Probed metadata of one accepted file.
enum MediaRow {
    /// Audio metadata.
    Audio(ProbedAudio),
    /// Image metadata.
    Image(ProbedImage),
    /// Video metadata.
    Video(ProbedVideo),
}

/// Use case implementation reconciling the library root with the records.
///
/// One unit of work per file: a single unprobeable or unrepresentable file costs
/// one skipped file instead of the whole scan. The scan and the change observer
/// drive the same reconciliation, so the startup walk is the authority and the
/// observer is an accelerator.
pub struct IngestMedia<F, N, P, S> {
    /// Probe extracting the media metadata.
    media_probe: Arc<P>,
    /// Source discovering the files under the library root.
    media_source: Arc<S>,
    /// Sniffer determining a file's media type from its content.
    sniffer: Arc<N>,
    /// Factory opening the unit of work wrapping one file.
    unit_of_work_factory: Arc<F>,
}

impl<F, N, P, S> IngestMedia<F, N, P, S>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
    N: ContentTypeSniffer,
    P: MediaProbe,
    S: MediaSource,
{
    /// Insert the missing media row of an already-registered file.
    async fn backfill(&self, file: File, media: MediaRow) -> Result<Verdict, IngestMediaError> {
        let mut unit_of_work = self.begin().await?;
        match self.insert_media_row(&mut unit_of_work, &file, media).await {
            Ok(()) => {
                unit_of_work
                    .commit()
                    .await
                    .map_err(|error| IngestMediaError::Unknown(error.into()))?;
                Ok(Verdict::Completed)
            }
            Err(IngestMediaError::Unknown(error))
                if error.chain().any(|cause| {
                    matches!(
                        cause.downcast_ref::<RepositoryError>(),
                        Some(RepositoryError::AlreadyExist)
                    )
                }) =>
            {
                discard(unit_of_work).await;
                Ok(Verdict::Untouched)
            }
            // A media row the datastore rejects is unrepresentable rather than
            // fatal: one skipped file must never stop the scan.
            Err(IngestMediaError::Unknown(error))
                if error.chain().any(|cause| {
                    matches!(
                        cause.downcast_ref::<RepositoryError>(),
                        Some(RepositoryError::DataIntegrityViolation)
                    )
                }) =>
            {
                discard(unit_of_work).await;
                Ok(Verdict::Skipped)
            }
            Err(error) => {
                discard(unit_of_work).await;
                Err(error)
            }
        }
    }

    /// Open a unit of work.
    async fn begin(&self) -> Result<F::Uow, IngestMediaError> {
        self.unit_of_work_factory
            .begin()
            .await
            .map_err(|error| IngestMediaError::Unknown(error.into()))
    }

    /// Return the registered file recorded at `path`, if any.
    async fn find(&self, path: &str) -> Result<Option<File>, IngestMediaError> {
        let mut unit_of_work = self.begin().await?;
        let found = unit_of_work
            .files()
            .search(&FileFilter {
                path: Some(path.to_owned()),
                ..FileFilter::default()
            })
            .await
            .map_err(|error| IngestMediaError::Unknown(error.into()))?
            .into_iter()
            .next();
        discard(unit_of_work).await;
        Ok(found)
    }

    /// Report one observed or removed path.
    async fn handle_path(
        &self,
        path: &str,
        supported: &HashSet<String>,
    ) -> Result<Verdict, IngestMediaError> {
        match self
            .media_source
            .stat(path)
            .await
            .map_err(|error| IngestMediaError::Unknown(error.into()))?
        {
            Some(entry) => {
                self.reconcile(
                    &entry,
                    self.find(&entry.relative_path).await?.as_ref(),
                    supported,
                )
                .await
            }
            None => self.report_absent(path).await,
        }
    }

    /// Insert the media row `media` describes for `file`.
    async fn insert_media_row<U>(
        &self,
        unit_of_work: &mut U,
        file: &File,
        media: MediaRow,
    ) -> Result<(), IngestMediaError>
    where
        U: LibraryUnitOfWork,
    {
        let name = Path::new(file.path()).file_name().map_or_else(
            || file.path().to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        match media {
            MediaRow::Image(probed) => {
                let pending = Image::try_new(
                    0,
                    name,
                    probed.width_px,
                    probed.height_px,
                    probed.orientation,
                    Utc::now().date_naive(),
                    file.id(),
                )
                .map_err(|error| IngestMediaError::Unknown(anyhow::Error::new(error)))?;
                unit_of_work
                    .images()
                    .create(pending)
                    .await
                    .map(|_created| ())
                    .map_err(|error| IngestMediaError::Unknown(error.into()))
            }
            MediaRow::Video(probed) => {
                let pending = Video::try_new(
                    0,
                    probed.duration_ms,
                    probed.codec,
                    probed.frame_count,
                    probed.width,
                    probed.height,
                    probed.color_id,
                    probed.scan_type,
                    file.id(),
                )
                .map_err(|error| IngestMediaError::Unknown(anyhow::Error::new(error)))?;
                unit_of_work
                    .videos()
                    .create(pending)
                    .await
                    .map(|_created| ())
                    .map_err(|error| IngestMediaError::Unknown(error.into()))
            }
            MediaRow::Audio(probed) => {
                let pending = Audio::try_new(
                    0,
                    name,
                    probed.duration_ms,
                    probed.codec,
                    probed.sample_rate_hz,
                    probed.bit_depth,
                    probed.block_size,
                    file.id(),
                )
                .map_err(|error| IngestMediaError::Unknown(anyhow::Error::new(error)))?;
                unit_of_work
                    .audios()
                    .create(pending)
                    .await
                    .map(|_created| ())
                    .map_err(|error| IngestMediaError::Unknown(error.into()))
            }
        }
    }

    /// Materialise the registered files, keyed by their relative path, and the
    /// supported mime type set.
    ///
    /// The two reads share one unit of work. The supported set is the seeded
    /// `mime_type` table: the sniffer only reports what the content is, and the
    /// ingest decides whether that type is representable.
    async fn load_snapshot(
        &self,
    ) -> Result<(HashMap<String, File>, HashSet<String>), IngestMediaError> {
        let mut unit_of_work = self.begin().await?;
        let files = unit_of_work
            .files()
            .search(&FileFilter::default())
            .await
            .map_err(|error| IngestMediaError::Unknown(error.into()))?;
        let supported = unit_of_work
            .mime_types()
            .all_ids()
            .await
            .map_err(|error| IngestMediaError::Unknown(error.into()))?;
        discard(unit_of_work).await;
        Ok((
            files
                .into_iter()
                .map(|file| (file.path().to_owned(), file))
                .collect(),
            supported.into_iter().collect(),
        ))
    }

    /// Load the supported mime type set for a single-path operation.
    async fn load_supported_mime_types(&self) -> Result<HashSet<String>, IngestMediaError> {
        let mut unit_of_work = self.begin().await?;
        let supported = unit_of_work
            .mime_types()
            .all_ids()
            .await
            .map_err(|error| IngestMediaError::Unknown(error.into()))?;
        discard(unit_of_work).await;
        Ok(supported.into_iter().collect())
    }

    /// Return whether `file` already carries the media row its type requires.
    async fn media_row_exists(
        &self,
        file: &File,
        kind: MediaKind,
    ) -> Result<bool, IngestMediaError> {
        let mut unit_of_work = self.begin().await?;
        let exists = match kind {
            MediaKind::Image => !unit_of_work
                .images()
                .search(&ImageFilter {
                    file_id: Some(file.id()),
                    ..ImageFilter::default()
                })
                .await
                .map_err(|error| IngestMediaError::Unknown(error.into()))?
                .is_empty(),
            MediaKind::Video => !unit_of_work
                .videos()
                .search(&VideoFilter {
                    file_id: Some(file.id()),
                    ..VideoFilter::default()
                })
                .await
                .map_err(|error| IngestMediaError::Unknown(error.into()))?
                .is_empty(),
            MediaKind::Audio => !unit_of_work
                .audios()
                .search(&AudioFilter {
                    file_id: Some(file.id()),
                    ..AudioFilter::default()
                })
                .await
                .map_err(|error| IngestMediaError::Unknown(error.into()))?
                .is_empty(),
        };
        discard(unit_of_work).await;
        Ok(exists)
    }

    /// Create a new use case.
    #[must_use]
    pub fn new(
        unit_of_work_factory: Arc<F>,
        media_source: Arc<S>,
        sniffer: Arc<N>,
        media_probe: Arc<P>,
    ) -> Self {
        Self {
            media_probe,
            media_source,
            sniffer,
            unit_of_work_factory,
        }
    }

    /// Probe `relative` for the metadata the media row needs.
    ///
    /// Every probe failure is a skipped file, never a scan failure: unsuitable
    /// content must not abort the walk.
    async fn probe(&self, kind: MediaKind, relative: &str) -> Option<MediaRow> {
        let path = media_path(relative);
        match kind {
            MediaKind::Image => self
                .media_probe
                .probe_image(&path)
                .await
                .ok()
                .flatten()
                .map(MediaRow::Image),
            MediaKind::Video => self
                .media_probe
                .probe_video(&path)
                .await
                .ok()
                .flatten()
                .map(MediaRow::Video),
            MediaKind::Audio => self
                .media_probe
                .probe_audio(&path)
                .await
                .ok()
                .flatten()
                .map(MediaRow::Audio),
        }
    }

    /// Reconcile one entry: verify it when registered, register it otherwise.
    async fn reconcile(
        &self,
        entry: &MediaEntry,
        registered: Option<&File>,
        supported: &HashSet<String>,
    ) -> Result<Verdict, IngestMediaError> {
        let Some(file) = registered else {
            return self.register(entry, supported).await;
        };

        // Identity is location plus size plus modification instant: the scan
        // never hashes.
        if file.size_bytes() != entry.size_bytes || file.modified_at() != entry.modified_at {
            return Ok(Verdict::Changed);
        }

        let Some(kind) = media_kind(file.mime_type_id()) else {
            return Ok(Verdict::Untouched);
        };
        if self.media_row_exists(file, kind).await? {
            return Ok(Verdict::Untouched);
        }

        // Backfill: a file row with no media row for its mime type is probed
        // and completed, which also repairs an uploaded audio file.
        let Some(media) = self.probe(kind, file.path()).await else {
            return Ok(Verdict::Skipped);
        };
        self.backfill(file.clone(), media).await
    }

    /// Register a file the root holds but the server has no record of.
    async fn register(
        &self,
        entry: &MediaEntry,
        supported: &HashSet<String>,
    ) -> Result<Verdict, IngestMediaError> {
        let absolute = media_path(&entry.relative_path);
        // A file the sniffer cannot read is an unreadable file, not a scan
        // failure: skip it and keep walking.
        let Ok(Some(mime_type_id)) = self.sniffer.sniff(&absolute).await else {
            return Ok(Verdict::Skipped);
        };
        let Some(kind) = media_kind(&mime_type_id) else {
            return Ok(Verdict::Skipped);
        };
        // The caller owns the accept-list. `infer` reports any type it
        // recognises; the datastore's `mime_type` table is the supported set,
        // so an unseeded type is skipped as unsupported rather than attempted
        // and rejected by the foreign key.
        if !supported.contains(&mime_type_id) {
            return Ok(Verdict::Skipped);
        }
        let Some(media) = self.probe(kind, &entry.relative_path).await else {
            return Ok(Verdict::Skipped);
        };

        let mut unit_of_work = self.begin().await?;
        let Some(root_admin) = unit_of_work
            .users()
            .search(&UserFilter {
                id: Some(ROOT_ADMIN_USER_ID),
                ..UserFilter::default()
            })
            .await
            .map_err(|error| IngestMediaError::Unknown(error.into()))?
            .into_iter()
            .next()
        else {
            // Without the Root Admin there is no owner to attribute the file
            // to; the scan skips it rather than failing.
            discard(unit_of_work).await;
            return Ok(Verdict::Skipped);
        };

        let pending = File::try_new(
            0,
            entry.relative_path.clone(),
            root_admin.id(),
            false,
            mime_type_id,
            Utc::now().date_naive(),
            entry.size_bytes,
            entry.modified_at,
            None,
            None,
        )
        .map_err(|error| IngestMediaError::Unknown(anyhow::Error::new(error)))?;

        let created = unit_of_work.files().create(pending).await;
        let file = match created {
            Ok(file) => file,
            // A concurrent driver registered the path first, or the file is
            // unrepresentable: either way this one file is skipped rather than
            // aborting the scan.
            Err(RepositoryError::AlreadyExist | RepositoryError::DataIntegrityViolation) => {
                discard(unit_of_work).await;
                return Ok(Verdict::Skipped);
            }
            Err(error) => {
                discard(unit_of_work).await;
                return Err(IngestMediaError::Unknown(error.into()));
            }
        };

        match self.insert_media_row(&mut unit_of_work, &file, media).await {
            Ok(()) => {
                unit_of_work
                    .commit()
                    .await
                    .map_err(|error| IngestMediaError::Unknown(error.into()))?;
                Ok(Verdict::Registered)
            }
            // The media row could not be represented: roll the file row back
            // and skip this one file rather than abort the entire scan.
            Err(IngestMediaError::Unknown(error))
                if error.chain().any(|cause| {
                    matches!(
                        cause.downcast_ref::<RepositoryError>(),
                        Some(RepositoryError::DataIntegrityViolation)
                    )
                }) =>
            {
                discard(unit_of_work).await;
                Ok(Verdict::Skipped)
            }
            Err(error) => {
                discard(unit_of_work).await;
                Err(error)
            }
        }
    }

    /// Move a recorded location, preserving the media row.
    async fn rename(&self, from: &str, to: &str) -> Result<Verdict, IngestMediaError> {
        let Some(file) = self.find(from).await? else {
            // Nothing was recorded at the source: treat the destination as an
            // arrival rather than as the move of a record we do not have.
            let supported = self.load_supported_mime_types().await?;
            return self.handle_path(to, &supported).await;
        };
        let path = Path::new(to);
        if path.as_os_str().is_empty()
            || !path
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
        {
            return Ok(Verdict::Skipped);
        }

        let mut unit_of_work = self.begin().await?;
        let mut renamed = file;
        if let Err(error) = renamed.set_path(to.to_owned()) {
            discard(unit_of_work).await;
            return Err(IngestMediaError::Unknown(anyhow::Error::new(error)));
        }
        let saved = unit_of_work.files().save(renamed).await;
        match saved {
            Ok(_saved) => {
                unit_of_work
                    .commit()
                    .await
                    .map_err(|error| IngestMediaError::Unknown(error.into()))?;
                Ok(Verdict::Untouched)
            }
            Err(RepositoryError::AlreadyExist) => {
                discard(unit_of_work).await;
                Ok(Verdict::Skipped)
            }
            Err(error) => {
                discard(unit_of_work).await;
                Err(IngestMediaError::Unknown(error.into()))
            }
        }
    }

    /// Report a path that no longer resolves as missing when it was recorded.
    async fn report_absent(&self, path: &str) -> Result<Verdict, IngestMediaError> {
        Ok(match self.find(path).await? {
            Some(_file) => Verdict::Missing,
            None => Verdict::Skipped,
        })
    }
}

impl<F, N, P, S> IngestMediaUseCase for IngestMedia<F, N, P, S>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
    N: ContentTypeSniffer,
    P: MediaProbe,
    S: MediaSource,
{
    fn execute<'future>(
        &'future self,
        command: IngestMediaCommand,
    ) -> Pin<Box<dyn Future<Output = Result<IngestMediaResponse, IngestMediaError>> + Send + 'future>>
    {
        Box::pin(async move {
            let mut counts = Counts::default();
            let request = command.request().clone();
            match request {
                IngestRequest::Scan => {
                    let Some(entries) = self
                        .media_source
                        .walk()
                        .await
                        .map_err(|error| IngestMediaError::Unknown(error.into()))?
                    else {
                        // An absent root registers nothing; the caller warns.
                        let report = counts.finish();
                        report_counts(&report);
                        return Ok(report);
                    };
                    let (registered, supported) = self.load_snapshot().await?;
                    let mut walked = HashSet::new();
                    for entry in &entries {
                        walked.insert(entry.relative_path.clone());
                        counts.record(
                            &self
                                .reconcile(entry, registered.get(&entry.relative_path), &supported)
                                .await?,
                        );
                    }
                    let missing = registered
                        .keys()
                        .filter(|path| !walked.contains(*path))
                        .count();
                    counts.missing = u64::try_from(missing).unwrap_or(u64::MAX);
                }
                IngestRequest::Path(path) => {
                    let supported = self.load_supported_mime_types().await?;
                    counts.record(&self.handle_path(&path, &supported).await?);
                }
                IngestRequest::Rename { from, to } => {
                    counts.record(&self.rename(&from, &to).await?);
                }
                IngestRequest::Remove(path) => {
                    counts.record(&self.report_absent(&path).await?);
                }
            }
            let report = counts.finish();
            report_counts(&report);
            Ok(report)
        })
    }
}

/// Emit the catalogued aggregate outcome of one ingest operation.
fn report_counts(report: &IngestMediaResponse) {
    security_event::media_ingest(
        report.registered(),
        report.completed(),
        report.untouched(),
        report.changed(),
        report.missing(),
        report.skipped(),
    );
}

/// Roll back a unit of work, preserving the operation's own outcome.
async fn discard<U>(unit_of_work: U)
where
    U: UnitOfWork,
{
    // The unit-of-work adapter owns the rollback-failure log (OBS-002).
    let _rollback = unit_of_work.rollback().await;
}

/// Return the media family `mime_type_id` denotes, if the ingest accepts it.
fn media_kind(mime_type_id: &str) -> Option<MediaKind> {
    if mime_type_id.starts_with("image/") {
        Some(MediaKind::Image)
    } else if mime_type_id.starts_with("video/") {
        Some(MediaKind::Video)
    } else if mime_type_id.starts_with("audio/") {
        Some(MediaKind::Audio)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

    use chrono::NaiveDate;
    use chrono::NaiveDateTime;
    use tracing_test::traced_test;

    use crate::application::port::ingest_media::IngestMediaCommand;
    use crate::application::port::ingest_media::IngestMediaUseCase as _;
    use crate::application::port::ingest_media::IngestRequest;
    use crate::domain::model::audio::Audio;
    use crate::domain::model::audio::AudioError;
    use crate::domain::model::audio::ProbedAudio;
    use crate::domain::model::file::File;
    use crate::domain::model::file::FileError;
    use crate::domain::model::file::Origin;
    use crate::domain::model::image::Image;
    use crate::domain::model::image::Orientation;
    use crate::domain::model::image::ProbedImage;
    use crate::domain::model::user::Role;
    use crate::domain::model::user::User;
    use crate::domain::model::video::ProbedVideo;
    use crate::domain::model::video::ScanType;
    use crate::domain::port::audio_repository::AudioFilter;
    use crate::domain::port::content_type_sniffer::MockContentTypeSniffer;
    use crate::domain::port::error::ContentTypeSnifferError;
    use crate::domain::port::error::MediaProbeError;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::image_repository::ImageFilter;
    use crate::domain::port::media_probe::MockMediaProbe;
    use crate::domain::port::media_source::MediaEntry;
    use crate::domain::port::media_source::MockMediaSource;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUow;

    use super::IngestMedia;

    type UseCase =
        IngestMedia<TestFactory, MockContentTypeSniffer, MockMediaProbe, MockMediaSource>;

    /// Fixture modification instant.
    fn instant() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2026-10-28 10:00:00", "%Y-%m-%d %H:%M:%S")
            .unwrap_or_default()
    }

    /// Build a walked entry.
    fn entry(path: &str, size_bytes: i64) -> MediaEntry {
        MediaEntry {
            modified_at: instant(),
            relative_path: path.to_owned(),
            size_bytes,
        }
    }

    /// Build a persisted file fixture, attributed to the Root Admin.
    fn registered_file(
        path: &str,
        mime_type_id: &str,
        size_bytes: i64,
    ) -> Result<File, Box<dyn Error>> {
        Ok(File::try_new(
            7,
            path.to_owned(),
            0,
            false,
            mime_type_id.to_owned(),
            NaiveDate::from_ymd_opt(2026, 10, 28).unwrap_or_default(),
            size_bytes,
            instant(),
            None,
            None,
        )?)
    }

    /// Build a prioritizing unit of work that stores the file and its media row.
    fn storing_uow(
        path: &'static str,
        mime_type_id: &'static str,
        size_bytes: i64,
    ) -> Result<(TestUow, Arc<AtomicBool>), Box<dyn Error>> {
        let root = User::try_new(0, "root".to_owned(), None, 0, Role::Admin)?;
        let mut uow = TestUow::default();
        uow.users.expect_search().times(1).returning(move |_| {
            let found = root.clone();
            Box::pin(async move { Ok(vec![found]) })
        });
        uow.files
            .expect_create()
            .times(1)
            .withf(move |file| {
                file.origin() == Origin::Scanned
                    && file.user_id() == 0
                    && file.size_bytes() == size_bytes
                    && file.modified_at() == instant()
                    && file.mime_type_id() == mime_type_id
                    && file.integrity_hash().is_none()
            })
            .returning(move |_| {
                let stored = registered_file(path, mime_type_id, size_bytes)
                    .map_err(|_| RepositoryError::OperationFailed);
                Box::pin(async move { stored })
            });
        let committed = Arc::clone(&uow.committed);
        Ok((uow, committed))
    }

    /// Build a reading unit of work returning `files`.
    fn reading_uow(files: Vec<File>) -> TestUow {
        let mut uow = TestUow::default();
        uow.files.expect_search().times(1).returning(move |_| {
            let found = files.clone();
            Box::pin(async move { Ok(found) })
        });
        uow
    }

    /// The mime types the scan fixtures consider supported.
    fn supported_mime_types() -> Vec<String> {
        vec![
            "audio/mpeg".to_owned(),
            "image/png".to_owned(),
            "video/mp4".to_owned(),
        ]
    }

    /// Build a snapshot unit of work returning `files` and the supported set.
    fn snapshot_uow(files: Vec<File>) -> TestUow {
        let mut uow = reading_uow(files);
        uow.mime_types
            .expect_all_ids()
            .times(1)
            .returning(|| Box::pin(async { Ok(supported_mime_types()) }));
        uow
    }

    /// Build a unit of work returning only the supported mime type set.
    fn mime_types_uow() -> TestUow {
        let mut uow = TestUow::default();
        uow.mime_types
            .expect_all_ids()
            .times(1)
            .returning(|| Box::pin(async { Ok(supported_mime_types()) }));
        uow
    }

    /// Build a factory handing out `unit_of_works` in order.
    fn factory(unit_of_works: Vec<TestUow>) -> Arc<TestFactory> {
        Arc::new(TestFactory {
            unit_of_works: Mutex::new(unit_of_works),
        })
    }

    /// Build the use case under test.
    fn use_case(
        factory: Arc<TestFactory>,
        source: MockMediaSource,
        sniffer: MockContentTypeSniffer,
        probe: MockMediaProbe,
    ) -> UseCase {
        IngestMedia::new(
            factory,
            Arc::new(source),
            Arc::new(sniffer),
            Arc::new(probe),
        )
    }

    /// Expect the walk to report `entries`.
    fn expect_walk(source: &mut MockMediaSource, entries: Vec<MediaEntry>) {
        source.expect_walk().times(1).returning(move || {
            let found = entries.clone();
            Box::pin(async move { Ok(Some(found)) })
        });
    }

    /// Expect the sniffer to report `mime_type_id`.
    fn expect_sniff(sniffer: &mut MockContentTypeSniffer, mime_type_id: &'static str) {
        sniffer
            .expect_sniff()
            .times(1)
            .returning(move |_| Box::pin(async move { Ok(Some(mime_type_id.to_owned())) }));
    }

    /// Build a probed image.
    fn probed_image() -> ProbedImage {
        ProbedImage {
            height_px: 480,
            orientation: Orientation::Landscape,
            width_px: 640,
        }
    }

    /// Build a probed video.
    fn probed_video() -> ProbedVideo {
        ProbedVideo {
            codec: "h264".to_owned(),
            color_id: "YCbCr".to_owned(),
            duration_ms: 1_000.0f64,
            frame_count: 30,
            height: 480,
            scan_type: ScanType::Progressive,
            width: 640,
        }
    }

    /// Build a probed audio row with absent measurements.
    fn probed_audio() -> ProbedAudio {
        ProbedAudio {
            bit_depth: None,
            block_size: None,
            codec: "mp3".to_owned(),
            duration_ms: 1_000.0f64,
            sample_rate_hz: 44_100.0f64,
        }
    }

    /// Return the content type the fixture's relative path implies.
    fn fixture_mime_type(path: &Path) -> &'static str {
        if path.to_string_lossy().ends_with("track.flac") {
            "audio/x-flac"
        } else {
            "image/png"
        }
    }

    /// Return the sniff outcome for the unreadable-file fixture.
    fn sniff_fixture(path: &Path) -> Result<Option<String>, ContentTypeSnifferError> {
        if path.to_string_lossy().ends_with("secret.png") {
            return Err(ContentTypeSnifferError::OperationFailed);
        }
        Ok(Some("image/png".to_owned()))
    }

    #[tokio::test]
    async fn scan_registers_an_image_with_its_file_and_media_row() -> Result<(), Box<dyn Error>> {
        // Arrange
        let (mut storing, committed) = storing_uow("photo.png", "image/png", 3)?;
        storing
            .images
            .expect_create()
            .times(1)
            .withf(|image: &Image| {
                image.width_px() == 640 && image.height_px() == 480 && image.name() == "photo.png"
            })
            .returning(|image| Box::pin(async move { Ok(image) }));
        let mut source = MockMediaSource::new();
        expect_walk(&mut source, vec![entry("photo.png", 3)]);
        let mut sniffer = MockContentTypeSniffer::new();
        expect_sniff(&mut sniffer, "image/png");
        let mut probe = MockMediaProbe::new();
        probe
            .expect_probe_image()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(probed_image())) }));
        let use_case = use_case(
            factory(vec![snapshot_uow(Vec::new()), storing]),
            source,
            sniffer,
            probe,
        );

        // Act
        let report = use_case
            .execute(IngestMediaCommand::new(IngestRequest::Scan))
            .await?;

        // Assert: the stored file is attributed to the Root Admin, marked as
        // scanned, and records both measurements.
        assert_eq!(report.registered(), 1);
        assert!(committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn scan_registers_a_video_with_its_media_row() -> Result<(), Box<dyn Error>> {
        // Arrange
        let (mut storing, committed) = storing_uow("clip.mp4", "video/mp4", 4)?;
        storing
            .videos
            .expect_create()
            .times(1)
            .withf(|video| video.width() == 640 && video.frame_count() == 30)
            .returning(|video| Box::pin(async move { Ok(video) }));
        let mut source = MockMediaSource::new();
        expect_walk(&mut source, vec![entry("clip.mp4", 4)]);
        let mut sniffer = MockContentTypeSniffer::new();
        expect_sniff(&mut sniffer, "video/mp4");
        let mut probe = MockMediaProbe::new();
        probe
            .expect_probe_video()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(probed_video())) }));
        let use_case = use_case(
            factory(vec![snapshot_uow(Vec::new()), storing]),
            source,
            sniffer,
            probe,
        );

        // Act
        let report = use_case
            .execute(IngestMediaCommand::new(IngestRequest::Scan))
            .await?;

        // Assert
        assert_eq!(report.registered(), 1);
        assert!(committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn scan_registers_an_audio_file_with_absent_measurements() -> Result<(), Box<dyn Error>> {
        // Arrange
        let (mut storing, committed) = storing_uow("track.mp3", "audio/mpeg", 5)?;
        storing
            .audios
            .expect_create()
            .times(1)
            .withf(|audio| audio.bit_depth().is_none() && audio.block_size().is_none())
            .returning(|audio| Box::pin(async move { Ok(audio) }));
        let mut source = MockMediaSource::new();
        expect_walk(&mut source, vec![entry("track.mp3", 5)]);
        let mut sniffer = MockContentTypeSniffer::new();
        expect_sniff(&mut sniffer, "audio/mpeg");
        let mut probe = MockMediaProbe::new();
        probe
            .expect_probe_audio()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(probed_audio())) }));
        let use_case = use_case(
            factory(vec![snapshot_uow(Vec::new()), storing]),
            source,
            sniffer,
            probe,
        );

        // Act
        let report = use_case
            .execute(IngestMediaCommand::new(IngestRequest::Scan))
            .await?;

        // Assert
        assert_eq!(report.registered(), 1);
        assert!(committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn scan_skips_a_file_that_is_not_media() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut source = MockMediaSource::new();
        expect_walk(&mut source, vec![entry("notes.txt", 3)]);
        let mut sniffer = MockContentTypeSniffer::new();
        expect_sniff(&mut sniffer, "text/plain");
        let mut probe = MockMediaProbe::new();
        probe.expect_probe_image().times(0);
        let use_case = use_case(
            factory(vec![snapshot_uow(Vec::new())]),
            source,
            sniffer,
            probe,
        );

        // Act
        let report = use_case
            .execute(IngestMediaCommand::new(IngestRequest::Scan))
            .await?;

        // Assert
        assert_eq!(report.registered(), 0);
        assert_eq!(report.skipped(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn scan_skips_an_image_the_probe_cannot_decode() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut source = MockMediaSource::new();
        expect_walk(&mut source, vec![entry("broken.png", 3)]);
        let mut sniffer = MockContentTypeSniffer::new();
        expect_sniff(&mut sniffer, "image/png");
        let mut probe = MockMediaProbe::new();
        probe
            .expect_probe_image()
            .times(1)
            .returning(|_| Box::pin(async { Err(MediaProbeError::UnsupportedMedia) }));
        let use_case = use_case(
            factory(vec![snapshot_uow(Vec::new())]),
            source,
            sniffer,
            probe,
        );

        // Act
        let report = use_case
            .execute(IngestMediaCommand::new(IngestRequest::Scan))
            .await?;

        // Assert: neither the file nor the media row is recorded.
        assert_eq!(report.registered(), 0);
        assert_eq!(report.skipped(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn scan_skips_a_file_whose_probe_times_out() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut source = MockMediaSource::new();
        expect_walk(&mut source, vec![entry("slow.mp4", 3)]);
        let mut sniffer = MockContentTypeSniffer::new();
        expect_sniff(&mut sniffer, "video/mp4");
        let mut probe = MockMediaProbe::new();
        probe
            .expect_probe_video()
            .times(1)
            .returning(|_| Box::pin(async { Err(MediaProbeError::Timeout) }));
        let use_case = use_case(
            factory(vec![snapshot_uow(Vec::new())]),
            source,
            sniffer,
            probe,
        );

        // Act
        let report = use_case
            .execute(IngestMediaCommand::new(IngestRequest::Scan))
            .await?;

        // Assert: a timeout skips one file, it does not abort the scan.
        assert_eq!(report.skipped(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn scan_skips_an_unsupported_mime_type_without_probing() -> Result<(), Box<dyn Error>> {
        // Arrange: the sniffer recognises the content, but the datastore's
        // supported set does not contain `audio/x-flac`.
        let (mut storing, committed) = storing_uow("photo.png", "image/png", 3)?;
        storing
            .images
            .expect_create()
            .times(1)
            .returning(|image| Box::pin(async move { Ok(image) }));
        let mut source = MockMediaSource::new();
        expect_walk(
            &mut source,
            vec![entry("track.flac", 5), entry("photo.png", 3)],
        );
        let mut sniffer = MockContentTypeSniffer::new();
        sniffer.expect_sniff().times(2).returning(|path| {
            let mime_type = fixture_mime_type(path);
            Box::pin(async move { Ok(Some(mime_type.to_owned())) })
        });
        let mut probe = MockMediaProbe::new();
        probe.expect_probe_audio().times(0);
        probe
            .expect_probe_image()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(probed_image())) }));
        let use_case = use_case(
            factory(vec![snapshot_uow(Vec::new()), storing]),
            source,
            sniffer,
            probe,
        );

        // Act
        let report = use_case
            .execute(IngestMediaCommand::new(IngestRequest::Scan))
            .await?;

        // Assert: the unsupported type is skipped before any probe, and the
        // supported file is still registered.
        assert_eq!(report.skipped(), 1);
        assert_eq!(report.registered(), 1);
        assert!(committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn scan_skips_a_file_the_sniffer_cannot_read() -> Result<(), Box<dyn Error>> {
        // Arrange: one file cannot be read, its neighbour can.
        let (mut storing, committed) = storing_uow("photo.png", "image/png", 3)?;
        storing
            .images
            .expect_create()
            .times(1)
            .returning(|image| Box::pin(async move { Ok(image) }));
        let mut source = MockMediaSource::new();
        expect_walk(
            &mut source,
            vec![entry("secret.png", 3), entry("photo.png", 3)],
        );
        let mut sniffer = MockContentTypeSniffer::new();
        sniffer.expect_sniff().times(2).returning(|path| {
            let result = sniff_fixture(path);
            Box::pin(async move { result })
        });
        let mut probe = MockMediaProbe::new();
        probe
            .expect_probe_image()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(probed_image())) }));
        let use_case = use_case(
            factory(vec![snapshot_uow(Vec::new()), storing]),
            source,
            sniffer,
            probe,
        );

        // Act
        let report = use_case
            .execute(IngestMediaCommand::new(IngestRequest::Scan))
            .await?;

        // Assert: the unreadable file is skipped, the scan continues.
        assert_eq!(report.skipped(), 1);
        assert_eq!(report.registered(), 1);
        assert!(committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn scan_skips_a_file_whose_registration_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange: the datastore rejects the first file's insert.
        let root = User::try_new(0, "root".to_owned(), None, 0, Role::Admin)?;
        let mut rejecting = TestUow::default();
        rejecting
            .users
            .expect_search()
            .times(1)
            .returning(move |_| {
                let found = root.clone();
                Box::pin(async move { Ok(vec![found]) })
            });
        rejecting
            .files
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::DataIntegrityViolation) }));
        let (mut storing, committed) = storing_uow("good.png", "image/png", 3)?;
        storing
            .images
            .expect_create()
            .times(1)
            .returning(|image| Box::pin(async move { Ok(image) }));
        let mut source = MockMediaSource::new();
        expect_walk(&mut source, vec![entry("bad.png", 3), entry("good.png", 3)]);
        let mut sniffer = MockContentTypeSniffer::new();
        sniffer
            .expect_sniff()
            .times(2)
            .returning(|_| Box::pin(async { Ok(Some("image/png".to_owned())) }));
        let mut probe = MockMediaProbe::new();
        probe
            .expect_probe_image()
            .times(2)
            .returning(|_| Box::pin(async { Ok(Some(probed_image())) }));
        let use_case = use_case(
            factory(vec![snapshot_uow(Vec::new()), rejecting, storing]),
            source,
            sniffer,
            probe,
        );

        // Act
        let report = use_case
            .execute(IngestMediaCommand::new(IngestRequest::Scan))
            .await?;

        // Assert: the rejected file is skipped, and the next file still
        // registers.
        assert_eq!(report.skipped(), 1);
        assert_eq!(report.registered(), 1);
        assert!(committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn scan_reports_a_matching_file_as_untouched() -> Result<(), Box<dyn Error>> {
        // Arrange
        let file = registered_file("photo.png", "image/png", 3)?;
        let mut media = TestUow::default();
        media
            .images
            .expect_search()
            .times(1)
            .withf(|filter: &ImageFilter| filter.file_id == Some(7))
            .returning(|_| {
                let image = Image::try_new(
                    3,
                    "photo.png".to_owned(),
                    640,
                    480,
                    Orientation::Landscape,
                    NaiveDate::from_ymd_opt(2026, 10, 28).unwrap_or_default(),
                    7,
                )
                .map_err(|_| RepositoryError::DataIntegrityViolation);
                Box::pin(async move { image.map(|found| vec![found]) })
            });
        let mut source = MockMediaSource::new();
        expect_walk(&mut source, vec![entry("photo.png", 3)]);
        let use_case = use_case(
            factory(vec![snapshot_uow(vec![file]), media]),
            source,
            MockContentTypeSniffer::new(),
            MockMediaProbe::new(),
        );

        // Act
        let report = use_case
            .execute(IngestMediaCommand::new(IngestRequest::Scan))
            .await?;

        // Assert
        assert_eq!(report.untouched(), 1);
        assert_eq!(report.registered(), 0);
        Ok(())
    }

    #[tokio::test]
    async fn scan_reports_a_changed_file_without_writing() -> Result<(), Box<dyn Error>> {
        // Arrange: the recorded size differs from the file's.
        let file = registered_file("photo.png", "image/png", 99)?;
        let mut source = MockMediaSource::new();
        expect_walk(&mut source, vec![entry("photo.png", 3)]);
        let use_case = use_case(
            factory(vec![snapshot_uow(vec![file])]),
            source,
            MockContentTypeSniffer::new(),
            MockMediaProbe::new(),
        );

        // Act
        let report = use_case
            .execute(IngestMediaCommand::new(IngestRequest::Scan))
            .await?;

        // Assert
        assert_eq!(report.changed(), 1);
        assert_eq!(report.registered(), 0);
        Ok(())
    }

    #[tokio::test]
    async fn scan_reports_a_registered_file_that_is_gone() -> Result<(), Box<dyn Error>> {
        // Arrange: the root no longer holds the file the record names.
        let file = registered_file("gone.png", "image/png", 3)?;
        let mut source = MockMediaSource::new();
        expect_walk(&mut source, Vec::new());
        let use_case = use_case(
            factory(vec![snapshot_uow(vec![file])]),
            source,
            MockContentTypeSniffer::new(),
            MockMediaProbe::new(),
        );

        // Act
        let report = use_case
            .execute(IngestMediaCommand::new(IngestRequest::Scan))
            .await?;

        // Assert: the record is retained and reported, not deleted.
        assert_eq!(report.missing(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn scan_completes_a_registered_audio_file_without_its_row() -> Result<(), Box<dyn Error>>
    {
        // Arrange: an uploaded audio file has a file row but no audio row.
        let file = registered_file("track.mp3", "audio/mpeg", 5)?;
        let mut media = TestUow::default();
        media
            .audios
            .expect_search()
            .times(1)
            .withf(|filter: &AudioFilter| filter.file_id == Some(7))
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut storing = TestUow::default();
        storing
            .audios
            .expect_create()
            .times(1)
            .withf(|audio| audio.file_id() == 7 && audio.bit_depth().is_none())
            .returning(|audio| Box::pin(async move { Ok(audio) }));
        let committed = Arc::clone(&storing.committed);
        let mut source = MockMediaSource::new();
        expect_walk(&mut source, vec![entry("track.mp3", 5)]);
        let mut probe = MockMediaProbe::new();
        probe
            .expect_probe_audio()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(probed_audio())) }));
        let use_case = use_case(
            factory(vec![snapshot_uow(vec![file]), media, storing]),
            source,
            MockContentTypeSniffer::new(),
            probe,
        );

        // Act
        let report = use_case
            .execute(IngestMediaCommand::new(IngestRequest::Scan))
            .await?;

        // Assert: completed, not untouched.
        assert_eq!(report.completed(), 1);
        assert_eq!(report.untouched(), 0);
        assert!(committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn path_request_registers_a_file_that_appeared() -> Result<(), Box<dyn Error>> {
        // Arrange
        let (mut storing, committed) = storing_uow("new.png", "image/png", 3)?;
        storing
            .images
            .expect_create()
            .times(1)
            .returning(|image| Box::pin(async move { Ok(image) }));
        let mut source = MockMediaSource::new();
        source
            .expect_stat()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(entry("new.png", 3))) }));
        let mut sniffer = MockContentTypeSniffer::new();
        expect_sniff(&mut sniffer, "image/png");
        let mut probe = MockMediaProbe::new();
        probe
            .expect_probe_image()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(probed_image())) }));
        let use_case = use_case(
            factory(vec![mime_types_uow(), reading_uow(Vec::new()), storing]),
            source,
            sniffer,
            probe,
        );

        // Act
        let report = use_case
            .execute(IngestMediaCommand::new(IngestRequest::Path(
                "new.png".to_owned(),
            )))
            .await?;

        // Assert
        assert_eq!(report.registered(), 1);
        assert!(committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn rename_updates_the_recorded_location_in_place() -> Result<(), Box<dyn Error>> {
        // Arrange
        let file = registered_file("old.png", "image/png", 3)?;
        let mut writing = TestUow::default();
        writing
            .files
            .expect_save()
            .times(1)
            .withf(|saved| saved.id() == 7 && saved.path() == "new.png")
            .returning(|saved| Box::pin(async move { Ok(saved) }));
        let committed = Arc::clone(&writing.committed);
        let use_case = use_case(
            factory(vec![reading_uow(vec![file]), writing]),
            MockMediaSource::new(),
            MockContentTypeSniffer::new(),
            MockMediaProbe::new(),
        );

        // Act
        let report = use_case
            .execute(IngestMediaCommand::new(IngestRequest::Rename {
                from: "old.png".to_owned(),
                to: "new.png".to_owned(),
            }))
            .await?;

        // Assert
        assert_eq!(report.untouched(), 1);
        assert!(committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn remove_reports_missing_and_retains_the_record() -> Result<(), Box<dyn Error>> {
        // Arrange: a removal event names a registered file.
        let file = registered_file("gone.png", "image/png", 3)?;
        let use_case = use_case(
            factory(vec![reading_uow(vec![file])]),
            MockMediaSource::new(),
            MockContentTypeSniffer::new(),
            MockMediaProbe::new(),
        );

        // Act
        let report = use_case
            .execute(IngestMediaCommand::new(IngestRequest::Remove(
                "gone.png".to_owned(),
            )))
            .await?;

        // Assert: the row is retained (no write unit of work was available).
        assert_eq!(report.missing(), 1);
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn ingest_reports_aggregate_counts_without_a_file_name() -> Result<(), Box<dyn Error>> {
        // Arrange: one skipped file named in the fixture.
        let mut source = MockMediaSource::new();
        expect_walk(&mut source, vec![entry("private-notes.txt", 3)]);
        let mut sniffer = MockContentTypeSniffer::new();
        expect_sniff(&mut sniffer, "text/plain");
        let use_case = use_case(
            factory(vec![snapshot_uow(Vec::new())]),
            source,
            sniffer,
            MockMediaProbe::new(),
        );
        // Act
        let report = use_case
            .execute(IngestMediaCommand::new(IngestRequest::Scan))
            .await?;

        // Assert: the counters are present and no fixture name reached the sink.
        assert_eq!(report.skipped(), 1);
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"ingest\""))
                .count();
            (count == 1)
                .then_some(())
                .ok_or_else(|| format!("expected exactly one ingest event, found {count}"))
        });
        assert!(logs_contain("registered=0"), "wrong registered count");
        assert!(logs_contain("completed=0"), "missing completed counter");
        assert!(logs_contain("untouched=0"), "missing untouched counter");
        assert!(logs_contain("changed=0"), "missing changed counter");
        assert!(logs_contain("missing=0"), "missing missing counter");
        assert!(logs_contain("skipped=1"), "wrong skipped count");
        assert!(
            !logs_contain("private-notes.txt"),
            "a file name reached the sink"
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_file_without_a_digest_defaults_its_origin_to_scanned() -> Result<(), Box<dyn Error>>
    {
        // Act
        let stored = File::try_new(
            1,
            "film.mp4".to_owned(),
            2,
            false,
            "video/mp4".to_owned(),
            NaiveDate::from_ymd_opt(2026, 10, 28).unwrap_or_default(),
            42,
            instant(),
            None,
            None,
        )?;

        // Assert: an unlabelled row must never answer that the server stored it.
        assert!(stored.integrity_hash().is_none());
        assert_eq!(stored.origin(), Origin::Scanned);
        assert_eq!(stored.size_bytes(), 42);
        Ok(())
    }

    #[tokio::test]
    async fn a_file_rejects_a_negative_size() {
        // Act
        let result = File::try_new(
            1,
            "film.mp4".to_owned(),
            2,
            false,
            "video/mp4".to_owned(),
            NaiveDate::from_ymd_opt(2026, 10, 28).unwrap_or_default(),
            -1,
            instant(),
            None,
            None,
        );

        // Assert
        assert!(matches!(result, Err(FileError::InvalidSizeBytes)));
    }

    #[tokio::test]
    async fn an_audio_row_accepts_absent_and_non_power_of_two_measurements()
    -> Result<(), Box<dyn Error>> {
        // Act: `ffprobe` reports 1152 for MP3, which the old rule rejected.
        let present = Audio::try_new(
            1,
            "track.mp3".to_owned(),
            1_000.0,
            "mp3".to_owned(),
            44_100.0,
            Some(16),
            Some(1152),
            2,
        )?;
        let absent = Audio::try_new(
            2,
            "track.mp3".to_owned(),
            1_000.0,
            "mp3".to_owned(),
            44_100.0,
            None,
            None,
            3,
        )?;

        // Assert
        assert_eq!(present.bit_depth(), Some(16));
        assert_eq!(present.block_size(), Some(1152));
        assert!(absent.bit_depth().is_none());
        assert!(absent.block_size().is_none());
        Ok(())
    }

    #[tokio::test]
    async fn an_audio_row_rejects_present_and_invalid_measurements() {
        // Act
        let depth = Audio::try_new(
            1,
            "track.mp3".to_owned(),
            1_000.0,
            "mp3".to_owned(),
            44_100.0,
            Some(0),
            None,
            2,
        );
        let block = Audio::try_new(
            2,
            "track.mp3".to_owned(),
            1_000.0,
            "mp3".to_owned(),
            44_100.0,
            None,
            Some(-1),
            3,
        );

        // Assert
        assert!(matches!(depth, Err(AudioError::InvalidBitDepth)));
        assert!(matches!(block, Err(AudioError::InvalidBlockSize)));
    }
}
