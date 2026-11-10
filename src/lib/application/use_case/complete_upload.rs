use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use tracing::{error, info, warn};

use crate::application::port::{
    CompleteUploadCommand, CompleteUploadError, CompleteUploadResponse, CompleteUploadUseCase,
};
use crate::application::use_case::{caller_file_id, expiry, media_path, staged_path};
use crate::domain::NumericID;
use crate::domain::model::{
    Action, Audio, File, Image, Origin, ProbedAudio, ProbedImage, ProbedVideo, Upload, Video,
};
use crate::domain::port::{
    AssetUnitOfWork, AudioRepository as _, FileRepository as _, FileStorage, ImageRepository as _,
    LibraryUnitOfWork, MediaProbe, MediaProbeError, RepositoryError, StorageError, UnitOfWork,
    UnitOfWorkFactory, UploadFilter, UploadRepository as _, UserFilter, UserRepository as _,
    UserUnitOfWork, VideoRepository as _,
};
use crate::domain::service::{Authorization, Decision};

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
    /// An `audio/*` file.
    Audio,
    /// An `image/*` file.
    Image,
    /// A `video/*` file.
    Video,
}

/// The probed metadata of a media upload.
enum Probed {
    /// Audio metadata.
    Audio(ProbedAudio),
    /// Image metadata.
    Image(ProbedImage),
    /// Video metadata.
    Video(ProbedVideo),
}

/// Use case implementation for completing an upload session.
///
/// Completion spans two bounded contexts: for a supported image, video or audio
/// file the Asset completion writes and the Library media insert share one
/// mutating unit of work, so the file is never committed without its media row
/// (`ARCH-003`, `STY-RUST-040`). A file whose mime type is another family
/// completes without a media row.
///
/// The session is sealed before its content is verified, so a chunk written
/// between the hash and the promote cannot change the bytes after the hash. A
/// completion that fails after sealing unseals the session so it can be retried.
pub(crate) struct CompleteUpload<F, S, P> {
    /// Lifetime of an upload session, in seconds.
    expiry_seconds: u64,
    /// Storage adapter promoting the staged file.
    file_storage: Arc<S>,
    /// Probe extracting the media metadata.
    media_probe: Arc<P>,
    /// Factory opening the unit of work wrapping the completion.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, S: FileStorage, P: MediaProbe> CompleteUpload<F, S, P> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(
        unit_of_work_factory: Arc<F>,
        file_storage: Arc<S>,
        media_probe: Arc<P>,
        expiry_seconds: u64,
    ) -> Self {
        Self {
            expiry_seconds,
            file_storage,
            media_probe,
            unit_of_work_factory,
        }
    }
}

