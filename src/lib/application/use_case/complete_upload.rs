use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use crate::application::port::complete_upload::CompleteUploadCommand;
use crate::application::port::complete_upload::CompleteUploadError;
use crate::application::port::complete_upload::CompleteUploadResponse;
use crate::application::port::complete_upload::CompleteUploadUseCase;
use crate::application::security_event;
use crate::application::use_case::upload_layout::final_path;
use crate::application::use_case::upload_layout::is_path_safe;
use crate::application::use_case::upload_layout::relative_final;
use crate::application::use_case::upload_layout::staged_path;
use crate::application::use_case::upload_session::caller_file_id;
use crate::application::use_case::upload_session::expiry;
use crate::domain::alias::NumericID;
use crate::domain::model::file::File;
use crate::domain::model::image::Image;
use crate::domain::model::integrity_hash::IntegrityHash;
use crate::domain::model::integrity_hash::SHA256_HEX_LENGTH;
use crate::domain::model::upload::Upload;
use crate::domain::model::video::Video;
use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::error::MediaProbeError;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::file_repository::FileRepository as _;
use crate::domain::port::file_storage::FileStorage;
use crate::domain::port::library_unit_of_work::LibraryUnitOfWork;
use crate::domain::port::media_probe::MediaProbe;
use crate::domain::port::media_probe::ProbedImage;
use crate::domain::port::media_probe::ProbedVideo;
use crate::domain::port::media_repository::MediaRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::upload_repository::UploadFilter;
use crate::domain::port::upload_repository::UploadRepository as _;

/// Outcome of the business logic wrapped by the unit-of-work lifecycle.
enum Flow<T, E> {
    /// The upload expired: its row and staged file have been deleted, so the
    /// unit of work must be committed before returning the error.
    Expired,
    /// The business logic failed and the unit of work must be rolled back.
    Failed(E),
    /// The business logic succeeded.
    Succeeded(T),
}

/// Outcome of resolving and validating an upload inside a unit of work.
///
/// Both phases resolve through this outcome so they apply the same owner,
/// expiry, finished and completeness rules; the second application is the
/// time-of-check/time-of-use re-check.
enum Resolution {
    /// The upload expired: its staged file and row have been deleted, so the
    /// unit of work must be committed before reporting the expiry.
    Expired,
    /// The upload could not be resolved or validated: the unit of work must be
    /// rolled back and the error returned.
    Failed(CompleteUploadError),
    /// The upload is already finished: carries the caller's matching file.
    Finished(NumericID),
    /// The upload is complete and ready to have its staged content verified.
    /// Boxed so it does not dominate the other variants.
    Ready(Box<Upload>),
}

/// Paths of a promoted file, so a failed completion can restore it.
struct Promoted {
    /// Absolute path of the promoted file.
    final_path: PathBuf,
    /// Absolute path of the staging file to restore to.
    staged_path: PathBuf,
}

/// Kind of media an upload's mime type denotes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MediaKind {
    /// An `image/*` file.
    Image,
    /// A `video/*` file.
    Video,
}

impl MediaKind {
    /// Classify `mime_type_id`, or `None` when the file is neither image nor
    /// video (a completion without media registration).
    #[expect(
        clippy::single_call_fn,
        reason = "the mime classification is named after the rule it encodes"
    )]
    fn from_mime_type_id(mime_type_id: &str) -> Option<Self> {
        if mime_type_id.starts_with("image/") {
            Some(Self::Image)
        } else if mime_type_id.starts_with("video/") {
            Some(Self::Video)
        } else {
            None
        }
    }
}

/// The probed metadata of a media upload.
enum Probed {
    /// Image metadata.
    Image(ProbedImage),
    /// Video metadata.
    Video(ProbedVideo),
}

/// Use case implementation for completing an upload session.
///
/// Completion spans two bounded contexts: for a supported image or video the
/// Asset completion writes and the Library media insert share one mutating unit
/// of work, so the file is never committed without its media row (`ARCH-003`,
/// `STY-RUST-045`). A file whose mime type is another family completes without a
/// media row.
pub struct CompleteUpload<F, S, P> {
    /// Lifetime of an upload session, in seconds.
    expiry_seconds: u64,
    /// Storage adapter promoting the staged file.
    file_storage: Arc<S>,
    /// Probe extracting the media metadata.
    media_probe: Arc<P>,
    /// Root directory holding the upload and file folders.
    root: PathBuf,
    /// Factory opening the unit of work wrapping the completion.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, S: FileStorage, P: MediaProbe> CompleteUpload<F, S, P> {
    /// Create a new use case.
    #[must_use]
    pub fn new(
        unit_of_work_factory: Arc<F>,
        file_storage: Arc<S>,
        media_probe: Arc<P>,
        root: PathBuf,
        expiry_seconds: u64,
    ) -> Self {
        Self {
            expiry_seconds,
            file_storage,
            media_probe,
            root,
            unit_of_work_factory,
        }
    }
}

