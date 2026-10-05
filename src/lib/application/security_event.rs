//! Emit the application-owned events of the security-event catalog.
//!
//! Every constructor emits exactly one normative event (`OBS-006`) at its
//! declared level and with its declared fields, on the reserved `security`
//! target (`OBS-007`). A use case calls these instead of the raw `tracing`
//! macros so the catalog lives in one place; the application layer is the
//! owning layer for a security decision (`OBS-002`).
//!
//! A `claimed_identity` is the raw submitted username by the explicit exception
//! recorded in `TechnicalDesign.md` § 9 (Security-event catalog): it is the only
//! identity available when authentication fails, and no other field can stand
//! in for it.

use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

/// A credential was accepted (`authn_succeeded`, `info`, field `actor`).
pub fn authentication_succeeded(actor: i64) {
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
/// distinguishes the two (`OBS-006`).
pub fn authentication_failed(claimed_identity: &str) {
    warn!(
        target: "security",
        event = "authn_failed",
        claimed_identity = %claimed_identity,
        reason = "invalid_credentials",
        "rejected an authentication attempt"
    );
}

/// An authorization decision denied an action (`authz_failed`, `warn`).
pub fn authorization_failed(actor: i64, action: &str, target: &str) {
    warn!(
        target: "security",
        event = "authz_failed",
        actor,
        action = %action,
        target = %target,
        "denied an action"
    );
}

/// A use case originated an internal `Unknown` (`application_error`, `error`).
pub fn application_error(operation: &str) {
    error!(
        target: "security",
        event = "application_error",
        operation = %operation,
        "a use case originated an internal error"
    );
}

/// A user-administration object changed (`user_admin`, `info`).
pub fn user_admin(actor: i64, action: &str, target: &str) {
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
pub fn credential_rotated(actor: i64) {
    info!(
        target: "security",
        event = "credential_rotated",
        actor,
        "rotated a credential"
    );
}

/// An upload was accepted, a chunk was written, or an upload completed
/// (`upload`, `info`).
pub fn upload(actor: i64, upload: &str, outcome: &str) {
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
pub fn sensitive_data_access(actor: i64, object: &str) {
    debug!(
        target: "security",
        event = "sensitive_data_access",
        actor,
        object = %object,
        "read personal data"
    );
}

/// A system-level object was created (`system_object`, `info`).
pub fn system_object(object: &str, action: &str) {
    info!(
        target: "security",
        event = "system_object",
        object = %object,
        action = %action,
        "created a system-level object"
    );
}

/// An action was out of order, nonsensical in context, or exceeded a limit
/// (`suspicious_business_logic`, `warn`).
pub fn suspicious_business_logic(actor: i64, action: &str, reason: &str) {
    warn!(
        target: "security",
        event = "suspicious_business_logic",
        actor,
        action = %action,
        reason = %reason,
        "an action was out of order or exceeded a limit"
    );
}