impl<F, S, P> CompleteUploadUseCase for CompleteUpload<F, S, P>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork + LibraryUnitOfWork + UserUnitOfWork,
    S: FileStorage,
    P: MediaProbe,
{
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
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            // Phase 1: a short unit of work that resolves and validates the
            // upload, then seals it. It is write-intent because it seals the
            // session and the expiry branch deletes the upload row and its
            // staged file. The seal commits before the content is hashed, so it
            // is durable for the whole verification.
            let sealed_media = {
                let mut unit_of_work = unit_of_work_factory
                    .begin_write()
                    .await
                    .map_err(|error| CompleteUploadError::Unknown(error.into()))?;
                match resolve(
                    &mut unit_of_work,
                    &file_storage,
                    &command,
                    expiry_seconds,
                    false,
                )
                .await
                {
                    Resolution::Ready(upload) => {
                        // The media kind and the staged path are kept so the
                        // probe runs with no unit of work open.
                        let mime_type_id = upload.mime_type_id();
                        let media_kind = if mime_type_id.starts_with("image/") {
                            Some(MediaKind::Image)
                        } else if mime_type_id.starts_with("video/") {
                            Some(MediaKind::Video)
                        } else if mime_type_id.starts_with("audio/") {
                            Some(MediaKind::Audio)
                        } else {
                            None
                        };
                        let upload_id = upload.upload_id();
                        let staged_media = media_kind.map(|kind| (kind, staged_path(upload_id)));
                        let sealed = seal(&mut unit_of_work, (*upload).clone()).await;
                        match sealed {
                            Ok(true) => {
                                unit_of_work
                                    .commit()
                                    .await
                                    .map_err(|error| CompleteUploadError::Unknown(error.into()))?;
                                staged_media
                            }
                            // Another writer finished the session or holds its
                            // seal: report the freshly-read state.
                            Ok(false) => {
                                let outcome = lost_seal(&mut unit_of_work, &command).await;
                                discard(unit_of_work).await;
                                return outcome;
                            }
                            Err(error) => {
                                discard(unit_of_work).await;
                                return Err(CompleteUploadError::Unknown(error.into()));
                            }
                        }
                    }
                    Resolution::Finished(file_id) => {
                        // Idempotent completion: the upload finished since the
                        // caller's last attempt; return the caller's file.
                        discard(unit_of_work).await;
                        info!(
                            target: "security",
                            event = "upload",
                            actor = command.user_id(),
                            upload = %command.upload_id().to_string(),
                            outcome = %"completed",
                            "upload activity"
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
                        warn!(
                            target: "security",
                            event = "suspicious_business_logic",
                            actor = command.user_id(),
                            action = %"complete",
                            reason = %"expired",
                            "an action was out of order or exceeded a limit"
                        );
                        return Err(CompleteUploadError::Expired);
                    }
                    Resolution::Failed(error) => {
                        discard(unit_of_work).await;
                        if matches!(&error, CompleteUploadError::Incomplete) {
                            warn!(
                                target: "security",
                                event = "suspicious_business_logic",
                                actor = command.user_id(),
                                action = %"complete",
                                reason = %"not_complete",
                                "an action was out of order or exceeded a limit"
                            );
                        }
                        return Err(error);
                    }
                }
            };

            // The staged content is hashed and probed with no unit of work
            // open, so the pooled connection is free and the 30-second probe
            // never holds it. A failure here unseals the session so the caller
            // can retry instead of waiting for the reaper.
            let verified = async {
                let computed = match file_storage
                    .integrity_hash(&staged_path(command.upload_id()))
                    .await
                {
                    Ok(Some(digest)) => digest,
                    Ok(None) => return Err(CompleteUploadError::NoSuchUpload),
                    Err(error) => return Err(CompleteUploadError::Unknown(error.into())),
                };

                let probed = match sealed_media {
                    Some((kind, path)) => Some(match kind {
                        MediaKind::Audio => match media_probe.probe_audio(&path).await {
                            Ok(Some(audio)) => Probed::Audio(audio),
                            Ok(None) => return Err(CompleteUploadError::NoSuchUpload),
                            Err(error) => return Err(probe_fault(error)),
                        },
                        MediaKind::Image => match media_probe.probe_image(&path).await {
                            Ok(Some(image)) => Probed::Image(image),
                            Ok(None) => return Err(CompleteUploadError::NoSuchUpload),
                            Err(error) => return Err(probe_fault(error)),
                        },
                        MediaKind::Video => match media_probe.probe_video(&path).await {
                            Ok(Some(video)) => Probed::Video(video),
                            Ok(None) => return Err(CompleteUploadError::NoSuchUpload),
                            Err(error) => return Err(probe_fault(error)),
                        },
                    }),
                    None => None,
                };

                Ok::<_, CompleteUploadError>((computed, probed))
            }
            .await;
            let (computed, probed) = match verified {
                Ok(outcome) => outcome,
                Err(error) => {
                    unseal(&unit_of_work_factory, command.upload_id()).await;
                    return Err(error);
                }
            };

            // Phase 2: a fresh mutating unit of work. The upload is re-read and
            // re-validated (a time-of-check/time-of-use re-check), requiring the
            // seal the first phase took, before the computed digest is compared
            // and the completion is persisted.
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| CompleteUploadError::Unknown(error.into()))?;
            let mut promoted: Option<Promoted> = None;

            let flow = async {
                let upload = match resolve(
                    &mut unit_of_work,
                    &file_storage,
                    &command,
                    expiry_seconds,
                    true,
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
                // stored. The session is left unfinished, so the seal is
                // released for a retry.
                match caller_file_id(&mut unit_of_work, &upload, command.user_id()).await {
                    Ok(Some(file_id)) => {
                        let mut released = upload.clone();
                        released.unseal();
                        let _unsealed = unit_of_work.uploads().save(released).await;
                        return Flow::Succeeded(CompleteUploadResponse::new(file_id, false));
                    }
                    Ok(None) => {}
                    Err(error) => {
                        return Flow::Failed(CompleteUploadError::Unknown(error.into()));
                    }
                }

                let file_name = upload.file_name();
                if file_name.is_empty()
                    || file_name.starts_with('.')
                    || file_name.contains(['/', '\\'])
                    || file_name.contains("..")
                {
                    error!(
                        target: "security",
                        event = "application_error",
                        operation = %"complete_upload",
                        "a use case originated an internal error"
                    );
                    return Flow::Failed(CompleteUploadError::Unknown(anyhow::anyhow!(
                        "the upload file name is not a safe path segment"
                    )));
                }
                let staged = staged_path(upload.upload_id());
                let relative_path = format!("{}_{}", upload.upload_id(), file_name);
                let final_file = media_path(&relative_path);
                let promoted_file = match file_storage.promote(&staged, &final_file).await {
                    // The staged file vanished after it was hashed: the upload
                    // is absent, mirroring the repository's missing-entity
                    // outcome rather than a storage failure.
                    Ok(None) => return Flow::Failed(CompleteUploadError::NoSuchUpload),
                    Ok(Some(promoted_file)) => promoted_file,
                    // The library root is the operator's mount and is never
                    // created: a missing destination is a deliberate `503`, not
                    // an accidental `500`.
                    Err(StorageError::Unavailable) => {
                        return Flow::Failed(CompleteUploadError::LibraryUnavailable);
                    }
                    Err(error) => return Flow::Failed(CompleteUploadError::Unknown(error.into())),
                };
                promoted = Some(Promoted {
                    final_path: final_file,
                    staged_path: staged,
                });

                let pending = match File::try_new(
                    0,
                    relative_path,
                    command.user_id(),
                    false,
                    upload.mime_type_id().to_owned(),
                    Utc::now().date_naive(),
                    promoted_file.size_bytes,
                    promoted_file.modified_at,
                    Some(upload.integrity_hash().clone()),
                    Some(Origin::Uploaded),
                ) {
                    Ok(pending) => pending,
                    Err(error) => {
                        error!(
                            target: "security",
                            event = "application_error",
                            operation = %"complete_upload",
                            "a use case originated an internal error"
                        );
                        return Flow::Failed(CompleteUploadError::Unknown(error.into()));
                    }
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
                        let mut released = upload.clone();
                        released.unseal();
                        let _unsealed = unit_of_work.uploads().save(released).await;
                        return match caller_file_id(&mut unit_of_work, &upload, command.user_id())
                            .await
                        {
                            Ok(Some(file_id)) => {
                                Flow::Succeeded(CompleteUploadResponse::new(file_id, false))
                            }
                            Ok(None) => {
                                error!(
                                    target: "security",
                                    event = "application_error",
                                    operation = %"complete_upload",
                                    "a use case originated an internal error"
                                );
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
                // file row and its media row commit together (`STY-RUST-040`).
                if let Some(media) = probed {
                    let registered = match media {
                        Probed::Audio(audio) => {
                            let pending_audio =
                                Audio::from_probed(0, file_name.to_owned(), audio, file.id());
                            match unit_of_work.audios().create(pending_audio).await {
                                // `Ok` and the defensive `AlreadyExist` are both
                                // success: the file row was created in this unit
                                // of work, so the insert cannot race.
                                Ok(_) | Err(RepositoryError::AlreadyExist) => Ok(()),
                                Err(error) => Err(CompleteUploadError::Unknown(error.into())),
                            }
                        }
                        Probed::Image(image) => {
                            let pending_image = Image::from_probed(
                                0,
                                file_name.to_owned(),
                                image,
                                Utc::now().date_naive(),
                                file.id(),
                                None,
                            );
                            match unit_of_work.images().create(pending_image).await {
                                Ok(_) | Err(RepositoryError::AlreadyExist) => Ok(()),
                                Err(error) => Err(CompleteUploadError::Unknown(error.into())),
                            }
                        }
                        Probed::Video(video) => {
                            let pending_video = Video::from_probed(0, video, file.id(), None);
                            match unit_of_work.videos().create(pending_video).await {
                                Ok(_) | Err(RepositoryError::AlreadyExist) => Ok(()),
                                Err(error) => Err(CompleteUploadError::Unknown(error.into())),
                            }
                        }
                    };
                    if let Err(error) = registered {
                        return Flow::Failed(error);
                    }
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
                        unseal(&unit_of_work_factory, command.upload_id()).await;
                        return Err(CompleteUploadError::Unknown(error.into()));
                    }
                    info!(
                        target: "security",
                        event = "upload",
                        actor = command.user_id(),
                        upload = %command.upload_id().to_string(),
                        outcome = %"completed",
                        "upload activity"
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
                    warn!(
                        target: "security",
                        event = "suspicious_business_logic",
                        actor = command.user_id(),
                        action = %"complete",
                        reason = %"expired",
                        "an action was out of order or exceeded a limit"
                    );
                    Err(CompleteUploadError::Expired)
                }
                Flow::Failed(error) => {
                    discard(unit_of_work).await;
                    restore_promoted(&file_storage, promoted.as_ref()).await;
                    unseal(&unit_of_work_factory, command.upload_id()).await;
                    if matches!(&error, CompleteUploadError::Incomplete) {
                        warn!(
                            target: "security",
                            event = "suspicious_business_logic",
                            actor = command.user_id(),
                            action = %"complete",
                            reason = %"not_complete",
                            "an action was out of order or exceeded a limit"
                        );
                    }
                    Err(error)
                }
            }
        })
    }
}

/// Report the state a completion found after losing the seal.
///
/// The guarded seal matched no row, so another writer either finished the
/// session or holds its seal. The fresh read classifies which: a finished
/// session answers idempotently with the caller's file, a live one is a
/// concurrent completion.
async fn lost_seal<U>(
    unit_of_work: &mut U,
    command: &CompleteUploadCommand,
) -> Result<CompleteUploadResponse, CompleteUploadError>
where
    U: AssetUnitOfWork + UserUnitOfWork,
{
    let found = unit_of_work
        .uploads()
        .search(&UploadFilter {
            upload_id: Some(command.upload_id()),
            ..UploadFilter::default()
        })
        .await
        .map_err(|error| CompleteUploadError::Unknown(error.into()))?;
    let Some(upload) = found.into_iter().next() else {
        return Err(CompleteUploadError::NoSuchUpload);
    };
    if !upload.is_finished() {
        return Err(CompleteUploadError::CompletionInProgress);
    }
    match caller_file_id(unit_of_work, &upload, command.user_id()).await {
        Ok(Some(file_id)) => Ok(CompleteUploadResponse::new(file_id, true)),
        Ok(None) => {
            error!(
                target: "security",
                event = "application_error",
                operation = %"complete_upload",
                "a use case originated an internal error"
            );
            Err(CompleteUploadError::Unknown(anyhow::anyhow!(
                "the finished upload has no matching file"
            )))
        }
        Err(error) => Err(CompleteUploadError::Unknown(error.into())),
    }
}

/// Seal `upload` for verification, reporting whether the transition applied.
///
/// A finished or already-sealed session is reported as `Ok(false)`, so the
/// caller can treat the race as a lost seal.
async fn seal<U>(unit_of_work: &mut U, mut upload: Upload) -> Result<bool, RepositoryError>
where
    U: AssetUnitOfWork,
{
    if upload.is_finished() || upload.is_verifying() {
        return Ok(false);
    }
    upload.seal(Utc::now().naive_utc());
    unit_of_work.uploads().save(upload).await?;
    Ok(true)
}

/// Best-effort clear the verification seal of `upload_id` in its own unit of
/// work.
///
/// A failure after the seal must not wedge the session: a seal left behind is
/// unsealed by the reaper after the verification bound. The unit-of-work and
/// repository adapters own the failure logs (`OBS-002`), so a failed unseal is
/// swallowed here.
async fn unseal<F>(unit_of_work_factory: &Arc<F>, upload_id: NumericID)
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork,
{
    let Ok(mut unit_of_work) = unit_of_work_factory.begin_write().await else {
        return;
    };
    let released = unit_of_work
        .uploads()
        .search(&UploadFilter {
            upload_id: Some(upload_id),
            ..UploadFilter::default()
        })
        .await;
    if let Ok(found) = released
        && let Some(mut upload) = found.into_iter().next()
    {
        upload.unseal();
        if unit_of_work.uploads().save(upload).await.is_err() {
            // The repository adapter owns the failure log (`OBS-002`).
        }
    }
    match unit_of_work.commit().await {
        Ok(()) => {}
        Err(error) => {
            // Best-effort: the reaper unseals a stale seal after the
            // verification bound, so the business error the caller sees is
            // preserved. The failure is still classified as the use case's
            // `CompleteUploadError::Unknown` rather than discarded, and the
            // unit-of-work adapter already logged it (`OBS-002`).
            let _unmapped = CompleteUploadError::Unknown(error.into());
        }
    }
}

/// Resolve and validate the upload through `unit_of_work`, applying the owner,
/// expiry, finished, seal and completeness rules shared by both phases.
///
/// The mutating phase calls this again as a time-of-check/time-of-use re-check
/// with `require_seal`: an upload another request finished, expired or completed
/// between the phases is reported from the freshly-read state.
async fn resolve<U, S>(
    unit_of_work: &mut U,
    file_storage: &Arc<S>,
    command: &CompleteUploadCommand,
    expiry_seconds: u64,
    require_seal: bool,
) -> Resolution
where
    U: AssetUnitOfWork + UserUnitOfWork,
    S: FileStorage,
{
    let caller = match unit_of_work
        .users()
        .search(&UserFilter {
            id: Some(command.user_id()),
            ..UserFilter::default()
        })
        .await
    {
        Ok(found) => match found.into_iter().next() {
            Some(caller) => caller,
            None => return Resolution::Failed(CompleteUploadError::NoSuchCaller),
        },
        Err(error) => return Resolution::Failed(CompleteUploadError::Unknown(error.into())),
    };

    let found = match unit_of_work
        .uploads()
        .search(&UploadFilter {
            upload_id: Some(command.upload_id()),
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

    // Owner-only: a caller who is not the upload's owner must not see it. The
    // denial is a security decision the use case owns, so it logs the
    // catalogued `authz_failed` event once (`OBS-002`, `OBS-006`); the caller
    // still receives the not-found result, never a disclosure of the owner.
    if let Decision::Deny(denial) =
        Authorization::decide(&caller, &[], Action::UploadWrite, &upload)
    {
        warn!(
            target: "security",
            event = "authz_failed",
            actor = caller.id(),
            action = %denial.action().verb(),
            target = %denial.action().target(),
            "denied an action"
        );
        return Resolution::Failed(CompleteUploadError::NoSuchUpload);
    }

    let expires_at = match expiry(expiry_seconds, upload.created_at()) {
        Ok(expires_at) => expires_at,
        Err(error) => return Resolution::Failed(CompleteUploadError::Unknown(error)),
    };
    if !upload.is_finished() && Utc::now().naive_utc() > expires_at {
        // The reaper removes an expired session in the background; this lazy
        // delete releases the row and the staged file on the next access. The
        // file-storage and repository adapters own the failure logs (`OBS-002`),
        // so a failed cleanup is swallowed here.
        if file_storage
            .delete_upload_file(&staged_path(command.upload_id()))
            .await
            .is_err()
        {
            // The file-storage adapter owns the failure log (`OBS-002`).
        }
        if unit_of_work
            .uploads()
            .delete(&UploadFilter {
                upload_id: Some(command.upload_id()),
                ..UploadFilter::default()
            })
            .await
            .is_err()
        {
            // The repository adapter owns the failure log (`OBS-002`).
        }
        return Resolution::Expired;
    }

    // Idempotent completion: an already finished upload returns the caller's
    // file for the same digest. The session is already finished, so there are
    // no staged bytes left to verify; the lookup is scoped to the caller.
    if upload.is_finished() {
        return match caller_file_id(unit_of_work, &upload, command.user_id()).await {
            Ok(Some(file_id)) => Resolution::Finished(file_id),
            Ok(None) => {
                error!(
                    target: "security",
                    event = "application_error",
                    operation = %"complete_upload",
                    "a use case originated an internal error"
                );
                Resolution::Failed(CompleteUploadError::Unknown(anyhow::anyhow!(
                    "the finished upload has no matching file"
                )))
            }
            Err(error) => Resolution::Failed(CompleteUploadError::Unknown(error.into())),
        };
    }

    // The second phase runs only under the seal the first phase took: a
    // completion whose seal was released by a failure elsewhere must not
    // persist against content that may have changed.
    if require_seal && !upload.is_verifying() {
        return Resolution::Failed(CompleteUploadError::CompletionInProgress);
    }

    if !upload.can_finish() {
        return Resolution::Failed(CompleteUploadError::Incomplete);
    }

    Resolution::Ready(Box::new(upload))
}

/// Best-effort roll back a unit of work, logging a failure.
///
/// The business outcome is preserved: a read-only phase has no write to persist,
/// and a failed phase reports its own error rather than the rollback failure.
async fn discard<U>(unit_of_work: U)
where
    U: UnitOfWork,
{
    // The unit-of-work adapter owns the rollback-failure log (OBS-002).
    let _rollback = unit_of_work.rollback().await;
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
        // The file-storage adapter owns the failure log (`OBS-002`). A leftover
        // promoted file is the operator's content: the ingest registers it as a
        // found file, which is harmless.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::use_case::test_fixtures::TTL_SECONDS;
    use crate::domain::model::{ChunkBitmap, IntegrityHash};

    use crate::domain::port::{
        MockAudioRepository, MockFileRepository, MockFileStorage, MockImageRepository,
        MockMediaProbe, MockUploadRepository, MockVideoRepository, PromotedFile, StorageError,
    };

    use crate::test_helpers::*;
    use chrono::{NaiveDate, NaiveDateTime};
    use std::error::Error;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tracing_test::traced_test;

    const WRONG_DIGEST: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

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
        /// Set when a failed completion unseals the session.
        unsealed: Arc<AtomicBool>,
        /// The use case under test.
        use_case: UseCase,
    }

    /// Tolerate a best-effort unseal through `uploads`, recording that it ran.
    fn expect_unseal(uploads: &mut MockUploadRepository, unsealed: Arc<AtomicBool>) {
        let flag = Arc::clone(&unsealed);
        uploads.expect_search().times(0..).returning(move |_| {
            flag.store(true, Ordering::SeqCst);
            Box::pin(async { Ok(Vec::new()) })
        });
        uploads.expect_save().times(0..).returning(move |upload| {
            unsealed.store(true, Ordering::SeqCst);
            Box::pin(async move { Ok(upload) })
        });
    }

    /// Build the unit of work a failed completion unseals through.
    ///
    /// Its upload repository records that the seal was cleared, and tolerates
    /// never being reached on a successful completion.
    fn unseal_unit_of_work(unsealed: Arc<AtomicBool>) -> TestUow {
        let mut uploads = MockUploadRepository::new();
        uploads.expect_search().times(0..).returning(move |_| {
            unsealed.store(true, Ordering::SeqCst);
            Box::pin(async { Ok(Vec::new()) })
        });
        uploads
            .expect_save()
            .times(0..)
            .returning(|upload| Box::pin(async move { Ok(upload) }));
        TestUnitOfWork {
            uploads,
            ..TestUnitOfWork::default()
        }
    }

    /// Build a completion whose two phases each open their own unit of work.
    ///
    /// `resolve` runs in both phases, so each phase needs its own mocked
    /// repositories; the same upload is usually expected on both upload
    /// repositories.
    fn use_case_with(
        mut phase_one_uploads: MockUploadRepository,
        phase_one_files: MockFileRepository,
        mut phase_two_uploads: MockUploadRepository,
        phase_two_files: MockFileRepository,
        file_storage: MockFileStorage,
    ) -> Harness {
        let unsealed = Arc::new(AtomicBool::new(false));
        expect_unseal(&mut phase_one_uploads, Arc::clone(&unsealed));
        expect_unseal(&mut phase_two_uploads, Arc::clone(&unsealed));
        let phase_one = TestUnitOfWork {
            users: any_caller(),
            files: phase_one_files,
            uploads: phase_one_uploads,
            ..TestUnitOfWork::default()
        };
        let phase_one_committed = Arc::clone(&phase_one.committed);
        let phase_one_rolled_back = Arc::clone(&phase_one.rolled_back);
        let phase_two = TestUnitOfWork {
            users: any_caller(),
            files: phase_two_files,
            uploads: phase_two_uploads,
            ..TestUnitOfWork::default()
        };
        let phase_two_committed = Arc::clone(&phase_two.committed);
        let phase_two_rolled_back = Arc::clone(&phase_two.rolled_back);
        let phase_one_commit_fails = Arc::clone(&phase_one.commit_fails);
        let phase_two_commit_fails = Arc::clone(&phase_two.commit_fails);
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![
                phase_one,
                phase_two,
                unseal_unit_of_work(Arc::clone(&unsealed)),
            ]),
        };
        Harness {
            phase_one_commit_fails,
            phase_one_committed,
            phase_one_rolled_back,
            phase_two_commit_fails,
            phase_two_committed,
            phase_two_rolled_back,
            unsealed,
            use_case: CompleteUpload::new(
                Arc::new(factory),
                Arc::new(file_storage),
                Arc::new(MockMediaProbe::new()),
                TTL_SECONDS,
            ),
        }
    }

    /// Build one phase's unit of work around explicit media repositories.
    fn phase_unit_of_work(
        uploads: MockUploadRepository,
        files: MockFileRepository,
        images: MockImageRepository,
        audios: MockAudioRepository,
        videos: MockVideoRepository,
    ) -> (TestUow, Arc<AtomicBool>, Arc<AtomicBool>) {
        let unit_of_work = TestUnitOfWork {
            users: any_caller(),
            audios,
            files,
            images,
            uploads,
            videos,
            ..TestUnitOfWork::default()
        };
        let committed = Arc::clone(&unit_of_work.committed);
        let rolled_back = Arc::clone(&unit_of_work.rolled_back);
        (unit_of_work, committed, rolled_back)
    }

    /// Build a completion whose phases carry explicit media repositories and a
    /// configured media probe, for the registration paths.
    fn use_case_with_media(
        mut phase_one_uploads: MockUploadRepository,
        phase_one_files: MockFileRepository,
        phase_one_images: MockImageRepository,
        phase_one_videos: MockVideoRepository,
        mut phase_two_uploads: MockUploadRepository,
        phase_two_files: MockFileRepository,
        phase_two_images: MockImageRepository,
        phase_two_videos: MockVideoRepository,
        file_storage: MockFileStorage,
        media_probe: MockMediaProbe,
    ) -> Harness {
        let unsealed = Arc::new(AtomicBool::new(false));
        expect_unseal(&mut phase_one_uploads, Arc::clone(&unsealed));
        expect_unseal(&mut phase_two_uploads, Arc::clone(&unsealed));
        let (phase_one, phase_one_committed, phase_one_rolled_back) = phase_unit_of_work(
            phase_one_uploads,
            phase_one_files,
            phase_one_images,
            MockAudioRepository::new(),
            phase_one_videos,
        );
        let (phase_two, phase_two_committed, phase_two_rolled_back) = phase_unit_of_work(
            phase_two_uploads,
            phase_two_files,
            phase_two_images,
            MockAudioRepository::new(),
            phase_two_videos,
        );
        let phase_one_commit_fails = Arc::clone(&phase_one.commit_fails);
        let phase_two_commit_fails = Arc::clone(&phase_two.commit_fails);
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![
                phase_one,
                phase_two,
                unseal_unit_of_work(Arc::clone(&unsealed)),
            ]),
        };
        Harness {
            phase_one_commit_fails,
            phase_one_committed,
            phase_one_rolled_back,
            phase_two_commit_fails,
            phase_two_committed,
            phase_two_rolled_back,
            unsealed,
            use_case: CompleteUpload::new(
                Arc::new(factory),
                Arc::new(file_storage),
                Arc::new(media_probe),
                TTL_SECONDS,
            ),
        }
    }

    /// Build a completion whose mutating phase carries an audio repository.
    fn use_case_with_audio(
        mut phase_one_uploads: MockUploadRepository,
        mut phase_two_uploads: MockUploadRepository,
        phase_two_files: MockFileRepository,
        phase_two_audios: MockAudioRepository,
        file_storage: MockFileStorage,
        media_probe: MockMediaProbe,
    ) -> Harness {
        let unsealed = Arc::new(AtomicBool::new(false));
        expect_unseal(&mut phase_one_uploads, Arc::clone(&unsealed));
        expect_unseal(&mut phase_two_uploads, Arc::clone(&unsealed));
        let (phase_one, phase_one_committed, phase_one_rolled_back) = phase_unit_of_work(
            phase_one_uploads,
            MockFileRepository::new(),
            MockImageRepository::new(),
            MockAudioRepository::new(),
            MockVideoRepository::new(),
        );
        let (phase_two, phase_two_committed, phase_two_rolled_back) = phase_unit_of_work(
            phase_two_uploads,
            phase_two_files,
            MockImageRepository::new(),
            phase_two_audios,
            MockVideoRepository::new(),
        );
        let phase_one_commit_fails = Arc::clone(&phase_one.commit_fails);
        let phase_two_commit_fails = Arc::clone(&phase_two.commit_fails);
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![
                phase_one,
                phase_two,
                unseal_unit_of_work(Arc::clone(&unsealed)),
            ]),
        };
        Harness {
            phase_one_commit_fails,
            phase_one_committed,
            phase_one_rolled_back,
            phase_two_commit_fails,
            phase_two_committed,
            phase_two_rolled_back,
            unsealed,
            use_case: CompleteUpload::new(
                Arc::new(factory),
                Arc::new(file_storage),
                Arc::new(media_probe),
                TTL_SECONDS,
            ),
        }
    }

    fn timestamp() -> NaiveDateTime {
        chrono::Utc::now().naive_utc()
    }

    /// Metadata of the file a successful promote writes into the media root.
    fn promoted_file() -> PromotedFile {
        PromotedFile {
            modified_at: NaiveDate::from_ymd_opt(2026, 1, 1)
                .unwrap_or_default()
                .and_hms_opt(0, 0, 0)
                .unwrap_or_default(),
            size_bytes: i64::try_from(CHUNK_SIZE).unwrap_or_default(),
        }
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
            false,
            None,
            timestamp(),
        )
        .map_err(|_| RepositoryError::OperationFailed)
    }

    /// Seal `upload` for verification, as the first completion phase does.
    fn sealed(mut upload: Upload) -> Upload {
        upload.seal(timestamp());
        upload
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
            false,
            None,
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

    /// Expect a pending upload that resolves and wins its verification seal.
    fn expect_pending_upload(uploads: &mut MockUploadRepository, upload: Upload) {
        expect_upload(uploads, upload);
        uploads
            .expect_save()
            .times(1)
            .returning(|saved| Box::pin(async move { Ok(saved) }));
    }

    fn stored_file(id: i64, user_id: i64) -> Result<File, RepositoryError> {
        File::try_new(
            id,
            format!("files/{id}_clip.mp4"),
            user_id,
            false,
            "video/mp4".to_owned(),
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap_or_default(),
            0,
            NaiveDate::from_ymd_opt(2026, 1, 1)
                .unwrap_or_default()
                .and_hms_opt(0, 0, 0)
                .unwrap_or_default(),
            Some(
                IntegrityHash::try_new(DIGEST.to_owned())
                    .map_err(|_| RepositoryError::OperationFailed)?,
            ),
            Some(Origin::Uploaded),
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
        expect_pending_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, sealed(resolved));
        let mut phase_two_files = MockFileRepository::new();
        phase_two_files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        phase_two_files
            .expect_create()
            .times(1)
            .withf(|file| {
                file.path() == "5_clip.mp4"
                    && file.origin() == Origin::Uploaded
                    && file.integrity_hash().is_some()
                    && file.size_bytes() == promoted_file().size_bytes
                    && file.modified_at() == promoted_file().modified_at
            })
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
            .returning(|_, _| Box::pin(async { Ok(Some(promoted_file())) }));
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
        assert!(harness.phase_one_committed.load(Ordering::SeqCst));
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
        expect_pending_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, sealed(resolved));
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
            .returning(|_, _| Box::pin(async { Ok(Some(promoted_file())) }));
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
        assert!(harness.phase_one_committed.load(Ordering::SeqCst));
        assert!(harness.phase_two_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_integrity_hash_mismatch_returns_integrity_mismatch()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload(5, 3, 4, &[0], false)?;
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, sealed(resolved));
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
        assert!(harness.phase_one_committed.load(Ordering::SeqCst));
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
    #[traced_test]
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
        // The owner check aborts the read-only phase, so the mutating phase is
        // never reached: the denial must be logged exactly once (`OBS-002`).
        logs_assert(|lines| {
            let count = lines.iter().filter(|line| line.contains("WARN")).count();
            (count == 1).then_some(()).ok_or_else(|| {
                format!("the owner check must emit exactly one warn event (found {count})")
            })
        });
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"authz_failed\""))
                .count();
            (count == 1).then_some(()).ok_or_else(|| {
                format!("the owner check must emit exactly one authz_failed event (found {count})")
            })
        });
        assert!(
            logs_contain("security"),
            "the event must target the reserved security target"
        );
        assert!(
            logs_contain(" actor=99 "),
            "the event must carry the caller as the actor"
        );
        assert!(
            logs_contain("action=write"),
            "the event must classify the denied action as an upload write"
        );
        assert!(
            logs_contain("target=upload"),
            "the event must name the denied target"
        );
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
            .withf(|filter| filter.upload_id == Some(5))
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
            .withf(|filter| filter.upload_id == Some(5))
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
        expect_pending_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, sealed(resolved));
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
            .returning(|_, _| Box::pin(async { Ok(Some(promoted_file())) }));
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
        expect_pending_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, sealed(resolved));
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
        assert!(harness.unsealed.load(Ordering::SeqCst));
        assert!(harness.phase_two_committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_concurrent_duplicate_returns_existing_file()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload(5, 3, 4, &[0], false)?;
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, sealed(resolved));
        let mut phase_two_files = MockFileRepository::new();
        let mut search_order = mockall::Sequence::new();
        phase_two_files
            .expect_search()
            .times(1)
            .in_sequence(&mut search_order)
            .return_once(|_| Box::pin(async { Ok(Vec::new()) }));
        phase_two_files
            .expect_search()
            .times(1)
            .in_sequence(&mut search_order)
            .return_once(|_| {
                let stored = stored_file(11, 3);
                Box::pin(async move { stored.map(|file| vec![file]) })
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
            .returning(|_, _| Box::pin(async { Ok(Some(promoted_file())) }));
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
        assert!(harness.unsealed.load(Ordering::SeqCst));
        assert!(harness.phase_two_committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_own_duplicate_integrity_mismatch_returns_integrity_mismatch()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload(5, 3, 4, &[0], false)?;
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, sealed(resolved));
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
    async fn complete_upload_promote_destination_missing_returns_library_unavailable()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload(5, 3, 4, &[0], false)?;
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, sealed(resolved));
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
        assert!(matches!(
            result,
            Err(CompleteUploadError::LibraryUnavailable)
        ));
        assert!(harness.phase_two_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_finish_conflict_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload(5, 3, 4, &[0], false)?;
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, sealed(resolved));
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
            .returning(|_, _| Box::pin(async { Ok(Some(promoted_file())) }));
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
        expect_pending_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, sealed(resolved));
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
            .returning(|_, _| Box::pin(async { Ok(Some(promoted_file())) }));
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
        expect_pending_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, sealed(resolved));
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
            .returning(|_, _| Box::pin(async { Ok(Some(promoted_file())) }));
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
        expect_pending_upload(&mut phase_one_uploads, resolved.clone());
        let phase_one = TestUnitOfWork {
            users: any_caller(),
            uploads: phase_one_uploads,
            ..TestUnitOfWork::default()
        };
        let _phase_one_committed = Arc::clone(&phase_one.committed);
        let phase_one_committed = Arc::clone(&phase_one.committed);

        // The hash closure records that the read-only phase has been rolled
        // back, releasing its connection, before the staged content is read.
        let hash_called = Arc::new(AtomicBool::new(false));
        let hash_called_by_hash = Arc::clone(&hash_called);
        let committed_by_hash = Arc::clone(&phase_one_committed);
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_integrity_hash()
            .times(1)
            .returning(move |_| {
                assert!(
                    committed_by_hash.load(Ordering::SeqCst),
                    "phase 1 must commit its seal before the hash"
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
            .returning(|_, _| Box::pin(async { Ok(Some(promoted_file())) }));

        // The phase-2 search records that the hash ran before the mutating
        // phase touched the datastore.
        let mut phase_two_uploads = MockUploadRepository::new();
        let finished = resolved.clone();
        let phase_two_upload = sealed(resolved.clone());
        let hash_called_by_search = Arc::clone(&hash_called);
        phase_two_uploads
            .expect_search()
            .times(1)
            .returning(move |_| {
                assert!(
                    hash_called_by_search.load(Ordering::SeqCst),
                    "phase 2 must begin after the hash"
                );
                let stored = phase_two_upload.clone();
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
        let phase_two = TestUnitOfWork {
            users: any_caller(),
            files: phase_two_files,
            uploads: phase_two_uploads,
            ..TestUnitOfWork::default()
        };
        let phase_two_committed = Arc::clone(&phase_two.committed);
        let _phase_two_rolled_back = Arc::clone(&phase_two.rolled_back);
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![phase_one, phase_two]),
        };
        let use_case = CompleteUpload::new(
            Arc::new(factory),
            Arc::new(file_storage),
            Arc::new(MockMediaProbe::new()),
            TTL_SECONDS,
        );
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let response = use_case.execute(command).await?;

        // Assert
        assert_eq!(response.file_id(), 0);
        assert!(phase_one_committed.load(Ordering::SeqCst));
        assert!(hash_called.load(Ordering::SeqCst));
        assert!(phase_two_committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_finished_between_phases_returns_idempotently()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(&mut phase_one_uploads, upload(5, 3, 4, &[0], false)?);
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
        assert!(harness.phase_one_committed.load(Ordering::SeqCst));
        assert!(harness.phase_two_committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_expired_between_phases_returns_expired() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(&mut phase_one_uploads, upload(5, 3, 4, &[0], false)?);
        // The upload expires between the phases; the re-check cleans it up in
        // the mutating phase and reports the expiry.
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, expired_upload()?);
        phase_two_uploads
            .expect_delete()
            .times(1)
            .withf(|filter| filter.upload_id == Some(5))
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
        assert!(harness.phase_one_committed.load(Ordering::SeqCst));
        assert!(harness.phase_two_committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_integrity_hash_failure_returns_unknown() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(&mut phase_one_uploads, upload(5, 3, 4, &[0], false)?);
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
        assert!(harness.phase_one_committed.load(Ordering::SeqCst));
        assert!(harness.unsealed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_vanished_staged_file_returns_no_such_upload()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the staged file no longer exists, so the adapter reports
        // absence while the phase-1 resolve still finds the upload row.
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(&mut phase_one_uploads, upload(5, 3, 4, &[0], false)?);
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
        assert!(harness.phase_one_committed.load(Ordering::SeqCst));
        assert!(harness.unsealed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_digest_compare_precedes_file_probe() -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload(5, 3, 4, &[0], false)?;
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, sealed(resolved));
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

    #[tokio::test]
    async fn complete_upload_registers_an_image() -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload_with_mime(5, 3, 4, &[0], false, "image/png")?;
        let finished = resolved.clone();
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, sealed(resolved));
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
        let mut phase_two_images = MockImageRepository::new();
        phase_two_images
            .expect_create()
            .times(1)
            .withf(|image| image.file_id() == 0 && image.width_px() == 640)
            .returning(|image| Box::pin(async move { Ok(image) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(Some(promoted_file())) }));
        let mut media_probe = MockMediaProbe::new();
        media_probe
            .expect_probe_image()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(probed_image())) }));
        let harness = use_case_with_media(
            phase_one_uploads,
            MockFileRepository::new(),
            MockImageRepository::new(),
            MockVideoRepository::new(),
            phase_two_uploads,
            phase_two_files,
            phase_two_images,
            MockVideoRepository::new(),
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
        expect_pending_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, sealed(resolved));
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
        let mut phase_two_videos = MockVideoRepository::new();
        phase_two_videos
            .expect_create()
            .times(1)
            .withf(|video| video.file_id() == 0 && video.codec() == "h264")
            .returning(|video| Box::pin(async move { Ok(video) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(Some(promoted_file())) }));
        let mut media_probe = MockMediaProbe::new();
        media_probe
            .expect_probe_video()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(probed_video())) }));
        let harness = use_case_with_media(
            phase_one_uploads,
            MockFileRepository::new(),
            MockImageRepository::new(),
            MockVideoRepository::new(),
            phase_two_uploads,
            phase_two_files,
            MockImageRepository::new(),
            phase_two_videos,
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
        expect_pending_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, sealed(resolved));
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
        let mut phase_two_videos = MockVideoRepository::new();
        phase_two_videos
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::AlreadyExist) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(Some(promoted_file())) }));
        let mut media_probe = MockMediaProbe::new();
        media_probe
            .expect_probe_video()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(probed_video())) }));
        let harness = use_case_with_media(
            phase_one_uploads,
            MockFileRepository::new(),
            MockImageRepository::new(),
            MockVideoRepository::new(),
            phase_two_uploads,
            phase_two_files,
            MockImageRepository::new(),
            phase_two_videos,
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
        expect_pending_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, sealed(resolved));
        let mut phase_two_files = MockFileRepository::new();
        phase_two_files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        phase_two_files
            .expect_create()
            .times(1)
            .returning(|file| Box::pin(async move { Ok(file) }));
        let mut phase_two_videos = MockVideoRepository::new();
        phase_two_videos
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(Some(promoted_file())) }));
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
            MockImageRepository::new(),
            MockVideoRepository::new(),
            phase_two_uploads,
            phase_two_files,
            MockImageRepository::new(),
            phase_two_videos,
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
        expect_pending_upload(
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
            MockImageRepository::new(),
            MockVideoRepository::new(),
            MockUploadRepository::new(),
            MockFileRepository::new(),
            MockImageRepository::new(),
            MockVideoRepository::new(),
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
        assert!(harness.unsealed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_absent_staged_media_returns_no_such_upload()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(
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
            MockImageRepository::new(),
            MockVideoRepository::new(),
            MockUploadRepository::new(),
            MockFileRepository::new(),
            MockImageRepository::new(),
            MockVideoRepository::new(),
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
        assert!(harness.unsealed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_probe_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(
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
            MockImageRepository::new(),
            MockVideoRepository::new(),
            MockUploadRepository::new(),
            MockFileRepository::new(),
            MockImageRepository::new(),
            MockVideoRepository::new(),
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
        assert!(harness.unsealed.load(Ordering::SeqCst));
        Ok(())
    }

    /// Wire a phase-two file repository that finds no duplicate and inserts the row.
    fn expect_file_create(files: &mut MockFileRepository) {
        files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        files
            .expect_create()
            .times(1)
            .returning(|file| Box::pin(async move { Ok(file) }));
    }

    /// Wire a file storage that promotes the staged file.
    fn expect_promote(file_storage: &mut MockFileStorage) {
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(Some(promoted_file())) }));
    }

    /// Wire a file storage that restores the promoted file after a failed write.
    fn expect_restore(file_storage: &mut MockFileStorage) {
        file_storage
            .expect_restore()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
    }

    #[tokio::test]
    async fn complete_upload_absent_staged_image_returns_no_such_upload()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the upload row exists but the staged bytes vanished between
        // the hash and the image probe.
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(
            &mut phase_one_uploads,
            upload_with_mime(5, 3, 4, &[0], false, "image/png")?,
        );
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        let mut media_probe = MockMediaProbe::new();
        media_probe
            .expect_probe_image()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));
        let harness = use_case_with_media(
            phase_one_uploads,
            MockFileRepository::new(),
            MockImageRepository::new(),
            MockVideoRepository::new(),
            MockUploadRepository::new(),
            MockFileRepository::new(),
            MockImageRepository::new(),
            MockVideoRepository::new(),
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
        assert!(harness.unsealed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_image_probe_timeout_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(
            &mut phase_one_uploads,
            upload_with_mime(5, 3, 4, &[0], false, "image/png")?,
        );
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        let mut media_probe = MockMediaProbe::new();
        media_probe
            .expect_probe_image()
            .times(1)
            .returning(|_| Box::pin(async { Err(MediaProbeError::Timeout) }));
        let harness = use_case_with_media(
            phase_one_uploads,
            MockFileRepository::new(),
            MockImageRepository::new(),
            MockVideoRepository::new(),
            MockUploadRepository::new(),
            MockFileRepository::new(),
            MockImageRepository::new(),
            MockVideoRepository::new(),
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
        assert!(harness.unsealed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_image_unsupported_media_returns_unsupported_media()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(
            &mut phase_one_uploads,
            upload_with_mime(5, 3, 4, &[0], false, "image/png")?,
        );
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        let mut media_probe = MockMediaProbe::new();
        media_probe
            .expect_probe_image()
            .times(1)
            .returning(|_| Box::pin(async { Err(MediaProbeError::UnsupportedMedia) }));
        let harness = use_case_with_media(
            phase_one_uploads,
            MockFileRepository::new(),
            MockImageRepository::new(),
            MockVideoRepository::new(),
            MockUploadRepository::new(),
            MockFileRepository::new(),
            MockImageRepository::new(),
            MockVideoRepository::new(),
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
        assert!(harness.unsealed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_concurrent_image_registration_is_idempotent()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload_with_mime(5, 3, 4, &[0], false, "image/png")?;
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, sealed(resolved.clone()));
        phase_two_uploads
            .expect_finish()
            .times(1)
            .returning(move |_| {
                let stored = resolved.clone();
                Box::pin(async move { Ok(Some(stored)) })
            });
        let mut phase_two_files = MockFileRepository::new();
        expect_file_create(&mut phase_two_files);
        let mut phase_two_images = MockImageRepository::new();
        phase_two_images
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::AlreadyExist) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        expect_promote(&mut file_storage);
        let mut media_probe = MockMediaProbe::new();
        media_probe
            .expect_probe_image()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(probed_image())) }));
        let harness = use_case_with_media(
            phase_one_uploads,
            MockFileRepository::new(),
            MockImageRepository::new(),
            MockVideoRepository::new(),
            phase_two_uploads,
            phase_two_files,
            phase_two_images,
            MockVideoRepository::new(),
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
    async fn complete_upload_image_media_write_failure_rolls_back() -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload_with_mime(5, 3, 4, &[0], false, "image/png")?;
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, sealed(resolved));
        let mut phase_two_files = MockFileRepository::new();
        expect_file_create(&mut phase_two_files);
        let mut phase_two_images = MockImageRepository::new();
        phase_two_images
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        expect_promote(&mut file_storage);
        expect_restore(&mut file_storage);
        let mut media_probe = MockMediaProbe::new();
        media_probe
            .expect_probe_image()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(probed_image())) }));
        let harness = use_case_with_media(
            phase_one_uploads,
            MockFileRepository::new(),
            MockImageRepository::new(),
            MockVideoRepository::new(),
            phase_two_uploads,
            phase_two_files,
            phase_two_images,
            MockVideoRepository::new(),
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
    async fn complete_upload_registers_an_audio_row() -> Result<(), Box<dyn Error>> {
        // Arrange
        let resolved = upload_with_mime(5, 3, 4, &[0], false, "audio/mpeg")?;
        let finished = resolved.clone();
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(&mut phase_one_uploads, resolved.clone());
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, sealed(resolved));
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
        let mut phase_two_audios = MockAudioRepository::new();
        phase_two_audios
            .expect_create()
            .times(1)
            .withf(|audio| {
                audio.file_id() == 0
                    && audio.name() == "clip.mp4"
                    && audio.duration_ms() == Some(1_000.0f64)
                    && audio.sample_rate_hz() == Some(44_100.0f64)
            })
            .returning(|audio| Box::pin(async move { Ok(audio) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(Some(promoted_file())) }));
        let mut media_probe = MockMediaProbe::new();
        media_probe
            .expect_probe_audio()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Some(probed_audio())) }));
        let harness = use_case_with_audio(
            phase_one_uploads,
            phase_two_uploads,
            phase_two_files,
            phase_two_audios,
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
    async fn complete_upload_absent_staged_audio_returns_no_such_upload()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(
            &mut phase_one_uploads,
            upload_with_mime(5, 3, 4, &[0], false, "audio/mpeg")?,
        );
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        let mut media_probe = MockMediaProbe::new();
        media_probe
            .expect_probe_audio()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));
        let harness = use_case_with_audio(
            phase_one_uploads,
            MockUploadRepository::new(),
            MockFileRepository::new(),
            MockAudioRepository::new(),
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
        assert!(harness.unsealed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_audio_content_without_an_audio_stream_returns_unsupported_media()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(
            &mut phase_one_uploads,
            upload_with_mime(5, 3, 4, &[0], false, "audio/mpeg")?,
        );
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        let mut media_probe = MockMediaProbe::new();
        media_probe
            .expect_probe_audio()
            .times(1)
            .returning(|_| Box::pin(async { Err(MediaProbeError::UnsupportedMedia) }));
        let harness = use_case_with_audio(
            phase_one_uploads,
            MockUploadRepository::new(),
            MockFileRepository::new(),
            MockAudioRepository::new(),
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
        assert!(harness.unsealed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_audio_probe_timeout_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(
            &mut phase_one_uploads,
            upload_with_mime(5, 3, 4, &[0], false, "audio/mpeg")?,
        );
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        let mut media_probe = MockMediaProbe::new();
        media_probe
            .expect_probe_audio()
            .times(1)
            .returning(|_| Box::pin(async { Err(MediaProbeError::Timeout) }));
        let harness = use_case_with_audio(
            phase_one_uploads,
            MockUploadRepository::new(),
            MockFileRepository::new(),
            MockAudioRepository::new(),
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
        assert!(harness.unsealed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_concurrent_completion_returns_completion_in_progress()
    -> Result<(), Box<dyn Error>> {
        // Arrange: another completion already holds the seal, so sealing is a
        // no-op and the fresh read reports the live session.
        let resolved = sealed(upload(5, 3, 4, &[0], false)?);
        let mut phase_one_uploads = MockUploadRepository::new();
        phase_one_uploads
            .expect_search()
            .times(2)
            .returning(move |_| {
                let stored = resolved.clone();
                Box::pin(async move { Ok(vec![stored]) })
            });
        phase_one_uploads.expect_save().times(0);
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            MockUploadRepository::new(),
            MockFileRepository::new(),
            MockFileStorage::new(),
        );

        // Act
        let result = harness
            .use_case
            .execute(CompleteUploadCommand::new(5, 3))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(CompleteUploadError::CompletionInProgress)
        ));
        assert!(harness.phase_one_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_lost_seal_to_finished_session_returns_the_file()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the session was already sealed by another completion, then
        // finished while this completion was resolving; the fresh read answers
        // idempotently.
        let pending = sealed(upload(5, 3, 4, &[0], false)?);
        let finished = upload(5, 3, 4, &[0], true)?;
        let mut phase_one_uploads = MockUploadRepository::new();
        phase_one_uploads
            .expect_search()
            .times(1)
            .returning(move |_| {
                let stored = pending.clone();
                Box::pin(async move { Ok(vec![stored]) })
            });
        phase_one_uploads
            .expect_search()
            .times(1)
            .returning(move |_| {
                let stored = finished.clone();
                Box::pin(async move { Ok(vec![stored]) })
            });
        phase_one_uploads.expect_save().times(0);
        let mut phase_one_files = MockFileRepository::new();
        phase_one_files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(vec![stored_file(7, 3)?]) }));
        let harness = use_case_with(
            phase_one_uploads,
            phase_one_files,
            MockUploadRepository::new(),
            MockFileRepository::new(),
            MockFileStorage::new(),
        );

        // Act
        let result = harness
            .use_case
            .execute(CompleteUploadCommand::new(5, 3))
            .await;

        // Assert
        assert_eq!(
            result.as_ref().map(CompleteUploadResponse::file_id).ok(),
            Some(7)
        );
        assert!(result.is_ok_and(|response| response.is_finished()));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_missing_seal_in_phase_two_returns_completion_in_progress()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the seal was released between the phases, so the mutating
        // phase must not persist against content that may have changed.
        let resolved = upload(5, 3, 4, &[0], false)?;
        let mut bitmap = ChunkBitmap::try_new(1)?;
        bitmap.mark_received(0)?;
        let unsealed = Upload::try_new(
            5,
            3,
            "clip.mp4".to_owned(),
            4,
            "application/octet-stream".to_owned(),
            CHUNK_SIZE,
            IntegrityHash::try_new(DIGEST.to_owned())?,
            bitmap,
            false,
            false,
            None,
            timestamp(),
        )?;
        let mut phase_one_uploads = MockUploadRepository::new();
        expect_pending_upload(&mut phase_one_uploads, resolved);
        let mut phase_two_uploads = MockUploadRepository::new();
        expect_upload(&mut phase_two_uploads, unsealed);
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        let harness = use_case_with(
            phase_one_uploads,
            MockFileRepository::new(),
            phase_two_uploads,
            MockFileRepository::new(),
            file_storage,
        );

        // Act
        let result = harness
            .use_case
            .execute(CompleteUploadCommand::new(5, 3))
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(CompleteUploadError::CompletionInProgress)
        ));
        assert!(harness.phase_two_rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