impl<F, S, P> CompleteUploadUseCase for CompleteUpload<F, S, P>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork + LibraryUnitOfWork,
    S: FileStorage,
    P: MediaProbe,
{
    #[expect(
        clippy::too_many_lines,
        reason = "the two-phase completion plus media registration is one cohesive flow"
    )]
    fn execute<'future>(
        &'future self,
        command: CompleteUploadCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<CompleteUploadResponse, CompleteUploadError>>
                + Send
                + 'future,
        >,
    > {
        let expiry_seconds = self.expiry_seconds;
        let file_storage = Arc::clone(&self.file_storage);
        let media_probe = Arc::clone(&self.media_probe);
        let root = self.root.clone();
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            // Phase 1: a short, read-only unit of work that resolves and
            // validates the upload. It is closed before the staged content is
            // hashed, releasing the pooled connection and holding no read
            // snapshot across the hash.
            let staged_media = {
                let mut unit_of_work = unit_of_work_factory
                    .begin()
                    .await
                    .map_err(|error| CompleteUploadError::Unknown(error.into()))?;
                match resolve(
                    &mut unit_of_work,
                    &file_storage,
                    &root,
                    &command,
                    expiry_seconds,
                )
                .await
                {
                    Resolution::Ready(upload) => {
                        // Nothing was written on this path: roll back to release
                        // the connection before hashing. The media kind and the
                        // staged path are kept so the probe runs with no unit of
                        // work open.
                        let staged_media = MediaKind::from_mime_type_id(upload.mime_type_id())
                            .map(|kind| (kind, staged_path(&root, upload.upload_id())));
                        discard(unit_of_work).await;
                        staged_media
                    }
                    Resolution::Finished(file_id) => {
                        // Idempotent completion: the upload finished since the
                        // caller's last attempt; return the caller's file.
                        discard(unit_of_work).await;
                        security_event::upload(
                            command.user_id(),
                            &command.upload_id().to_string(),
                            "completed",
                        );
                        return Ok(CompleteUploadResponse::new(file_id, true));
                    }
                    Resolution::Expired => {
                        // The expired upload row and staged file were already
                        // deleted inside the transaction; commit so the deletion
                        // survives, then report the expiry. A commit failure is a
                        // server error and takes precedence.
                        unit_of_work
                            .commit()
                            .await
                            .map_err(|error| CompleteUploadError::Unknown(error.into()))?;
                        security_event::suspicious_business_logic(
                            command.user_id(),
                            "complete",
                            "expired",
                        );
                        return Err(CompleteUploadError::Expired);
                    }
                    Resolution::Failed(error) => {
                        discard(unit_of_work).await;
                        if matches!(&error, CompleteUploadError::Incomplete) {
                            security_event::suspicious_business_logic(
                                command.user_id(),
                                "complete",
                                "not_complete",
                            );
                        }
                        return Err(error);
                    }
                }
            };

            // The staged content is hashed with no unit of work open, so the
            // pooled connection is free and a concurrent writer cannot
            // invalidate a read snapshot underneath the hash.
            let computed =
                compute_integrity_hash(&file_storage, &root, command.upload_id()).await?;

            // The media probe runs with no unit of work open, alongside the
            // hash, so the 30-second probe never holds a pooled connection.
            let probed = match staged_media {
                Some((kind, path)) => Some(probe_media(&*media_probe, kind, &path).await?),
                None => None,
            };

            // Phase 2: a fresh mutating unit of work. The upload is re-read and
            // re-validated (a time-of-check/time-of-use re-check) before the
            // computed digest is compared and the completion is persisted.
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| CompleteUploadError::Unknown(error.into()))?;
            let mut promoted: Option<Promoted> = None;

            let flow = async {
                let upload = match resolve(
                    &mut unit_of_work,
                    &file_storage,
                    &root,
                    &command,
                    expiry_seconds,
                )
                .await
                {
                    Resolution::Ready(upload) => *upload,
                    Resolution::Finished(file_id) => {
                        return Flow::Succeeded(CompleteUploadResponse::new(file_id, true));
                    }
                    Resolution::Expired => return Flow::Expired,
                    Resolution::Failed(error) => return Flow::Failed(error),
                };

                // The computed digest is compared before any file-table lookup,
                // so a caller cannot probe the file table for a guessed hash
                // without first proving it holds the matching content.
                if computed != *upload.integrity_hash() {
                    return Flow::Failed(CompleteUploadError::IntegrityMismatch);
                }

                // Deduplication is caller-scoped: a caller who already owns a
                // file with this digest gets that file back and no new copy is
                // stored. The session is left unfinished and the staged file is
                // left for the reaper.
                match caller_file_id(&mut unit_of_work, &upload, command.user_id()).await {
                    Ok(Some(file_id)) => {
                        return Flow::Succeeded(CompleteUploadResponse::new(file_id, false));
                    }
                    Ok(None) => {}
                    Err(error) => {
                        return Flow::Failed(CompleteUploadError::Unknown(error.into()));
                    }
                }

                if !is_path_safe(upload.file_name()) {
                    security_event::application_error("complete_upload");
                    return Flow::Failed(CompleteUploadError::Unknown(anyhow::anyhow!(
                        "the upload file name is not a safe path segment"
                    )));
                }
                let staged = staged_path(&root, upload.upload_id());
                let final_file = final_path(&root, upload.upload_id(), upload.file_name());
                match file_storage.promote(&staged, &final_file).await {
                    // The staged file vanished after it was hashed: the upload
                    // is absent, mirroring the repository's missing-entity
                    // outcome rather than a storage failure.
                    Ok(false) => return Flow::Failed(CompleteUploadError::NoSuchUpload),
                    Ok(true) => {}
                    Err(error) => return Flow::Failed(CompleteUploadError::Unknown(error.into())),
                }
                promoted = Some(Promoted {
                    final_path: final_file,
                    staged_path: staged,
                });

                let relative = relative_final(upload.upload_id(), upload.file_name())
                    .to_string_lossy()
                    .into_owned();
                let pending = match File::try_new(
                    0,
                    relative,
                    command.user_id(),
                    false,
                    upload.mime_type_id().to_owned(),
                    Utc::now().date_naive(),
                    upload.integrity_hash().clone(),
                ) {
                    Ok(pending) => pending,
                    Err(error) => return Flow::Failed(CompleteUploadError::Unknown(error.into())),
                };
                let created = unit_of_work.files().create(pending).await;
                let file = match created {
                    Ok(file) => file,
                    // A concurrent completion of the same digest by this caller
                    // inserted the row first. Return that file and leave the
                    // session unfinished, mirroring the non-concurrent duplicate
                    // path; the promoted file is restored so it is not orphaned.
                    Err(RepositoryError::AlreadyExist) => {
                        restore_promoted(&file_storage, promoted.as_ref()).await;
                        promoted = None;
                        return match caller_file_id(&mut unit_of_work, &upload, command.user_id())
                            .await
                        {
                            Ok(Some(file_id)) => {
                                Flow::Succeeded(CompleteUploadResponse::new(file_id, false))
                            }
                            Ok(None) => {
                                security_event::application_error("complete_upload");
                                Flow::Failed(CompleteUploadError::Unknown(anyhow::anyhow!(
                                    "the duplicate file vanished while completing"
                                )))
                            }
                            Err(error) => Flow::Failed(CompleteUploadError::Unknown(error.into())),
                        };
                    }
                    Err(error) => return Flow::Failed(CompleteUploadError::Unknown(error.into())),
                };

                // The media insert shares the completion's unit of work, so the
                // file row and its image or video row commit together
                // (`STY-RUST-045`).
                if let Some(media) = probed
                    && let Err(error) =
                        register_media(&mut unit_of_work, file.id(), upload.file_name(), media)
                            .await
                {
                    return Flow::Failed(error);
                }

                match unit_of_work.uploads().finish(upload.upload_id()).await {
                    Ok(Some(_)) => Flow::Succeeded(CompleteUploadResponse::new(file.id(), true)),
                    Ok(None) => Flow::Failed(CompleteUploadError::NoSuchUpload),
                    Err(RepositoryError::ConcurrencyConflict) => {
                        Flow::Failed(CompleteUploadError::Unknown(anyhow::anyhow!(
                            "the upload was modified while completing"
                        )))
                    }
                    Err(error) => Flow::Failed(CompleteUploadError::Unknown(error.into())),
                }
            }
            .await;

            match flow {
                Flow::Succeeded(value) => {
                    if let Err(error) = unit_of_work.commit().await {
                        restore_promoted(&file_storage, promoted.as_ref()).await;
                        return Err(CompleteUploadError::Unknown(error.into()));
                    }
                    security_event::upload(
                        command.user_id(),
                        &command.upload_id().to_string(),
                        "completed",
                    );
                    Ok(value)
                }
                Flow::Expired => {
                    // The expired upload row and staged file were already
                    // deleted inside the transaction; commit so the deletion
                    // survives, then report the expiry. A commit failure is a
                    // server error and takes precedence.
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| CompleteUploadError::Unknown(error.into()))?;
                    security_event::suspicious_business_logic(
                        command.user_id(),
                        "complete",
                        "expired",
                    );
                    Err(CompleteUploadError::Expired)
                }
                Flow::Failed(error) => {
                    discard(unit_of_work).await;
                    restore_promoted(&file_storage, promoted.as_ref()).await;
                    if matches!(&error, CompleteUploadError::Incomplete) {
                        security_event::suspicious_business_logic(
                            command.user_id(),
                            "complete",
                            "not_complete",
                        );
                    }
                    Err(error)
                }
            }
        })
    }
}

