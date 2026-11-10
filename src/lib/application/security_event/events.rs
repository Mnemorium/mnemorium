use tracing::{debug, error, info, warn};

/// Maximum number of characters of a claimed identity kept on the event.
///
/// The submitted username is unbounded, so the field is truncated to keep a
/// single event from flooding the sink.
const MAX_CLAIMED_IDENTITY_CHARS: usize = 256;

/// A credential was accepted (`authn_succeeded`, `info`, field `actor`).
pub(crate) fn authentication_succeeded(actor: i64) {
    info!(
        target: "security",
        event = "authn_succeeded",
        actor,
        "authenticated a user"
    );
}

/// A credential was rejected (`authn_failed`, `warn`).
///
/// The reason is uniform across a bad identity and a bad secret: it never
/// distinguishes the two (`OBS-006`). The claimed identity is bounded and its
/// control characters are escaped, so it cannot forge a record on the
/// non-suppressible `security` channel (`OBS-003`).
pub(crate) fn authentication_failed(claimed_identity: &str) {
    let sanitized: String = claimed_identity
        .chars()
        .take(MAX_CLAIMED_IDENTITY_CHARS)
        .flat_map(char::escape_debug)
        .collect();
    warn!(
        target: "security",
        event = "authn_failed",
        claimed_identity = %sanitized,
        reason = "invalid_credentials",
        "rejected an authentication attempt"
    );
}

/// An authorization decision denied an action (`authz_failed`, `warn`).
pub(crate) fn authorization_failed(actor: i64, action: &str, target: &str) {
    warn!(
        target: "security",
        event = "authz_failed",
        actor,
        action = %action,
        target = %target,
        "denied an action"
    );
}

/// A client-supplied value failed validation (`input_validation_failed`,
/// `warn`).
///
/// The `field` names the rejected attribute and `reason` its classification;
/// the rejected value itself never reaches the sink (`OBS-003`, `OBS-006`).
pub(crate) fn input_validation_failed(field: &str, reason: &str) {
    warn!(
        target: "security",
        event = "input_validation_failed",
        field = %field,
        reason = %reason,
        "rejected a client-supplied value"
    );
}

/// A use case originated an internal `Unknown` (`application_error`, `error`).
pub(crate) fn application_error(operation: &str) {
    error!(
        target: "security",
        event = "application_error",
        operation = %operation,
        "a use case originated an internal error"
    );
}

/// A user-administration object changed (`user_admin`, `info`).
pub(crate) fn user_admin(actor: i64, action: &str, target: &str) {
    info!(
        target: "security",
        event = "user_admin",
        actor,
        action = %action,
        target = %target,
        "changed a user-administration object"
    );
}

/// A credential was replaced or rehashed (`credential_rotated`, `info`).
pub(crate) fn credential_rotated(actor: i64) {
    info!(
        target: "security",
        event = "credential_rotated",
        actor,
        "rotated a credential"
    );
}

/// An upload was accepted, a chunk was written, or an upload completed
/// (`upload`, `info`).
pub(crate) fn upload(actor: i64, upload: &str, outcome: &str) {
    info!(
        target: "security",
        event = "upload",
        actor,
        upload = %upload,
        outcome = %outcome,
        "upload activity"
    );
}

/// Personal data was read (`sensitive_data_access`, `debug`).
pub(crate) fn sensitive_data_access(actor: i64, object: &str) {
    debug!(
        target: "security",
        event = "sensitive_data_access",
        actor,
        object = %object,
        "read personal data"
    );
}

/// A secret was generated at startup (`secret_initialized`, `info`, field
/// `component`).
///
/// Emitted when the pepper or the token signing secret is generated while the
/// instance identity is created, once per generated secret. The `component`
/// names which secret was generated; the secret value itself never reaches the
/// sink (`OBS-003`, `OBS-004`).
pub(crate) fn secret_initialized(component: &str) {
    info!(
        target: "security",
        event = "secret_initialized",
        component = %component,
        "generated a secret"
    );
}

