//! Emit the application-owned events of the security-event catalog.
//!
//! Every constructor emits exactly one normative event (`OBS-006`) at its
//! declared level and with its declared fields, on the reserved `security`
//! target (`OBS-007`). A use case calls these instead of the raw `tracing`
//! macros so the catalog lives in one place; the application layer is the
//! owning layer for a security decision (`OBS-002`).
//!
//! A `claimed_identity` is the submitted username by the explicit exception
//! recorded in `TechnicalDesign.md` § 9 (Security-event catalog): it is the only
//! identity available when authentication fails, and no other field can stand
//! in for it. The field escapes control characters and bounds its length, so a
//! client cannot forge a log record through it (`OBS-003`).

/// The security-event constructors, one per catalog entry.
mod events;

pub(crate) use self::events::{
    application_error, authentication_failed, authentication_succeeded, authorization_failed,
    credential_rotated, input_validation_failed, media_ingest, media_ingest_fault,
    metadata_artwork_failed, metadata_genre_unmatched, metadata_provider_fallback,
    metadata_provider_unconfigured, secret_initialized, sensitive_data_access,
    suspicious_business_logic, system_object, upload, upload_reap, user_admin,
};