/// Resolve and validate the upload through `unit_of_work`, applying the owner,
/// expiry, finished and completeness rules shared by both phases.
///
/// The mutating phase calls this again as a time-of-check/time-of-use re-check:
/// an upload another request finished, expired or completed between the phases
/// is reported from the freshly-read state.
async fn resolve<U, S>(
    unit_of_work: &mut U,
    file_storage: &Arc<S>,
    root: &Path,
    command: &CompleteUploadCommand,
    expiry_seconds: u64,
) -> Resolution
where
    U: AssetUnitOfWork,
    S: FileStorage,
{
    let found = match unit_of_work
        .uploads()
        .search(&UploadFilter {
            id: Some(command.upload_id()),
            ..UploadFilter::default()
        })
        .await
    {
        Ok(found) => found,
        Err(error) => return Resolution::Failed(CompleteUploadError::Unknown(error.into())),
    };
    let Some(upload) = found.into_iter().next() else {
        return Resolution::Failed(CompleteUploadError::NoSuchUpload);
    };

    // Owner-only: a caller who is not the upload's owner must not see it.
    if upload.user_id() != command.user_id() {
        return Resolution::Failed(CompleteUploadError::NoSuchUpload);
    }

    let expires_at = match expiry(expiry_seconds, upload.created_at()) {
        Ok(expires_at) => expires_at,
        Err(error) => return Resolution::Failed(CompleteUploadError::Unknown(error)),
    };
    if !upload.is_finished() && Utc::now().naive_utc() > expires_at {
        return expire(unit_of_work, file_storage, root, command.upload_id()).await;
    }

    // Idempotent completion: an already finished upload returns the caller's
    // file for the same digest. The session is already finished, so there are
    // no staged bytes left to verify; the lookup is scoped to the caller.
    if upload.is_finished() {
        return match caller_file_id(unit_of_work, &upload, command.user_id()).await {
            Ok(Some(file_id)) => Resolution::Finished(file_id),
            Ok(None) => {
                security_event::application_error("complete_upload");
                Resolution::Failed(CompleteUploadError::Unknown(anyhow::anyhow!(
                    "the finished upload has no matching file"
                )))
            }
            Err(error) => Resolution::Failed(CompleteUploadError::Unknown(error.into())),
        };
    }

    if !upload.can_finish() {
        return Resolution::Failed(CompleteUploadError::Incomplete);
    }

    Resolution::Ready(Box::new(upload))
}

/// Best-effort delete the expired upload's staged file and row, then signal the
/// caller to commit before returning the expiry error.
///
/// The row deletion is left to the reaper once one exists.
#[expect(
    clippy::single_call_fn,
    reason = "the expiry cleanup is named after the rule it enforces"
)]
async fn expire<U, S>(
    unit_of_work: &mut U,
    file_storage: &Arc<S>,
    root: &Path,
    upload_id: i64,
) -> Resolution
where
    U: AssetUnitOfWork,
    S: FileStorage,
{
    // TODO(reaper): move the expiry cleanup to a background task; this lazy
    // delete keeps the row and the staged file only until the next access.
    // The file-storage and repository adapters own the failure logs
    // (`OBS-002`), so a failed cleanup is swallowed here.
    if file_storage
        .delete_upload_file(&staged_path(root, upload_id))
        .await
        .is_err()
    {
        // The file-storage adapter owns the failure log (`OBS-002`).
    }
    if unit_of_work.uploads().delete(upload_id).await.is_err() {
        // The repository adapter owns the failure log (`OBS-002`).
    }
    Resolution::Expired
}

/// Stream the staged file's content and return its digest.
///
/// The hash is computed with no unit of work open, so it neither holds the
/// pooled connection nor sits inside a transaction a concurrent writer could
/// invalidate. A staged file that no longer exists is reported as
/// [`CompleteUploadError::NoSuchUpload`]: it is absent, not a storage failure.
///
/// # Errors
///
/// Returns [`CompleteUploadError::NoSuchUpload`] when the staged file is absent,
/// and [`CompleteUploadError::Unknown`] when the digest cannot be computed.
#[expect(
    clippy::single_call_fn,
    reason = "the staged-content hashing is named after the step it performs"
)]
async fn compute_integrity_hash<S>(
    file_storage: &Arc<S>,
    root: &Path,
    upload_id: i64,
) -> Result<IntegrityHash<SHA256_HEX_LENGTH>, CompleteUploadError>
where
    S: FileStorage,
{
    match file_storage
        .integrity_hash(&staged_path(root, upload_id))
        .await
    {
        Ok(Some(digest)) => Ok(digest),
        Ok(None) => Err(CompleteUploadError::NoSuchUpload),
        Err(error) => Err(CompleteUploadError::Unknown(error.into())),
    }
}

/// Best-effort roll back a unit of work, logging a failure.
///
/// The business outcome is preserved: a read-only phase has no write to persist,
/// and a failed phase reports its own error rather than the rollback failure.
async fn discard<U>(unit_of_work: U)
where
    U: UnitOfWork,
{
    if unit_of_work.rollback().await.is_err() {
        // The unit-of-work adapter owns the rollback-failure log (OBS-002).
    }
}

/// Best-effort move a promoted file back to its staging path after a failed
/// completion, so the upload can be retried and no final file is orphaned.
async fn restore_promoted<S>(file_storage: &Arc<S>, promoted: Option<&Promoted>)
where
    S: FileStorage,
{
    let Some(promoted_paths) = promoted else {
        return;
    };
    if file_storage
        .restore(&promoted_paths.final_path, &promoted_paths.staged_path)
        .await
        .is_err()
    {
        // The file-storage adapter owns the failure log (`OBS-002`).
        // TODO(reaper): a failed restore leaves the final file orphaned; delete
        // it once a background reaper exists.
    }
}

/// Probe the staged file according to its media kind.
///
/// # Errors
///
/// Returns [`CompleteUploadError::UnsupportedMedia`] when the bytes are not a
/// decodable image or video, [`CompleteUploadError::NoSuchUpload`] when the
/// staged file vanished, and [`CompleteUploadError::Unknown`] when the probe
/// cannot complete.
#[expect(
    clippy::single_call_fn,
    reason = "the probe dispatch is named after the step it performs"
)]
async fn probe_media<P: MediaProbe + ?Sized>(
    media_probe: &P,
    kind: MediaKind,
    path: &Path,
) -> Result<Probed, CompleteUploadError> {
    match kind {
        MediaKind::Image => match media_probe.probe_image(path).await {
            Ok(Some(image)) => Ok(Probed::Image(image)),
            Ok(None) => Err(CompleteUploadError::NoSuchUpload),
            Err(error) => Err(probe_fault(error)),
        },
        MediaKind::Video => match media_probe.probe_video(path).await {
            Ok(Some(video)) => Ok(Probed::Video(video)),
            Ok(None) => Err(CompleteUploadError::NoSuchUpload),
            Err(error) => Err(probe_fault(error)),
        },
    }
}

/// Map a media-probe failure onto the completion error.
fn probe_fault(error: MediaProbeError) -> CompleteUploadError {
    match error {
        MediaProbeError::UnsupportedMedia => CompleteUploadError::UnsupportedMedia,
        MediaProbeError::OperationFailed
        | MediaProbeError::Timeout
        | MediaProbeError::Unknown(_) => CompleteUploadError::Unknown(anyhow::Error::new(error)),
    }
}

