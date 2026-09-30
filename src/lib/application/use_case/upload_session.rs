//! Projection helpers shared by the upload-session use cases.

use chrono::Duration;
use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;
use crate::domain::model::upload::Upload;
use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::file_repository::FileFilter;
use crate::domain::port::file_repository::FileRepository as _;

/// Find the caller's file matching the upload's digest, if any.
///
/// # Errors
///
/// Returns a [`RepositoryError`] when the file lookup fails.
pub(crate) async fn caller_file_id<U>(
    unit_of_work: &mut U,
    upload: &Upload,
    user_id: NumericID,
) -> Result<Option<NumericID>, RepositoryError>
where
    U: AssetUnitOfWork,
{
    let files = unit_of_work
        .files()
        .search(&FileFilter {
            integrity_hash: Some(upload.integrity_hash().as_str().to_owned()),
            user_id: Some(user_id),
            ..FileFilter::default()
        })
        .await?;
    Ok(files.into_iter().next().map(|file| file.id()))
}

/// Return the instant at which an upload created at `created_at` expires.
///
/// # Errors
///
/// Returns an error when the expiry does not fit in `i64` or the sum overflows
/// the timestamp.
pub(crate) fn expiry(
    expiry_seconds: u64,
    created_at: NaiveDateTime,
) -> Result<NaiveDateTime, anyhow::Error> {
    let seconds = i64::try_from(expiry_seconds)
        .map_err(|error| anyhow::anyhow!(error).context("the expiry does not fit in i64"))?;
    created_at
        .checked_add_signed(Duration::seconds(seconds))
        .ok_or_else(|| anyhow::anyhow!("the upload expiry overflows the created_at timestamp"))
}

/// Build the per-chunk received bitmap, one character per chunk.
#[must_use]
pub(crate) fn received_bitmap(upload: &Upload) -> String {
    let mut bitmap = String::with_capacity(upload.total_chunks());
    for chunk_number in 0..upload.total_chunks() {
        let received = upload
            .chunk_bitmap()
            .is_received(chunk_number)
            .unwrap_or(false);
        if received {
            bitmap.push('1');
        } else {
            bitmap.push('0');
        }
    }
    bitmap
}