/// A system-level object was created (`system_object`, `info`).
pub(crate) fn system_object(object: &str, action: &str) {
    info!(
        target: "security",
        event = "system_object",
        object = %object,
        action = %action,
        "created a system-level object"
    );
}

/// An ingest operation finished (`ingest`, `info`).
///
/// The record carries aggregate counts only: no file name and no path ever
/// reaches the sink (`OBS-003`, `OBS-006`).
pub(crate) fn media_ingest(
    registered: u64,
    completed: u64,
    untouched: u64,
    changed: u64,
    missing: u64,
    removed: u64,
    skipped: u64,
) {
    info!(
        target: "security",
        event = "ingest",
        registered,
        completed,
        untouched,
        changed,
        missing,
        removed,
        skipped,
        "reconciled the library root"
    );
}

/// A reaping run removed upload lifecycle state (`upload_reap`, `info`).
///
/// The record carries aggregate counts only: no file name and no path ever
/// reaches the sink (`OBS-003`, `OBS-006`).
pub(crate) fn upload_reap(expired: u64, orphaned: u64, unsealed: u64) {
    info!(
        target: "security",
        event = "upload_reap",
        expired,
        orphaned,
        unsealed,
        "reaped upload sessions"
    );
}

/// The change observer could not deliver every event (`ingest_fault`, `warn`).
///
/// A failed watch registration or a queue overflow is reported once; neither
/// fails startup, and the startup scan remains the mechanism of record.
pub(crate) fn media_ingest_fault(kind: &str) {
    warn!(
        target: "security",
        event = "ingest_fault",
        kind = %kind,
        operation = "media_change_source",
        "the change observer degraded; the startup scan remains authoritative"
    );
}

/// A metadata provider was skipped (`metadata_provider_fallback`, `warn`).
///
/// The record carries the provider and the failure classification only: never
/// the query, a title, a file name or a key (`OBS-003`, `OBS-006`).
pub(crate) fn metadata_provider_fallback(provider: &str, classification: &str) {
    warn!(
        target: "security",
        event = "metadata_provider_fallback",
        provider = %provider,
        classification = %classification,
        "fell over to the next metadata provider"
    );
}

/// A lookup named a provider that is not configured
/// (`metadata_provider_unconfigured`, `warn`).
///
/// The record names only the provider; the reference itself never reaches the
/// sink (`OBS-003`, `OBS-006`).
pub(crate) fn metadata_provider_unconfigured(provider: &str) {
    warn!(
        target: "security",
        event = "metadata_provider_unconfigured",
        provider = %provider,
        "a lookup named a provider that is not configured"
    );
}

/// A provider reported a genre the vocabulary does not contain
/// (`metadata_genre_unmatched`, `debug`).
///
/// The record carries the count only: the term itself never reaches the sink
/// (`OBS-003`, `OBS-006`).
pub(crate) fn metadata_genre_unmatched(count: u64) {
    debug!(
        target: "security",
        event = "metadata_genre_unmatched",
        count,
        "a provider reported a genre outside the shared vocabulary"
    );
}

/// Artwork a provider reported could not be downloaded or accepted
/// (`metadata_artwork_failed`, `warn`).
///
/// The record carries the provider and a stable reason: never the address, the
/// bytes or a key (`OBS-003`, `OBS-004`).
pub(crate) fn metadata_artwork_failed(provider: &str, reason: &str) {
    warn!(
        target: "security",
        event = "metadata_artwork_failed",
        provider = %provider,
        reason = %reason,
        "a provider artwork download failed"
    );
}

/// An action was out of order, nonsensical in context, or exceeded a limit
/// (`suspicious_business_logic`, `warn`).
pub(crate) fn suspicious_business_logic(actor: i64, action: &str, reason: &str) {
    warn!(
        target: "security",
        event = "suspicious_business_logic",
        actor,
        action = %action,
        reason = %reason,
        "an action was out of order or exceeded a limit"
    );
}