/// Insert the image or video row for `file_id` in the completion transaction.
///
/// A concurrent registration that wins the insert race is an idempotent
/// success: the unique constraint on the file identifier reports
/// [`RepositoryError::AlreadyExist`].
#[expect(
    clippy::single_call_fn,
    reason = "the media persistence is named after the step it performs"
)]
async fn register_media<U: LibraryUnitOfWork>(
    unit_of_work: &mut U,
    file_id: NumericID,
    name: &str,
    media: Probed,
) -> Result<(), CompleteUploadError> {
    match media {
        Probed::Image(image) => {
            let pending = Image::try_new(
                0,
                name.to_owned(),
                image.width_px,
                image.height_px,
                image.orientation,
                Utc::now().date_naive(),
                file_id,
            )
            .map_err(|error| CompleteUploadError::Unknown(anyhow::Error::new(error)))?;
            match unit_of_work.media().create_image(pending).await {
                Ok(_) | Err(RepositoryError::AlreadyExist) => Ok(()),
                Err(error) => Err(CompleteUploadError::Unknown(error.into())),
            }
        }
        Probed::Video(video) => {
            let pending = Video::try_new(
                0,
                video.duration_ms,
                video.codec,
                video.frame_count,
                video.width,
                video.height,
                video.color_id,
                video.scan_type,
                file_id,
            )
            .map_err(|error| CompleteUploadError::Unknown(anyhow::Error::new(error)))?;
            match unit_of_work.media().create_video(pending).await {
                Ok(_) | Err(RepositoryError::AlreadyExist) => Ok(()),
                Err(error) => Err(CompleteUploadError::Unknown(error.into())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use chrono::NaiveDate;
    use chrono::NaiveDateTime;

    use crate::application::port::complete_upload::CompleteUploadCommand;
    use crate::application::port::complete_upload::CompleteUploadError;
    use crate::application::port::complete_upload::CompleteUploadUseCase as _;
    use crate::domain::model::file::File;
    use crate::domain::model::image::Orientation;
    use crate::domain::model::integrity_hash::IntegrityHash;
    use crate::domain::model::upload::ChunkBitmap;
    use crate::domain::model::upload::Upload;
    use crate::domain::model::video::ScanType;
    use crate::domain::port::configuration_repository::MockConfigurationRepository;
    use crate::domain::port::credential_repository::MockCredentialRepository;
    use crate::domain::port::error::MediaProbeError;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::error::StorageError;
    use crate::domain::port::file_repository::MockFileRepository;
    use crate::domain::port::file_storage::MockFileStorage;
    use crate::domain::port::gallery_repository::MockGalleryRepository;
    use crate::domain::port::media_probe::MockMediaProbe;
    use crate::domain::port::media_probe::ProbedImage;
    use crate::domain::port::media_probe::ProbedVideo;
    use crate::domain::port::media_repository::MockMediaRepository;
    use crate::domain::port::mime_type_repository::MockMimeTypeRepository;
    use crate::domain::port::upload_repository::MockUploadRepository;
    use crate::domain::port::user_repository::MockUserRepository;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::TestUnitOfWorkFactory;
    use crate::test_helpers::TestUow;
    use crate::test_helpers::asset_unit_of_work;

    use super::CompleteUpload;

    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const WRONG_DIGEST: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    const TTL_SECONDS: u64 = 3600;
    const CHUNK_SIZE: u64 = 4;
    /// Storage root the use case composes paths against.
    const ROOT: &str = "/storage";

    type UseCase = CompleteUpload<TestFactory, MockFileStorage, MockMediaProbe>;

    /// A use case under test together with the transaction-lifecycle flags of
    /// its two phases.
    struct Harness {
        /// Set to force the phase-1 commit to fail.
        phase_one_commit_fails: Arc<AtomicBool>,
        /// Set when the phase-1 unit of work is committed.
        phase_one_committed: Arc<AtomicBool>,
        /// Set when the phase-1 unit of work is rolled back.
        phase_one_rolled_back: Arc<AtomicBool>,
        /// Set to force the phase-2 commit to fail.
        phase_two_commit_fails: Arc<AtomicBool>,
        /// Set when the phase-2 unit of work is committed.
        phase_two_committed: Arc<AtomicBool>,
        /// Set when the phase-2 unit of work is rolled back.
        phase_two_rolled_back: Arc<AtomicBool>,
        /// The use case under test.
        use_case: UseCase,
    }

    /// Build a completion whose two phases each open their own unit of work.
    ///
    /// `resolve` runs in both phases, so each phase needs its own mocked
    /// repositories; the same upload is usually expected on both upload
    /// repositories.
    fn use_case_with(
        phase_one_uploads: MockUploadRepository,
        phase_one_files: MockFileRepository,
        phase_two_uploads: MockUploadRepository,
        phase_two_files: MockFileRepository,
        file_storage: MockFileStorage,
    ) -> Harness {
        let (phase_one, phase_one_committed, phase_one_rolled_back) = asset_unit_of_work(
            phase_one_uploads,
            phase_one_files,
            MockMimeTypeRepository::new(),
        );
        let (phase_two, phase_two_committed, phase_two_rolled_back) = asset_unit_of_work(
            phase_two_uploads,
            phase_two_files,
            MockMimeTypeRepository::new(),
        );
        let phase_one_commit_fails = Arc::clone(&phase_one.commit_fails);
        let phase_two_commit_fails = Arc::clone(&phase_two.commit_fails);
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![phase_one, phase_two]),
        };
        Harness {
            phase_one_commit_fails,
            phase_one_committed,
            phase_one_rolled_back,
            phase_two_commit_fails,
            phase_two_committed,
            phase_two_rolled_back,
            use_case: CompleteUpload::new(
                Arc::new(factory),
                Arc::new(file_storage),
                Arc::new(MockMediaProbe::new()),
                PathBuf::from(ROOT),
                TTL_SECONDS,
            ),
        }
    }

    /// Build one phase's unit of work around an explicit media repository.
    fn phase_unit_of_work(
        uploads: MockUploadRepository,
        files: MockFileRepository,
        media: MockMediaRepository,
    ) -> (TestUow, Arc<AtomicBool>, Arc<AtomicBool>) {
        let committed = Arc::new(AtomicBool::new(false));
        let rolled_back = Arc::new(AtomicBool::new(false));
        let unit_of_work = TestUow {
            committed: Arc::clone(&committed),
            commit_fails: Arc::new(AtomicBool::new(false)),
            configuration: MockConfigurationRepository::new(),
            credentials: MockCredentialRepository::new(),
            files,
            galleries: MockGalleryRepository::new(),
            media,
            mime_types: MockMimeTypeRepository::new(),
            rolled_back: Arc::clone(&rolled_back),
            uploads,
            users: MockUserRepository::new(),
        };
        (unit_of_work, committed, rolled_back)
    }

    /// Build a completion whose phases carry explicit media repositories and a
    /// configured media probe, for the registration paths.
    #[expect(
        clippy::too_many_arguments,
        reason = "the registration harness names every mock the test wires"
    )]
    fn use_case_with_media(
        phase_one_uploads: MockUploadRepository,
        phase_one_files: MockFileRepository,
        phase_one_media: MockMediaRepository,
        phase_two_uploads: MockUploadRepository,
        phase_two_files: MockFileRepository,
        phase_two_media: MockMediaRepository,
        file_storage: MockFileStorage,
        media_probe: MockMediaProbe,
    ) -> Harness {
        let (phase_one, phase_one_committed, phase_one_rolled_back) =
            phase_unit_of_work(phase_one_uploads, phase_one_files, phase_one_media);
        let (phase_two, phase_two_committed, phase_two_rolled_back) =
            phase_unit_of_work(phase_two_uploads, phase_two_files, phase_two_media);
        let phase_one_commit_fails = Arc::clone(&phase_one.commit_fails);
        let phase_two_commit_fails = Arc::clone(&phase_two.commit_fails);
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![phase_one, phase_two]),
        };
        Harness {
            phase_one_commit_fails,
            phase_one_committed,
            phase_one_rolled_back,
            phase_two_commit_fails,
            phase_two_committed,
            phase_two_rolled_back,
            use_case: CompleteUpload::new(
                Arc::new(factory),
                Arc::new(file_storage),
                Arc::new(media_probe),
                PathBuf::from(ROOT),
                TTL_SECONDS,
            ),
        }
    }

    fn timestamp() -> NaiveDateTime {
        chrono::Utc::now().naive_utc()
    }

    /// Build a complete upload whose mime type is not a media type, so the
    /// completion tests that do not target registration skip the probe.
    fn upload(
        upload_id: i64,
        user_id: i64,
        file_size: u64,
        received: &[usize],
        is_finished: bool,
    ) -> Result<Upload, RepositoryError> {
        upload_with_mime(
            upload_id,
            user_id,
            file_size,
            received,
            is_finished,
            "application/octet-stream",
        )
    }

    /// Build a complete upload with an explicit `mime_type_id`.
    fn upload_with_mime(
        upload_id: i64,
        user_id: i64,
        file_size: u64,
        received: &[usize],
        is_finished: bool,
        mime_type_id: &str,
    ) -> Result<Upload, RepositoryError> {
        let total_chunks = usize::try_from(file_size.div_ceil(CHUNK_SIZE)).unwrap_or(1);
        let mut bitmap = ChunkBitmap::try_new(total_chunks.max(1))
            .map_err(|_| RepositoryError::OperationFailed)?;
        for chunk_number in received {
            bitmap
                .mark_received(*chunk_number)
                .map_err(|_| RepositoryError::OperationFailed)?;
        }
        Upload::try_new(
            upload_id,
            user_id,
            "clip.mp4".to_owned(),
            file_size,
            mime_type_id.to_owned(),
            CHUNK_SIZE,
            IntegrityHash::try_new(DIGEST.to_owned())
                .map_err(|_| RepositoryError::OperationFailed)?,
            bitmap,
            is_finished,
            timestamp(),
        )
        .map_err(|_| RepositoryError::OperationFailed)
    }

    /// Build an upload whose creation instant is one second past the TTL.
    fn expired_upload() -> Result<Upload, RepositoryError> {
        let ttl_seconds = i64::try_from(TTL_SECONDS).unwrap_or(i64::MAX);
        let expired_at = chrono::Utc::now()
            .naive_utc()
            .checked_sub_signed(chrono::Duration::seconds(ttl_seconds))
            .and_then(|instant| instant.checked_sub_signed(chrono::Duration::seconds(1)))
            .unwrap_or_else(timestamp);
        let mut bitmap = ChunkBitmap::try_new(1).map_err(|_| RepositoryError::OperationFailed)?;
        bitmap
            .mark_received(0)
            .map_err(|_| RepositoryError::OperationFailed)?;
        Upload::try_new(
            5,
            3,
            "clip.mp4".to_owned(),
            4,
            "video/mp4".to_owned(),
            CHUNK_SIZE,
            IntegrityHash::try_new(DIGEST.to_owned())
                .map_err(|_| RepositoryError::OperationFailed)?,
            bitmap,
            false,
            expired_at,
        )
        .map_err(|_| RepositoryError::OperationFailed)
    }

    fn expect_upload(uploads: &mut MockUploadRepository, upload: Upload) {
        uploads.expect_search().times(1).returning(move |_| {
            let stored = upload.clone();
            Box::pin(async move { Ok(vec![stored]) })
        });
    }

    fn stored_file(id: i64, user_id: i64) -> Result<File, RepositoryError> {
        File::try_new(
            id,
            format!("files/{id}_clip.mp4"),
            user_id,
            false,
            "video/mp4".to_owned(),
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap_or_default(),
            IntegrityHash::try_new(DIGEST.to_owned())
                .map_err(|_| RepositoryError::OperationFailed)?,
        )
        .map_err(|_| RepositoryError::OperationFailed)
    }

    fn expect_integrity_hash(file_storage: &mut MockFileStorage, digest: &'static str) {
        file_storage
            .expect_integrity_hash()
            .times(1)
            .returning(move |_| {
                let hash = IntegrityHash::try_new(digest.to_owned())
                    .map(Some)
                    .map_err(|_| StorageError::OperationFailed);
                Box::pin(async move { hash })
            });
    }

    #[tokio::test]
    async fn complete_upload_complete_upload_promotes_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload(5, 3, 4, &[0], false)?;
        let finished = resolved.clone();
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, resolved);
        let mut phase_two_files = MockFileRepository::new();
        phase_two_files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        phase_two_files
            .expect_create()
            .times(1)
            .returning(|mut file| {
                file.set_is_public(false);
                Box::pin(async move { Ok(file) })
            });
        phase_two_uploads
            .expect_finish()
            .times(1)
            .returning(move |_| {
                let stored = finished.clone();
                Box::pin(async move { Ok(Some(stored)) })
            });
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(true) }));
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            phase_two_uploads,
            phase_two_files,
            file_storage,
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.file_id(), 0);
        assert!(response.is_finished());
        assert!(harness.phase_one_rolled_back.load(Ordering::SeqCst));
        assert!(harness.phase_two_committed.load(Ordering::SeqCst));
        assert!(!harness.phase_two_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_vanished_upload_returns_no_such_upload() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let resolved = upload(5, 3, 4, &[0], false)?;
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, resolved);
        phase_two_uploads
            .expect_finish()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));
        let mut phase_two_files = MockFileRepository::new();
        phase_two_files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        phase_two_files
            .expect_create()
            .times(1)
            .returning(|file| Box::pin(async move { Ok(file) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(true) }));
        file_storage
            .expect_restore()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            phase_two_uploads,
            phase_two_files,
            file_storage,
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::NoSuchUpload)));
        assert!(harness.phase_one_rolled_back.load(Ordering::SeqCst));
        assert!(harness.phase_two_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_integrity_hash_mismatch_returns_integrity_mismatch()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload(5, 3, 4, &[0], false)?;
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, resolved);
        // The mismatch is detected before any digest lookup, so the file table
        // is never probed.
        let mut phase_two_files = MockFileRepository::new();
        phase_two_files.expect_search().times(0);
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, WRONG_DIGEST);
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            phase_two_uploads,
            phase_two_files,
            file_storage,
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(CompleteUploadError::IntegrityMismatch)
        ));
        assert!(harness.phase_one_rolled_back.load(Ordering::SeqCst));
        assert!(harness.phase_two_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_unknown_upload_returns_no_such_upload() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        phase_one_uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            MockUploadRepository::new(),
            MockFileRepository::new(),
            MockFileStorage::new(),
        );
        let command = CompleteUploadCommand::new(999, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::NoSuchUpload)));
        assert!(harness.phase_one_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_other_user_returns_no_such_upload() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, upload(5, 3, 4, &[0], false)?);
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            MockUploadRepository::new(),
            MockFileRepository::new(),
            MockFileStorage::new(),
        );
        let command = CompleteUploadCommand::new(5, 99);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::NoSuchUpload)));
        assert!(harness.phase_one_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_expired_upload_returns_expired() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, expired_upload()?);
        phase_one_uploads
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_delete_upload_file()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            MockUploadRepository::new(),
            MockFileRepository::new(),
            file_storage,
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Expired)));
        assert!(harness.phase_one_committed.load(Ordering::SeqCst));
        assert!(!harness.phase_one_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_expiry_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, expired_upload()?);
        phase_one_uploads
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_delete_upload_file()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            MockUploadRepository::new(),
            MockFileRepository::new(),
            file_storage,
        );
        harness.phase_one_commit_fails.store(true, Ordering::SeqCst);
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Unknown(_))));
        assert!(harness.phase_one_committed.load(Ordering::SeqCst));
        assert!(!harness.phase_one_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_incomplete_upload_returns_incomplete() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, upload(5, 3, 8, &[0], false)?);
        // The incomplete upload is rejected before any digest lookup.
        let mut phase_one_files = MockFileRepository::new();
        phase_one_files.expect_search().times(0);
        let harness = use_case_with(
            phase_one_uploads,
            phase_one_files,
            MockUploadRepository::new(),
            MockFileRepository::new(),
            MockFileStorage::new(),
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Incomplete)));
        assert!(harness.phase_one_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_incomplete_with_cross_user_digest_does_not_probe()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, upload(5, 3, 8, &[0], false)?);
        // A cross-user digest must not be probed: the incomplete upload is
        // rejected by can_finish before any file-table lookup happens.
        let mut phase_one_files = MockFileRepository::new();
        phase_one_files.expect_search().times(0);
        let harness = use_case_with(
            phase_one_uploads,
            phase_one_files,
            MockUploadRepository::new(),
            MockFileRepository::new(),
            MockFileStorage::new(),
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Incomplete)));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_idempotent_finished_upload_returns_own_file()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, upload(5, 3, 4, &[0], true)?);
        let mut phase_one_files = MockFileRepository::new();
        phase_one_files.expect_search().times(1).returning(|_| {
            let stored = stored_file(11, 3);
            Box::pin(async move { stored.map(|file| vec![file]) })
        });
        let harness = use_case_with(
            phase_one_uploads,
            phase_one_files,
            MockUploadRepository::new(),
            MockFileRepository::new(),
            MockFileStorage::new(),
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.file_id(), 11);
        assert!(response.is_finished());
        assert!(harness.phase_one_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_dedup_is_scoped_to_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload(5, 3, 4, &[0], false)?;
        let finished = resolved.clone();
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, resolved);
        let mut phase_two_files = MockFileRepository::new();
        phase_two_files
            .expect_search()
            .times(1)
            .returning(|filter| {
                // The lookup is caller-scoped: another user's row is filtered out
                // by the repository, so a cross-user duplicate never conflicts and
                // the caller stores their own copy.
                assert_eq!(filter.user_id, Some(3));
                assert_eq!(filter.integrity_hash.as_deref(), Some(DIGEST));
                Box::pin(async { Ok(Vec::new()) })
            });
        phase_two_files
            .expect_create()
            .times(1)
            .returning(|file| Box::pin(async move { Ok(file) }));
        phase_two_uploads
            .expect_finish()
            .times(1)
            .returning(move |_| {
                let stored = finished.clone();
                Box::pin(async move { Ok(Some(stored)) })
            });
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(true) }));
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            phase_two_uploads,
            phase_two_files,
            file_storage,
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.file_id(), 0);
        assert!(response.is_finished());
        assert!(harness.phase_two_committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_own_duplicate_returns_existing_file_id() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let resolved = upload(5, 3, 4, &[0], false)?;
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, resolved);
        let mut phase_two_files = MockFileRepository::new();
        phase_two_files.expect_search().times(1).returning(|_| {
            let stored = stored_file(11, 3);
            Box::pin(async move { stored.map(|file| vec![file]) })
        });
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            phase_two_uploads,
            phase_two_files,
            file_storage,
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.file_id(), 11);
        assert!(!response.is_finished());
        assert!(harness.phase_two_committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_concurrent_duplicate_returns_existing_file()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload(5, 3, 4, &[0], false)?;
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, resolved);
        let mut phase_two_files = MockFileRepository::new();
        let search_calls = Arc::new(AtomicUsize::new(0));
        let next_call = Arc::clone(&search_calls);
        phase_two_files
            .expect_search()
            .times(2)
            .returning(move |_| {
                let call = next_call.fetch_add(1, Ordering::SeqCst);
                let result = if call == 0 {
                    Ok(Vec::new())
                } else {
                    stored_file(11, 3).map(|file| vec![file])
                };
                Box::pin(async move { result })
            });
        phase_two_files
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::AlreadyExist) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(true) }));
        file_storage
            .expect_restore()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            phase_two_uploads,
            phase_two_files,
            file_storage,
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.file_id(), 11);
        assert!(!response.is_finished());
        assert!(harness.phase_two_committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_own_duplicate_integrity_mismatch_returns_integrity_mismatch()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload(5, 3, 4, &[0], false)?;
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, resolved);
        // The mismatch is detected before the duplicate lookup.
        let mut phase_two_files = MockFileRepository::new();
        phase_two_files.expect_search().times(0);
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, WRONG_DIGEST);
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            phase_two_uploads,
            phase_two_files,
            file_storage,
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(CompleteUploadError::IntegrityMismatch)
        ));
        assert!(harness.phase_two_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_promote_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload(5, 3, 4, &[0], false)?;
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, resolved);
        let mut phase_two_files = MockFileRepository::new();
        phase_two_files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Err(StorageError::Unavailable) }));
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            phase_two_uploads,
            phase_two_files,
            file_storage,
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Unknown(_))));
        assert!(harness.phase_two_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_finish_conflict_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload(5, 3, 4, &[0], false)?;
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, resolved);
        phase_two_uploads
            .expect_finish()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::ConcurrencyConflict) }));
        let mut phase_two_files = MockFileRepository::new();
        phase_two_files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        phase_two_files
            .expect_create()
            .times(1)
            .returning(|file| Box::pin(async move { Ok(file) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(true) }));
        file_storage
            .expect_restore()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            phase_two_uploads,
            phase_two_files,
            file_storage,
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Unknown(_))));
        assert!(harness.phase_two_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_create_failure_restores_staged_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload(5, 3, 4, &[0], false)?;
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, resolved);
        let mut phase_two_files = MockFileRepository::new();
        phase_two_files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        phase_two_files
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(true) }));
        file_storage
            .expect_restore()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            phase_two_uploads,
            phase_two_files,
            file_storage,
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Unknown(_))));
        assert!(harness.phase_two_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_commit_failure_restores_staged_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload(5, 3, 4, &[0], false)?;
        let finished = resolved.clone();
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, resolved);
        phase_two_uploads
            .expect_finish()
            .times(1)
            .returning(move |_| {
                let stored = finished.clone();
                Box::pin(async move { Ok(Some(stored)) })
            });
        let mut phase_two_files = MockFileRepository::new();
        phase_two_files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        phase_two_files
            .expect_create()
            .times(1)
            .returning(|file| Box::pin(async move { Ok(file) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(true) }));
        file_storage
            .expect_restore()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            phase_two_uploads,
            phase_two_files,
            file_storage,
        );
        harness.phase_two_commit_fails.store(true, Ordering::SeqCst);
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Unknown(_))));
        assert!(harness.phase_two_committed.load(Ordering::SeqCst));
        assert!(!harness.phase_two_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_hashes_after_phase_one_closes() -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload(5, 3, 4, &[0], false)?;
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, resolved.clone());
        let (phase_one, _phase_one_committed, phase_one_rolled_back) = asset_unit_of_work(
            phase_one_uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );

        // The hash closure records that the read-only phase has been rolled
        // back, releasing its connection, before the staged content is read.
        let hash_called = Arc::new(AtomicBool::new(false));
        let hash_called_by_hash = Arc::clone(&hash_called);
        let rolled_back_by_hash = Arc::clone(&phase_one_rolled_back);
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_integrity_hash()
            .times(1)
            .returning(move |_| {
                assert!(
                    rolled_back_by_hash.load(Ordering::SeqCst),
                    "phase 1 must be closed before the hash"
                );
                hash_called_by_hash.store(true, Ordering::SeqCst);
                let hash = IntegrityHash::try_new(DIGEST.to_owned())
                    .map(Some)
                    .map_err(|_| StorageError::OperationFailed);
                Box::pin(async move { hash })
            });
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(true) }));

        // The phase-2 search records that the hash ran before the mutating
        // phase touched the datastore.
        let mut phase_two_uploads = MockUploadRepository::new();
        let finished = resolved.clone();
        let hash_called_by_search = Arc::clone(&hash_called);
        phase_two_uploads
            .expect_search()
            .times(1)
            .returning(move |_| {
                assert!(
                    hash_called_by_search.load(Ordering::SeqCst),
                    "phase 2 must begin after the hash"
                );
                let stored = resolved.clone();
                Box::pin(async move { Ok(vec![stored]) })
            });
        phase_two_uploads
            .expect_finish()
            .times(1)
            .returning(move |_| {
                let stored = finished.clone();
                Box::pin(async move { Ok(Some(stored)) })
            });
        let mut phase_two_files = MockFileRepository::new();
        phase_two_files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        phase_two_files
            .expect_create()
            .times(1)
            .returning(|file| Box::pin(async move { Ok(file) }));
        let (phase_two, phase_two_committed, _phase_two_rolled_back) = asset_unit_of_work(
            phase_two_uploads,
            phase_two_files,
            MockMimeTypeRepository::new(),
        );
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![phase_one, phase_two]),
        };
        let use_case = CompleteUpload::new(
            Arc::new(factory),
            Arc::new(file_storage),
            Arc::new(MockMediaProbe::new()),
            PathBuf::from(ROOT),
            TTL_SECONDS,
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let response = use_case.execute(command).await?;

        // Assert
        assert_eq!(response.file_id(), 0);
        assert!(phase_one_rolled_back.load(Ordering::SeqCst));
        assert!(hash_called.load(Ordering::SeqCst));
        assert!(phase_two_committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_finished_between_phases_returns_idempotently()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, upload(5, 3, 4, &[0], false)?);
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, upload(5, 3, 4, &[0], true)?);
        let mut phase_two_files = MockFileRepository::new();
        phase_two_files.expect_search().times(1).returning(|_| {
            let stored = stored_file(11, 3);
            Box::pin(async move { stored.map(|file| vec![file]) })
        });
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            phase_two_uploads,
            phase_two_files,
            file_storage,
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.file_id(), 11);
        assert!(response.is_finished());
        assert!(harness.phase_one_rolled_back.load(Ordering::SeqCst));
        assert!(harness.phase_two_committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_expired_between_phases_returns_expired() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, upload(5, 3, 4, &[0], false)?);
        // The upload expires between the phases; the re-check cleans it up in
        // the mutating phase and reports the expiry.
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, expired_upload()?);
        phase_two_uploads
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_delete_upload_file()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            phase_two_uploads,
            MockFileRepository::new(),
            file_storage,
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Expired)));
        assert!(harness.phase_one_rolled_back.load(Ordering::SeqCst));
        assert!(harness.phase_two_committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_integrity_hash_failure_returns_unknown() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, upload(5, 3, 4, &[0], false)?);
        let mut file_storage = MockFileStorage::new();
        // The digest cannot be computed with no unit of work open, so the
        // mutating phase is never reached.
        file_storage
            .expect_integrity_hash()
            .times(1)
            .returning(|_| Box::pin(async { Err(StorageError::OperationFailed) }));
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            MockUploadRepository::new(),
            MockFileRepository::new(),
            file_storage,
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Unknown(_))));
        assert!(harness.phase_one_rolled_back.load(Ordering::SeqCst));
        assert!(!harness.phase_two_committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_vanished_staged_file_returns_no_such_upload()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the staged file no longer exists, so the adapter reports
        // absence while the phase-1 resolve still finds the upload row.
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, upload(5, 3, 4, &[0], false)?);
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_integrity_hash()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            MockUploadRepository::new(),
            MockFileRepository::new(),
            file_storage,
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::NoSuchUpload)));
        assert!(harness.phase_one_rolled_back.load(Ordering::SeqCst));
        assert!(!harness.phase_two_committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_digest_compare_precedes_file_probe() -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload(5, 3, 4, &[0], false)?;
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, resolved);
        // The freshly-read digest is compared before the file table is probed,
        // so a mismatching caller cannot use completion as a membership oracle.
        let mut phase_two_files = MockFileRepository::new();
        phase_two_files.expect_search().times(0);
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, WRONG_DIGEST);
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            phase_two_uploads,
            phase_two_files,
            file_storage,
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(CompleteUploadError::IntegrityMismatch)
        ));
        assert!(harness.phase_two_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    /// Build an image probe result.
    #[expect(
        clippy::single_call_fn,
        reason = "the image fixture is built through a named helper"
    )]
    fn probed_image() -> ProbedImage {
        ProbedImage {
            height_px: 480,
            orientation: Orientation::Landscape,
            width_px: 640,
        }
    }

    /// Build a video probe result.
    fn probed_video() -> ProbedVideo {
        ProbedVideo {
            codec: "h264".to_owned(),
            color_id: "YCbCr".to_owned(),
            duration_ms: 1000.0,
            frame_count: 30,
            height: 480,
            scan_type: ScanType::Progressive,
            width: 640,
        }
    }

    #[tokio::test]
    async fn complete_upload_registers_an_image() -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload_with_mime(5, 3, 4, &[0], false, "image/png")?;
        let finished = resolved.clone();
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, resolved);
        phase_two_uploads
            .expect_finish()
            .times(1)
            .returning(move |_| {
                let stored = finished.clone();
                Box::pin(async move { Ok(Some(stored)) })
            });
        let mut phase_two_files = MockFileRepository::new();
        phase_two_files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        phase_two_files
            .expect_create()
            .times(1)
            .returning(|file| Box::pin(async move { Ok(file) }));
        let mut phase_two_media = MockMediaRepository::new();
        phase_two_media
            .expect_create_image()
            .times(1)
            .withf(|image| image.file_id() == 0 && image.width_px() == 640)
            .returning(|image| Box::pin(async move { Ok(image) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(true) }));
        let mut media_probe = MockMediaProbe::new();
        media_probe
            .expect_probe_image()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(probed_image())) }));
        let harness = use_case_with_media(
            phase_one_uploads,
            MockFileRepository::new(),
            MockMediaRepository::new(),
            phase_two_uploads,
            phase_two_files,
            phase_two_media,
            file_storage,
            media_probe,
        );

        // Act
        let result = harness
            .use_case
            .execute(CompleteUploadCommand::new(5, 3))
            .await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.phase_two_committed.load(Ordering::SeqCst));
        assert!(!harness.phase_two_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_registers_a_video() -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload_with_mime(5, 3, 4, &[0], false, "video/mp4")?;
        let finished = resolved.clone();
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, resolved);
        phase_two_uploads
            .expect_finish()
            .times(1)
            .returning(move |_| {
                let stored = finished.clone();
                Box::pin(async move { Ok(Some(stored)) })
            });
        let mut phase_two_files = MockFileRepository::new();
        phase_two_files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        phase_two_files
            .expect_create()
            .times(1)
            .returning(|file| Box::pin(async move { Ok(file) }));
        let mut phase_two_media = MockMediaRepository::new();
        phase_two_media
            .expect_create_video()
            .times(1)
            .withf(|video| video.file_id() == 0 && video.codec() == "h264")
            .returning(|video| Box::pin(async move { Ok(video) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(true) }));
        let mut media_probe = MockMediaProbe::new();
        media_probe
            .expect_probe_video()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(probed_video())) }));
        let harness = use_case_with_media(
            phase_one_uploads,
            MockFileRepository::new(),
            MockMediaRepository::new(),
            phase_two_uploads,
            phase_two_files,
            phase_two_media,
            file_storage,
            media_probe,
        );

        // Act
        let result = harness
            .use_case
            .execute(CompleteUploadCommand::new(5, 3))
            .await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.phase_two_committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_concurrent_registration_is_idempotent() -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload_with_mime(5, 3, 4, &[0], false, "video/mp4")?;
        let finished = resolved.clone();
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, resolved);
        phase_two_uploads
            .expect_finish()
            .times(1)
            .returning(move |_| {
                let stored = finished.clone();
                Box::pin(async move { Ok(Some(stored)) })
            });
        let mut phase_two_files = MockFileRepository::new();
        phase_two_files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        phase_two_files
            .expect_create()
            .times(1)
            .returning(|file| Box::pin(async move { Ok(file) }));
        let mut phase_two_media = MockMediaRepository::new();
        phase_two_media
            .expect_create_video()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::AlreadyExist) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(true) }));
        let mut media_probe = MockMediaProbe::new();
        media_probe
            .expect_probe_video()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(probed_video())) }));
        let harness = use_case_with_media(
            phase_one_uploads,
            MockFileRepository::new(),
            MockMediaRepository::new(),
            phase_two_uploads,
            phase_two_files,
            phase_two_media,
            file_storage,
            media_probe,
        );

        // Act
        let result = harness
            .use_case
            .execute(CompleteUploadCommand::new(5, 3))
            .await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.phase_two_committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_media_write_failure_rolls_back() -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload_with_mime(5, 3, 4, &[0], false, "video/mp4")?;
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, resolved);
        let mut phase_two_files = MockFileRepository::new();
        phase_two_files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        phase_two_files
            .expect_create()
            .times(1)
            .returning(|file| Box::pin(async move { Ok(file) }));
        let mut phase_two_media = MockMediaRepository::new();
        phase_two_media
            .expect_create_video()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(true) }));
        file_storage
            .expect_restore()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        let mut media_probe = MockMediaProbe::new();
        media_probe
            .expect_probe_video()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(probed_video())) }));
        let harness = use_case_with_media(
            phase_one_uploads,
            MockFileRepository::new(),
            MockMediaRepository::new(),
            phase_two_uploads,
            phase_two_files,
            phase_two_media,
            file_storage,
            media_probe,
        );

        // Act
        let result = harness
            .use_case
            .execute(CompleteUploadCommand::new(5, 3))
            .await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Unknown(_))));
        assert!(harness.phase_two_rolled_back.load(Ordering::SeqCst));
        assert!(!harness.phase_two_committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_unsupported_media_returns_unsupported_media()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(
            &mut phase_one_uploads,
            upload_with_mime(5, 3, 4, &[0], false, "video/mp4")?,
        );
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        let mut media_probe = MockMediaProbe::new();
        media_probe
            .expect_probe_video()
            .times(1)
            .returning(|_| Box::pin(async { Err(MediaProbeError::UnsupportedMedia) }));
        let harness = use_case_with_media(
            phase_one_uploads,
            MockFileRepository::new(),
            MockMediaRepository::new(),
            MockUploadRepository::new(),
            MockFileRepository::new(),
            MockMediaRepository::new(),
            file_storage,
            media_probe,
        );

        // Act
        let result = harness
            .use_case
            .execute(CompleteUploadCommand::new(5, 3))
            .await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::UnsupportedMedia)));
        assert!(!harness.phase_two_committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_absent_staged_media_returns_no_such_upload()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(
            &mut phase_one_uploads,
            upload_with_mime(5, 3, 4, &[0], false, "video/mp4")?,
        );
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        // The upload row exists but the staged bytes vanished between the hash
        // and the probe.
        let mut media_probe = MockMediaProbe::new();
        media_probe
            .expect_probe_video()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));
        let harness = use_case_with_media(
            phase_one_uploads,
            MockFileRepository::new(),
            MockMediaRepository::new(),
            MockUploadRepository::new(),
            MockFileRepository::new(),
            MockMediaRepository::new(),
            file_storage,
            media_probe,
        );

        // Act
        let result = harness
            .use_case
            .execute(CompleteUploadCommand::new(5, 3))
            .await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::NoSuchUpload)));
        assert!(!harness.phase_two_committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_probe_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_upload(
            &mut phase_one_uploads,
            upload_with_mime(5, 3, 4, &[0], false, "video/mp4")?,
        );
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        let mut media_probe = MockMediaProbe::new();
        media_probe
            .expect_probe_video()
            .times(1)
            .returning(|_| Box::pin(async { Err(MediaProbeError::Timeout) }));
        let harness = use_case_with_media(
            phase_one_uploads,
            MockFileRepository::new(),
            MockMediaRepository::new(),
            MockUploadRepository::new(),
            MockFileRepository::new(),
            MockMediaRepository::new(),
            file_storage,
            media_probe,
        );

        // Act
        let result = harness
            .use_case
            .execute(CompleteUploadCommand::new(5, 3))
            .await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Unknown(_))));
        assert!(!harness.phase_two_committed.load(Ordering::SeqCst));
        Ok(())
    }
}
